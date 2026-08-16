# Persistence

永続化層（DBアクセス）のスコープと信頼境界。

関連: [`mutation-contract.md`](./mutation-contract.md) / [`unverified-boundaries.md`](./unverified-boundaries.md) / [`effect-inference.md`](./effect-inference.md)

---

## 決定: Repository traitのみ提供する

Verumは**DBを知らない**。Repository traitのみを定義し、実装は利用者がsqlx等で書く。

```rust
// Verum が定義するのは trait と Capability 制約のみ
pub trait UserRepository {
    async fn find(&self, id: UserId) -> Result<User>;
    async fn set_email(&self, u: &mut User, v: Email) -> Result<()>;
    async fn set_name(&self, u: &mut User, v: String) -> Result<()>;
}
```

> **Capabilityは引数として渡さない。** 当初 `cap: &Cap<Mutate<User, user::Email>>` を引数に取る形を記載していたが、これは「Capabilityは値として実体化せず `Ctx<'req, E>` の型パラメータで表現する」という方針（[`rust-type-model.md`](./rust-type-model.md)）と矛盾するため削除した。
>
> Capability検査は `Repo<User, R, M>` の拡張traitのwhere節で行う。

```rust,ignore   // fragment, not a complete item
// derive が生成する拡張 trait（Capability 検査はここ）
pub trait UserRepo<M> {
    fn set_email<I>(&self, u: &mut User, v: Email) -> Result<()>
    where M: Has<Mutate<User, user::Email>, I>;
}

impl<R, M> UserRepo<M> for Repo<User, R, M> { ... }
```

`Repo<D, R, M>` が公開される唯一のCapability検査面であり、`UserRepository` は素の永続化traitとしてその内側に位置する。

### 判断理由

[`runtime-stack.md`](./runtime-stack.md) の判断基準と同一。

> **仕様が固まっている概念は自作しない。未解決の設計問題に全リソースを投下する。**

SQL生成・クエリビルダは既に解かれた問題であり、sqlx / SeaORM / Diesel が長年投資している領域。

---

## Domain不透明化との相互運用

Domainは不透明型（privateフィールド）として公開される（[`mutation-contract.md`](./mutation-contract.md)）。一方 sqlx の `query_as!` は `pub` フィールドを要求する。

deriveが `pub(crate)` な `Repr` を生成する。

```rust,ignore   // fragment, not a complete item
// マクロ生成（derive か属性かは未確定 — 下記「未確定の点」を見ること）
pub(crate) struct UserRepr {
    pub id:    UserId,      // pub 必須。query_as! は呼び出し側で構造体リテラルに展開する
    pub name:  String,
    pub email: Email,
    // Debug / Clone / Serialize を derive してはならない（台帳 path 3 / 4 が Repr 経由で復活する）
}

// Domain は借用可能な Repr を所有していなければならない（newtype はその一形態）
pub struct User(UserRepr);   // 内側フィールドは pub(crate) ではなく private

impl User {
    pub(crate) fn from_repr(r: UserRepr) -> Self;
    pub(crate) fn as_repr(&self) -> &UserRepr;
}

// 利用者の Repository 実装
impl UserRepository for PgUserRepository {
    async fn find(&self, id: UserId) -> Result<User> {
        let repr = sqlx::query_as!(UserRepr, "SELECT * FROM users WHERE id = $1", id)
            .fetch_one(&self.pool).await?;
        Ok(User::from_repr(repr))
    }
}
```

### 判定（T-M1-01 / #13。**21プローブ、コンパイル検証済み**）

再現: `spikes/domain-opacity-sqlx/`（`bash run.sh` → `21 as specified, 0 unexpected`）。
**プローブ表がこの節の正典である。** この判定は2回レビューされ、**表は2回とも正しく、散文は2回とも誤っていた**（測定を超えた一般化が計6件）。以下は表が establish したことに限る。

1. **sqlx 連携は成立する。** 上の形はコンパイルし、実行もする。
2. **「信頼境界 = Repository 実装」は成立しない。** マクロは利用者のクレートで展開されるので `pub(crate)` はアプリクレート全体を指し、Repository の置き場所を知らないため `pub(in ...)` を出せない。同一クレートならあらゆるハンドラが任意の値の Domain を組み、別クレートなら `Repr` が見えない（`E0603` + `E0624`）。→ 台帳 **path 21**
3. **生成形は未確定。** derive は**入力と同名のアイテムを追加できない**（`E0428`）ので、`pub struct User(UserRepr)` は `#[derive(Domain)]` が出せる形ではない。

**Field-level Mutation の型強制自体は生きている。** 崩れたのは sqlx でも型強制でもなく、`Repr` という横の抜け道である。

#### マクロが守らなければならない形（実測）

