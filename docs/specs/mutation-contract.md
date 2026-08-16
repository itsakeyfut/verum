# Mutation Contract

Field-levelの可変性を型で表現する。本プロジェクトで特に重要なテーマ。

関連: [`read-contract.md`](./read-contract.md) / [`capability-system.md`](./capability-system.md) / [`handler-rules.md`](./handler-rules.md) / [`unverified-boundaries.md`](./unverified-boundaries.md)

---

## 例: User Update

対象のDomain Model:

```text
User
├── id
├── name
├── email
├── password
├── status
├── last_login_at
└── created_at
```

Contract宣言:

```rust,ignore   // needs a macro that arrives in M2
#[endpoint(PUT "/users/{id}")]
#[contract(
    domain    = User,
    reads     = [User::id, User::status],
    mutates   = [User::name, User::email],
    when(EmailChanged) => {
        emits = [EmailVerificationRequested],
    },
)]
pub struct UpdateUser;
```

---

## Goal

AIがEndpoint内部を読まなくても、

- どのFieldが変更され得るか
- どのFieldは絶対に変更されないか
- どの条件で変更されるか
- どのEventが発生するか

を理解できるようにする。

---

## 決定: Domainは不透明型として公開する

**これが最も重要な設計判断である。** Domainを通常のRust structとして公開すると、Contract全体が無効化される。

```rust,compile_fail
// ❌ この形では Contract が意味を持たない
pub struct User { pub email: Email, pub status: UserStatus, ... }

// ハンドラ側
let mut user = ctx.users().find(req.id).await?;
user.email = req.email;              // Contract 無視。コンパイルは通る
user.status = UserStatus::Admin;     // 宣言外 Field も自由
```

setterを呼ぶにはハンドラが `&mut User` を持つ必要があり、フィールドが `pub` なら任意の代入が合法になる。**GET Endpointでも `let mut user = ctx.users().find(id).await?` と書けるため、「GETはMutateを呼べない」という保証すら回避できる。**

### 採用する形

```rust,ignore   // needs a macro that arrives in M2
#[derive(Domain)]
pub struct User {
    id:            UserId,      // pub を付けると derive がコンパイルエラーを出す
    name:          String,
    email:         Email,
    password:      PasswordHash,
    status:        UserStatus,
    last_login_at: Option<DateTime<Utc>>,
    created_at:    DateTime<Utc>,
}
```

deriveが生成するもの:

```text
1. Field マーカ型（ZST）           → mod user { pub struct Name; ... }
2. Capability 要求付き getter       → read-contract.md（**それが `reads` の強制になるかは未決** — [ADR-0004](../adr/0004-reads-enforcement-level.md) / #15）
3. pub(crate) な Repr               → 内部表現（⚠️「Repository 実装専用」にはならない — path 21）
4. 宣言 Field のみを出す Debug       → ログへの機密漏れを防ぐ
5. 宣言 Field のみを出す Serialize   → 同上
```

### `&mut User` を渡すことは安全になる

フィールドがprivateであれば、`&mut User` を保持していても直接代入はできない。

```rust,ignore   // fragment, not a complete item
ctx.users().set_email(&mut user, req.email)?;   // ✅ Capability 経由
```

```rust,compile_fail
user.email = req.email;                          // ❌ private field
```

**したがってsetterのシグネチャを変える必要はない。** ergonomicsを保ったまま直接代入経路を塞げる。

### `pub` フィールドを拒否する理由

deriveが `pub` フィールドを検出したらコンパイルエラーにする。

```text
error: Domain fields must be private
  --> src/domain/user.rs:4:5
   |
 4 |     pub email: Email,
   |     ^^^ remove `pub` — access is granted through the contract
   |
   = note: `#[derive(Domain)]` generates capability-checked accessors
   = help: if this field must be public, it does not belong in a Domain
