# Rust Type Model

Verumの型表現にRustのどの機能を使うか。および実コンパイルで確認した制約。

関連: [`capability-system.md`](./capability-system.md) / [`diagnostics.md`](./diagnostics.md) / [`unverified-boundaries.md`](./unverified-boundaries.md)

> このファイルの制約は `rustc 1.99.0-nightly` で実際にコンパイルして確認した結果を反映している。

---

## 前提条件

| 項目 | 要件 | 理由 |
|---|---|---|
| **edition** | **2024** | `when` スコープに async closure (`AsyncFnOnce`) が必須 |
| **MSRV** | **1.85+** | async closure / `#[diagnostic::do_not_recommend]` |

`runtime-stack.md` の依存方針と併せて仕様として固定する。

---

## 利用する機能

- Trait / Associated Type / Associated Const
- Generic / Phantom Type / Typestate
- Newtype
- Proc Macro / Derive Macro
- **Associated type equality bound**（`Endpoint<Mutates = ()>`）— stable
- **`#[diagnostic::on_unimplemented]`**（1.78+）
- **`#[diagnostic::do_not_recommend]`**（1.85+）— 再帰implのノイズ除去
- **async closure / `AsyncFnOnce`**（1.85+, edition 2024）
- sealed trait（privateなsupertrait）

### 使えない機能

| 機能 | 状態 |
|---|---|
| **Associated const equality bound**（`Endpoint<METHOD = Method::GET>`） | **unstable**。`min_generic_const_args` に統合済み（incomplete）。trait側を `type const METHOD: Method;` と宣言する新構文も必要 |
| negative trait bound（`!Trait`） | unstable |
| 型パラメータのワイルドカード（`NotHas<Mutate<_, _>>`） | 書けない |
| inherent impl（フレームワーク型に対する利用者クレートからのimpl） | E0116。拡張traitで代替 |

---

## Endpoint trait

```rust,ignore   // needs a crate or a verum-private module this harness does not carry
pub trait Endpoint: derive_facing::SealedEndpoint {
    type Method;                 // Get / Head / Post / Put / Patch / Delete
    const PATH: &'static str;

    type Domain;
    type Request;
    type Response;

    // 無条件に起こり得る Effect
    type Reads;
    type Mutates;
    type Creates;
    type Deletes;
    type Emits;
    type Calls;

    // 条件下でのみ起こり得る Effect
    // (When<C, CondMutates, CondEmits, CondCalls>, (..., ()))
    type Conditional;
}
```

`Conditional` の要素は**カテゴリ別に分割された `When`** である。混在させると型レベル `Filter` が必要になり、catch-all implが必ず衝突する。

```rust
pub struct When<C, CondMutates, CondEmits, CondCalls>(PhantomData<(C, CondMutates, CondEmits, CondCalls)>);
```

宣言場所の規則（トップレベル = 無条件、`when` 内 = 条件付き、重複禁止）は [`conditional-effects.md`](./conditional-effects.md) を参照。

### Methodは型レベルマーカにする

`const METHOD: Method` ではなく `type Method = Get` とする。理由は2つ。

1. associated **const** equality bound はunstable（上記）
2. **より本質的に、`impl<E: Endpoint<METHOD = Get>> ReadOnly for E {}` は論理が成立しない**

`ReadOnly: Endpoint<Mutates = (), ...>` をsupertraitに持つ以上、blanket impl側でも `Mutates = ()` を要求せざるを得ない。

```rust,compile_fail
impl<E: Endpoint<Method = Get>> ReadOnly for E {}
// error[E0271]: type mismatch resolving `<E as Endpoint>::Deletes == ()`
```

つまりimplできるのは `impl<E: Endpoint<Mutates=(), Creates=(), Deletes=()>> ReadOnly for E {}` だけで、これは**METHODについて何も強制しない**。「GETなら必ずReadOnly」をimplで強制する経路は存在しない。

### GET ⇒ ReadOnly の強制方法

deriveが生成するコンパイル時アサーションで強制する。

```rust
// derive 生成
const _: () = {
    fn assert_readonly<E: Endpoint<Method = Get> + ReadOnly>() {}
    fn check() { assert_readonly::<GetUser>(); }
};
```

