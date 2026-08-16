# Performance

Runtime性能の目標と、Semantic MetadataをRuntime Overheadにしないための方針。

関連: [`runtime-stack.md`](./runtime-stack.md) / [`evaluation.md`](./evaluation.md)

---

## 原則

AI-firstであることを理由にRuntime Performanceを犠牲にしない。

---

## Performance Goal

- **Axum級の性能を目標とする**
- 可能であればActix Web級も研究する
- ただし初期段階からActix Web超えを絶対条件にはしない

Hyperを直接利用するため、Axum級は事実上の下限になる見込み。

---

## Compile-time Consumption

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

---

## 最適化の余地

### Compile-time route table

derive macro + inventoryにより全Endpointがコンパイル時に既知になるため、radix trieの実行時構築を避け、`match`式やperfect hashingに落とせる。

Axumの構造では原理的に不可能な最適化。ただし初期は素直なmatcherで十分。

### Capability トークンのゼロコスト化

Capabilityは可能な限りZST（Zero-Sized Type）とし、Runtimeに実体を持たせない。型検査のためだけに存在させる。

---

## 監視すべきコスト

| 項目 | リスク |
|---|---|
| 型レベルの集合演算 | trait解決の爆発 → コンパイル時間の悪化 |
| Effectタプルの肥大化 | 同上 |
| derive macroの生成量 | コンパイル時間 |
| Runtime Metadata保持 | メモリ / 実行速度 |

コンパイル時間は開発者体験に直結するため、性能指標として測定する。

---

## 未解決の問い

- パフォーマンス・コンパイル時間への影響をどこまで許容するか
- Developer ExperienceとAI Experienceの両立

[`research-questions.md`](./research-questions.md) を参照。
