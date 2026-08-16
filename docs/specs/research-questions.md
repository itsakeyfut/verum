# Research Questions

未解決の設計課題。Verumが勝負する領域はすべてここに属する。

関連: [`unverified-boundaries.md`](./unverified-boundaries.md) / [`../roadmap/roadmap.md`](../roadmap/roadmap.md)

---

## 最優先で決着させるべき3つ

### ~~Q-A. 「実装からContractを生成する」方式を採るか~~ → **決定済み（2026-08-15）**

**採る。ただし型強制を置き換えるのではなく、両方を持って差分を検出器にする。**

目標が2つあり別々の機構を要求するため。「抜け穴を作らせない」＝上界＝型強制（成立済み）、「嘘を作らせない」＝下界＝生成（型では原理的に出せない）。**2つの差分が、本ファイル §過剰宣言の検出 の未解決問題そのものを解く。**

| 決めたこと | 内容 |
|---|---|
| 方式 | 型強制はそのまま + `handle` のトークン走査で `observed_effects` を生成 |
| 範囲 | First PoC は **`handle` のみ**。`scope: "handle_only"` として AI Context に明示 |
| 過剰宣言 | **CI が落とす**（3層防御のいずれでもない、ビルド時の第4の機構）。`@service` の逃げ道は使用自体を記録 |
| 前提の検証 | **T-M1-07** として Phase 1 に spike を追加（トークン走査の成立は未検証） |

全文と却下した4案は [`effect-inference.md`](./effect-inference.md) §決定（Q-A）。

### Q-B. token収支は本当に黒字か

`../concepts.md` の「数千行の代わりに数十〜数百token」は場面を区別していない。

| タスク | 収支 |
|---|---|
| 多数のEndpointを概観する | 黒字 |
| **1つのEndpointを編集する** | **赤字**（Contractと実装の両方を読み、加えて規約の知識が必要） |

実際のAI Codingは後者が大半。加えてAI Contextは1 endpointで400〜600token、200 endpointで約100k tokenになる。Verumの概念数は約40（Axum 8〜10、Rails 15〜20）で、しかも学習データに存在しないため毎セッションcontextに載せる必要がある。

**損益分岐点を数値で出す必要がある。** 目的を「token削減」から「コンパイラをAIのフィードバックループにする」に付け替える選択肢も含めて判断する。

### ~~Q-C. 前提検証をいつやるか~~ → **実施済み（2026-08-14）**

実験の設計・結果・損益分岐点（約5エンドポイント）・中止基準は [`evaluation.md`](./evaluation.md) §実施済み実験: Q-C 前提検証。成果の一つが `operation` の削除で、本ファイル §`operation` に記録がある。

以下は決定当時の記述である。

`evaluation.md` の9指標には測定方法・判定者・試行回数・合格閾値・**中止基準**がない。しかも実施はPhase 2（型設計に全リソースを投下した後）。

Axumを外す判断には3つの実測基準があるのに、**プロジェクト自体の中止/方針転換基準がない**という逆転が起きている。

コードを1行も書かずにできる実験がある: 同一課題を「素のAxum / 構造化アノテーション付きAxum / 手書きVerum疑似コード + チートシート」の3条件でAIに解かせ、token・iteration・違反率を測る。

---

## 方針が確定した問い

