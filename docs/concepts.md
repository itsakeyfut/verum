# Verum — Concepts

プロジェクトの思想・ビジョン・設計原則。

技術仕様は [`docs/specs/`](./specs/README.md) を参照。

---

## 1. Project Vision

AIがWebアプリケーションを実装することを前提に、**Endpointの意味論・可変性・副作用を強力な型システムで表現し、AIが実装内部を大量に読まなくても正確な処理契約を理解できるWebフレームワーク**を作る。

単なる「AIが読みやすいWebフレームワーク」ではなく、

> **AIが正しいWebアーキテクチャを生成し、契約から逸脱した実装をコンパイラ/静的解析で検出・拒否できるWebフレームワーク**

を目指す。

### Fundamental philosophy

- AIに自由にコードを書かせるのではなく、**正しい設計空間を狭くする**
- ConventionだけでAIを誘導するのではなく、**型付き契約でAIの実装を拘束する**
- コメント・README・命名だけに意味を依存しない
- Endpointのメタ情報そのものを**型システムで保証**する
- 「コードを読むことでしか分からない情報」を、可能な限り型付きメタ情報として明示する
- 型システムを「AIへの情報圧縮装置」として利用する

---

## 2. Core Philosophy

このFrameworkを一文で表現すると:

> **AIがコードベース全体を探索しなくてもEndpointの意味を理解でき、自由に実装できる一方で、意図から逸脱した実装は型システム・Effect System・Capability System・Architecture Contractによって許さない、高性能なAI-first Web Framework。**

別の短い表現:

> **Freedom without chaos, semantics without comments.**

究極的には:

> **「AIにコードを書かせる」のではなく、「AIが正しいコードしか書きにくいWebアプリケーションの世界を作る」。**

---

## 3. Design Principles

> 旧 §15 Design Principles と §50 Updated Core Principles を統合したもの。

### AI as Primary Developer

1. **AI First**
   - AIをPrimary Developerとして設計する。

2. **AI Context Is a First-class Artifact**
   - AIが読むためのSemantic ContextをFrameworkの第一級成果物として扱う。

3. **Token-efficient Context**
   - AIに必要な意味論を少ないTokenで提供する。

### Contract over Convention

4. **Convention over Configuration**
   - Railsから継承する。

5. **Contract over Convention**
   - Conventionだけでなく型付きContractにする。

6. **Semantics over Syntax**
   - HTTP Methodや関数名だけでなく、Endpointの意味を表現する。

7. **Semantic Endpoint**
   - EndpointはHTTP routeだけではなく、Domain / Effects / Mutation / Capabilityを表現する。

### Types Are the Source of Truth

8. **Types Are Authoritative**
   - コメントではなく型・契約を信頼する。

9. **Metadata Is Executable Truth**
   - Semantic Metadataは単なるDocumentationではなく、実装を拘束する契約。

10. **Comments Are Non-authoritative**
    - コメントは補助情報。仕様のSource of Truthではない。

11. **Comments Are Not Contracts**
    - コメントがなくてもAI・人間が迷わないことを目標とする。

12. **Single Source of Truth**
    - AI Context / Documentation / OpenAPI / IDE情報を同じContractから生成する。

13. **Self-describing Codebase**
    - コードベース自身が型付き意味論を持つ。

14. **Contract Must Be Trustworthy**
    - AIに見せるMetadataと実装の乖離を静的に検出する。

### Effects and Capabilities

15. **Explicit Effects**
    - Effectを隠さない。

16. **Fine-grained Effects**
    - `IO`のような粗いEffectではなく、AIが判断できる粒度で表現する。

17. **Capability *and* Permission Checks**
    - Capabilityは**Endpointの能力上界**であり、呼び出し主体の権限とは別概念。
    - **認可（Authorization）は必ず別途必要。** Capabilityは認可の代替ではない。
    - 「このEndpointは何ができるか」（コンパイル時）と「この主体は何をしてよいか」（実行時）を混同しない。詳細は [`specs/capability-system.md`](./specs/capability-system.md)。

