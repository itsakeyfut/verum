# Conditional Effects

条件付きで発生するEffectの表現。設計上もっとも難しい領域。

関連: [`handler-rules.md`](./handler-rules.md) / [`mutation-contract.md`](./mutation-contract.md) / [`rust-type-model.md`](./rust-type-model.md) / [`unverified-boundaries.md`](./unverified-boundaries.md)

---

## 課題

単純なEffect宣言だけでは不十分。

```text
if email_changed:
    Mutate(User.email)
    Emit(EmailVerificationRequested)

if status == suspended:
    Mutate(User.status)
    Revoke(UserSession)
```

---

## 判明した制約

**Rustの型システムでは `if email_changed then Emit<X>` を直接表現できない。** 完全なdependent typeは不可能。

```text
型      → 「どのスコープで何が起こり得るか」を保証
条件    → runtime witness + Metadata
```

型が保証するのは「**このスコープの外では絶対に起きない**」こと。「この条件のときだけ起きる」ことは型では保証しない（条件の中身は検証不能）。

---

## 決定: 宣言場所の規則

> Q-C実験で「conditional mutationをwhen内に書くのかトップレベルに書くのか、コード例から逆算する必要があった」と指摘された箇所。仕様として確定させる。

### 規則

```text
トップレベルの mutates / emits / calls  → 無条件に起こり得る
when(C) 内の mutates / emits / calls    → その条件下でのみ起こり得る
同一要素を両方に書くことは禁止（macro が弾く）
```

### 宣言例

```rust,ignore   // needs a macro that arrives in M2
#[endpoint(PUT "/users/{id}")]
#[contract(
    domain    = User,
    request   = UpdateUserRequest,
    response  = UserView,

    reads     = [User::id],
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

### 型への展開

```rust
type Mutates = (Mutate<User, user::Name>, ());
type Emits   = (Emit<UserUpdated>, ());
type Calls   = ();

type Conditional = (
    When<EmailChanged,
         /* CondMutates */ (Mutate<User, user::Email>, ()),
         /* CondEmits   */ (Emit<EmailVerificationRequested>, ()),
         /* CondCalls   */ (Call<EmailService>, ())>,
    (),
);
```

**カテゴリ別に分割することは必須である。** 混在させると `Conditional` からMutateだけを取り出す型レベル `Filter` が必要になり、catch-all implが必ず衝突する（[`rust-type-model.md`](./rust-type-model.md)）。

### 実装での帰結

```rust,ignore   // fragment, not a complete item
ctx.users().set_name(&mut user, req.name)?;      // ✅ トップレベルに宣言済み
```

```rust,compile_fail
ctx.users().set_email(&mut user, req.email)?;    // ❌ 型エラー
//          ^^^^^^^^^ 外側の ctx は Mutate<User, user::Email> を持たない