| # | 問い | 決定 | 参照 |
|---|---|---|---|
| 1 | Effectをどの粒度まで型に表現できるか | State / External / Infrastructure の3系統。カテゴリ別associated type。語彙は閉じる | [`effect-system.md`](./effect-system.md) |
| 2 | Field-level Mutationをどう型表現するか | **Domainを不透明型にし**、FieldごとのZSTマーカ + Capability要求setter | [`mutation-contract.md`](./mutation-contract.md) |
| 3 | Conditional Effectをどう表現するか | 型では不可。`ctx.when::<C>` スコープでCapability発行（async closure必須）。**条件付きMutation / Emit / Call は `when` 内に宣言し、トップレベルとの重複を禁止する**（Q-C実験で発見された穴を仕様化） | [`conditional-effects.md`](./conditional-effects.md) |
| 5 | Capability Systemをどう設計するか | `Ctx<'req, E>` がContractでパラメタライズされ、**拡張trait**のwhere節で検査 | [`capability-system.md`](./capability-system.md) |
| 6 | GETのRead-only guaranteeをどう証明するか | `Endpoint<Mutates=(), Creates=(), Deletes=()>` + **deriveのコンパイル時アサーション**（blanket implでは不可） | [`rust-type-model.md`](./rust-type-model.md) |
| 8 | Architecture Contractをどう強制するか | `Self::Owner: Includes<User>` をメソッド側where節に置く（`Includes` の主語は Endpoint 型 — [ADR-0001](../adr/0001-includes-is-implemented-on-the-endpoint.md)） | [`architecture-contract.md`](./architecture-contract.md) |
| Q-A | 実装からContractを生成する方式を採るか | **両方を持ち差分を検出器にする。** 型強制＝上界（抜け穴）、生成＝下界（嘘）。過剰宣言は CI が落とす。範囲は First PoC では `handle` のみ。**前提は T-M1-07 で実測する** | [`effect-inference.md`](./effect-inference.md) |
| 10 | Proc Macroだけで可能か | 3層防御（macro / equality bound / trait bound）**＋ ビルド時のトークン走査**（Q-A の決定で追加。宣言と実装の差分を出す第4の機構で、proc macro の外にある）。独自Linterが必要なのは Escape Hatch と生SQL | [`diagnostics.md`](./diagnostics.md) |

### 実コンパイル検証で確定した技術的制約

| 論点 | 結論 |
|---|---|
| `Has` の再帰impl | 素朴な形はcoherence違反（E0119）。**indexパラメータ版が必須** |
| Effect集合の表現 | フラットタプルでは所属判定が不可能。**cons list に統一** |
| `Endpoint<METHOD = Method::GET>` | associated const equality boundはunstable。かつblanket implの論理が成立しない。**Methodを型レベルマーカにする** |
| `impl<E> Ctx<E> { fn users() }` | E0116。**拡張traitが必須** |
| `when` の借用 | `&user` を貸しつつ `async move` は借用エラー。**async closure（edition 2024 / 1.85+）が必須** |
| AFIT の Handler | dyn非互換 + Future が非Send。**RPITIT + Send + 消去レイヤ** |
| 型レベル演算 | `Has` / `Append` / `Lookup` は安全。`Subset` / `Filter` / negative reasoning は不可 |
| `on_unimplemented` | messageは制御できるがhelp/note連鎖は残る。**`do_not_recommend` が必須** |
| Contract宣言箇所への `note` | trait bound経由では**出せない**。equality bound経由のみ |

---

## First PoCで検証すべき未検証事項

| 項目 | 何が不明か | 失敗した場合の影響 |
|---|---|---|
| ~~**Domain不透明化 × sqlx**~~ | **判定済み（T-M1-01 / #13、コンパイル検証済み）**: sqlx 連携は成立。「信頼境界 = Repository 実装」は**不成立**（`Repr` は同一クレートのどこからでも到達可能 — 台帳 path 21）。加えて仕様が名指しする derive では必要な形を生成できない。詳細は [`persistence.md`](./persistence.md) §判定、再現は `spikes/domain-opacity-sqlx/` | — |
| **`Ctx<'req, E>` × async** | RPITIT / async closure との組み合わせが成立するか | 成立しなければ spawn 経路を型で塞げない。**測定は完了（T-M1-02 / #14、`spikes/ctx-lifetime-rpitit/` が19プローブで再現）だが、仕様への判定反映は #38 の完了待ち** |
| **Domain getterだけで `reads` 強制が足りるか** | Projection型が不要になる可能性 | 足りればProjectionの複雑さを丸ごと削れる |
| **`trybuild` の安定性** | cons list / `There<There<..>>` を含むエラー文言がrustcバージョン間で揺れるか | 揺れるなら正規化の仕組みが必要 |
| **コンパイル時間** | Endpoint数 × Effect数での `Has` 解決コスト | 悪化するなら型強制の範囲を縮小 |

---

## Contract表現の未解決課題

### 一覧・検索・集計・JOIN（優先度: 最高）