18. **Capability-based Safety**
    - 「使ってはいけない」と説明するのではなく、「呼び出すとコンパイルが通らない」状態を型で作る。
    - ただし**型検査が届かない範囲が存在する**。すべて [`specs/unverified-boundaries.md`](./specs/unverified-boundaries.md) に列挙し、AI Contextに出力する。

### Freedom and Performance

19. **Freedom Without Chaos**
    - Middlewareや低レイヤーへの自由を奪わない。

20. **Roads to Low-level**
    - 低レイヤーにも型付きの道を用意する。

21. **Escape Hatch**
    - 必要ならRaw HTTP / Network / Runtimeへ降りられる。80〜90%程度は強いRailに乗せつつ、特殊ケースには低レベルAPIを提供する。ただしEscape HatchはAI/IDEに明示する。

22. **Compile-time First**
    - Semantic Contractは可能な限りCompile Timeで検証する。

23. **Runtime Lean**
    - AI向けの豊富なMetadataをRuntime Overheadにしない。

24. **High Performance**
    - Axum級の性能を目標とし、可能ならActix Web級も研究する。

---

## 4. Prior Art / Existing Frameworks

### Ruby on Rails

RailsのConvention over Configurationは重要な先行事例。

Rails:
- Conventionによって探索空間を狭める
- MVC / REST / Active Recordなどの構造を標準化
- AI Agentにもpredictable architectureが有利

本プロジェクトとの差:
- Rails: **ConventionでAIが推測しやすくする**
- 本プロジェクト: **Convention + 型付きEffect ContractでAIが正しい実装しか作りにくくする**

### Goa

Goa:
- Design-first
- Typed Contract
- DSL
- Code Generation

参考にする思想:
- API契約をコード生成・型に落とす
- 実装より先にサービスの意味を宣言する

### Goaとの正確な差分

**「型が権威 vs 外部ファイルが権威」という対立軸は成立しない。** Verumの `#[contract(...)]` の中身もRustの型式ではなくproc macroが解釈するトークン列であり、型はその生成物である。宣言の権威構造はGoaと同型。

実際に成立している差別化は以下の2点。

1. **契約の対象範囲** — API契約だけでなく **State Mutation / External Effect / Conditional Effect / Capability / Architecture** まで対象にする
2. **エラーの局所性** — 違反が**宣言箇所を指すコンパイルエラー**として返る（Goaは生成時点で検証が終わる）

なおGoaが既にカバーしているError宣言とOpenAPI生成は、Verumでは未決定 / Full PoC送りである。**API契約の軸では現時点でGoaが優位**であることを認識しておく。

### Igniter.js

AI-native TypeScript framework。
- Predictable architecture
- Explicit structure
- AI-friendly codebase

参考にする思想:
- AIが理解しやすい構造
- Convention / Predictability

ただし本プロジェクトではさらに型付きEffect/Mutation Contractへ踏み込む。

### Nifra

AI-edited codebaseを意識したTypeScript framework。
- AI Context
- Scaffold
- Validation
- Architecture drift detection

参考にする思想:
- AIにコードベースを操作するための構造化されたContextを提供
- Architecture validation

### AI Agent Frameworks

Google ADK Go / Microsoft Agent Frameworkなどは、「AIを組み込んだアプリケーション」を作る方向。

本プロジェクトは逆方向:
- **AIそのものを作るFrameworkではない**
- **AIがWebアプリケーションを正しく実装するためのFramework**

---

## 5. Core Differentiation

### EndpointをHTTP関数としてではなく、Semantic Contractとして扱う

通常:

```rust,ignore   // needs a macro that arrives in M2
#[put("/users/{user_id}")]
async fn update_user(...) -> Result<User>
```

これだけではEndpoint内部を読まないと、

- 何を変更するのか
- 何を読むのか
- DBを書き換えるのか
- 外部サービスを呼ぶのか
- Eventを発行するのか
- どの条件で何が変わるのか

が分からない。

本プロジェクトでは、Endpointそのものにこれらを表現する。これらの情報は単なるコメントではなく、**型システムによって保証されること**が重要。

具体的な表現方法は [`specs/semantic-endpoint.md`](./specs/semantic-endpoint.md) を参照。

---

## 6. Positioning

