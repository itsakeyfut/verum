# Handler Rules

ハンドラ実装の規約。**「実装を見ればEndpointの処理が自明である」ことを担保する仕組み。**

関連: [`capability-system.md`](./capability-system.md) / [`unverified-boundaries.md`](./unverified-boundaries.md) / [`persistence.md`](./persistence.md)

---

## 設計判断の背景

Capabilityトークンをハンドラ内でどう受け渡すかを検討した結果、**Context経由（暗黙）**を採用した。

```rust,ignore   // fragment, not a complete item
// 採用: Context 経由
ctx.users().set_email(&mut user, req.email)?;

// 却下: 明示引数
ctx.users().set_email(&mut user, req.email, caps.mutate::<user::Email>())?;
```

却下理由は「長いから」ではない。**明示引数が追加している情報 `user::Email` は、`set_email` という名前から既に自明**であり、長さは増えるが情報は増えないため。

ただし、これは**Context経由でも自明性が保たれる場合に限って成立する**。自明性はCapabilityトークンの有無ではなく、APIの形で決まる。したがって以下3つのルールを仕様として定める。

> **ルールが守られない場合、Context経由の設計は自明性を失う。** この3つはオプションではなく、Capability設計の前提条件である。

---

## Rule 1 — Repositoryは Field 単位のメソッドのみを持つ

```rust,ignore   // fragment, not a complete item
// ✅ 提供する
ctx.users().set_email(&mut user, v)?;   // 何が変わるか自明
ctx.users().set_name(&mut user, v)?;
ctx.users().set_status(&mut user, v)?;
```

```rust,compile_fail
// ❌ 提供しない
ctx.users().save(&mut user)?;            // 何が変わったか不明
ctx.users().update(&mut user, patch)?;   // 同上
ctx.users().apply(&mut user, changes)?;  // 同上
```

包括メソッドを許すと、Contractに `mutates = [name, email]` と書いてあっても、**実装のどの行で何が変わるかが読めない**。

### `&mut User` を渡してよい理由

Domainは不透明型（privateフィールド）として公開される（[`mutation-contract.md`](./mutation-contract.md)）。したがって `&mut User` を保持していても直接代入はできない。

```rust,ignore   // fragment, not a complete item
ctx.users().set_email(&mut user, req.email)?;   // ✅
```

```rust,compile_fail
user.email = req.email;                          // ❌ private field
```

**Domainが `pub` フィールドを持つ形では、このルールは無意味になる。** deriveが `pub` を拒否することがRule 1の前提条件である。

### Read側の例外（未解決）

N+1回避（eager loading）はField単位メソッドと構造的に衝突する。「Userを100件取得し、それぞれのOrder件数も返す」を1メソッドで書くと、Rule 1が拒否する包括メソッドと形が似る。

Read側のみ「宣言されたFieldの組み合わせに限り複合クエリを許可する」例外規定が必要になる見込み。[`research-questions.md`](./research-questions.md) を参照。

---

## Rule 2 — Effectを起こす操作は必ず `ctx` 経由

```text
ctx.users()      → State Effect (Read / Mutate / Delete)
ctx.audit_logs() → Create
ctx.events()     → Emit
ctx.email()      → External Effect
ctx.cache()      → Infrastructure Effect
ctx.spawn()      → Spawn<Job>
```

**`ctx.` で始まる行だけを目で追えば、そのハンドラの全Effectが列挙できる**状態を保つ。

### 型で強制される部分

| 経路 | 強制手段 |
|---|---|
| Endpoint構造体に `PgPool` を持って直接SQL | `#[endpoint]` がunit struct以外を拒否 |
| `tokio::spawn` でCapabilityを持ち出す | `Ctx<'req, E>` が `'static` でない |
| Serviceに `dyn Repository` を渡す | `dyn Repository` を公開しない |

### 規約に留まる部分（重要な限界）

Rule 2の「grepで全Effectが列挙できる」という帰結は、**自由関連関数が純粋であることに依存している**。

```rust,compile_fail
ctx.audit_logs().create(AuditLog::user_updated(&user))?;
//                      ^^^^^^^^^^^^^^^^^^^^^^ 内部でDBを叩いても検出できない
ctx.events().emit(UserUpdated::from(&user))?;
Ok(UserView::from(user))
```

以下は**純粋でなければならない**（規約）。

- Contract で宣言された型のコンストラクタ（`AuditLog::user_updated` 等）
- `Condition::holds`
- View変換（`UserView::from`）

将来 `#[derive(Event)]` / `#[derive(View)]` でコンストラクタを生成し、手書きの余地を消す。[`unverified-boundaries.md`](./unverified-boundaries.md) #18 として追跡する。

---

## Rule 3 — 条件付きEffectは `ctx.when::<Cond>` スコープ内でのみ発火

```rust,ignore   // fragment, not a complete item
ctx.when::<EmailChanged, _>(&mut user, &req, async |ctx, user, req| {
    ctx.users().set_email(user, req.email.clone())?;
    ctx.events().emit(EmailVerificationRequested::for_user(user))?;
    Ok(())
}).await?;
```

クロージャに渡される `ctx` だけが `EmailVerificationRequested` のCapabilityを持つ。外側の `ctx` は持たないため、スコープ外での発火は型エラーになる。

### シグネチャ

`user` / `req` は**キャプチャさせず、クロージャの引数として貸す**。

