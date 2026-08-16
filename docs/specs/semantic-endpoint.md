# Semantic Endpoint

EndpointをHTTP関数ではなくSemantic Contractとして表現する。Contract宣言構文の仕様。

関連: [`handler-rules.md`](./handler-rules.md) / [`effect-system.md`](./effect-system.md) / [`capability-system.md`](./capability-system.md) / [`diagnostics.md`](./diagnostics.md)

---

## 解決したい問題

通常のWebフレームワークでは、以下のシグネチャだけではEndpoint内部を読まないと何も分からない。

```rust,ignore   // needs a macro that arrives in M2
#[put("/users/{user_id}")]
async fn update_user(...) -> Result<User>
```

分からないこと: 何を変更するのか / 何を読むのか / DBを書き換えるのか / 外部サービスを呼ぶのか / Eventを発行するのか / どの条件で何が変わるのか。

---

## 決定: 属性で宣言し、deriveが型に展開する

```rust,ignore   // needs a macro that arrives in M2
#[endpoint(PUT "/users/{id}")]
#[contract(
    domain    = User,
    request   = UpdateUserRequest,
    response  = UserView,

    reads     = [User::id, User::status],
    mutates   = [User::name],              // 無条件に変更する
    forbidden = [User::password_hash],
    creates   = [AuditLog],
    emits     = [UserUpdated],

    when(EmailChanged) => {
        mutates = [User::email],           // この条件下でのみ変更する
        emits   = [EmailVerificationRequested],
        calls   = [EmailService],
    },
)]
pub struct UpdateUser;
```

deriveが展開する型:

```rust,compile_fail
impl Endpoint for UpdateUser {
    type Method = Put;                    // 型レベルマーカ（const ではない）
    const PATH: &'static str = "/users/{id}";

    type Domain    = User;
    type Request   = UpdateUserRequest;
    type Response  = UserView;

    // cons list 表現（フラットタプルでは所属判定が実装できない）
    // `mutates` の Field は変更前の値を読む必要があるため自動的に `reads` に入る
    // （[`read-contract.md`](./read-contract.md)）。宣言は id / status の2つだが、
    // 展開後は `mutates = [User::name]` の name が加わって3要素になる。
    type Reads   = (Read<User, user::Id>,
                   (Read<User, user::Status>,
                   (Read<User, user::Name>, ())));
    type Mutates = (Mutate<User, user::Name>, ());
    type Creates = (Create<AuditLog>, ());
    type Emits   = (Emit<UserUpdated>, ());
    type Deletes = ();
    type Calls   = ();

    type Conditional = (
        When<EmailChanged,
             (Mutate<User, user::Email>, ()),          // CondMutates
             (Emit<EmailVerificationRequested>, ()),   // CondEmits
             (Call<EmailService>, ())>,                // CondCalls
        (),
    );
}
```

> **宣言場所の規則**: トップレベルの `mutates` / `emits` / `calls` は無条件に起こり得るもの、`when(C)` 内はその条件下でのみ起こり得るもの。同一要素を両方に書くことは禁止（macroが弾く）。詳細は [`conditional-effects.md`](./conditional-effects.md)。

### サポートするHTTP Method

型レベルマーカとして以下を提供する。

| Method | マーカ型 | read-only |
|---|---|---|
| GET | `Get` | ✅ |
| HEAD | `Head` | ✅ |
| POST | `Post` | |
| PUT | `Put` | |
| PATCH | `Patch` | |
| DELETE | `Delete` | |

`OPTIONS` はEndpointとして宣言しない（CORSは tower-http のレイヤが処理する — [`middleware.md`](./middleware.md)）。

read-only なMethodでは `mutates` / `creates` / `deletes` を宣言できない（`when` 内も含めてmacroが弾く）。

> `Method` を型にする理由、cons listである理由、`Conditional` をカテゴリ別に分割する理由はすべて [`rust-type-model.md`](./rust-type-model.md) に記載（実コンパイルで確認済みの制約）。