> 旧 §17 Potential Project Positioning と §46 Framework Positioning を統合。

単なる:

> AI-friendly Web Framework

ではなく、

> **AI-native Web Framework**

または、

> **Semantic / Effect-aware Web Framework**

として位置付ける。

### 既存Frameworkとの位置付け

```text
Hyper / Tower
    ↓
HTTP / Middleware foundation

Axum
    ↓
Composable Web Framework

Actix Web
    ↓
High-performance Web Framework

Rocket
    ↓
Declarative / Compile-time checked Web Framework

Loco
    ↓
Rails-like Full-stack Framework

Pavex
    ↓
Compile-time Dependency Injection / Architecture

Verum
    ↓
AI-first Semantic Web Framework
```

### 本Frameworkの独自性

```text
HTTP
+
Domain Semantics
+
Mutation
+
Conditional Effects
+
External Effects
+
Capabilities
+
Architecture
+
AI Context
```

を型付きMetadataとして扱うこと。

### 最終的な思想

> **AIがWebアプリケーションのコードを読む量を減らし、Endpointの意味・可変性・副作用・Capability・Architectureを型付きメタ情報から理解できるようにする。さらに、そのメタ情報と実装が一致することを静的に保証する。**

---

## 7. Comments Are Not the Source of Truth

本Frameworkでは、**コメントを仕様の信頼できる情報源として扱わない**。

最重要原則:

> **Types and Semantic Metadata are authoritative. Comments are supplementary.**

コメントが一切存在しなくても、AI・人間の実装者・IDE・静的解析がEndpointの意味を理解できることを目指す。

### 情報の信頼順位

概念的には以下の順序で信頼する。

1. Type / Contract
2. Semantic Metadata
3. Static Analysis / Inferred Semantics
4. Implementation
5. Generated Documentation
6. Human Comments

コメントを禁止するわけではない。コメントは補足説明として利用できるが、**仕様のAuthorityにはしない**。

### 例

```rust,ignore   // fragment, not a complete item
/// This endpoint only updates the user's name.
fn update_user(...) {
    user.name = new_name;
    user.email = new_email;
}
```

このような場合、コメントを信頼するのではなく、型・Contractによって実際に許可されているMutationを判断する。

---

## 8. Self-describing Codebase

最終的には、Frameworkによってコードベースそのものが型付きSemantic Metadataを持つ状態を目指す。

```text
Codebase
├── Types
├── Contracts
├── Effects
├── Capabilities
├── Architecture
└── State Transitions
```

ここから以下を生成できるようにする。

```text
              Type / Contract
                     │
        ┌────────────┼────────────┐
        ↓            ↓            ↓
       AI       Documentation    IDE
        │
        ↓
   Implementation
```

これらを別々に管理するのではなく、**Semantic ContractをSingle Source of Truthにする**。

生成対象の詳細は [`specs/ai-context.md`](./specs/ai-context.md) を参照。

---

## 9. Token Efficiency Goal

AIがEndpointを理解するために、Handler → Service → Repository → Model → Event → Middlewareと大量のコードを探索する必要をなくす。

従来:

```text
Endpoint
  ↓
Handler
  ↓
Service
  ↓
Repository
  ↓
Model
  ↓
Event
  ↓
Middleware
  ↓
大量のコード探索
```

本Framework:

```text
Endpoint
  ↓
Semantic Contract
  ↓
必要な部分だけ探索
```

### Goal

> **数千行のコードを読む代わりに、数十〜数百token程度のSemantic Contractを最初に読む。**

AIはContractを入口として必要なコードだけを追加探索する。

### この主張は場面を区別する必要がある（未検証）

| タスク | 収支 |
|---|---|
| 多数のEndpointを**概観する** | 黒字。Contractだけ読めば済む |
| 1つのEndpointを**編集する** | **赤字の可能性**。Contractと実装の両方を読み、加えてフレームワークの規約知識が必要 |

実際のAI Codingは後者が大半である。加えて:

- AI Contextは1 endpointで約400〜600token。200 endpointで約100k token（[`specs/ai-context.md`](./specs/ai-context.md) 自身がサイズ管理を未解決としている）
- Verumの概念数は約40（Axum 8〜10、Rails 15〜20）。しかも**学習データに存在しないため毎セッションcontextに載せる必要がある**

