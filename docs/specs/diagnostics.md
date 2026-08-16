# Diagnostics

コンパイルエラーメッセージの設計。AIのiteration数に直結する第一級の仕様。

関連: [`rust-type-model.md`](./rust-type-model.md) / [`evaluation.md`](./evaluation.md) / [`unverified-boundaries.md`](./unverified-boundaries.md)

> このファイルの内容は `rustc 1.99.0-nightly` で実際にコンパイルして確認した結果を反映している。

---

## なぜ仕様として扱うか

Verumは「AIが間違ったコードを書いたらコンパイルで弾く」設計である。したがって**AIが最も頻繁に読むのはエラーメッセージ**になる。

```text
契約違反を検出できた
        ↓
しかしAIが原因を理解できない
        ↓
iteration数が増加
        ↓
AI Coding性能が従来Frameworkより悪化する
```

**エラーメッセージの質は、型システムの強さと同等に重要な設計対象である。**

---

## 3層の防御 — どこでエラーを出すか

エラーの精度は**どの層で検出するか**で決まる。上の層ほど精密。

| 層 | 検出できるもの | span の精度 |
|---|---|---|
| **1. proc macro（展開時）** | `pub` フィールド、重複要素、GETなのに mutates がある。**存在しないField / Domain は層1では検出できない** — 下記 | **最高**（属性内トークンを指せる） |
| **2. associated type equality bound** | `Mutates = ()` 違反（GETのread-only） | 高（`type Mutates` の定義位置を指す） |
| **3. trait bound（`Has` / `Includes`）** | 宣言外のMutation、Domainアクセス違反 | **低**（span を持てない — 下記） |

### 設計ルール: 上の層で弾けるものは上で弾く

```text
proc macro で弾ける → macro で弾く（span が精密、エラーが1つに収まる）
        ↓ 弾けないもの
equality bound で表現できる → そうする（span 付き note が出る）
        ↓ それも無理
trait bound + on_unimplemented + do_not_recommend
```

---

## 層1: proc macro のエラー

最も精密。**Contract宣言箇所のspanを指せる。**

### 存在しないField

> ### ⚠️ 下の `note:` は出せない（#43 で訂正）
>
> `#[contract(...)]` は Endpoint の unit struct に付く。**proc macro は単一アイテムのトークンしか見えない**ため（[`rust-type-model.md`](./rust-type-model.md)、実測）、別アイテムである `struct User` のフィールド一覧を知る手段が無い。したがって「`User` has fields: …」は**どの層からも出せない**。
>
> **出せる半分**: マクロが `user::Statuss` のようなマーカ型への参照を展開すれば、rustc 自身の名前解決が `help: did you mean` を出す。つまり typo 補正は層1の機能ではなく **rustc の機能**であり、`did you mean` の品質はマクロ側で制御できない。

```text
error[E0412]: cannot find type `Statuss` in module `user`
  --> src/endpoints/user.rs:18:32
   |
18 |     mutates   = [User::name, User::statuss],
   |                              ^^^^^^^^^^^^^ help: a struct with a similar name exists: `Status`
```

### `pub` フィールドの拒否

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

`pub` フィールドは Contract 全体を無効化する唯一の経路なので、macro で必ず弾く（[`mutation-contract.md`](./mutation-contract.md)）。

### GET なのに mutates がある

```text
error: GET endpoint `GetUser` cannot declare mutations
  --> src/endpoints/user.rs:16:5
   |
16 |     mutates = [User::name],
   |     ^^^^^^^^^^^^^^^^^^^^^^ GET endpoints are read-only by construction
   |
   = help: use PUT / PATCH / POST / DELETE, or remove this declaration
```

型検査（層2）でも検出できるが、macro の方がエラーが精密なので**両方実装する**。

`when` ブロック内の `mutates` については**macroだけが検出できる**（`Conditional` に対する再帰的な畳み込みを避けるため — [`rust-type-model.md`](./rust-type-model.md)）。

```text
error: GET endpoint `GetUser` cannot declare mutations
  --> src/endpoints/user.rs:18:9
   |
18 |         mutates = [User::status],
   |         ^^^^^^^^^^^^^^^^^^^^^^^^ inside `when(...)` on a GET endpoint
   |
   = note: read-only methods are GET and HEAD
```

### 無条件と条件付きの重複宣言