このとき出るエラーは目標形式に一致する（検証済み）。

```text
error[E0271]: type mismatch resolving `<BadGet as Endpoint>::Mutates == ()`
note: expected this to be `()`
   |     type Mutates = (MutateEmail, ());
   = note: expected unit type `()` found tuple `(MutateEmail, ())`
```

しかも `note:` がderive生成の `type Mutates` のspanを指す。**spanをcontract属性のトークンに付け替えれば [`diagnostics.md`](./diagnostics.md) の理想形に到達できる。**

さらに単純な代替として、proc macroが展開時点で「GETなのにmutates/creates/deletesがある」を弾く方法もある。エラーが最も精密になるため、両方実装する。

### `Conditional` 内の Mutation は macro で弾く

条件付きMutationを `when` 内に宣言する規則（[`conditional-effects.md`](./conditional-effects.md)）を採ったため、`Mutates = ()` だけではread-onlyを保証できない。`Conditional` 内の `CondMutates` も空でなければならない。

型で検査するには `Conditional` に対する再帰的な畳み込み（`AllCondMutatesEmpty`）が必要で、negative reasoningに近づき、エラーメッセージも「どの要素が原因か分からない」形に悪化する。

**macroで弾く。** read-only なMethod（`Get` / `Head`）では `when` 内にも `mutates` / `creates` / `deletes` を書けない。層1で弾けるものは層1で弾く（[`diagnostics.md`](./diagnostics.md)）。

---

## Effect集合は cons list で表現する

**フラットタプル `(A, B, C)` では所属判定を実装できない。**

```rust,compile_fail
// ❌ フラットタプルに位置ごとの impl を書くと必ず E0119
impl<A, B> Has<A> for (A, B) {}
impl<A, B> Has<B> for (A, B) {}
// error[E0119]: conflicting implementations of trait `Has<_>` for type `(_, _)`
```

利用者が重複要素を書くかどうかに関係なく、**impl定義の時点で落ちる**。

したがって cons list に統一する。

```rust
type Mutates = (Mutate<User, user::Name>, (Mutate<User, user::Email>, ()));
```

deriveが生成するため利用者は書かないが、**エラーメッセージには cons list が露出する**。

| 集合 | 表現 |
|---|---|
| 空 | `()` |
| 1要素 | `(A, ())` |
| 2要素 | `(A, (B, ()))` |

---

## `Has<Set, Elem, Idx>` — index パラメータが必須

素朴な再帰implはcoherence違反になる。

```rust,compile_fail
// ❌ coherence 違反
pub trait Has<T> {}
impl<H, T> Has<H> for (H, T) {}
impl<H, X, T> Has<H> for (X, T) where T: Has<H> {}
// error[E0119]: conflicting implementations
```

`H == X` のときに2つのimplが重複する。理由は「where節が無視される」ことでは**なく**、tail側の `T: Has<H>` が**その交差点では充足可能**なので impl を分離しないことである（T-M0-08 で訂正。この区別が seal の設計根拠 — [`../rules/api-surface.md`](../rules/api-surface.md) §2）。

frunk方式のindex型パラメータで解決する（検証済み）。

```rust,ignore   // needs a crate or a verum-private module this harness does not carry
pub trait Has<T, Idx>: private::SealedHas<T, Idx> {}

// T-M0-07 で追加。bound は trait ではなく impl 側に置く（呼び出し側に再掲させない）。
pub trait ConsList: private::SealedConsList {}   // 形の well-formedness
pub trait Index: private::SealedIndex {}         // 所属位置

pub struct Here(PhantomData<()>);              // 私有フィールド: downstream で構築不可（E0423）
pub struct There<I>(PhantomData<fn() -> I>);   // `fn() -> I`: `I` の auto trait を継承しない

#[diagnostic::do_not_recommend]
impl<H, T: ConsList> Has<H, Here> for (H, T) {}

#[diagnostic::do_not_recommend]
impl<H, X, T: ConsList, I: Index> Has<H, There<I>> for (X, T) where T: Has<H, I> {}
```