```

「うっかり `pub` を付ける」ことは保証を破る経路のうち**マクロが弾ける**ものなので、macro段階で弾く。

> **唯一の経路ではない**（T-M1-01 / #13 で実測）。同じマクロが生成する `Repr` が横に抜け道を開けており（台帳 path 21）、`Repr` に `Debug` / `Clone` を derive すると path 3 / 4 も復活する。この検査は必要だが十分ではない。

### Repository実装との相互運用

sqlxの `query_as!` は `pub` フィールドを要求する。そのため `pub(crate)` な `Repr` を生成する。

```rust,ignore   // fragment, not a complete item
// マクロ生成（derive か属性かは未確定 — 下の注記を見ること）
pub(crate) struct UserRepr {
    pub id: UserId,          // pub 必須（query_as! は呼び出し側で構造体リテラルに展開する）
    pub name: String,
    pub email: Email,
    // Debug / Clone / Serialize は derive しない（台帳 path 3 / 4 が Repr 経由で復活する）
}

// Domain は借用可能な Repr を所有する（newtype はその一形態。強制ではない）。
// 内側フィールドは private — pub(crate) にすると `u.0.email = v` が通り、
// 上の不透明化が直接無効化される。
// ⚠️ この newtype 形は derive では出せない（E0428）。下の注記を見ること。
pub struct User(UserRepr);

impl User {
    pub(crate) fn from_repr(r: UserRepr) -> Self { ... }
    pub(crate) fn as_repr(&self) -> &UserRepr { ... }
}
```

> **⚠️ この形は sqlx とは噛み合うが、「信頼境界 = Repository 実装」を型で表現しない**（T-M1-01 / #13 でコンパイル検証済み）。
>
> `#[derive(Domain)]` は**利用者のクレートで展開される**ため `pub(crate)` はアプリクレート全体を指す。したがって上の `from_repr` / `as_repr` は**そのクレートのあらゆるハンドラから到達可能**であり、`User::from_repr(UserRepr { email: 任意, .. })` で任意の値の Domain を組める。Repository を別クレートに置いた場合は逆に `Repr` が全く見えず（`E0603`）、設計として機能しない。
>
> つまり**フィールドを private にすること自体は機能している**が、`Repr` が横に開いた抜け道になっている。台帳の **path 21**。
>
> **ただし private 化の保証範囲は「定義モジュールの外から」であって、型の境界ではない**（実測）。定義モジュールとその子モジュールからは `u.0.email = v` が通り、**マクロは利用者の `struct User` と同じモジュールに展開される**ので、利用者がその横に書くヘルパは緩い側に立つ。
>
> エラーコードも形で変わる（実測）: newtype で `email()` getter がある場合 `u.email = v` は **`E0615`**、getter が無ければ `E0609`、フラットな private 名前付きフィールドを**モジュール外から**触った場合だけ `E0616`。**`E0615` / `E0609` は `#[diagnostic::…]` で文言を差し替えられない**ので、`E0616` が持つ誘導は得られない。
>
> 加えて、**derive は入力と同名のアイテムを追加できない**ので、上の `pub struct User(UserRepr)` は `#[derive(Domain)]` が出せる形ではない（`E0428`）。ただし `as_repr(&self) -> &Repr` という**署名自体は derive で満たせる**形が複数ある（実測）ので、「署名と derive が両立しない」わけではない。どのマクロ形にするかは**未確定**（#18）。
>
> 判定の全文と21プローブの表は [`persistence.md`](./persistence.md) §判定 と `spikes/domain-opacity-sqlx/README.md`。

---

## 型表現

### 1. Field マーカ型（derive生成）

```rust,ignore   // module shown without its imports; `Field` is in scope in the real file
pub mod user {
    pub struct Id;
    pub struct Name;
    pub struct Email;

    impl Field<User> for Name {
        const NAME: &'static str = "name";
        type Ty = String;
    }
}
```

### 2. Mutationは Capability を要求する

Repositoryへのアクセスは拡張trait経由で提供される（inherent implはorphan ruleにより不可 — [`rust-type-model.md`](./rust-type-model.md)）。

