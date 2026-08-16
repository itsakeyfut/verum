# Capability System

Endpoint / Service / Middlewareが保持できる権限を型で表現する。Verumの中核機構。

関連: [`handler-rules.md`](./handler-rules.md) / [`unverified-boundaries.md`](./unverified-boundaries.md) / [`rust-type-model.md`](./rust-type-model.md)

---

## 中核思想

> AIに「それはやってはいけない」と説明するだけではなく、**呼び出しがコンパイルエラーになる状態を作る**。

「使ってはいけない」と説明するのではなく、「使えない」状態を型で作る。

> **表現の正確さについて**: 「そもそもメソッドが存在しない」ではない。setterはderiveが生成するため存在し、満たされないのはwhere節である（E0277）。rust-analyzerは補完し続ける。正確な保証は「**呼び出すとコンパイルが通らない**」である。

---

## 決定: Capabilityは `Ctx<'req, Self>` が保持する

```rust,compile_fail
async fn handle(&self, req: Req, ctx: Ctx<'req, Self>) -> Result<Res> {
    let mut user = ctx.users().find(req.id).await?;
    ctx.users().set_email(&mut user, req.email)?;
    //  ^ where Self::Mutates: Has<Mutate<User, user::Email>, I>
    //    宣言していなければここで型エラー
}
```

### `'req` lifetime を持つ理由

`Ctx` は**リクエスト寿命に縛られる**。`'static` ではないため、`tokio::spawn` に渡せない。

```rust,ignore   // fragment, not a complete item
tokio::spawn(async move { ctx.email().send(...).await });
//           ^^^^^^^^^^ error: `ctx` does not live long enough
```

これにより以下の経路が塞がれる。

| 経路 | 塞がれる理由 |
|---|---|
| `tokio::spawn` でCapabilityをリクエスト外に持ち出す | `'static` を満たさない |
| `static Sender<Ctx<E>>` へ譲渡し長寿命ワーカで行使する | 同上 |

**`Send` は保つ。** hyperのmulti-thread runtimeに載せるにはhandlerのFutureが `Send` である必要があるため、`Ctx` も `Send` でなければならない。`'static` を外すだけで目的は達成できる。

> invariantなブランドlifetime（GhostCell方式）まで踏み込めば持ち出しを完全に封じられるが、エラーメッセージが極端に読みにくくなり [`diagnostics.md`](./diagnostics.md) の目標と正面衝突する。`'req` で十分と判断した。

### spawnの代替経路を提供する

塞ぐだけでは、利用者は `tokio::spawn` を使わずに済む方法を失う。Verum経由のspawnを用意する。

```rust,ignore   // fragment, not a complete item
ctx.spawn::<SendEmailJob>(|jctx| async move { ... });
```

- `Spawn<SendEmailJob>` Effect の宣言が必要
- 子タスクには**縮小したCapabilityセット**を渡す
- Contractに現れるため、AI Contextから追跡できる

**「より楽な未検査経路を消す」ためには、塞ぐと同時に検査済みの代替経路を用意する必要がある。**

### 構築経路をsealedにする

`Ctx` のコンストラクタはsealedな `Runtime` トークンを要求する。

```rust,ignore   // fragment, not a complete item
impl<'req, E: Endpoint> Ctx<'req, E> {
    pub(crate) fn new(rt: &'req Runtime<Sealed>, ...) -> Self;
}
```

利用者は `Runtime<Sealed>` を構築できないため、任意のEndpoint型で `Ctx` を作れない。

これは**テストのgod-modeコンストラクタ問題**への対処である。`Ctx::for_test()` を公開すると、利用者が任意の `impl Endpoint` を定義して全Capabilityを持つ `Ctx` を構築できてしまう。`#[cfg(test)]` はクレート境界を越えないため、`test-util` featureが推移的に有効化されると本番バイナリに残る。

テストは**Endpoint型を利用者が自由に選べないAPI**経由に限定する。

```rust,ignore   // fragment, not a complete item
verum::test::run::<UpdateUser>(req, mocks).await
```

### 却下した案: Capabilityを明示引数として渡す

```rust,ignore   // fragment, not a complete item
ctx.users().set_email(&mut user, req.email, caps.mutate::<user::Email>())?;
```

明示引数が追加している情報 `user::Email` は、**`set_email` という名前から既に自明**である。記述量は増えるが情報は増えない。

さらに、明示引数案ではEndpointとRepositoryの紐付けを別途用意する必要があり、後述するArchitecture Contractの検査が無料で付いてこない。

ただしContext経由の設計は、自明性が保たれることを前提とする。前提条件は [`handler-rules.md`](./handler-rules.md) の3ルールとして仕様化されている。

### 却下した案: trait boundで表現

```rust,ignore   // fragment, not a complete item
async fn handle<C>(&self, req: Req, ctx: C) -> Result<Res>
where C: CanMutate<User, user::Name> + CanMutate<User, user::Email> + ...
```

where節がContractと同じ情報の二重管理になり、Single Source of Truthに違反する。

---

## sealed trait — 型による保証の前提条件