### 代償

`Has` を使う**全メソッドに推論専用の型パラメータ `I` が付く**。

```rust,ignore   // fragment, not a complete item
fn set_email<I>(&self, u: &mut User, v: Email) -> Result<()>
where M: Has<Mutate<User, user::Email>, I>;
```

deriveが生成するため利用者は書かないが、ドキュメント中の全シグネチャがこの形になる。

### 重複要素で E0283 になる

index方式は「要素がちょうど1回だけ現れる」ことを前提とする。重複すると `I` が一意に決まらない。

```text
error[E0283]: type annotations needed
note: multiple `impl`s satisfying `(Mn, (Mn, ())): Has<Mn, _>` found
```

発生経路が2つある。

1. AIが `mutates = [User::email, User::email]` と重複を書く
2. **`when` スコープで外側のEmitsと条件付きEmitsをAppendした結果、同じEffectが重複する**（`emits = [UserUpdated]` かつ `when(X) => { emits = [UserUpdated] }`）

2は正当なContractなのに壊れる。**deriveが重複を検出して弾き、**`Append` を呼ぶ前に**dedupする（**`Append` 自身は dedup できない** — T-M0-09 で確定。[`../rules/type-level.md`](../rules/type-level.md) §3）。**

---

## 型レベル演算の可否

| 演算 | 可否 | 用途 |
|---|---|---|
| `Has<Set, Elem, Idx>`（実装は `Has<T, Idx>`、`Self` が集合） — 単一要素の所属判定 | **安全**（要素数に線形） | Capability検査 |
| `Append<A, B>`（実装は `Append<B>`、`Self` が左辺） — cons listの連結 | **安全**（coherence問題なし、index 不要、検証済み） | `when` スコープのCapability合成 |
| `Lookup<Set, Key, Idx>` — 型レベルmap検索 | **安全**（indexパラメータ版なら可、検証済み） | `Conditional` から条件を引く |
| `Subset<A, B>` — 集合同士の包含判定 | **避ける**（組み合わせ爆発） | — |
| `Filter<Set, Pred>` — 型レベルfilter | **避ける**（catch-all implが必ず衝突する） | — |
| negative reasoning（`NotHas`） | **不可** | `Mutates = ()` で代替 |

> 当初「集合演算を避ける、必要なのは単一要素の所属判定だけ」と記述していたが、**Conditional Effect は Lookup と Append を要求する**。方針を上記の表に精緻化した。
>
> `Filter` が必要になるのを避けるため、**deriveは `Conditional` をカテゴリ別に分割して生成する**（`When<C, CondEmits, CondCalls, ...>`）。これは必須の設計制約である。

---

## Field マーカ型

```rust,ignore   // needs a macro that arrives in M2
#[derive(Domain)]
pub struct User {
    id:    UserId,      // private 必須（pub は derive がエラー）
    email: Email,
}

// 生成
pub mod user {
    pub struct Id;
    pub struct Email;

    impl Field<User> for Email {
        const NAME: &'static str = "email";
        type Ty = Email;
    }
}
```

`const NAME: &str`（ライフタイム省略込み）と `type Ty` はともに問題なく動作する（検証済み）。

Domainを不透明型にする理由は [`mutation-contract.md`](./mutation-contract.md) を参照。

---

## Ctx / Repo は拡張traitで提供する

`Ctx` / `Repo` / `Projection` はフレームワーク側の型。**inherent implは型を定義したクレートでしか書けない**（E0116）。deriveは利用者クレートで動くため、ドキュメント初期案の形は原理的に不可能。

```rust,compile_fail
// ❌ 利用者クレートでは書けない
impl<E: Endpoint> Ctx<E> { fn users(&self) -> Repo<User, ...> { ... } }
// error[E0116]: cannot define inherent `impl` for a type outside of the crate
```

deriveがDomainごとにローカルな拡張traitを生成する（2クレート構成で検証済み）。