```rust
// derive が Domain ごとに生成する拡張 trait
pub trait UserRepo<M> {
    fn set_email<I>(&self, u: &mut User, v: Email) -> Result<()>
    where M: Has<Mutate<User, user::Email>, I>;

    fn set_name<I>(&self, u: &mut User, v: String) -> Result<()>
    where M: Has<Mutate<User, user::Name>, I>;
}
```

`M` は `E::Mutates`（Contractから展開されたcons list）。宣言していないFieldのsetterはwhere節が満たされないため、**呼び出しがコンパイルエラーになる**。

> 注: `I` は所属判定の推論用パラメータ。`Has` の再帰implがcoherenceを満たすために必要（[`rust-type-model.md`](./rust-type-model.md)）。deriveが生成するため利用者は書かない。

### 3. ハンドラからの呼び出し

```rust,ignore   // fragment, not a complete item
ctx.users().set_email(&mut user, req.email)?;
```

Capabilityトークンは引数に現れない（`Ctx<'req, Self>` が保持）。判断理由は [`capability-system.md`](./capability-system.md) を参照。

---

## MustNotMutate は宣言不要

「そのFieldのCapabilityが発行されない」ことで自然に成立する。

```rust,compile_fail
// Contract に User::id / User::created_at がない
ctx.users().set_id(&mut user, other_id)?;
//          ^^^^^^ 型エラー（E0277）: where 節が満たされない
```

> **正確な表現について**: setterはDomainのFieldごとにderiveが生成するため、`set_id` というメソッド自体は**存在する**。満たされないのはwhere節であり、出るエラーはE0277（メソッド不在のE0599ではない）。
>
> 実務上の帰結として、**rust-analyzerはGETのハンドラでも `set_email` を補完し続ける**。「そもそも呼び出せない」という体感は得られず、「呼び出すとコンパイルエラーになる」が正確な保証である。

---

## `forbidden` の意味論

> Q-C実験で「`forbidden` の仕様がチートシートに未記載で、マクロが実際に何を検証するのか確証がない」と指摘された箇所。仕様として確定させる。

### 定義

```rust,ignore   // needs a macro that arrives in M2
#[contract(
    mutates   = [User::name, User::email],
    forbidden = [User::status, User::password_hash],
)]
```

**`forbidden` は宣言的な意図表明であり、型検査上は冗長である。** `mutates` に無いFieldはCapabilityが発行されないため、`forbidden` に書かなくても呼び出しはコンパイルエラーになる。

macroが検査するのは**1点だけ**。

```text
error: `User::status` is declared both in `mutates` and `forbidden`
  --> src/endpoints/user.rs:18:5
   |
17 |     mutates   = [User::name, User::status],
   |                              ^^^^^^^^^^^^ declared mutable here
18 |     forbidden = [User::status],
   |                  ^^^^^^^^^^^^ and forbidden here
   |
   = help: remove one of them
```

条件付きMutation（`when` 内の `mutates`）との重複も同様に弾く。

### 冗長なのに残す理由 — 実験で判明した価値

Q-C実験のタスクCで、被験者は「email変更時に status を Unverified に戻す」という要件に対し、**`forbidden` から `User::status` を削除してから `mutates` に追加した**。

この削除操作が**diffに残った**。

一方、生成メタデータ方式（条件2）では契約ファイルが更新されず、**同じ緩和がdiffに一切現れなかった**。

> `forbidden` は型強制の手段ではなく、**意図の記録装置**である。
>
> 「絶対に変更しない」と宣言したものを解除する操作を、明示的な削除としてdiffに残す。これがQ-C実験で確認された唯一の実質的な差別化点だった。

詳細は [`evaluation.md`](./evaluation.md) を参照。

### 使いどころ

全Fieldについて禁止を書かせると冗長になり、書き忘れと意図の区別もつかなくなる。**原則は「宣言されたmutatesのみが可能」**とし、`forbidden` は以下の場合に限る。

- セキュリティ上「絶対に触らない」ことを記録したいField（`password_hash` 等）
- 業務上の不変条件（`created_at`、`id`）
- レビュー時に注意を向けたいField

### AI Contextでの扱い

型で強制していないことを隠さない。

