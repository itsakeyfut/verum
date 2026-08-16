# Effect System

Web/API向けに意味論を拡張したEffect System。宣言粒度とカテゴリ分割の仕様。

関連: [`capability-system.md`](./capability-system.md) / [`conditional-effects.md`](./conditional-effects.md) / [`rust-type-model.md`](./rust-type-model.md)

---

## 基本方針

> 「副作用」を一つの巨大な `IO` にまとめない。
>
> **AIが判断できる粒度までEffectを分解する。**

---

## Effectの分類

Effectは3系統に分かれ、**宣言される場所と語彙が系統ごとに異なる**。

### State Effects — `reads` / `mutates` / `creates` / `deletes`

```text
Read<User, user::Name>
Mutate<User, user::Email>
Create<AuditLog>
Delete<Session>
```

### External Effects — `emits` / `calls`

```text
Emit<UserUpdated>
Call<EmailService>
Call<PaymentService>
```

### Infrastructure Effects — `effects`（差分のみ）

```text
DatabaseRead
DatabaseMutation
CacheRead
CacheWrite
Metrics
Logging
Tracing
FileRead
FileWrite
Network
Time
Spawn<Job>
```

> **語彙は閉じている。** 上記以外のInfrastructure Effect名を使ってはならない。
>
> 特に `SendEmail` / `MessagePublish` / `ExternalMutation` は**Infrastructure Effectではない**。これらは External Effect であり `calls = [EmailService]` / `emits = [X]` で表現する。同じ副作用に2つの宣言経路を作ると、AIがどちらを書くべきか判定できず、両方書くか片方を落とす。

---

## 決定1: Effectカテゴリを分けて宣言する

統一された `effects = [...]` ではなく、カテゴリ別の属性キーを持つ。

```rust,ignore   // needs a macro that arrives in M2
#[contract(
    reads   = [User::id, User::status],
    mutates = [User::name, User::email],
    creates = [AuditLog],
    emits   = [UserUpdated],
)]
```

deriveがカテゴリ別のassociated typeに展開する（cons list表現）。

```rust
// `mutates` の Field は自動的に `reads` に含まれる（[`read-contract.md`](./read-contract.md)）。
// 宣言は reads=[id, status] の2つだが、展開後は name / email が加わって4要素になる。
type Reads   = (Read<User, user::Id>,
               (Read<User, user::Status>,
               (Read<User, user::Name>,
               (Read<User, user::Email>, ()))));
type Mutates = (Mutate<User, user::Name>, (Mutate<User, user::Email>, ()));
type Creates = (Create<AuditLog>, ());
type Deletes = ();
type Emits   = (Emit<UserUpdated>, ());
type Calls   = ();
```

> cons list（`(A, (B, ()))`）である理由: フラットタプル `(A, B)` では所属判定のimplがcoherence違反になる。[`rust-type-model.md`](./rust-type-model.md) を参照。

### 理由

単一Effectの所属判定については、統一案とカテゴリ別案で**強制力は完全に同一**である。差が出るのは以下。

#### (a) 「不在」の表明 — 決定的な差

GETのread-only保証は「Mutationを1つも持たない」という表明である。

```rust
trait ReadOnly: Endpoint<Mutates = (), Creates = (), Deletes = ()> {}
```

associated **type** equality bound は stable で、エラーも明快になる。

```text
expected unit type `()` found tuple `(Mutate<User, user::Email>, ())`
```

統一案では「Mutateを含まないこと」を要求する必要があるが、Rustには**negative trait boundが存在しない**（`!Trait` はunstable、型パラメータのワイルドカードも書けない）。型レベルBoolによる畳み込みで代替可能だが、エラーが「どの要素が原因か分からない」形に悪化する。

#### (b) Repositoryへの受け渡し

カテゴリ別なら分類済みの型をそのまま渡せる。統一案では `Effects` からMutateだけを取り出す型レベル `Filter` が必要になり、**catch-all implが必ず衝突する**（[`rust-type-model.md`](./rust-type-model.md)）。

#### (c) trait解決コスト

`Has<Set, Elem, Idx>` は要素数に線形。カテゴリ別なら短いcons list（3-4要素）を走査、統一案なら全Effect（10-15要素）を走査する。

#### (d) AIの書きやすさ

カテゴリ別のキー名は**構造化された穴埋め**になる。統一案は自由記述に近く、抜けが出やすい。

### GET ⇒ ReadOnly の強制方法

`impl<E: Endpoint<Method = Get>> ReadOnly for E {}` は**書けない**。`ReadOnly` が `Mutates = ()` をsupertraitに持つため、blanket impl側でもそれを要求せざるを得ず、Methodについて何も強制しなくなる（実コンパイルで確認済み）。

deriveが生成するコンパイル時アサーションで強制する。