ctx.when::<EmailChanged, _>(&mut user, &req, async |ctx, user, req| {
```

```rust,ignore   // fragment, not a complete item
    ctx.users().set_email(user, req.email.clone())?;   // ✅ 内側 ctx のみ
    ctx.events().emit(EmailVerificationRequested { .. })?;
    Ok(())
}).await?;
```

`when` スコープ内のContextは以下の型を持つ。

```text
Mutates = <E::Mutates as Append<CondMutates>>::Out
Emits   = <E::Emits   as Append<CondEmits>>::Out
Calls   = <E::Calls   as Append<CondCalls>>::Out
```

`Append` はcons listの連結で、coherence問題なく実装できる（T-M0-09 で実装・検証済み）。`Lookup` で `E::Conditional` から該当する `When<C, ..>` を引く。

> **訂正（T-M0-09）**: 旧版は「どちらもindexパラメータ版が必要」と書いていたが、**`Append` に index パラメータは不要**である。`Append` の2つの impl は `()` と `(H, T)` を対象とし構造的に disjoint なので、そもそも重複しない。index が必要なのは `Has` / `Lookup` のように **impl が2つとも `(_, _)` を対象とする**場合だけである。[`rust-type-model.md`](./rust-type-model.md) の表は当初から `Append<A, B>`（index なし）/ `Lookup<Set, Key, Idx>`（index あり）と書き分けており、**2つの spec が矛盾していた。コンパイラは後者に一致した。**

`Lookup` の map は **`(鍵, 値)` ペアの cons list** である（`((C, When<C, ..>), rest)`）。エントリ自身が鍵を宣言する `Keyed` 方式ではない — `typelevel` は依存の最下層で `When` を知ってはならないためである（[`../rules/design.md`](../rules/design.md) §2）。鍵が2回現れる冗長性は derive が吸収する。`Keyed` の追加は非破壊なので、必要になれば後から足せる。

**`Append` は dedup できない。** 「要素が他方に**無い**」で分岐する必要があり、それには全域の Bool 値 membership 判定が要る — catch-all impl が衝突し（E0119）、index witness の置き場も無い（E0207）。`Has` が成立するのは*部分*関係だからである。**`Subset` が禁止だからではない**（`Subset` は部分述語としては書けるし、禁止理由はコスト）— T-M0-09 で訂正。したがって `emits = [X]` と `when(C) => { emits = [X] }` の合成は `(X, (X, ()))` を黙って作り、E0283 は離れた `Has` の地点で出る。**dedup は無条件に macro の責務**であり、`compile_fail/append_duplicate_breaks_membership.rs` がこの経路を固定している。

### なぜトップレベルに全部書く案を採らなかったか

```rust,ignore   // fragment, not a complete item
// 却下した案
mutates = [User::name, User::email],   // email も無条件扱い
when(EmailChanged) => { emits = [..] },
```

この形だと `set_email` を `when` の外で無条件に呼べてしまう。「emailは条件下でのみ変わる」という**Verumの中核主張（agenda §5, §6）がMutationについて実現しない**。

型がシンプルになる（`CondMutates` 不要）という利点はあるが、`Append` はEmit/Callのために既に必要なので追加コストはほぼない。

### なぜ重複を禁止するか

理由が2つある。

1. **意味論の矛盾** — 同じFieldが「無条件でも条件付きでもある」状態は定義できない
2. **技術的な破綻** — Append後に重複が生じると `Has` のindex推論が壊れ、E0283（型注釈が必要）という無関係なエラーになる

```text
error[E0283]: type annotations needed
note: multiple `impl`s satisfying `(Mn, (Mn, ())): Has<Mn, _>` found
```

macro段階で弾く（[`diagnostics.md`](./diagnostics.md) 層1）。

```text
error: `User::email` is declared both unconditionally and under `when(EmailChanged)`
  --> src/endpoints/user.rs:12:16
   |
12 |     mutates = [User::name, User::email],
   |                            ^^^^^^^^^^^^ declared unconditionally here
...
17 |         mutates = [User::email],
   |                    ^^^^^^^^^^^^ and conditionally here
   |
   = help: remove one of them — a field is either unconditional or conditional
```

### 実効集合（上界）

「このEndpointが変更しうるField全体」は以下になる。

```text
effective_mutates = mutates ∪ (全 when の CondMutates)
```

ソース上は2箇所に分かれるため、**AI Contextには合算した完全形を出力する**。これは [`effect-system.md`](./effect-system.md) の「書く側は差分、読む側は完全形」と同じ構造である。

---

## GET の read-only 保証との関係

`Mutates = ()` だけでは不十分になる。`Conditional` 内の `CondMutates` も空でなければならない。

型で検査するには `Conditional` に対する再帰的な畳み込みが必要で、negative reasoningに近づく（[`rust-type-model.md`](./rust-type-model.md) が避けると定めた領域）。

**macroで弾く。**

```text
error: GET endpoint `GetUser` cannot declare mutations
  --> src/endpoints/user.rs:16:9
   |
16 |         mutates = [User::status],
   |         ^^^^^^^^^^^^^^^^^^^^^^^^ inside `when(...)` on a GET endpoint
   |
   = note: GET endpoints are read-only by construction
   = help: use PUT / PATCH / POST / DELETE
