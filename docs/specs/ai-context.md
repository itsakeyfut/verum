# AI Context / Semantic Code Graph

AIがコードベースを探索するコストを減らすための構造化情報。Frameworkの第一級成果物。

関連: [`unverified-boundaries.md`](./unverified-boundaries.md) / [`effect-system.md`](./effect-system.md) / [`effect-inference.md`](./effect-inference.md)

---

## 中核思想

> AIに大量のソースコードを読ませるのではなく、まず意味論的メタ情報を読ませる。

---

## 設計原則: 書く側と読む側を分離する

ソース上のContractは短く、**AI Contextには展開後の完全な情報を出力する**。

```text
ソース (Contract)      → 差分・省略あり。token効率を優先
AI Context (JSON)      → 完全形。曖昧さゼロを優先
```

> **この分離のコスト**: 「ソース単体では完全な意味が読めない」ことを受け入れる判断である。[`../concepts.md`](../concepts.md) の信頼順位において、AIに読ませたい完全形は**生成物側**にある。したがって生成物の鮮度保証が必須になる（下記）。

---

## 出力例

```json
{
  "endpoint": "UpdateUser",
  "method": "PUT",
  "path": "/users/{id}",
  "domain": "User",

  "request":  "UpdateUserRequest",
  "response": "UserView",

  "reads": {
    "fields": ["User.id", "User.status", "User.name", "User.email"],
    "declared": ["User.id", "User.status"],
    "implied_by_mutates": ["User.name", "User.email"],
    "enforcement": "metadata_only"
  },

  "mutates": {
    "unconditional": ["User.name"],
    "conditional": [
      { "condition": "EmailChanged", "fields": ["User.email"] }
    ],
    "effective": ["User.name", "User.email"],
    "enforcement": "upper_bound_checked",
    "observed": {
      "fields": ["User.name", "User.email"],
      "scope": "handle_only",
      "deferred": []
    }
  },

  "forbidden": {
    "fields": ["User.password_hash"],
    "enforcement": "intent_only",
    "note": "Not type-enforced. Fields absent from `mutates` are already uncallable; this records intent."
  },

  "creates": { "domains": ["AuditLog"], "enforcement": "upper_bound_checked" },
  "deletes": { "domains": [], "enforcement": "upper_bound_checked" },

  "effects": {
    "declared_delta": ["+CacheWrite"],
    "effective": [
      "DatabaseRead", "DatabaseMutation", "CacheRead", "CacheWrite",
      "Logging", "Metrics", "Tracing"
    ],
    "enforcement": "none"
  },

  "unconditional": {
    "emits": ["UserUpdated"],
    "calls": []
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
  ],

  "dependencies": ["UserRepository", "AuditLogRepository", "EventBus", "EmailService"],

  "scope_of_readonly_guarantee": "handler_only",
  "atomicity": "none",

  "escape_hatches": "unknown",

  "unverified_boundaries": [
    { "kind": "condition_body", "detail": "EmailChanged::holds は型検証不可",
      "location": "src/conditions/user.rs:12", "permanent": true },
    { "kind": "row_scope", "detail": "行レベル権限は型検査の対象外。認可は別途必要",
      "permanent": true },
    { "kind": "middleware", "detail": "適用される middleware の Effect は未宣言",
      "permanent": false },
    { "kind": "event_subscriber", "detail": "UserUpdated の購読側 Effect は未検査",
      "permanent": false },
    { "kind": "service_body", "detail": "observed_effects の走査は handle の中だけ。Service 本体で起きる Effect は下界に現れない（path 22）",
      "permanent": false },
    { "kind": "domain_repr", "detail": "Domain の Repr は同一クレートのどこからでも到達可能（path 21）",
      "location": "src/domain/user.rs", "permanent": false },
    { "kind": "malformed_set", "detail": "壊れた effect 集合を capability 検査に通せる（path 14f）",
      "permanent": false },
    { "kind": "domain_swap", "detail": "*user = other_user は閉じられない（path 2）",
      "permanent": true },
    { "kind": "repository_impl", "detail": "Repository 実装内部の SQL は未検査",
      "location": "src/repositories/user.rs", "permanent": false }
  ]
}
```

---

## 出力に必ず含める6つの情報

通常のドキュメント生成には無い、Verum固有の要件。

### 1. `enforcement` — 強制レベル

各Contract項目が**どの程度型で保証されているか**を明記する。

| 値 | 意味 |
|---|---|
| `upper_bound_checked` | 型検査済み。ただし「実装 ⊆ 契約」の**上界**のみ。宣言したのに使わないEffectは検出されない |
| `intent_only` | 意図の記録。型検査上は冗長（`forbidden` — [`mutation-contract.md`](./mutation-contract.md)） |
| `metadata_only` | 宣言のみ。型検査なし。実装が従っている保証はない |

#### `observed` — 下界（Q-A の決定、2026-08-15）

`enforcement` が答えるのは「**これ以外は起きない**」だけである。「**これが起きる**」は別のフィールドで、`handle` のトークン走査で**生成**する。

```json
"observed": { "fields": [...], "scope": "handle_only", "deferred": [] }
```

| キー | 意味 |
|---|---|
| `fields` | `handle` の中で実際に起きる Effect。**生成物であり手書きしない** |
| `scope` | 走査が届いた範囲。First PoC は `"handle_only"`。**これを出さないと AI は下界が全経路に及ぶと誤読する** |
| `deferred` | `@service` で逃がした項目。Service 本体は走査対象外なので、ここに出ると同時に `unverified_boundaries` に `service_body` が立つ |

