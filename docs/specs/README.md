# Verum — Specs

技術仕様。思想・ビジョンは [`../concepts.md`](../concepts.md)、開発計画は [`../roadmap/roadmap.md`](../roadmap/roadmap.md) を参照。

---

## 実装前に必ず読むもの

| ファイル | 内容 |
|---|---|
| [`unverified-boundaries.md`](./unverified-boundaries.md) | **型検査が届かない全経路の台帳。** 埋め残しをゼロにするためのファイル |
| [`rust-type-model.md`](./rust-type-model.md) | 実コンパイルで確認した制約。cons list / indexパラメータ / 拡張trait / MSRV |
| [`diagnostics.md`](./diagnostics.md) | エラーメッセージは仕様である。3層防御と到達できないこと |
| [`research-questions.md`](./research-questions.md) | 何が未決定か。最優先3件を含む |

---

## Core Model

| ファイル | 内容 |
|---|---|
| [`semantic-endpoint.md`](./semantic-endpoint.md) | Contract宣言構文（属性→型展開）とEndpointの構成要素 |
| [`handler-rules.md`](./handler-rules.md) | 実装の自明性を担保する4ルール。Capability設計の前提条件 |
| [`effect-system.md`](./effect-system.md) | Effect分類、カテゴリ分割、宣言粒度、GETのRead-only保証 |
| [`mutation-contract.md`](./mutation-contract.md) | Field-levelの可変性。**Domainの不透明化** |
| [`read-contract.md`](./read-contract.md) | `reads` をProjection型で強制する |
| [`conditional-effects.md`](./conditional-effects.md) | `when` スコープによるCapability発行と、原理的な限界 |
| [`capability-system.md`](./capability-system.md) | 中核機構。`Ctx<'req, E>` / sealed trait / 認可との区別 |
| [`architecture-contract.md`](./architecture-contract.md) | Handler → Service → Repository の経路を型で制約する |

## Verification

| ファイル | 内容 |
|---|---|
| [`effect-inference.md`](./effect-inference.md) | 上界検査の限界と、「実装から生成する」代替案 |
| [`diagnostics.md`](./diagnostics.md) | エラーメッセージ設計 |
| [`rust-type-model.md`](./rust-type-model.md) | Rustのどの型機能を使うか |

## Runtime

| ファイル | 内容 |
|---|---|
| [`runtime-stack.md`](./runtime-stack.md) | 依存する層と自作する層。Dependency Hiding Rule。MSRV |
| [`middleware.md`](./middleware.md) | Middlewareの型付け。tower / tower-httpとの境界 |
| [`persistence.md`](./persistence.md) | Repository traitのスコープと信頼境界。Domain不透明化との相互運用 |
| [`performance.md`](./performance.md) | 性能目標とCompile-time消費の方針 |

## Output & Evaluation

| ファイル | 内容 |
|---|---|
| [`ai-context.md`](./ai-context.md) | Semantic Code Graph。強制レベルと未検査境界の明示 |
| [`evaluation.md`](./evaluation.md) | AI Coding Benchmarkの測定指標 |

## Open Problems

| ファイル | 内容 |
|---|---|
| [`unverified-boundaries.md`](./unverified-boundaries.md) | 未検査経路の台帳 |
| [`research-questions.md`](./research-questions.md) | 未解決の設計課題 |

---

## 読む順序

初めて読む場合:

```text
../concepts.md            — 何を作ろうとしているか
        ↓
semantic-endpoint.md      — Contract をどう宣言するか
        ↓
handler-rules.md          — 実装がどう書かれるか
        ↓
capability-system.md      — それをどう型で強制するか
        ↓
unverified-boundaries.md  — どこが強制されないか
        ↓
../roadmap/roadmap.md     — 何から作るか
```

**`unverified-boundaries.md` を飛ばさないこと。** Contractの網羅性が高いほど「これで全部だ」という誤った安心を与えるため、保証されない範囲を知らずに使うことが最大のリスクになる。