**`Read<Domain, Field>` が単一インスタンス前提であることの帰結。** 実Webアプリで最も画面数が多い形が書けない。

- 一覧取得: `Vec<Projection<..>>` を返すAPI、件数・順序・動的フィルタの表現
- ページネーション: 絞り込み条件は実行時に決まるため型パラメータに載らない
- 集計（COUNT / SUM / GROUP BY）: 特定Fieldの値ではなく、結果はどのDomainインスタンスにも属さない
- JOIN: 複合Projection（`Projection<(User, Order), (..)>`）が未定義
- **N+1 / eager loading**: Field単位メソッド（Rule 1）と構造的に衝突する。Read側の例外規定が必要

これは機能の欠落ではなく**モデルの前提の綻び**である。`GetUser`/`UpdateUser` という ID 指定の単一取得例だけでモデル全体を組んだ結果。

### Validation

Requestの制約（必須 / 範囲 / 形式）を宣言する仕組みが存在しない。`reads`/`mutates` はDomainのFieldに対する権限であり、Request のFieldは射程外。

実務でバグ・セキュリティ問題の温床になりやすい領域が、「契約違反をコンパイラで拒否する」という主張の対象範囲から抜けている。

### Error

- `fails = [NotFound, Conflict]` のように宣言するか
- HTTP statusとドメインエラーのマッピングをどこで定義するか
- Errorを一種のEffectとして扱うか
- OpenAPI生成には必須の情報

### Transaction

- Endpoint = 1トランザクションを標準とするか
- **トランザクション内でExternal Effectの発火を型で禁止できるか**（`ctx.after_commit` スコープを [`handler-rules.md`](./handler-rules.md) Rule 4 で提案）
- **部分失敗の意味論** — Contractは上界なので「宣言したEffectの部分集合しか起きなかった状態」が表現されていない。`atomicity: "none"` を出力する暫定対応
- Savepoint / ネストしたトランザクション

### ソフトデリート（優先度: 高）

`mutates = [User::deleted_at]` と `mutates = [User::name]` が**構文上区別できない**。Contractを読むAIには「name変更」も「論理削除」も「復元」も同じに見える。

**"Semantics over Syntax" がCRUD最頻出パターンで機能していない。** `SoftDelete<Domain>` / `Restore<Domain>` のような追加タグが必要か。

### 楽観ロック / 悲観ロック

- Field単位setterでは `WHERE id=? AND version=?` のCompare-and-Swapを原子操作として表現できない
- 「Fieldごとに独立して成功/失敗する」というMutation Contractの前提が崩れる
- `SELECT ... FOR UPDATE` に相当する `Lock<Domain>` Effectがない

### Bulk operation

100件一括更新をField単位setterでどう書くか。`One<User>` / `Many<User>` のような対象数の分離が必要か。

### 静的Capabilityと動的Authorization

```text
静的 (コンパイル時) — 「このEndpointは何ができるか」
動的 (実行時)      — 「この呼び出し主体は何をしてよいか」
```

現在の設計は前者のみ。**加えて `../concepts.md` の原則「Capability over Permission Checks」という文言が、認可を Capability で置き換えられると誤読させる。**

- `authz` をContractの必須項目にするか（`authz = [Owner]` / `authz = [Public]`）
- AI Contextに `authorization` フィールドを追加し、空を許さないか
- 行レベル権限（IDOR）は型検査の対象外であることの明示

### Multi-domain Endpoint

- `creates` / `emits` に現れるDomainは自動的にアクセス可能とすべきか
- 業務的に独立した2つのDomainを同時更新するEndpointを許すか
- Aggregate境界を越えたトランザクション

### State Transition Contract

`status: active → suspended` をContractで表現するか。Typestateで可能か。遷移の網羅性を検査できるか。

### 条件の合成

`when(A and B)` / `when(A or B)` / `when(not A)` のCapability合成規則。`not` はnegative reasoningの問題に触れる。

### Condition の非同期化

`Condition::holds` は同期の純関数として定義されている。Feature flag / A-Bテスト / 時刻依存の条件は外部I/Oを必要とするため表現できない。`async fn holds(ctx: &Ctx<..>, ..)` への拡張はEffect Systemとの整合を再検討する必要がある。

