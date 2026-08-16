# Middleware

Middlewareと低レイヤーAPIに型付きの「道」を敷く。tower / tower-httpとの境界設計を含む。

> 旧 §42 Low-level APIs と §56 Tower Boundary Design を統合。

関連: [`runtime-stack.md`](./runtime-stack.md) / [`capability-system.md`](./capability-system.md)

---

## 方針

Middlewareや低レイヤーを自由に利用できることを重視する。一方、以下のような何でも入りMiddlewareは避けたい。

```rust,ignore   // fragment, not a complete item
async fn middleware(req, next) {
    // Authentication
    // Database
    // Cache
    // External API
    // Logging
    // Response manipulation
    // etc.
}
```

代わりに、意味の明確な構成要素を提供する。

```text
AuthenticationMiddleware
LoggingMiddleware
RateLimitMiddleware
TracingMiddleware
CacheMiddleware
```

各Middlewareについて、以下を型またはMetadataで表現する。

- Allowed Effects
- Forbidden Effects
- Capabilities
- Inputs
- Outputs

---

## `tower::Service` への評価

```rust
trait Service<Request> {
    type Response;
    type Error;
    type Future: Future<Output = Result<Self::Response, Self::Error>>;
    fn poll_ready(&mut self, cx: &mut Context) -> Poll<Result<(), Self::Error>>;
    fn call(&mut self, req: Request) -> Self::Future;
}
```

これは**2019年、`async fn` in traitが存在しなかった時代の設計**であり、Verumとの噛み合わせに3つの問題がある。

1. **型パラメータがRequest / Response / Errorの3つだけ**
   - EffectやCapabilityを載せる場所が構造的に存在しない
   - 「MiddlewareにAllowed / Forbidden Effectsを型で表現」はtowerの型では書けない

2. **`&mut self` + `poll_ready`**
   - backpressureは概念的に美しいが、HTTP middlewareの実態は99%が`Poll::Ready(Ok(()))`を返すだけ
   - `&mut self`のため並行呼び出しに`Clone`が必要になる
   - 現在は `fn call(&self, req) -> impl Future<Output = Response> + Send`（**RPITIT**）で書ける
   - **AFIT（`async fn` in trait）ではない。** chain が `Box<dyn Middleware>` を持てず（`E0038`）、Future が `Send` にならないため hyper の multi-thread runtime に載らない（[`rust-type-model.md`](./rust-type-model.md)、[RK-012](../dev/code/review-knowledge.md)、いずれも実測）。dyn 互換性は**消去レイヤ**で解決する — それが上の200行に含まれるコストである

3. **`type Error`**
   - HTTPサーバでは「エラーもレスポンス」
   - Axumは`Error = Infallible`に固定しており、この型パラメータは実質死んでいる

**結論:** `Service` traitはVerumが積極的に採用したい抽象ではなく、tower-httpを使うために必要なadapterである。

---

## 境界の設計

towerは「依存」ではなく「境界」として扱う。

```text
hyper connection
      ↓
┌─ tower Service の世界（最外周インフラ層）───────────┐
│  tower-http: CORS / Compression / Trace / Timeout  │
└────────────────────────────────────────────────────┘
      ↓  ← 唯一の境界。adapter 1枚
┌─ Verum の世界 ──────────────────────────────────────┐
│  Router                                             │
│      ↓                                              │
│  Semantic Middleware chain (RPITIT, &self, Effects) │
│      ↓                                              │
│  Endpoint<Effects, Capabilities>                     │
└─────────────────────────────────────────────────────┘
```

これにより:

- tower-httpの実績あるロジックをそのまま使える
- Verum の Middleware APIを最新のRust idiom（**RPITIT + `Send`**, `&self`, Effect型パラメータ）で書ける
- tower 0.5 → 0.6の破壊的変更は**adapter 1枚に閉じる**
- ユーザーとAIはtowerの型を一切見ない

### towerを自作する必要はない

Verumが必要とするのはMiddleware chainの合成だけ。towerの `discover` / `balance` / `retry` / `buffer` / `load` は一切不要。

自前のMiddleware trait + chain合成は概算**200行程度**。towerの再実装ではなく、**必要な部分だけを最新の形で持つ**。

> **当初は100行と見積もっていた。** [`runtime-stack.md`](./runtime-stack.md) が消去レイヤのコストを算入して200行に訂正しており（RK-012）、本ファイルはその訂正を受け取っていなかった（#43 項目9）。**見積もりの正典は [`runtime-stack.md`](./runtime-stack.md) の表**で、ここはそれを引く。

---

## Middlewareの分割線

Middlewareリストは、この境界でちょうど二分される。

| Middleware | 担当 | 理由 |
|---|---|---|
| CORS / Compression / Tracing / Logging | tower-http | 仕様が不変。GETにも許可されるEffectの種類 |
| **Authentication** | **Verum** | Capabilityを発行する側。型で表現すべき本体 |
| **RateLimit** | **Verum** | CacheRead / CacheWrite Effectを持つ。`tower::limit`ではなくCapabilityとして扱う |
| **Cache** | **Verum** | CacheRead / CacheWriteが絡む |

Authentication MiddlewareがCapabilityを発行し、それがEndpointへ流れる構造は Capability System の中核であり、towerの型では表現できない。

分割線が「**意味を持つか / インフラか**」で入っている。
