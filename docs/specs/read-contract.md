# Read Contract

`reads` 宣言を型で強制する。[`mutation-contract.md`](./mutation-contract.md) と対称の仕組み。

関連: [`mutation-contract.md`](./mutation-contract.md) / [`unverified-boundaries.md`](./unverified-boundaries.md) / [`ai-context.md`](./ai-context.md)

---

## 解決する問題

`reads` を宣言しても、Repositoryが Domain Model 全体を返すなら、その宣言は**実質的に効いていない**。

```rust,ignore   // needs a macro that arrives in M2
#[contract(reads = [User::id, User::name, User::email, User::status])]
pub struct GetUser;

let user = ctx.users().find(req.id).await?;
user.password()   // ← 宣言していないが読めてしまう
```

この状態では `reads` は Metadata に過ぎず、[`../concepts.md`](../concepts.md) が否定した「コメント同然の仕様」になる。

---

## 決定: Projection型で強制する

`find()` は Domain Model 全体ではなく、**宣言されたFieldのみを読めるProjection**を返す。

```rust,ignore   // fragment, not a complete item
let user = ctx.users().find(req.id).await?;
// 型: Projection<User, (user::Id, (user::Name, (user::Email, (user::Status, ()))))>

user.name()       // ✅ OK
```

```rust,compile_fail
user.password()   // ❌ 型エラー
```

Projection のgetterは、拡張trait内でフィールドごとに `where` 節付きメソッドを並べる形で実装する（実コンパイルで確認済み）。

```rust,ignore   // fragment, not a complete item
pub trait UserProjection<F> {
    fn name<I>(&self) -> &String where F: Has<user::Name, I>;
    fn password<I>(&self) -> &PasswordHash where F: Has<user::Password, I>;
}

impl<F> UserProjection<F> for Projection<User, F> { ... }
```

> `Projection` はフレームワーク型なので inherent impl は書けない（E0116）。拡張trait化が必須。[`rust-type-model.md`](./rust-type-model.md) を参照。

---

## 得られる価値

### 1. Contract全体が信頼できる

`mutates` だけが型で効いて `reads` が効かない状態は、Contractの信頼性を部分的に損なう。AIは「どの宣言が本物か」を判断できない。

### 2. 個人情報の読み取り範囲を型で制限できる

```rust,ignore   // needs a macro that arrives in M2
#[contract(reads = [User::id, User::name])]   // password / email に触れない
pub struct GetUserPublicProfile;
```

> **ただしデータ最小化の保証ではない。** Projectionは**コンパイル時のマスクであり、データ上のマスクではない**。SELECT句生成が実装されるまで `find()` は `SELECT *` 相当であり、password ハッシュはメモリ上に存在する。
>
> したがって:
> - `Projection` に `Debug` / `Serialize` を derive しない（宣言Fieldのみを出す独自実装をderive生成する）
> - Domain への `Deserialize` を禁止する（任意値でのDomain構築を防ぐ）
> - GDPR等のデータ最小化への「機械的な裏付け」という主張は、SELECT句生成が入るまで**しない**

### 3. SELECT句の最適化に使える

宣言されたFieldが分かるため、Repository実装（derive生成時）が `SELECT id, name FROM users` を生成できる。

---

## 複雑さのトレードオフ

| コスト | 内容 | 緩和策 |
|---|---|---|
| Field アクセスがメソッドになる | `user.name` ではなく `user.name()` | Domainも不透明型なので一貫する（[`mutation-contract.md`](./mutation-contract.md)） |
| Response変換が煩雑になる | `UserView::from(user)` が Projection を受け取る | `#[derive(View)]` で変換を生成 |
| 型が長くなる | cons list が展開される | deriveが型エイリアスを生成 |
| setter シグネチャへの影響 | `set_email` が `&mut Projection<User, F>` を取る形になる | **Full PoC での作業として明記**（下記） |

### `into_owned()` は提供しない