```rust
const _: () = {
    fn assert_readonly<E: Endpoint<Method = Get> + ReadOnly>() {}
    fn check() { assert_readonly::<GetUser>(); }
};
```

加えて、proc macroが展開時点で「GETなのにmutates/creates/deletesがある」を弾く（エラーが最も精密になる）。詳細は [`rust-type-model.md`](./rust-type-model.md)。

### 統一ビュー（将来）

横断的な用途のために、deriveが全カテゴリの連結も生成できる。利用者は書かない。**First PoCでは不要。**

---

## 決定2: Infrastructure Effectは「Methodデフォルト + 差分」

### フレームワークが持つデフォルト

```text
GET / HEAD  → DatabaseRead, CacheRead, Logging, Metrics, Tracing
POST        → 上記 + DatabaseMutation
PUT / PATCH → 上記 + DatabaseMutation
DELETE      → 上記 + DatabaseMutation
```

### Endpoint側は逸脱のみ宣言

```rust,ignore   // needs a macro that arrives in M2
#[contract(
    mutates = [User::email],
    effects = [+CacheWrite],     // 追加
)]

#[contract(
    reads   = [User::id],
    effects = [-CacheRead],      // 明示的に禁止
)]
```

### 理由

| 案 | 評価 |
|---|---|
| 全Effect明示 | Endpointごとに8-10行の定型句。token効率が悪化し、**書き忘れと意図的な非宣言が区別できない** |
| Infrastructure宣言不要 | Contractは読みやすいが、「このEndpointがキャッシュを書くか」が消え、意図的にLoggingしないEndpointを表現できない |
| **デフォルト + 差分** | **書く側は短く、読む側は完全形** |

### 重要な制約: Infrastructure Effect は型で強制されない

Methodデフォルト表は**単なるドキュメントの表であり、型検査はゼロ**である。`effects = [+CacheWrite]` を書かずに `ctx.cache()` を呼んでも、現状の設計では止まらない。

したがってAI Contextには `enforcement: "none"` を明記する。

```json
"effects": {
  "declared_delta": ["+CacheWrite"],
  "effective": ["DatabaseRead", "DatabaseMutation", "CacheRead", "CacheWrite", "Logging", "Metrics", "Tracing"],
  "enforcement": "none"
}
```

**強制レベルの差を隠さないことが、この設計を採る条件である。** [`ai-context.md`](./ai-context.md) を参照。

> この軸は「概念あたりの強制力」が全Contract項目中で最も低い。将来 `ctx.cache()` のwhere節で強制するか、この軸自体を捨てるかの判断が必要。[`research-questions.md`](./research-questions.md) に記録。

### 書く側と読む側の分離について

ソース上のContractは差分、AI Contextは完全形とすることで token効率とExplicit Effectsを両立させる。ただしこれは**「ソース単体では完全な意味が読めない」ことを受け入れる**判断である。

`../concepts.md` の信頼順位（Type/Contract → ... → Generated Documentation）において、**AIに読ませたい完全形は生成物側にある**。この矛盾を認識し、生成物の鮮度保証（CIでの再生成 + 差分ゼロ検査）を仕様に含める必要がある。

---

## GET / Immutability Guarantee

GET Endpointについて、

> GETなら必ずImmutableである

を型レベルで保証する。ただし「Immutable」の定義を慎重にする。GETでも Logging / Metrics / Tracing / CacheRead / CacheWrite は発生し得る。

```text
GET User

Allowed:
    DatabaseRead / CacheRead / Metrics / Logging / Tracing

Forbidden:
    DatabaseMutation / MessagePublish（= emits）/ ExternalMutation（= calls）/ FileWrite
```

### 重要な言い換え

```text
GETだから副作用がない
```

ではなく、

```text
GET Endpointには Mutation Capability が存在しない
```

### 保証の範囲は「ハンドラスコープ」

**MiddlewareがContractを持たない限り、この保証はリクエストスコープでは成立しない。**

```text
Auth Middleware が last_login_at を更新する場合:
  ハンドラスコープ  : Mutates = () → read-only（真）
  リクエストスコープ: User.last_login_at が更新される（偽）
```

AI Contextに `scope_of_readonly_guarantee: "handler_only"` として明示する。Middleware Contractを導入した時点で `"request"` に昇格させる。[`unverified-boundaries.md`](./unverified-boundaries.md) を参照。

---

## 型レベルの制約

| 演算 | 可否 |
|---|---|
| `Has<Set, Elem, Idx>` — 単一要素の所属判定 | 安全（線形） |
| `Append<A, B>` — cons listの連結 | 安全 |
| `Lookup<Set, Key, Idx>` — 型レベルmap検索 | 安全 |
| `Subset<A, B>` / `Filter<Set, Pred>` | **避ける** |
| negative reasoning | **不可** |

詳細は [`rust-type-model.md`](./rust-type-model.md) を参照。