**損益分岐点を数値で出すまで、この主張を無条件に掲げるべきではない。** 目的を「token削減」から「**コンパイラをAIのフィードバックループにして契約違反を実行前に止める**」に付け替える選択肢も含めて検討中。[`specs/research-questions.md`](./specs/research-questions.md) Q-B を参照。

具体的なContract形式は [`specs/ai-context.md`](./specs/ai-context.md) を参照。

---

## 10. AI Codingを第一級の設計制約にする

Framework APIの設計時に、人間向けのErgonomicsだけではなく以下を第一級の指標として扱う。

- AI discoverability
- AI context size
- AI ambiguity
- AI error rate
- AI exploration cost
- Specification violation rate
- Unexpected behavior rate

測定指標の詳細は [`specs/evaluation.md`](./specs/evaluation.md) を参照。

---

## 11. Freedom Without Chaos

本Frameworkの「自由度」は、低レイヤーへのアクセスを制限することではない。

### 目指さないもの

```text
Frameworkが決めた場所にしかコードを書けない
```

という過度にOpinionatedな設計。

### 目指すもの

```text
Middleware（Endpointの外側）
      ↓
High-level API
      ↓
Service
      ↓
Repository
      ↓
Runtime
      ↓
Raw HTTP / Network
```

どの階層にも自由に降りられる。ただし、各階層に**型付きの道**を用意する。

> **Freedom without chaos.**

自由度を奪うのではなく、**自由に進める道を舗装する**。

> **注意: この両立は現時点で未証明である。** 「無申告の抜け穴」と「Escape Hatch」を区別する唯一の根拠は「宣言されているか否か」だが、その宣言機構（`#[escape_hatch]`）はまだ設計されていない。記録は現状自己申告であり、属性を書き忘れれば記録されない。
>
> 原則18（Capability-based Safety）と原則21（Escape Hatch）を接続する部品が存在しないため、この節は**目標であって達成された性質ではない**。[`specs/research-questions.md`](./specs/research-questions.md) を参照。

---

## 12. Performance Philosophy

AI-firstであることを理由にRuntime Performanceを犠牲にしない。

Semantic Metadataは可能な限りCompile Timeで消費する。

```text
Semantic Contract
        ↓
Compile Time
        ↓
Validation
        ↓
Optimization
        ↓
Lean Runtime
```

理想的には、RuntimeにはAI向けMetadataによる大きなOverheadを残さない。

性能目標の詳細は [`specs/performance.md`](./specs/performance.md) を参照。

---

## 13. Naming

### プロジェクト名: Verum

**Verum** — 真実 / 真なるもの。

「AIが出したコードを信じる」のではなく、

コードが Verum = 真実であることを保証する。

### 検討したConcept

```text
Intent
Contract
Semantic
Effect
Mutation
Capability
Proof
Invariant
Axiom
Verity
Pact
Rail
```

#### Intent

AIが「何を実現しようとしているか」を表す。

```text
AI
 ↓
Intent
 ↓
Implementation
```

#### Pact

AI / Framework / Codeの間の契約。

```text
Endpoint Pact
Effect Pact
Mutation Pact
Architecture Pact
```

#### Axiom

破ってはいけない不変条件。

```text
Axiom
 ↓
Invariant
 ↓
Proof
```

#### Verity

AI生成コードを単純に信頼するのではなく、型・契約によって正しさを保証する思想。

#### Rail / Path / Way

AIの自由を奪うのではなく、AIが迷わないように「道」を敷く思想。

ただしRailsとの混同などを考慮し、正式命名時に再検討する。

### Naming Strategy

現段階では仮名で開発を開始してもよい。

推奨プロセス:

```text
Prototype
 ↓
Hello World
 ↓
CRUD
 ↓
TODO App
 ↓
Semantic / Effect / Capabilityが固まる
 ↓
正式命名
```

名前を先に設計思想へ固定しすぎず、TODOアプリが完成してFrameworkの本質が見えた段階で正式名称を決定する。
