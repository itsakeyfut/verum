# Runtime Stack

どの層に依存し、どの層を自作するか。および依存を隠蔽する運用ルール。

> 旧 §45 / §53 / §55 / §57 / §58 を統合。決定日: 2026-08-13

関連: [`middleware.md`](./middleware.md) / [`performance.md`](./performance.md) / [`../roadmap/roadmap.md`](../roadmap/roadmap.md)

---

## 前提条件（言語バージョン）

| 項目 | 要件 | 理由 |
|---|---|---|
| **edition** | **2024** | `when` スコープに async closure (`AsyncFnOnce`) が必須 |
| **MSRV** | **1.85+** | async closure / `#[diagnostic::do_not_recommend]` |

`when` の実装で `&user` を貸しつつクロージャに渡す形は、`FnOnce(..) -> Fut` 方式では借用を跨げず成立しない（実コンパイルで確認済み）。async closure が必須要件になる。詳細は [`rust-type-model.md`](./rust-type-model.md) を参照。

---

## 判断基準

> **仕様が固まっている概念は自作しない。未解決の設計問題に全リソースを投下する。**

---

## 決定

```text
Tokio                     — 使う（自作しない）
Hyper (+ hyper-util)      — 使う（自作しない）
Tower / tower-http        — 使う（自作しない）
─────────────────────────────────────────────
Router                    — Verum自作
Extractor                 — Verum自作
Handler / Endpoint        — Verum自作
Response                  — Verum自作
Middleware chain          — Verum自作
─────────────────────────────────────────────
Effect / Capability       — Verum本体
Mutation Contract         — Verum本体
Semantic Contract         — Verum本体
AI Context                — Verum本体
```

---

## 自作しない理由

| 領域 | 状態 | 担当 |
|---|---|---|
| HTTP/1.1, HTTP/2 | RFC 9112 / 9113で確定 | Hyper |
| CORS | WHATWG Fetchで確定 | tower-http |
| Content negotiation / Compression | RFC 9110 | tower-http |
| Tracing span設計 | OpenTelemetry semantic conventions | tower-http |

これらは**教材的で不変**な概念であり、自作しても差別化にならない。

特にHTTPプロトコル層の自作は、request smuggling / HPACK bomb / h2 rapid reset (CVE-2023-44487) などのセキュリティリスクを自ら抱えることになる。Verumの独自性はHTTPプロトコル層に一切存在しない。

さらに、性能目標である「Axum級」はHyperを使えばほぼ自動達成される。自作した場合、Axum級への到達自体が課題になり、目標を自ら遠ざける。

---

## Axumを使わない理由

Axumは0.xのままメジャーバージョンに到達していない。実際に破壊的変更の履歴がある。

```text
0.6 → 0.7
    axum::Server 削除 → axum::serve
    hyper 1.0 移行
    FromRequest 周りの変更

0.7 → 0.8
    path syntax 変更（/:id → /{id}）※全ルート定義が壊れる
    async_trait 削除
    Option extractor の扱い変更
```

加えて、VerumはRouter / Extractor / Handlerを**元々独自に持つ**。つまりAxumの提供価値のほぼ全部を置き換えるのに、その破壊的変更だけを引き受けることになる。

---

## 版安定性の棚卸し

| crate | 版 | 判断 |
|---|---|---|
| tokio | 1.x | 安定。2020年から互換保証 |
| hyper | 1.x | 安定。2023年11月に1.0到達 |
| http / http-body | 1.x | 安定 |
| tower | 0.5 | 0.xだが`Service` traitは2019年から実質不変 |
| hyper-util | 0.1 | 0.x。hyper 1.0が低レベルAPIに絞った結果。避けにくい |
| matchit | 0.8 | 0.x。小さいので自作またはvendoring可 |

0.x依存は排除するのではなく、**Verum内部の薄い層に閉じ込める**（下記 Dependency Hiding Rule）。

---

## Dependency Hiding Rule

最重要の運用ルール。

> **`verum` crateのpublic APIに、置き換え予定の依存の型を1つも出さない。**

隠蔽が「後で降りる自由」を買う。最初から隠しておけば、Axumを外す時にpublic APIが一切変わらない。逆に露出させたら、二度と外せない。

### 隠すもの（Verumの型で置き換える）

```text
axum::extract::State        ← 最重要
axum::Router
axum::response::IntoResponse
axum::Json
axum::extract::Path / Query
axum::handler::Handler
tower::Service / Layer
hyper_util::*
matchit::*
```

#### `State`が最重要な理由

`State<AppState>`からは何でも取得できる。これを露出させると「そもそも呼び出せない状態を作る」が嘘になり、**Capability Systemが根元から破れる**。