### Job / Background

HTTPリクエストが存在しない処理に `Endpoint`（Method + Path + Request/Response）の枠組みが適用できない。`#[job(schedule = "...")]` のような第一級のContract単位が必要。

### `operation` — 削除して解決（Q-C実験の成果）

**削除した。** 詳細は [`semantic-endpoint.md`](./semantic-endpoint.md)。

実験の被験者はこう報告した。

> `operation` フィールドの取り得る値が不明（`Read`/`Update`/`Suspend`/`Delete` のみ観測）。新エンドポイントに `UpdateEmail` のような専用値を作るべきか迷ったが、**存在しない enum variant を捏造するリスクを避け**、既存の `Update` を再利用した。

「AIが毎回迷い、かつ何も保証しない」フィールドであり、情報は `Method` + `Domain` + `mutates` と**Endpoint型名**から導出できるため削除した。業務的な操作名は型名（`SuspendUser`）が担う。

同時に指摘された「サポートするHTTP Method一覧が未記載」も解決（Get / Head / Post / Put / Patch / Delete を明記）。

> **Q-C実験は「AIが判断に迷った点」を集めることで、仕様の穴を3件検出した**（conditional mutationの宣言場所 / `forbidden` の意味論 / `operation`）。今後の実験でも必ず収集する。

### Middleware Contract

MiddlewareのEffectがContractにもAI Contextにも現れない。Auth Middlewareが `last_login_at` を更新すると「GETはread-only」がハンドラスコープでのみ真になる。

Router が「Endpoint宣言 + 適用される全Middlewareの宣言」を合成する仕組みが必要。

### Event 購読側 Contract

`emits` の宣言コストはほぼゼロだが、購読側は任意のEffectを起こせる。**Emitは任意Effectへの汎用ゲートウェイになっている。** 推移閉包をAI Contextに出力するには購読側にもContractが必要（別crateにある可能性がある）。

---

## 実装技術の未解決課題

### Repository trait 定義の生成（優先度: 高）

Field ごとに `set_<field>` を trait と impl の両方に手書きする状態では、

1. 新Domainごとにboilerplateが増え、token効率の主張が「書く」場面で崩れる
2. **`set_email` のwhere節に誤って `user::Name` を書いても検出されない** — 「rustcが照合を代行する」という主張が手書きboilerplateの正しさに依存する

impl生成より先にtrait定義の生成を前倒しすべき。

### ~~過剰宣言の検出~~ → **Q-A の決定で解ける**

宣言したのに使わないCapabilityはエラーにならない（Contractが上界であることの帰結）。**`declared_ceiling \ observed_effects` がそのまま検出器になる** — Q-A（2026-08-15）で、これを副産物ではなく差分を取る主目的として位置づけた。CI が落とす。[`effect-inference.md`](./effect-inference.md) §決定（Q-A）。

### Database Mutation の検出

Repository実装内部のSQLは信頼境界の外。derive生成で境界を移すか、sqlx `query!` のカラム静的検査（SQLパースが必要、動的SQLに無力）。

### エラーメッセージの品質

- deriveが型エイリアスを生成してエラー中の型名を短縮できるか
- `when` スコープ外での発火時のエラー（入れ子projectionが露出する）
- 拡張traitの `use` 忘れによる「no method named `users`」への対処（prelude / `pub use` の自動生成）

### Handler Rule の強制

- Rule 1: 利用者が独自の包括メソッドを足す場合の Lint
- Rule 2: 自由関連関数（`AuditLog::user_updated` 等）の純粋性。`#[derive(Event)]` / `#[derive(View)]` でコンストラクタを生成して手書きを消せるか

### Infrastructure Effect の強制

現状 `enforcement: "none"`。`ctx.cache()` のwhere節で強制するか、**この軸自体を捨てるか**。概念あたりの強制力が全Contract項目中で最も低い。

---

## AI Coding 実務の未解決課題

### Contract緩和バイアス（優先度: 高）

AIはコンパイルエラーに対して**実装を直すよりContractを1行広げる方を選ぶ**。「helpは2方向を示す」は文言レベルの対策で、選択そのものは制約できない。