**読み方**: `enforcement: upper_bound_checked` かつ `observed.fields == effective` かつ `deferred` が空なら、その範囲（`scope`）において**集合は厳密**である。どれか1つでも欠ければ厳密ではない。

`declared \ observed ≠ ∅`（過剰宣言）は CI が落とすので、**この出力に過剰宣言が残っていることは通常ない**。残っている場合は `deferred` を見ること。**`enforcement` に「両方向検証済み」を意味する値は作らない** — `type_checked` を禁じているのと同じ理由で、層が違うものを1語に畳むと誤読される。詳細は [`effect-inference.md`](./effect-inference.md) §決定（Q-A）。
| `none` | 型検査が存在しない軸（Infrastructure Effect等） |

> **`type_checked` という値は使わない。** 「双方向に検証済み」と読まれるが、Verumの検査は上界のみである。`mutates = [name, email]` は「name と email を変更する」ではなく「**name と email 以外は変更しない**」を意味する。この区別はAIの推論に決定的に影響する。

強制レベルの差を隠すと、Contractの一部が「コメント同然」であることをAIが知らずに信頼してしまう。

### 2. `effective` — 展開後の完全なEffect集合

ソース上は `effects = [+CacheWrite]` という差分だけだが、出力ではMethodデフォルトを展開した完全形を出す。AIがフレームワークのデフォルト仕様を知らなくても判断できる。

### 3. `unconditional` / `conditional` の区別

「常に起きること」と「条件次第で起きること」を混ぜない。

**`condition_verified: false` は省略不可。** 条件の中身は型で検証できないため、これを出さないと**メタデータが能動的に嘘をつく**（`conditional` と書いてあるのに `holds` が `true` を返すだけかもしれない）。

### 4. `unverified_boundaries` — 型検査が届かない箇所

全経路の台帳は [`unverified-boundaries.md`](./unverified-boundaries.md) にある。`permanent: true` は原理的に埋まらないもの。

**この出力機構はFirst PoCから実装する。** 後から追加すると、それまでのAI Contextが「嘘をついていた」ことになる。

### 5. `scope_of_readonly_guarantee`

| 値 | 意味 |
|---|---|
| `handler_only` | ハンドラ内では read-only。Middleware が Mutation する可能性がある |
| `request` | リクエスト全体で read-only（Middleware Contract 導入後） |

「GETは read-only」を無条件に主張してはならない。

### 6. `escape_hatches`

```json
"escape_hatches": "unknown"
```

Escape Hatch の記録は現状**自己申告**（属性を書き忘れれば記録されない）である。したがって空配列 `[]` を出力してはならない。`[]` は「脱出なし」と読まれるが、実際には「未申告の脱出があるかもしれない」である。

低レイヤAPIが属性マクロ生成のZST証拠を引数で要求する形になれば、記録漏れが構造的に防げるため `[]` を出せるようになる。

---

## 生成物の鮮度保証

ソースとJSONの両方が存在する状態で、**AIがどちらを信じるべきか判断できなければ、この設計は「コメントを信頼しない」原則を JSON で再現するだけになる**。

| 手段 | 内容 |
|---|---|
| git管理外にする | ビルドの一部として生成し、コミットしない。古いファイルが存在しない状態を作る |
| CIで差分ゼロ検査 | 再生成して差分が出たら失敗させる |
| タイムスタンプを含める | 生成時刻とソースのハッシュを JSON に埋める |

---

## AIがいつ読むか — 運用の定義が必要

**スキーマを設計しても、AIがそれを読まなければ意味がない。** コーディングエージェントは明示的に指示されない限りソースを直接読む。

したがって以下が必要（現状未定義）。

- `CLAUDE.md` 相当に「Endpointを触る前に `cargo verum contract` を読む」手順を明記する
- 出力コマンドを1つに固定する
- AI向けの最小リファレンス（フレームワークの作法を100行程度に圧縮したもの）を別途用意する

**この運用が定義されなければ、AI Context は「作ったが読まれない成果物」になる。** [`research-questions.md`](./research-questions.md) に記録。

---

## 出力形式

| 形式 | 用途 | 優先度 |
|---|---|---|
| JSON | AI Context / CI検証 | First PoC |
| Markdown | 人間向けドキュメント | Full PoC |
| OpenAPI | 既存ツールチェーン連携 | Full PoC |
| MCP | AI Agentへの動的提供 | Full PoC以降 |

---

## 実装方針

derive macro + inventory（またはlinkme）により、各Endpointのcontractをコンパイル時に収集する。

```text
#[derive(Endpoint)] → contract を inventory に登録
        ↓
cargo verum contract --format json
```

この仕組みは compile-time route table の生成にも再利用できる（[`runtime-stack.md`](./runtime-stack.md)）。

---

## 未解決の課題

- **Contextのサイズ管理** — 1 endpoint で約 400〜600 token。200 endpoint なら約 100k token になり、「数千行の代わりに数十〜数百token」という主張と衝突する。分割・要約・必要な Endpoint だけを引く仕組みが必要
- Endpoint間の関係（Eventの発行者と購読者）をGraphとして表現するか
- Schema のバージョニング

[`research-questions.md`](./research-questions.md) を参照。