```rust,ignore   // fragment, not a complete item
pub trait CtxUsers {
    type R; type M;
    fn users(&self) -> Repo<User, Self::R, Self::M>;
}

impl<'req, E: Endpoint> CtxUsers for Ctx<'req, E> { ... }

pub trait UserRepo<M> {
    fn set_email<I>(&self, u: &mut User, v: Email) -> Result<()>
    where M: Has<Mutate<User, user::Email>, I>;
}

impl<R, M> UserRepo<M> for Repo<User, R, M> { ... }
```

### 副作用

- 利用者が拡張traitを `use` する必要がある。忘れると「no method named `users`」という無関係なエラーになる → **deriveが `pub use` を吐くか `verum::prelude` を提供する**
- associated type経由になるため型名が伸びる（`Repo<User, <Ctx<E> as CtxUsers>::R, _>`）

### where節はメソッド側に置く

implに置くとE0599になり、`on_unimplemented` が無視される（検証済み）。

```text
// ❌ impl に where → 意図したメッセージが出ない
// error[E0599]: the method `orders` exists ... but its trait bounds were not satisfied

// ✅ method に where → 意図通り
// error[E0277]: `Order` is not in this endpoint's domain contract
```

**deriveの生成テンプレートで固定する。**

---

## Handler trait — RPITIT + Send + 消去レイヤ

AFIT（`async fn` in trait）をそのまま使うと2つの問題がある（検証済み）。

```text
// 1. Router が Box<dyn Handler> を持てない
// error[E0038]: the trait `Handler` is not dyn compatible

// 2. tokio::spawn / hyper に載せられない
// error: future cannot be sent between threads safely
```

対処:

```rust
// Send は RPITIT で解決
pub trait Handler: Endpoint {
    fn handle(&self, req: Self::Request, ctx: Ctx<'_, Self>)
        -> impl Future<Output = Result<Self::Response>> + Send;
}
```

dyn互換性は解決しないため、**deriveがobject-safeな消去レイヤを生成する**。

```rust,ignore   // fragment, not a complete item
fn call(&self, req: Request<Body>)
    -> Pin<Box<dyn Future<Output = Response> + Send + '_>>;
```

Routerは `dyn ErasedHandler` を持つ。Middleware chainも同じ制約を受けるため、[`runtime-stack.md`](./runtime-stack.md) の行数見積もりに消去レイヤのコストを含める必要がある。

---

## Capability の実行時表現

Capabilityは値として実体化せず、**`Ctx<'req, E>` の型パラメータを通じて表現する**。すべてZSTであり、Runtimeに実体を持たない（[`performance.md`](./performance.md)）。

> [`persistence.md`](./persistence.md) のRepository trait定義に `cap: &Cap<...>` 引数を書いていたが、この方針と矛盾するため削除した。Capabilityは引数として渡さない。

---

## proc macroの可視範囲

proc macroは単一アイテムのトークンしか見えず、呼び出し先の本体は見えない。

ただし [`handler-rules.md`](./handler-rules.md) Rule 2 により、**Effectは `handle` という単一アイテムの中に構文的に閉じ込められている**。impl ブロックの属性マクロは `handle` の本体トークンを全て見られる。

この事実は「実装からContractを生成する」方式の実現可能性を意味する。[`effect-inference.md`](./effect-inference.md) を参照。

一方、macroが単一アイテムを見られることは属性内spanの保持にも使える（[`diagnostics.md`](./diagnostics.md)）。

---

## 未解決の問い

- cons list + `There<There<...>>` がエラーメッセージに露出する問題（`do_not_recommend` で緩和済みだが完全ではない）
- deriveが型エイリアスを生成して短い名前を出せるか
- ~~Domain不透明化とsqlx `query_as!` / `FromRow` の相互運用~~ — **検証済み（T-M1-01 / #13）**。連携は成立、信頼境界の主張は不成立。[`persistence.md`](./persistence.md) §判定
- `Ctx<'req, E>` と RPITIT / async closure の組み合わせ — **測定済み**（T-M1-02 / #14、`spikes/ctx-lifetime-rpitit/`）。判定の仕様反映は #38 の完了待ち
- Projection型のergonomics
- Rust以外（Go / TypeScript）への展開可能性

[`research-questions.md`](./research-questions.md) を参照。