当初「既存コードとの相互運用」の緩和策として `into_owned()`（Projection から生 `User` を取り出す）を挙げていたが、**削除した**。

理由: 取り出した瞬間にread制約が消える。しかも「Contractに記録する」と書いたが、**メソッド呼び出しが自分自身を属性マクロに記録する手段は存在しない**。

**設計者が「ここが一番つらい」と認めた場所に脱出口を置くと、そこが例外ではなく主要動線になる。** どうしても必要な場合は、属性マクロが生成するZST証拠を引数で要求する形にして、記録漏れを構造的に防ぐ。

```rust,ignore   // fragment, not a complete item
fn into_owned(self, proof: EscapeHatchProof) -> User;   // 属性なしでは呼べない
```

### Mutation との組み合わせ

`mutates` に宣言したFieldは、変更前の値を読む必要があるため**自動的に `reads` に含まれる**。

```rust,ignore   // needs a macro that arrives in M2
#[contract(
    reads   = [User::id, User::status],
    mutates = [User::name],
    when(EmailChanged) => {
        mutates = [User::email],
    },
)]
// 実効的な read 集合: id, status, name, email
```

**`when` 内の `mutates` も自動的に `reads` に含まれる。** 条件成立時に変更前の値を読む必要があるため。ただし読み取り権限はスコープを問わず有効とする（条件下でのみ読めるという制約は、`when` の内側で `find` を呼び直す必要が生じ、実用性を損なうため設けない）。

Projection導入時、setterのシグネチャは `&mut User` から `&mut Projection<User, F>` に変わる。First PoC（Projection なし）と Full PoC でシグネチャが変わることを明記する。

---

## PoCでの扱い

**First PoCでは Projection を実装しない。**

理由:

- First PoCの証明対象は「GETがMutateを呼べない」の1点
- Projection は Mutation Contract の型設計が固まってから対称的に作る方が早い
- Domain不透明化（privateフィールド + Capability付きgetter）だけでも、宣言外Fieldの**読み取り**は制限できる可能性がある（getterのwhere節で `Reads` を検査すれば、Projection型なしで同じ効果が得られるか要検証）

### 段階差を隠さない

`reads` が当面 Metadata のみであることを AI Context に明示する。

> **Capability 付き getter が `reads` の強制になるかは測定していない。** 生成されること自体は決まっているが、それが宣言外 Field の読み取りをコンパイルエラーにするかは #15 / T-M1-03 の対象で、未実施である。ここで `metadata_only` と出すのは**弱い側の主張を選んでいる**からであって、getter に効果が無いと確かめたからではない。経緯は [ADR-0004](../adr/0004-reads-enforcement-level.md)。

```json
{
  "reads": {
    "fields": ["User.id", "User.name", "User.email", "User.status"],
    "enforcement": "metadata_only"
  },
  "mutates": {
    "fields": ["User.name", "User.email"],
    "enforcement": "upper_bound_checked"
  }
}
```

Full PoC で `reads` が `upper_bound_checked` に昇格する。

> `type_checked` という値は使わない。Contractは「実装 ⊆ 契約」の上界検査であり、双方向の検証ではない。[`effect-inference.md`](./effect-inference.md) を参照。

---

## 未解決の課題

- **一覧取得** — `find(id) -> Projection<User, F>` の形しか定義されていない。`Vec<Projection<..>>` を返す一覧APIと、そこでのページネーション / ソート / 動的フィルタの表現
- **集計** — COUNT / SUM / GROUP BY は特定Fieldの値ではなく、結果はどのDomainインスタンスにも属さない
- **JOIN** — `Projection<Domain, Fields>` は単一Domain。複合Projection（`Projection<(User, Order), (..)>`）が未定義
- **N+1 / eager loading** — Field単位メソッド（Rule 1）と構造的に衝突する
- Domain不透明化のgetterだけで `reads` 強制が足りるか（Projection型が不要になる可能性）

[`research-questions.md`](./research-questions.md) を参照。