| 制約 | 守らないと |
|---|---|
| `Repr` のフィールドを**完全 private にしない** | `query_as!` は呼び出し側で構造体リテラルに展開するので `E0451`（`pub(crate)` はクレート内では足りる） |
| `Repr` に `Debug` / `Clone` / `Serialize` / `Deserialize` を derive しない | 台帳 path 4 / 3 が `Repr` 経由で復活する（**同一クレート内から**。仕様形では外部クレートは `as_repr` に到達できず `E0624`） |
| Domain の内側フィールドは private（`pub(crate)` にしない） | `u.0.email = v` がクレート内どこからでも通る |
| Domain は借用可能な `Repr` を**所有**する | 一時値を返す `as_repr` は `E0515`。**newtype はその一形態で、強制ではない** |

**保証範囲は「定義モジュールの外から」であって型の境界ではない。** 定義モジュールとその子からは `u.0.email = v` が通る。マクロは利用者の `struct User` と同じモジュールに展開されるので、その横に書かれたコードは緩い側に立つ。エラーコードも形で変わる — newtype + getter は `E0615`、getter なしは `E0609`、フラットな private 名前付きをモジュール外から触った場合だけ `E0616`。**`E0615` / `E0609` は `#[diagnostic::…]` で差し替えられない**（trait 定義と trait impl にしか付かないため。実測）。

#### 未確定 — #17 / #18 で決める

1. **どのマクロ形にするか。** `as_repr(&self) -> &Repr` は derive で満たせる形が複数ある（利用者が newtype を書いて derive は `Repr` だけ出す / `Repr` を Domain の型エイリアスにする、いずれも実測で通る）。したがって「署名と derive は両立しない」は**測っていない**。測ったのは `E0428` だけである。属性マクロ化は層1 検査を何も失わないが（実測）、`Domain` を derive と名指す記述が **15ファイル / 23箇所**（うち2件は既発行 issue の本文）に連鎖する。versioning 上の破壊性は実質ゼロ（`verum-macros` は何も生成していない）。
2. **`#[derive(sqlx::FromRow)]` を誰が付けるか。** 利用者は生成されたアイテムに derive を足せない（実測）。パススルー（`#[domain(repr_derive(sqlx::FromRow))]`）は**実装して動作を確認済み** — この形なら生成された derive は利用者クレートで解決されるので `verum-macros` は sqlx に依存しない。「verum が `sqlx::FromRow` を決め打ちで出す」案だけが依存表に反する。
3. **`pub` フィールド拒否の強制レベルが 1 の選択に依存する。** 属性形ではマクロが入力の `pub` を消費するので**リント**、derive + フラット形では利用者の `pub` が本物なので**保証**。`.claude/commands/bump.md` は強制レベルの変化を破壊的と定めている。

#### 代替案（すべて実測。**どれも現状を改善しない**）

| 代替案 | 実測 |
|---|---|
| `Repr` を専用モジュールに置き module privacy で守る | **悪化する。** private モジュール内の `pub` 型にすると `E0446` が出ず trait 経路が開き、外部クレートが射影で全フィールドを読み偽造できる（`-D warnings` でも警告ゼロ） |
| `Repr` を `pub` にしてフィールドを private | **境界を守らない。** load でき、構造体**リテラル**は `E0451`、`query_as!` は失われ、**それでも偽造される**（呼び出し側が用意した行から `FromRow` が組む） |
| Repr 変換を `verum` の trait に載せる | `Repr` が `pub(crate)` の間は `E0446`。`pub` にすると開くが、上の射影バイパスがついてくる |
| **sealed トークン** | **境界にならない**（撤回。以前ここに「唯一の生存候補」と書いた）。トークンは**利用者が実装する trait の引数**でしか渡せないので、ハンドラが3行の `impl Repository` を書けば verum がトークンを手渡す（実測）。by-value では複数行ロードが書けず（`E0382`）`Copy` を強制され、`Copy` にすると静的変数に stash できる |
| `FromRow` を手実装 / 不透明化を諦める | **未測定** |

一般形は「**定義側モジュール内で構造体を組む derive 由来のコンストラクタは何であれ偽造経路**」で、`FromRow` / `Deserialize` / 将来の任意の derive が該当する。`E0451` が閉じるのは構文形ひとつであり、**`Repr` を使える程度に開くと偽造できる程度に開く。**

#### 閉じ方の診断上の制約（実測）

**可視性で塞ぐと、その診断は永久に文言を持てない。** `E0603` / `E0615` / `E0609` / `E0616` はフィールド・パス解決の診断で trait 解決を経由しないため `#[diagnostic::…]` が届かない。**trait bound で塞ぐと `E0277` になり、`message` と `label` の両方を Verum が書ける**（実測）。`CLAUDE.md` の非交渉事項「経路を塞ぐときは検査済みの代替を用意する」を満たせるのは後者だけである。

---

## 信頼境界 — Repository実装

**この決定により、Repository実装が信頼境界になる。**