```text
error: `User::email` is declared both unconditionally and under `when(EmailChanged)`
  --> src/endpoints/user.rs:12:28
   |
12 |     mutates = [User::name, User::email],
   |                            ^^^^^^^^^^^^ declared unconditionally here
...
17 |         mutates = [User::email],
   |                    ^^^^^^^^^^^^ and conditionally here
   |
   = help: remove one of them — a field is either unconditional or conditional
```

macroで弾かなければ、Append後に重複が生じて `Has` のindex推論が壊れ、E0283（型注釈が必要）という無関係なエラーになる。

### `mutates` と `forbidden` の矛盾

```text
error: `User::status` is declared both in `mutates` and `forbidden`
  --> src/endpoints/user.rs:18:18
   |
17 |     mutates   = [User::name, User::status],
   |                              ^^^^^^^^^^^^ declared mutable here
18 |     forbidden = [User::status],
   |                  ^^^^^^^^^^^^ and forbidden here
   |
   = help: remove one of them
```

`forbidden` が検査するのはこの1点のみ（型強制ではなく意図の記録装置 — [`mutation-contract.md`](./mutation-contract.md)）。

### 重複要素

index パラメータ方式は「要素がちょうど1回だけ現れる」ことを前提とし、重複すると E0283（型注釈が必要）という無関係なエラーになる。macro で弾く。

```text
error: duplicate mutation `User::email`
  --> src/endpoints/user.rs:18:30
   |
18 |     mutates = [User::email, User::email],
   |                             ^^^^^^^^^^^^ already declared here
```

---

## 層2: associated type equality bound

`Mutates = ()` 違反では**span付きnoteが出る**（検証済み）。

```text
error[E0271]: type mismatch resolving `<GetUser as Endpoint>::Mutates == ()`
   |
note: expected this to be `()`
   |     type Mutates = (Mutate<User, user::Email>, ());
   |                    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   = note: expected unit type `()` found tuple `(Mutate<User, user::Email>, ())`
```

`note:` がderive生成の `type Mutates` のspanを指す。**deriveがこの型を contract 属性のトークンspanで生成すれば、Contract宣言箇所を指すnoteに変わる。**

これは層3では実現できない唯一の経路なので、equality bound で表現できるものは積極的にこの形にする。

---

## 層3: trait bound — 到達可能な形

### 目標

```text
error[E0277]: undeclared mutation `User::status`
  --> src/endpoints/user.rs:42:21
   |
42 |         ctx.users().set_status(&mut user, UserStatus::Suspended)?;
   |                     ^^^^^^^^^^ not declared in this endpoint's contract
   |
   = note: `UpdateUser` declares mutates = [User::name, User::email]
   = help: add `User::status` to the contract, or remove this call
```

### 実現手段

**(a) `#[diagnostic::on_unimplemented]`（1.78+）**

```rust,ignore   // needs a macro that arrives in M2
#[diagnostic::on_unimplemented(
    message = "undeclared mutation `{Domain}::{Field}`",
    label = "not declared in this endpoint's contract",
    note = "add it to #[contract(mutates = [...])] or remove this call"
)]
pub trait CanMutate<Domain, Field> {}
```

`{Domain}` / `{Field}` プレースホルダは型パラメータ名で正しく展開される（検証済み）。

> ただし**パス修飾は落ちる**。`{Field}` は `user::Email` ではなく `Email` になる。contract に書く形（`User::email`、小文字）とも一致しない中間形になるため、message には `Field::NAME` 由来の文字列を埋める工夫が必要。

**(b) `#[diagnostic::do_not_recommend]`（1.85+）— 必須**

`on_unimplemented` は message / label / note を制御するだけで、**その下に続く help / note の連鎖は残る**。素朴な実装では約20行になり、cons list と `There<There<...>>` の index 型が丸ごと露出する。

```text
help: the following other types implement trait `Has<T, I>`
   | impl<H, T> Has<H, Here> for (H, T) {}
   | impl<H, X, T, I> Has<H, There<I>> for (X, T) where T: Has<H, I> {}
note: required for `(Mutate<User, Email>, ())` to implement `Has<Mutate<User, Status>, There<_>>`
   = note: 1 redundant requirement hidden
   = note: required for `(Mutate<User, Name>, (Mutate<User, Email>, ()))` to implement `Has<..., There<There<_>>>`
```