同様に、Axumの`Handler` traitは任意のextractorを任意個受け取れる。これはescape hatchではなく**無申告の抜け穴**である。

### 隠さないもの（むしろre-exportする）

```text
http::StatusCode
http::HeaderMap
http::Method
http::Uri
```

これらはAxum固有ではなく**http 1.x安定**の基盤であり、自作後もまったく同じものを使う。隠すのは無駄。

### 露出させて良いもの

Escape Hatchでは意図的に低レイヤを露出させる。ただし、

> **escape hatchを通ったことがContractに記録される**

これは隠蔽の例外ではなく、隠蔽があって初めて成立する機能である。

### Backend traitを今は作らない

Axumに触るコードは1モジュール（`src/runtime/` 等）に集めるだけとする。

**実装が1つしかないtraitは作らない。** 2つ目のbackend（Hyper backend）が必要になった時点で切り出す。その時点で実装が2つ存在するので、正当な抽象になる。

### PoCで使うAxum機能を最小に絞る

使うほど後で外すコストが上がる。

```text
使う:
    Router::route
    path parameter
    body 読み取り
    response 返却
    axum::serve

使わない:
    State              ← Capability Systemを破壊し、外すコストが最も高い
    Json extractor     ← Verum独自extractorを使う
    middleware         ← PoCでは不要
    WebSocket / SSE    ← PoCの検証項目に含まれない
```

---

## 自作範囲と概算（Axumを外す段階）

| 必要なもの | 手段 | 概算 |
|---|---|---|
| accept loop + graceful shutdown | `hyper_util::server::conn::auto`（HTTP/1+2自動判定） | ~100行 |
| path matching | `matchit`または自作（静的 + `{param}` + `{*rest}`） | ~200行 |
| Response変換 | 独自trait + `http-body-util`の`Full`/`BoxBody` | ~150行 |
| Extractor | **元々自作予定**（Capabilityベース） | 追加なし |
| Handler trait | RPITIT + `Send`。**加えてobject-safeな消去レイヤ** | ~100行 |
| Middleware chain | 自前trait（RPITIT）+ 消去レイヤ | ~200行 |

**実質的な追加コストは600〜800行程度。** WebSocket（hyper upgrade + tokio-tungstenite）とSSEを足しても+300行だが、これらは初期PoCに不要。

> **消去レイヤが必要な理由**: `async fn` in trait（AFIT）はdyn非互換であり、Routerが `Box<dyn Handler>` を持てない（E0038）。さらにAFITのままではFutureが `Send` にならずhyperのmulti-thread runtimeに載らない。
>
> - `Send` は RPITIT（`-> impl Future<Output = ..> + Send`）で解決
> - dyn互換性は derive が `Pin<Box<dyn Future<Output = Response> + Send + '_>>` を返す消去レイヤを生成して解決
>
> Middleware chain も同じ制約を受ける。当初の見積もり（Middleware chain ~100行）に消去レイヤのコストが入っていなかったため修正した。詳細は [`rust-type-model.md`](./rust-type-model.md)。

### 後回しにするもの

```text
WebSocket
SSE
multipart
TLS（初期はリバースプロキシに委譲）
compile-time route最適化
```

### 最初から入れるもの（trust boundaryのため省略しない）

```text
body size limit
request timeout
path正規化（`..` / エンコード済みセパレータの扱い）
```

HyperがHTTPプロトコルの安全性を担保するが、この3つは自作側の責任である。

---

## Axumを外して得られる設計上の利益

コスト削減ではなく、積極的な利益が存在する。

### 1. 可変長handler magicを捨てられる

Axumの`Handler`はmacroで16個のtuple implを生成し、任意のextractorを任意個受け取れる。これは人間向けのergonomicsだが、Verumにとっては**害**である。

「handlerが何を受け取れるか」をCapabilityで縛りたいのに、受け取り口が開きすぎている。固定シグネチャの方が縛れる。

```text
async fn handle(&self, req: Request, caps: &Caps<Self::Effects>) -> Response
```

### 2. Compile-time route table

Verumはderive macro + inventoryにより、**全Endpointがコンパイル時に既知**になる。したがってradix trieを実行時に構築する必要がなく、`match`式やperfect hashingに落とせる。

これはAxumの構造では原理的に不可能な最適化。

ただし初期は素直なmatcherで十分。後で効かせる余地として記録する。

### 3. 性能目標が素直に達成される

Hyperを直接叩くため、Axumのレイヤ分のオーバーヘッドが存在しない。「Axum級」は下限になる。

---

## 将来的なCustom Runtime

必要性が明確になった場合のみ検討する。

```text
Semantic Framework
        ↓
Custom optimized runtime
```

現時点では、成熟したRust Web ecosystemを利用し、Semantic Layerの構築に集中する。