```rust,ignore   // needs a crate or a verum-private module this harness does not carry
impl UserRepository for PgUserRepository {
    async fn set_email(&self, u: &mut User, v: Email) -> Result<()> {
        sqlx::query!(
            "UPDATE users SET email = $1, status = 'verified' WHERE id = $2",
            //                            ^^^^^^^^^^^^^^^^^^^ 宣言外のMutation
            v, u.id()
        ).execute(&self.pool).await?;
        Ok(())
    }
}
```

この違反は**Verumでは検出できない**。Capabilityの型検査はメソッド呼び出しの可否までしか及ばず、メソッド実装内部のSQLには届かない。

### これは欠陥ではなく、明示された境界である

```text
Endpoint / Service 層  → 型で保証される
Repository 実装        → 信頼境界（レビュー・監査の対象）
DB                     → 対象外
```

> **⚠️ 上の図の1行目は現時点では成立していない**（T-M1-01 / #13 で実測）。台帳 **path 21** が開いている間、Endpoint / Service 層の普通のコードが Capability も Repository も SQL も `unsafe` も無しに `User::from_repr(UserRepr { .. })` で Domain を捏造できる。図は path 21 を閉じたあとの姿である。

**AI Contextに `unverified_boundaries` として出力する**（[`unverified-boundaries.md`](./unverified-boundaries.md)）。境界がどこにあるかを文書化しないことが欠陥になる。

---

## 信頼境界を狭める手段

### 1. 1 Field = 1 メソッドを強制する

[`handler-rules.md`](./handler-rules.md) Rule 1 の帰結。各メソッドが1つのカラムしか触らないため、実装は数行で済みレビューが容易になる。

### 2. Repository実装をderiveで生成する（優先度を上げる）

```rust,ignore   // needs a macro that arrives in M2
#[derive(Repository)]
#[repository(domain = User, table = "users")]
pub struct PgUserRepository { pool: PgPool }
// → set_email / set_name / find を自動生成
```

生成された実装は定義上Contractに従うため、**信頼境界がVerum内部に移動する**。

> **重要**: Repository **trait 定義**の生成も必要である。Field ごとに `set_<field>` を trait と impl の両方に手書きする状態では、
>
> 1. 新しいDomainを追加するたびにFieldの数だけboilerplateを書く（token効率の主張が「書く」場面で崩れる）
> 2. **`set_email` のwhere節に誤って `user::Name` を書いても検出されない** — 「rustcが照合を代行する」という主張が、手書きboilerplateの正しさという弱い前提に乗ってしまう
>
> 当初「将来」としていたが、2の問題があるため**trait定義の生成をimpl生成より先に前倒しする**。

### 3. 生SQLをEscape Hatchとして明示させる

複雑なクエリでは Escape Hatch 経由とし、Contractに記録する。

```text
escape_hatch: raw_sql
  reason: "complex aggregation across users and orders"
```

> **注意**: 記録は現状**自己申告**である。属性を書き忘れれば記録されない。`escape_hatches: []` を「脱出なし」と読ませてはならない。低レイヤAPIが属性マクロ生成のZST証拠を引数で要求する形にすれば記録漏れが構造的に防げる。それができない範囲は `"unknown"` と出力する。

---

## 却下した選択肢

### 型付きQuery Builderを持つ

宣言外MutationをDB層まで貫通して防げるが却下。SQL生成は「既に解かれた問題」の自作にあたり、型設計に使う時間が奪われる。複雑なクエリで結局Escape Hatch頼りになりがち。

### UPDATE文の静的検査（Lint）

Query Builderより小さい実装で境界を貫通できる中間解だが、SQLパースが必要で動的SQLに無力、sqlx固有になる。**型設計が固まる前に着手するのは早すぎる**。型設計完了後に再検討。

---

## 未解決の課題

### Transaction

- Endpoint = 1トランザクションを標準とするか
- 複数Mutationのatomicityを Contract で表現するか
- **トランザクション内でExternal Effectの発火を型で禁止できるか**
  - [`handler-rules.md`](./handler-rules.md) Rule 4 で `ctx.after_commit` スコープを提案しているが、Transaction境界の設計と合流させる必要がある
- Savepoint / ネストしたトランザクションの扱い
- **部分失敗の意味論** — Contractは「上界」なので「宣言したEffectの部分集合しか起きなかった状態」が表現されていない。`emits: [UserUpdated]` を読んだAIは「更新されたなら必ずイベントが出る」と解釈するが、逆（イベントだけ出て更新されない）も起こる

### 楽観ロック / 悲観ロック

- Field単位setterでは `WHERE id=? AND version=?` のCompare-and-Swapを原子操作として表現できない
- `SELECT ... FOR UPDATE` に相当する `Lock<Domain>` Effectがない

### 一覧 / 集計 / JOIN / N+1

`Read<Domain, Field>` が単一インスタンス前提であることの帰結。`find(id)` 以外のRepository APIが仕様に存在しない。

[`research-questions.md`](./research-questions.md) を参照。