```

`creates` / `deletes` も同様。層1で弾けるものは層1で弾く（[`diagnostics.md`](./diagnostics.md)）。

---

## 実装シグネチャ

```rust,ignore   // fragment, not a complete item
pub async fn when<C, F>(&self, u: &mut E::Domain, r: &E::Request, f: F) -> Result<()>
where
    C: Condition<E::Domain, E::Request>,
    F: AsyncFnOnce(Ctx<'_, WhenScope<E, C, I>>, &mut E::Domain, &E::Request) -> Result<()>;
```

- `user` / `req` は**キャプチャさせず引数として貸す**。`&user` を渡しつつ `async move` でキャプチャする形は借用エラーになる（実コンパイルで確認済み: E0382 / E0505 ×2 / E0382）
- **Rust 2024 edition の async closure（`AsyncFnOnce`, 1.85+）が必須。** `FnOnce(..) -> Fut` 方式は借用を跨げない
- **戻り型は `Result<()>` に固定する。** そうしないと `Ok(ctx)` で昇格されたContextをスコープ外へ持ち出せる

```rust,compile_fail
let elevated = ctx.when::<C, _>(.., async |ctx, ..| Ok(ctx)).await?;
//                                                   ^^^^^^^ 型エラー
```

---

## 得られる保証

| 保証されること | 仕組み |
|---|---|
| 条件付きMutation / Emit / Call がスコープ外で発火しない | 外側の`ctx`がCapabilityを持たない → 型エラー |
| 宣言していない条件付きEffectは発火できない | `Conditional`に無い → `Lookup`が失敗する |
| 昇格Contextをスコープ外へ持ち出せない | クロージャ戻り型が `Result<()>` に固定されている |
| 条件とEffectの対応がコード上で視認できる | ブロック構造として視覚化される |

---

## 保証されないこと — 原理的な限界

**`Condition::holds` の中身は型で検証できない。**

```rust
impl Condition<User, UpdateUserRequest> for EmailChanged {
    const NAME: &'static str = "EmailChanged";
    fn holds(user: &User, req: &UpdateUserRequest) -> bool {
        true      // ← これで条件付きEffectが全件無条件化する
    }
}
```

`when` は「条件がEffectを制限する」機構ではなく、**「利用者が書いた未検証のboolがCapabilityを解錠する」機構**である。

さらに、**AI Contextが `"conditional": [...]` と出力し続けるためメタデータが能動的に嘘をつく**。

### 対処

- AI Contextに `condition_verified: false` を**必ず**出力する（[`unverified-boundaries.md`](./unverified-boundaries.md) #20）
- `Condition` の実装は**純関数**であることを規約化する（外部I/O・時刻・乱数の禁止）
- 条件をnamed typeとして1箇所に定義させ、レビュー・テストの対象として特定可能にする

**「型で保証されている」と表現してはならない。**

---

## Condition trait

```rust,ignore   // needs a crate or a verum-private module this harness does not carry
pub trait Condition<Domain, Request>: derive_facing::SealedCondition<Domain, Request> {
    const NAME: &'static str;
    fn holds(domain: &Domain, req: &Request) -> bool;
}
```

### 同期・純関数であることの限界

以下は表現できない。

- **Feature flag / A-Bテスト** — 外部サービスへの非同期問い合わせが必要
- **時刻・ロールアウト率に依存する条件** — Domain / Requestから到達できない

`async fn holds(ctx: &Ctx<..>, ..)` への拡張はCapability境界内での外部I/Oを許すことになり、Effect Systemとの整合を再検討する必要がある。[`research-questions.md`](./research-questions.md) に記録。

---

## 条件の合成（未決定）

```text
when(EmailChanged and NotSuspended)
when(EmailChanged or PhoneChanged)
when(not Verified)
```

- `and` → 両方の条件付きEffectの和集合か、個別宣言か
- `or` → どちらが成立したか実行時に分からないため、和集合を許可するしかない
- `not` → negative reasoningの問題に触れる可能性

First PoCでは単一条件のみ扱う。

---

## ネストした条件

構造的には可能だが、Contract側の宣言形式が未定。読みやすさが急速に低下するため、**ネストは2段までに制限する**ことを検討する。それ以上は条件の合成を使う。

---

## AI Context出力

```json
{
  "mutates": {
    "unconditional": ["User.name"],
    "conditional": [
      { "condition": "EmailChanged", "fields": ["User.email"] }
    ],
    "effective": ["User.name", "User.email"],
    "enforcement": "upper_bound_checked"
  },
  "conditional": [
    {
      "condition": "EmailChanged",
      "condition_defined_at": "src/conditions/user.rs:12",
      "condition_verified": false,
      "mutates": ["User.email"],
      "emits":   ["EmailVerificationRequested"],
      "calls":   ["EmailService"]
    }
  ]
}
```

AIが以下の3つを区別できる必要がある。

1. 常に起きること（`unconditional`）
2. 条件次第で起きること（`conditional`）
3. 条件自体が信頼できないこと（`condition_verified: false`）

---

## 優先度

First PoCでは `when` を実装しない。ただし**宣言場所の規則はmacroに最初から実装する**（後から変えると全Contractの書き換えになる）。

`unverified_boundaries` への `condition_body` 出力もFirst PoCから含める（`when` 未実装の段階では該当項目が空になるだけ）。

[`../roadmap/roadmap.md`](../roadmap/roadmap.md) を参照。