### Endpointは unit struct のみ

```rust
pub struct UpdateUser;                   // ✅
```

```rust,compile_fail
pub struct UpdateUser { pool: PgPool }   // ❌ derive がエラー
```

フィールドを持てると `self.pool` から直接SQLを実行して `ctx` を迂回できる。[`handler-rules.md`](./handler-rules.md) Rule 2 を型で成立させるための条件。

### この方式を選んだ理由

| 観点 | 属性→型展開 | 純粋associated type | 宣言マクロDSL | 外部ファイル |
|---|---|---|---|---|
| 型検査の強さ | 強 | 強 | 強 | 弱（生成境界あり） |
| **macro段階のエラー精度** | **Field名のtypoに `did you mean` を出せる** | 出せない | spanがずれやすい | 型と乖離 |
| IDE補完 | 効く（`User::name` は実在パス） | 効く | **効かない** | 効かない |
| 記述量 | 少 | 多 | 最少 | 中 |

**決め手は macro 段階で弾けるエラーの精度**である。derive macroは属性内トークンのspanを保持するため、存在しないField / Domainを型検査前に精密なエラーで弾ける。

> **訂正**: 当初「trait bound違反時にContract宣言箇所への `note` を出せる」ことを決め手としていたが、これは**成立しない**。`on_unimplemented` のnoteはプレーンテキストでspanを持たず、rustcが出すspanは `Has` のimpl定義位置である。span付きnoteが出るのは associated type equality bound の場合のみ（`Mutates = ()` 違反など）。詳細は [`diagnostics.md`](./diagnostics.md)。

### 外部ファイル方式（Goa方式）との関係

**「型が権威 vs 外部ファイルが権威」という対立軸は成立しない。** `#[contract(...)]` の中身もRustの型式ではなくproc macroが解釈するトークン列であり、型はその生成物である。構造はGoaと同型。

実際に成立している差別化は以下の2点。

1. **契約の対象範囲** — HTTP契約だけでなく内部状態変更 / Effect / Capability / Architecture まで対象にする
2. **エラーの局所性** — 違反が宣言箇所を指すコンパイルエラーとして返る

[`../concepts.md`](../concepts.md) の差別化記述もこの2点に整理する。

### Field指定の形式

属性内は `User::name`（フィールド名）で書き、deriveが実在チェック後に `user::Name` マーカ型へ変換する。

- AIが自然に書ける形を保つ
- 存在しないフィールドはmacroが `did you mean` 付きで弾ける
- `User::name` は実在するパスなのでIDE補完・ジャンプが効く

---

## Semantic Endpointの構成要素

```text
Endpoint
├── Method（型レベルマーカ）
├── Path
├── Domain
├── Request / Response
├── Reads          → read-contract.md
├── Mutates        → mutation-contract.md
├── Creates / Deletes
├── Emits / Calls  → effect-system.md
├── Conditional    → conditional-effects.md
└── Capabilities   → capability-system.md
```

### `operation` は削除した

初期案には `operation = Update` があったが**削除した**。

理由:

- **値集合が定義できなかった** — Q-C実験の被験者は新Endpointを追加する際「存在しないenum variantを捏造するリスクを避け、既存の `Update` を再利用した」と報告した。AIが毎回迷い、かつ何も保証しないフィールドだった
- **情報が重複していた** — 操作の種類は `Method` + `Domain` + `mutates`/`creates`/`deletes` から導出できる
- **Effect名と衝突していた** — `operation = Read` と `Read<User, user::Id>`、`operation = Create` と `Create<AuditLog>`

**業務的な操作名はEndpointの型名が担う。** `SuspendUser` という型名が「User に対する Suspend 操作」を表しており、`domain = User, operation = Suspend` と書くのは冗長だった。

型名はAI Contextの `"endpoint": "SuspendUser"` として出力されるため、情報は失われない。

> 命名規約: Endpoint の型名は `<Operation><Domain>` 形式を推奨する（`GetUser` / `UpdateUser` / `SuspendUser` / `DeleteUser`）。ただし強制はしない。