```rust,ignore   // fragment, not a complete item
pub async fn when<C, F>(&self, u: &mut Domain, r: &Req, f: F) -> Result<()>
where
    C: Condition<Domain, Req>,
    F: AsyncFnOnce(Ctx<'_, Extended<E, C>>, &mut Domain, &Req) -> Result<()>;
```

- `&user` を渡しつつ `async move` でキャプチャする形は**借用エラーになる**（検証済み: E0382 / E0505）
- `FnOnce(...) -> Fut` 方式は借用を跨げない（`lifetime may not live long enough`）
- **Rust 2024 edition の async closure（`AsyncFnOnce`, 1.85+）が必須**

### 戻り型は `Result<()>` に固定する

そうしないと昇格されたContextをスコープ外へ持ち出せる。

```rust,compile_fail
let elevated = ctx.when::<C, _>(.., async |ctx, ..| Ok(ctx)).await?;
//                                                   ^^^^^^^ 型エラー
```

### 保証されないこと

**`Condition::holds` の中身は型で検証できない。**

```rust
fn holds(user: &User, req: &Req) -> bool { true }   // 全件無条件化する
```

これは原理的に埋まらない経路であり、AI Contextに `condition_verified: false` として明示する。[`unverified-boundaries.md`](./unverified-boundaries.md) #20 を参照。

---

## Rule 4 — 外部Effectはコミット後に発火する

トランザクション内で取り消せない外部Effect（メール送信・決済・Webhook）を発火してはならない。

```rust,compile_fail
// ❌ コミット前にメールを送っている
ctx.users().set_email(&mut user, req.email)?;   // 未コミット
ctx.email().send_verification(&user).await?;     // 取り消せない
ctx.audit_logs().create(...)?;                   // この後で失敗しうる
```

```rust,ignore   // fragment, not a complete item
// ✅ コミット後に発火する
ctx.users().set_email(&mut user, req.email)?;
ctx.audit_logs().create(...)?;
ctx.after_commit(|ctx| async move {
    ctx.email().send_verification(&user).await
}).await?;
```

`ctx.after_commit` スコープ内でのみ External Effect のCapabilityを発行する形にすれば、`when` と同じ機構で型強制できる。

> Transaction境界そのものは未設計（[`research-questions.md`](./research-questions.md)）。ただし**サンプルコードは正しい順序で書く**。Verumは「AIが模倣するテンプレート」を提供するフレームワークであり、サンプルが誤った順序を教えると、それがそのまま複製される。

---

## 完全な実装例

```rust,ignore   // needs a macro that arrives in M2
#[endpoint(PUT "/users/{id}")]
#[contract(
    domain    = User,
    request   = UpdateUserRequest,
    response  = UserView,

    reads     = [User::id, User::status],
    mutates   = [User::name],
    forbidden = [User::password_hash],
    creates   = [AuditLog],
    emits     = [UserUpdated],

    when(EmailChanged) => {
        mutates = [User::email],
        emits   = [EmailVerificationRequested],
        calls   = [EmailService],
    },
)]
pub struct UpdateUser;

impl Handler for UpdateUser {
    fn handle(&self, req: UpdateUserRequest, ctx: Ctx<'_, Self>)
        -> impl Future<Output = Result<UserView>> + Send
    {
        async move {
            let mut user = ctx.users().find(req.id).await?;

            ctx.users().set_name(&mut user, req.name)?;

            ctx.when::<EmailChanged, _>(&mut user, &req, async |ctx, user, req| {
                ctx.users().set_email(user, req.email.clone())?;
                ctx.events().emit(EmailVerificationRequested::for_user(user))?;
                Ok(())
            }).await?;

            ctx.audit_logs().create(AuditLog::user_updated(&user))?;
            ctx.events().emit(UserUpdated::from(&user))?;

            ctx.after_commit(|ctx| async move {
                ctx.email().send_verification(&user).await
            }).await?;

            Ok(UserView::from(user))
        }
    }
}
```

### この実装が自明である理由

| 読み取れること | 根拠 |
|---|---|
| 変更されるFieldは name と email のみ | `set_name` / `set_email` の2行しかない（Rule 1） |
| **name は無条件、email は条件付き** | Contract の宣言場所（トップレベル vs `when` 内）。`set_email` を `when` の外で呼ぶと型エラー |
| Effectは全部で6つ | `ctx.` の行を数えれば分かる（Rule 2） |
| メール送信はコミット後 | `after_commit` ブロック内にある（Rule 4） |
| status は読むだけ | `set_status` の呼び出しがなく、Contract にも無い |
| password は触らない | `forbidden` に明示。Capability も存在しない |

**コメントは1行もない。** これが [`../concepts.md`](../concepts.md) の "semantics without comments" の具体形である。

---

## ルールの強制状況

| Rule | 強制手段 | 状態 |
|---|---|---|
| Rule 1（Field単位メソッド） | Verumが生成する範囲では構造的に保証。Domain不透明化が前提 | 利用者が独自メソッドを足す場合はLint（未実装） |
| Rule 2（ctx経由） | unit struct強制 / `Ctx<'req>` / `dyn` 非公開 で主要経路を型で塞ぐ | 自由関数コンストラクタの純粋性は**規約** |
| Rule 3（whenスコープ） | Capabilityをスコープ内でのみ発行。戻り型固定 | `Condition::holds` の中身は**検証不能** |
| Rule 4（コミット後） | `after_commit` スコープでのみ External Capability 発行 | Transaction設計が未確定 |

**規約に留まる部分と型で強制される部分を混同しないこと。** 未検査部分はすべて [`unverified-boundaries.md`](./unverified-boundaries.md) に列挙し、AI Contextに出力する。