CIでContract拡大差分を検出する等の運用対策が必要。**これは型の問題ではない。**

### AI Context をいつ・どう読むか

スキーマを設計してもAIが読まなければ意味がない。`CLAUDE.md` 相当への手順明記、出力コマンドの固定、鮮度保証（git管理外 / CI差分検査）が未定義。

### 学習データの不在への対処

Verumは学習データに存在せず、ドキュメントの実装例は `UpdateUser` / `GetUser` の1パターンのみ。DELETE / POST / Service経由 / 一覧 / エラーハンドリングの実例がない。

AIは学習データに大量にあるAxumイディオム（`State<AppState>`、汎用 `save()`、可変長extractor）に引き寄せられる。

- パターンごとに最低1つの完全な実例を用意する
- 却下例のコードブロックには `// ❌ REJECTED` を必ず入れる（見出しだけに依存しない）
- 失敗パターンを記録してfew-shot例を増強するループを最初期から回す

### テスト戦略（完全な空白）

`Ctx<'req, E>` をテストからどう構築するか。sealedコンストラクタを決めたが、**テスト用APIの設計が未定**。

- `verum::test::run::<UpdateUser>(req, mocks)` のような Endpoint 型固定のAPI
- Repository の mock をどう与えるか
- Contract 自体のテスト

### CLAUDE.md 戦略

フレームワークの作法（属性DSLのキー名、`Ctx` 経由の規約、`when` スコープ、Field単位setterの命名）を毎プロジェクトで書き込む必要がある。100行程度に圧縮した最小リファレンスが必要。

**構造的に強制される部分と規約に留まる部分を区別して書く**（忘れても安全な部分 / 忘れると危険な部分）。

---

## AI Context の未解決課題

### Context サイズ管理

1 endpoint で 400〜600 token、200 endpoint で約 100k token。「数千行の代わりに数十〜数百token」と衝突する。分割・要約・必要な Endpoint だけを引く仕組み。

### Semantic Code Graph の Schema

現在のJSON案は暫定。Endpoint間の関係（Eventの発行者と購読者）をGraphとして表現するか。Schemaのバージョニング。

### MCP 提供

静的JSONか、MCPサーバとして動的提供か。

---

## Framework設計の未解決課題

### Escape Hatch

- 宣言形式（`#[escape_hatch(reason = "...")]`）
- **記録が自己申告である問題** — 属性なしでも低レイヤAPIは呼べる。ZST証拠を引数で要求する形にすれば構造的に防げる
- 低レイヤへ降りる際、Capabilityの検査をどこまで維持するか
- 「Freedom Without Chaos」と「Capability-based Safety」の両立は、この機構が設計されるまで**未証明**である

### Service 層の位置づけ

`architecture-contract.md` は Handler → Service → Repository を「正しい経路」とするが、**全コード例に Service が登場しない**。`ctx.users()` が Repo を直接返す設計が Service 迂回を最短経路にしている。

Service を任意とするか必須とするか、必須なら Capability をどう引き継ぐかを決める必要がある。

### 設計原則の整理

`../concepts.md` の24原則には優先順位も裁定規則もなく、**同じ論法が場所によって逆向きに使われている**。

- `effect-system.md`: 「書き忘れと意図的な非宣言が区別できない」→ 暗黙は悪
- `read-contract.md`: 「明示させると書き忘れが増える」→ 暗黙は善

実質的に異なる主張は6〜7個。統合して優先順位を付ければ、衝突を機械的に裁定できる。

---

## 展開

### 他言語への展開可能性

Goにはassociated typeもZSTマーカもない。TypeScriptは構造的型付けで、リテラル型とmapped typeで別の表現になる。設計の可搬性は低い可能性がある。

### 既存研究・競合との差分

- Effect System研究（Koka / Eff / OCaml 5 effects）
- Capability-based security の先行研究
- Session Types（WebSocket / Streaming に効く可能性）
- **Nifra との差分** — Nifraは既にAI Context + architecture drift detectionを持つ。Verumの差分は「型で強制」の一点に収束する。この整理が未着手のまま競合主張を書いている状態