```json
"forbidden": {
  "fields": ["User.status", "User.password_hash"],
  "enforcement": "intent_only",
  "note": "Not type-enforced. Fields absent from `mutates` are already uncallable; this records intent."
}
```

`enforcement: "intent_only"` として、`upper_bound_checked` と区別する。

---

## 条件付き Mutation

条件下でのみ変更されるFieldは、**`when` ブロック内に宣言する**。

```rust,ignore   // needs a macro that arrives in M2
#[contract(
    mutates   = [User::name],              // 無条件
    when(EmailChanged) => {
        mutates = [User::email],           // この条件下でのみ
    },
)]
```

トップレベルと `when` 内に同じFieldを書くことは禁止（macroが弾く）。

実効的な変更可能Field集合は `mutates ∪ 全 when の mutates` であり、AI Contextには合算した完全形を出力する。

詳細は [`conditional-effects.md`](./conditional-effects.md) を参照。

---

## Field単位メソッドの強制

Repositoryは包括的な `save()` / `update()` を**提供しない**。

```rust,compile_fail
// ❌ 提供しない
ctx.users().save(&mut user)?;
```

理由は2つ。

1. **型検査が効かなくなる** — `save` は「全Fieldを書き戻す」操作であり、どのCapabilityを要求すべきか決まらない
2. **実装が読めなくなる** — Contractが正しくても、実装のどの行で何が変わったか分からない

これは [`handler-rules.md`](./handler-rules.md) Rule 1 として仕様化されている。

> ただしRead側については、N+1回避（eager loading）との衝突が未解決。[`research-questions.md`](./research-questions.md) を参照。

---

## reads との関係

`mutates` に宣言したFieldは、変更前の値を読む必要があるため**自動的に `reads` に含まれる**。

```rust,ignore   // needs a macro that arrives in M2
#[contract(
    reads   = [User::id, User::status],
    mutates = [User::name, User::email],
)]
// 実効的な read 集合: id, status, name, email
```

詳細は [`read-contract.md`](./read-contract.md) を参照。

---

## 到達範囲の限界

型検査はメソッド呼び出しの可否までしか及ばない。**Repository実装内部のSQLには届かない。**

```rust,compile_fail
impl UserRepository for PgUserRepository {
    async fn set_email(&self, u: &mut User, v: Email) -> Result<()> {
        sqlx::query!("UPDATE users SET email = $1, status = 'x' WHERE id = $2", ...)
        //                                        ^^^^^^^^^^^^ 宣言外だが検出できない
    }
}
```

Repository実装が信頼境界になる。詳細と緩和策は [`persistence.md`](./persistence.md) を参照。

**Domain不透明化後も残る経路**は [`unverified-boundaries.md`](./unverified-boundaries.md) に列挙されている。特に:

- `*user = other_user`（`find` で2件取って全体置換）
- 行レベル権限（`Mutate<User, Email>` は「この User」を意味しない）

---

## 未解決の課題

- **ソフトデリート** — `mutates = [User::deleted_at]` と `mutates = [User::name]` が構文上区別できない。"Semantics over Syntax" がCRUD最頻出パターンで機能しない
- **楽観ロック** — Field単位setterでは `WHERE id=? AND version=?` のCompare-and-Swapを原子操作として表現できない
- **Bulk operation** — 100件一括更新をField単位setterでどう書くか
- **一覧・集計・JOIN** — `Read<Domain, Field>` が単一インスタンス前提であることの帰結

[`research-questions.md`](./research-questions.md) を参照。

---

## 検証項目

- Field-level Mutationを型で表現できる
- 宣言外のMutationがコンパイルエラーになる
- MustNotMutateなFieldへの変更経路が存在しない
- **Domain不透明化がsqlxと噛み合う** — ✅ **成立**（T-M1-01 / #13 で実測）。ただし「信頼境界 = Repository 実装」は不成立（台帳 path 21）
- **`pub` フィールドをderiveが拒否できる**
- エラーメッセージがContract宣言箇所を指す（[`diagnostics.md`](./diagnostics.md)）
