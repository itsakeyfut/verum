# Architecture Contract

Handler → Service → Repository の経路を、Conventionではなく型/静的解析で制約する。

関連: [`capability-system.md`](./capability-system.md) / [`semantic-endpoint.md`](./semantic-endpoint.md)

---

## 基本構造

```text
Handler
   ↓
Service
   ↓
Repository
```

をConventionだけでなく、型/静的解析によって制約する。

### 正しい経路

```text
UserUpdateEndpoint
    ↓
UserUpdateService
    ↓
UserUpdateRepository
```

### 禁止する経路

```text
UserHandler
    ↓
OrderRepository
```

---

## 実現方法: `Ctx` の where 節で検査する

Capability Systemと同じ仕組みで成立する。`ctx.users()` の where 節が、Contractに宣言されたDomainであることを要求する。

`Ctx` はフレームワーク型なので inherent impl は書けない（E0116）。deriveがDomainごとに拡張traitを生成する。

```rust,ignore   // fragment, not a complete item
pub trait CtxUsers {
    type R; type M;
    // ⚠️ `Owner` の意味は未決。`E::Domain` を指す意図に見えるが、どの文書も
    // 定義していない。ここでは宣言だけ置いてある（#43）。
    type Owner;
    fn users(&self) -> Repo<User, Self::R, Self::M>
    where <Self as CtxUsers>::Owner: Includes<User>;   // ← where はメソッド側
}

impl<'req, E: Endpoint> CtxUsers for Ctx<'req, E> { ... }
```

> **where節はimplではなくメソッド側に置く。** implに置くとE0599（メソッドは存在するがtrait boundを満たさない）になり、`#[diagnostic::on_unimplemented]` が**無視される**（実コンパイルで確認済み）。
>
> ```text
> // ❌ impl に where
> error[E0599]: the method `orders` exists for struct `Ctx<UpdateUser>`,
>               but its trait bounds were not satisfied
>
> // ✅ method に where
> error[E0277]: `Order` is not in this endpoint's domain contract
> ```
>
> deriveの生成テンプレートで固定する。詳細は [`diagnostics.md`](./diagnostics.md)。

```rust,compile_fail
// UpdateUser の Contract は domain = User
ctx.orders()
//  ^ 型エラー: `Order` is not in this endpoint's domain contract
```

**Repositoryの取得自体が検査点になるため、専用のLinterは不要。**

これはCapabilityをContext経由で受け渡す設計（[`capability-system.md`](./capability-system.md)）の副次的な利益である。明示引数案（`self.repo` から取得する形）では、EndpointとRepositoryの紐付けを別途用意する必要があり、この検査は無料で付いてこない。

---

## Service 層 — 位置づけが未確定

上記の「正しい経路」に Service が含まれているが、**全コード例に Service が登場しない**（[`handler-rules.md`](./handler-rules.md) / [`semantic-endpoint.md`](./semantic-endpoint.md) の実装例はどちらも `ctx.users()` を handler から直接呼んでいる）。

さらに `ctx.users()` が Repo を直接返す設計自体が、**Service 迂回を最短経路にしている**。

### Capability が失われる経路

Service に `dyn Repository` を渡すと型パラメータが消え、Service は全 setter を呼べるようになる。

```rust,compile_fail
// ❌ これを許すと Capability の制約が消える
let svc = UserUpdateService::new(Arc::new(repo) as Arc<dyn UserRepository>);
```

**`dyn Repository` を公開しない。** Service に渡せるのはパラメタライズ済みの `Repo<D, R, M>` のみとし、Service 自身も `Service<Reads, Mutates>` として Capability を型で引き継ぐ。

また、Service 経由の Effect は [`handler-rules.md`](./handler-rules.md) Rule 2 の grep 保証（`ctx.` の行を数える）から漏れる。

### 決めるべきこと

| 案 | 内容 |
|---|---|
| **A. Service を任意とする** | 上図を Endpoint → Repository に直し、「Service は業務ロジックが複数 Endpoint で共有されるときのみ」と条件を明記する |
| **B. Service を必須とする** | Service が Capability をどう受けるかを型で示した例を用意し、First PoC の検証項目に「Service 越しでも Capability が漏れない」を入れる |

PoC スコープを膨らませないため A を推す。[`research-questions.md`](./research-questions.md) に記録。

---

## Endpointパターンごとのアーキテクチャ

すべてを固定するのではなく、Endpointパターンごとに適切なArchitectureを定義する。

```text
CRUD API
    → Domain / Endpoint / Service / Repository

Read-heavy API
    → Query / Repository

WebSocket
    → Connection / Handler / Session

Background Job
    → Job / Service

Streaming
    → Stream / Handler
```

---

## Multi-domain Endpoint（未決定）

1つのEndpointが複数Domainを触る場合の宣言形式が未定。

```rust,ignore   // needs a macro that arrives in M2
// 案: domains として複数宣言する
#[contract(
    domains = [User, AuditLog],
    reads   = [User::id],
    mutates = [User::status],
    creates = [AuditLog],
)]
```

`AuditLog` のように付随的に作成されるDomainは既に `creates` に現れるため、`domain` 宣言と重複する。整理が必要。

検討すべき問い:

- `creates` / `emits` に現れるDomainは自動的にアクセス可能とすべきか
- 業務的に独立した2つのDomain（User と Order）を同時に更新するEndpointを許すか、禁止してServiceレイヤに寄せるか
- 許す場合、Aggregate境界を越えたトランザクションの扱い（[`persistence.md`](./persistence.md)）

[`research-questions.md`](./research-questions.md) に記録。

---

## 検証項目

- Service / Repositoryの経路を型で検証できる
- 宣言されていないRepositoryへの依存がコンパイルエラーになる
- Endpointパターンごとに異なるArchitectureを表現できる
- エラーメッセージがContract宣言箇所を指す（[`diagnostics.md`](./diagnostics.md)）