再帰implに `#[diagnostic::do_not_recommend]` を付けると20行→10行になり、**失敗する型が `()`（末尾）ではなく実際のcontractタプルとして表示される**（検証済み）。

```rust,ignore   // verum-internal: legal only inside the crate that owns the trait or type
#[diagnostic::do_not_recommend]
impl<H, T> Has<H, Here> for (H, T) {}

#[diagnostic::do_not_recommend]
impl<H, X, T, I> Has<H, There<I>> for (X, T) where T: Has<H, I> {}
```

**(c) where節はメソッド側に置く**

implに置くとE0599になり `on_unimplemented` が**無視される**（検証済み）。

```text
// ❌ impl に where
error[E0599]: the method `orders` exists for struct `Ctx<UpdateUser>`,
              but its trait bounds were not satisfied

// ✅ method に where
error[E0277]: `Order` is not in this endpoint's domain contract
```

deriveの生成テンプレートで固定する。

---

## 到達できないこと

### Contract宣言箇所を指す `note:` は trait bound 経由では出せない

```text
note: `UpdateUser` declares mutates = [User::name, User::email]
  --> src/endpoints/user.rs:18:5      ← この行番号は出ない
18 |     mutates   = [User::name, User::email],
```

`on_unimplemented` の note は**プレーンテキストのみでspanを持たない**。rustcが出すspanは `Has` のimpl定義位置であって contract 属性ではない。

> **訂正**: [`semantic-endpoint.md`](./semantic-endpoint.md) で当初「これが属性方式を選んだ主要な理由」としていたが、**trait bound違反については成立しない**。属性方式の優位は「層1（macro）で弾けるエラーの精度」にある。

### 代替手段

**deriveがEndpointごとに専用traitを生成し、宣言内容を文字列リテラルとしてnoteに埋める。**

```rust
// derive 生成
#[diagnostic::on_unimplemented(
    note = "`UpdateUser` declares mutates = [User::name, User::email]"
)]
trait UpdateUserCanMutate<F> {}
```

行番号は出ないが内容は伝わる。**AI向けとしてはこれで十分な可能性が高い**（AIは行番号よりも「今何が宣言されているか」を必要とする）。

---

## 設計ルール

| ルール | 理由 |
|---|---|
| 層1（macro）で弾けるものは層1で弾く | spanが精密になり、エラーが1つに収まる |
| equality bound で表現できるものはそうする | span付きnoteが出る唯一の経路 |
| 型パラメータを含むtraitには必ず `on_unimplemented` を付ける | 生のtrait解決エラーを露出させない |
| 再帰implには必ず `do_not_recommend` を付ける | cons list と index 型の露出を抑える |
| helpは必ず2方向を示す（契約を広げる / 実装を直す） | 片方だけだとAIが機械的にContractを緩める |
| where節はメソッド側に置く | implに置くと `on_unimplemented` が無視される |
| エラー1件につき1つの原因を示す | タプル型の展開が連鎖して複数エラーになるのを避ける |

### 「helpは2方向」の限界

これは文言レベルの対策であり、**AIの選択そのものは制約できない**。AIには常に第3の選択肢（Service層でやる / 生SQL / イベント経由）もある。

型では解決しないため、CIでContract拡大差分を検出する等の運用対策が必要。[`unverified-boundaries.md`](./unverified-boundaries.md) を参照。

---

## 検証方法

エラーメッセージは仕様であるため、テストで固定する。

```text
tests/ui/undeclared_mutation.rs
tests/ui/undeclared_mutation.stderr   ← 期待されるエラー全文
```

`trybuild` によるUIテストを標準とし、**First PoCから導入する**。型設計とエラー設計を同時に検証しなければ、後から文言を整えるコストが高くなる。

> **運用上のリスク**: `There<There<...>>` や cons list を含むエラー文言は rustc バージョン間で揺れやすい。`do_not_recommend` で露出を抑えた上で、揺れる部分をテストから除外する仕組み（正規化）が必要になる可能性がある。

---

## 未解決の課題

- deriveが型エイリアスを生成してエラー中の型名を短縮できるか
- `when` スコープ外での条件付きEffect発火時のエラー設計（入れ子projectionが露出する）
- Projection型のフィールドアクセスエラーの設計
- 拡張traitの `use` 忘れによる「no method named `users`」という無関係なエラーへの対処（prelude / `pub use` の自動生成）

[`research-questions.md`](./research-questions.md) を参照。