以下のtraitはすべて**sealed**（privateなsupertraitを持つ）にする。

```text
Endpoint    ← 手書き impl で任意の Capability を宣言できてしまう
Has         ← 所属判定を偽装できてしまう
Includes    ← impl Includes<Order> for User が orphan rule を通る
Field       ← Field::NAME を偽装すると生成SQLの列名を偽れる
Condition   ← 条件の実装は許すが、trait 自体の偽装は禁じる
ConsList    ← 形の証明を偽装でき、壊れた集合が well-formed として通る（T-M0-07）
Index       ← 所属位置を偽装できる（T-M0-07）
Append      ← 連結結果を偽装できる。type Out を持つので合成後の集合を名指しできる（T-M0-09）
Lookup      ← 「その鍵のエントリはこれ」を偽装でき、条件付きスコープを差し替えられる（T-M0-09）
```

> **このリストは3回続けて更新漏れになっていた**（T-M0-07 で2つ、T-M0-09 で2つ増えたのに追記されなかった）。
> **sealed 化だけでは足りない。** seal の解集合が trait の解集合と一致していなければ、差分の分だけ偽装できる —
> #6 / #8 / #9 で3回出荷された。判定基準と機械的な強制は [`../rules/api-surface.md`](../rules/api-surface.md) §2
> が正典で、[RK-015](../dev/code/review-knowledge.md) に経緯がある。**新しい sealed trait を足したらこの表にも足すこと。**

```rust,ignore   // fragment, not a complete item
mod private { /* seal! macro が trait ごとの seal を生成 */ }

pub trait Endpoint: derive_facing::SealedEndpoint { ... }
```

`Endpoint` の実装はderive経由のみとする（deriveがprivateマーカを実装する）。

### なぜこれが必須か

Verumは設計上**AIに大量のtrait boundエラーを見せる**。そしてAIがtrait boundエラーに対して最初に試す修正は「**不足しているimplを書く**」である。

```rust,compile_fail
// AI がやりうる「修正」— sealed 化する前はこれが通っていた
impl Includes<Order> for User {}   // ← Architecture Contract が消える
```

`User` はローカル型なのでorphan ruleを通り、`cargo build` は成功し、AI Contextは何も報告しない。**1行で保証が無効化されていた。** 現在は `Includes` が sealed なので `E0277` で拒否される（`spikes/doc-code-blocks` が毎回検証している）。sealed 化する前の状態を記録したものである。

sealed化は数行の作業だが、**後から適用すると破壊的変更になる**。First PoCの必須項目とする。

> `impl Has<Mutate<User, Password>> for ()` については、`Has` も `()` も外部型で `Mutate<User, ..>` はlocal typeではないため、orphan ruleで防がれる可能性が高い（未検証）。ただし `Includes` は確実に通るため、区別せず全traitをsealedにする。

---

## Endpointが持つCapability

```text
Endpoint<User, Update>
    ↓
Capabilities
    ├── Read<User, user::Status>
    ├── Mutate<User, user::Name>
    ├── Mutate<User, user::Email>
    └── Create<AuditLog>
```

Endpointは**unit struct のみ**を許可する。

```rust,ignore   // needs a macro that arrives in M2
#[endpoint(PUT "/users/{id}")]
pub struct UpdateUser;          // ✅

#[endpoint(PUT "/users/{id}")]
```

```rust,compile_fail
pub struct UpdateUser { pool: PgPool }   // ❌ derive がエラー
```

フィールドを持てると、`self.pool` から直接SQLを実行してctxを迂回できる。[`handler-rules.md`](./handler-rules.md) Rule 2 を型で成立させるための条件である。

---

## 実現方法: Repositoryの型をContractでパラメタライズする

`Ctx` はフレームワーク側の型なので、Domain固有のメソッドをinherent implで生やすことはできない（E0116）。deriveがDomainごとに**拡張trait**を生成する。

```rust,ignore   // fragment, not a complete item
// derive 生成
pub trait CtxUsers {
    type R; type M;
    type Owner;                    // = Endpoint 型（ADR-0002）
    fn users(&self) -> Repo<User, Self::R, Self::M>
    where Self::Owner: Includes<User>;   // ← where はメソッド側（下の注記）
}

impl<'req, E: Endpoint> CtxUsers for Ctx<'req, E> {
    type R = E::Reads;
    type M = E::Mutates;
    type Owner = E;
    fn users(&self) -> Repo<User, E::Reads, E::Mutates> { ... }
}
```

> **重要**: where節は**メソッド側**に置く。implに置くとE0599になり `#[diagnostic::on_unimplemented]` が無視される。詳細は [`diagnostics.md`](./diagnostics.md)。
> `Includes` の主語が Endpoint 型であること、そのために `Owner` 関連型が要ることは
> [ADR-0001](../adr/0001-includes-is-implemented-on-the-endpoint.md) /
> [ADR-0002](../adr/0002-ctxusers-exposes-the-endpoint-as-owner.md)。**この節は結論だけを述べ、理由は複製しない。**

`Mutates = ()` のEndpoint（GET）では、setterのwhere節が満たされないため呼び出しがコンパイルエラーになる。