---

## Handlerシグネチャ

固定シグネチャを採用する。可変長handler（任意のextractorを任意個受け取る形）は、Capabilityを型で縛る目的に適さない。

```rust,ignore   // fragment, not a complete item
impl Handler for UpdateUser {
    fn handle(&self, req: UpdateUserRequest, ctx: Ctx<'_, Self>)
        -> impl Future<Output = Result<UserView>> + Send;
}
```

**AFIT（`async fn` in trait）ではなくRPITIT + `Send` を使う。** AFITだとFutureが `Send` にならずhyperのmulti-thread runtimeに載らない（実コンパイルで確認済み）。dyn互換性は別途、deriveがobject-safeな消去レイヤを生成して解決する。

`Ctx<'req, Self>` はリクエスト寿命に縛られ（`'static` でない）、Capabilityを保持する。実装規約は [`handler-rules.md`](./handler-rules.md) を参照。

---

## 完全な例

### Domain定義

```rust,ignore   // needs a macro that arrives in M2
#[derive(Domain)]
pub struct User {
    id:            UserId,      // private 必須（pub は derive がエラー）
    name:          String,
    email:         Email,
    password:      PasswordHash,
    status:        UserStatus,
    last_login_at: Option<DateTime<Utc>>,
    created_at:    DateTime<Utc>,
}
```

マクロが生成: Field マーカ型 / Capability要求付きアクセサ / `pub(crate)` な `Repr` / 宣言Fieldのみを出す `Debug` と `Serialize`。

> **⚠️ 2点、T-M1-01 / #13 で覆っている。** `Repr` は「**Repository 実装専用**」にはならない（`pub(crate)` はアプリクレート全体を指す — 台帳 path 21）。そして `#[derive(...)]` か属性マクロかは**未確定**（derive は入力と同名のアイテムを追加できないため）。[`persistence.md`](./persistence.md) §判定 を見ること。

**フィールドを `pub` にすると `user.email = v` で Contract 全体が無効化される。** 理由は [`mutation-contract.md`](./mutation-contract.md) を参照。

### GET

```rust,ignore   // needs a macro that arrives in M2
#[endpoint(GET "/users/{id}")]
#[contract(
    domain    = User,
    request   = GetUserRequest,
    response  = UserView,
    reads     = [User::id, User::name, User::email, User::status],
)]
pub struct GetUser;

impl Handler for GetUser {
    fn handle(&self, req: GetUserRequest, ctx: Ctx<'_, Self>)
        -> impl Future<Output = Result<UserView>> + Send
    {
        async move {
            let user = ctx.users().find(req.id).await?;
            Ok(UserView::from(user))
        }
    }
}
```

`mutates` / `creates` / `deletes` を宣言していないため、これらは `()` になる。GETに対しては `Mutates = ()` が構造的に要求される。

### PUT

完全な実装例（`when` / `after_commit` を含む）は [`handler-rules.md`](./handler-rules.md) を参照。**`when` の呼び出しは async closure（edition 2024 / MSRV 1.85+）が必須**で、`user` / `req` をキャプチャさせずクロージャ引数として貸す形になる。

---

## 未決定の論点

- **Error** — どのエラーが返り得るかをContractに含めるか（`fails = [NotFound, Conflict]`）。OpenAPI生成には必須
- **Validation** — Requestの制約をContractで宣言するか（現在は完全に射程外）
- **Transaction** — Endpointとトランザクション境界の関係
- **Multi-domain** — 1つのEndpointが複数Domainを触る場合の宣言形式
- **一覧 / 集計 / JOIN** — `Read<Domain, Field>` が単一インスタンス前提であることの帰結。実Webアプリで最も画面数が多い形が書けない
- **Job / Background** — HTTPリクエストが存在しない処理にEndpointの枠組みが適用できない
- **State Transition** — `status: active → suspended` をContractで表現するか

[`research-questions.md`](./research-questions.md) に記録。