---

## 副次的な利益: Architecture Contractが同時に成立する

`ctx.users()` のwhere節が `Self::Owner: Includes<User>`（= Endpoint 型が `User` を宣言していること）を要求するため、Contractに宣言されていないDomainのRepositoryは**取得できない**。

```rust,compile_fail
ctx.orders()
//  ^ 型エラー: `Order` is not in this endpoint's domain contract
```

これにより [`architecture-contract.md`](./architecture-contract.md) の要求が、Capability Systemと同じ仕組みで成立する。**専用のLinterは不要**。

### Service層への伝播

Serviceに `dyn Repository` を渡すと型パラメータが消え、Capabilityの制約が失われる。

```rust,compile_fail
// ❌ これを許すと Service は全 setter を呼べる
let svc = UserUpdateService::new(Arc::new(repo) as Arc<dyn UserRepository>);
```

**`dyn Repository` を公開しない。** Serviceに渡せるのはパラメタライズ済みの `Repo<D, R, M>` のみとし、Service自身も `Service<Reads, Mutates>` としてCapabilityを型で引き継ぐ。

---

## この設計から副産物として成立するもの

| 項目 | 成立の仕組み |
|---|---|
| GETのread-only保証 | `Mutates = ()` → setterのwhere節が満たされない |
| MustNotMutate | 「そのFieldのCapabilityが発行されない」で自然に成立 |
| Field-level Mutation | Domainを不透明化し、Field単位setterのみを提供 |
| Read範囲の制限 | `E::Reads` でProjection型をパラメタライズ（Full PoC） |
| Architecture Contract | `Includes<Domain>` のwhere節 |
| spawn境界 | `Ctx<'req, E>` が `'static` でない |
| Effect Inference の大部分 | rustcがDeclared vs Implementationの照合を代行する |

ただし到達範囲には境界がある。**すべての未検査経路は [`unverified-boundaries.md`](./unverified-boundaries.md) に列挙されている。**

---

## 条件付きCapability

条件下でのみ許可されるEffectは、`ctx.when::<Cond>` スコープ内でのみCapabilityが発行される。

```rust,ignore   // fragment, not a complete item
ctx.when::<EmailChanged, _>(&mut user, &req, async |ctx, user, req| {
    ctx.users().set_email(user, req.email.clone())?;
    ctx.events().emit(EmailVerificationRequested::for_user(user))?;
    Ok(())
}).await?;
```

**クロージャの戻り型は `Result<()>` に固定する。** そうしないと `Ok(ctx)` で昇格されたContextをスコープ外へ持ち出せる。

```rust,compile_fail
let elevated = ctx.when::<C, _>(.., async |ctx, ..| Ok(ctx)).await?;
//                                                   ^^^^^^^ 型エラー
```

詳細は [`conditional-effects.md`](./conditional-effects.md) を参照。

---

## Capabilityの発行元

Authentication MiddlewareがCapabilityを発行し、それがEndpointへ流れる構造を想定する。

```text
Authentication Middleware
        ↓ Capability発行
Endpoint<Effects, Capabilities>
        ↓ Ctx<'req, Self> 経由で要求
Service / Repository
```

これは`tower::Service`の型（Request / Response / Errorの3パラメータのみ）では表現できない。[`middleware.md`](./middleware.md) を参照。

---

## Capabilityは認可ではない

**最も事故を招きやすい誤解。**

```text
静的 Capability (コンパイル時) — 「この Endpoint は何ができるか」
動的 Authorization (実行時)   — 「この呼び出し主体は何をしてよいか」
```

Capabilityは前者のみを扱う。**後者は必ず別途必要である。**

```rust,ignore   // needs a macro that arrives in M2
#[contract(domain = User, deletes = [User])]
pub struct DeleteUser;

impl Handler for DeleteUser {
    async fn handle(&self, req: Req, ctx: Ctx<'req, Self>) -> Result<()> {
        ctx.users().delete(req.id).await   // 認可チェックなし。コンパイルは通る
    }
}
```

Contractは完全で、コンパイラは満足し、AI Contextも整った出力を出す。**しかし誰でも他人のアカウントを消せる。**

さらに `Mutate<User, user::Email>` は「User型のemail列を書ける」であって「**この** Userを書ける」ではないため、行レベル権限（IDOR）はContract上で完全に不可視になる。

### 対処

- [`../concepts.md`](../concepts.md) の原則を「Capability **and** Permission Checks」とする
- Contractに `authz` を明示宣言させ、未記載を許さないことを検討する
- AI Contextに `row_scope` を未検査境界として出力する（[`unverified-boundaries.md`](./unverified-boundaries.md)）

**Contractの網羅性が高いほど「これで全部だ」という誤った安心を与える。** 認可が Contract の外にあることは、隠さず明記する。

---

## 設計上の注意

Axumの `State<AppState>` のような「何でも取得できる」入り口を公開してはならない。Capability Systemが根元から破れる。

[`runtime-stack.md`](./runtime-stack.md) の Dependency Hiding Rule を参照。
