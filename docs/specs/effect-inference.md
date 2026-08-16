# Effect Inference & Static Verification

宣言されたContractと実装の乖離を検出する仕組み。および「実装から生成する」代替案。

関連: [`capability-system.md`](./capability-system.md) / [`unverified-boundaries.md`](./unverified-boundaries.md) / [`persistence.md`](./persistence.md)

---

## なぜ必要か

Contractと実装が乖離した瞬間、Contractはコメントと同じ信頼度に落ちる。これはVerumが否定した状態そのものである。

```text
Declared Effects  VS  実装が実際に起こす Effect
```

---

## 採用している方式: Capabilityを型で要求し、rustcに照合させる

```text
Repository の setter が where M: Has<Mutate<User, user::Email>, I> を要求
        ↓
Endpoint が宣言していない Capability は E::Mutates に含まれない
        ↓
コンパイルエラー
```

crate横断の呼び出しグラフ解析（rustc_private / MIR）を書かずに済む。

### この方式の2つの限界

**1. Contract は「上界」でしかない**

型検査は「実装 ⊆ 契約」の一方向のみ。宣言したのに使わないEffect（過剰宣言）は検出されない。

```rust,ignore   // needs a macro that arrives in M2
#[contract(mutates = [User::name, User::email])]
// 実装は name しか変えていない → エラーにならない
```

したがって `mutates = [name, email]` を読んだAIが「このEndpointは name と email を変更する」と解釈すると**誤る**。正しい解釈は「name と email 以外は変更しない」である。

AI Contextの `enforcement` はこれを区別できる値にする必要がある。

```json
"mutates": { "enforcement": "upper_bound_checked" }
```

`type_checked` という表記は「双方向に検証済み」と読まれるため使わない。

**2. 照合の正しさが手書きboilerplateに依存している**

`set_email` のwhere節に誤って `Has<Mutate<User, user::Name>, I>` と書いても、それは検出されない。「rustcが照合を代行する」という主張は、**Repository traitを書いた人が間違えていないこと**というより弱い前提に乗っている。

→ Repository **trait定義**のderive生成を優先する（[`persistence.md`](./persistence.md)）。

---

## 決定（Q-A）: 型強制と生成の**両方**を持ち、差分を検出器にする

**2026-08-15 決定。** 却下した案は末尾。

### 決定の骨子

目標は2つあり、**別々の機構を要求する**。

| 目標 | 意味 | 機構 |
|---|---|---|
| **抜け穴を作らせない** | 宣言していない Effect は起きない = **上界** | 型強制（`Has` + 拡張 trait の where 節）。既に成立 |
| **嘘を作らせない** | 宣言した Effect は実際に起きる = **下界** | **生成**（`handle` のトークン走査）。型では原理的に出せない |

型検査は「実装 ⊆ 契約」の一方向しか見ないので**過剰宣言を検出できない**（上記§この方式の2つの限界）。したがって `mutates = [name]` が保証するのは「name 以外を変えない」だけで、「name を変える」ではない。**片方だけでは目標の片方しか満たさない。**

```text
declared_ceiling   手書き #[contract] + 型強制  → 「これ以外は起きない」
observed_effects   handle のトークン走査で生成  → 「これが起きる」

observed ⊄ declared     → コンパイルエラー（型半分。既に成立）
declared \ observed ≠ ∅ → 過剰宣言。CI が落とす（下記）
```

**2つの差分が、[`research-questions.md`](./research-questions.md) §過剰宣言の検出 が挙げていた未解決問題そのものを解く。** 副産物ではなく、差分を取ることがこの決定の主目的である。

### 生成の範囲 — First PoC は `handle` のみ

**proc macro は自分が付いたアイテムのトークンしか見えない。** `handle` に付けた属性マクロは本体を全て見られるが、そこから呼ばれる Service の中身は見えない。crate 横断の呼び出しグラフ解析は書かない（本ファイル §採用している方式 が、それを書かずに済むことを利点として挙げている）。

したがって First PoC の生成範囲は `handle` の中だけであり、**その範囲を AI Context に明示する**。

```json
"observed": { "fields": ["User::name"], "scope": "handle_only", "deferred": [] }
```

`scope` を出さないと AI は下界が全経路に及ぶと誤読する。見えない範囲（Service 本体 / 自由関数コンストラクタ / Repository 実装内部の生 SQL）は [`unverified-boundaries.md`](./unverified-boundaries.md) に記録し AI Context に出す。

**将来の拡張形（未採用・記録のみ）**: Effect を持つアイテムを全て注釈し、各アイテムがフラグメントを出してビルド時に推移閉包を取れば、crate 横断解析なしで Service まで届く。First PoC では Service が範囲外なので採らない。

### 過剰宣言が出たとき — CI が落とす

**差分は1つの proc macro では計算できない。** 宣言は unit struct の `#[contract(...)]`、実装は `impl Handler for X` の `handle` で、**別のアイテム**だからである。照合はビルド時に、両者が出した成果物を読んで行う。

> **したがってこれは3層防御のいずれでもない。** macro / equality bound / trait bound のどれでもなく、**ビルド時の第4の機構**である。[`diagnostics.md`](./diagnostics.md) の層の表をこれで読み替えないこと。**コンパイルエラーにはならない** — 型半分（宣言外の Effect）とは強度が違う。

```text
error: `User::email` is declared in `mutates` but never mutated in `handle`
  help: remove it from the contract, or mark it `@service` if a Service performs it
```

**過剰宣言はセキュリティホールではなく「誤った読み手」を生む問題**なので、CI ゲートで重さとして釣り合う。宣言外の Effect（抜け穴）はコンパイルエラーのまま。

**逃げ道は明示的にし、使用自体を記録する。** Service が実行する Effect を宣言したい場合は `@service` を付け、`deferred` に出す。`deferred` が空でないことは `unverified_boundaries` にも現れるので、**逃げたこと自体が記録に残る** — `forbidden` が意図の記録装置として機能するのと同じ理屈である。

### 前提が未検証である — Phase 1 で spike する

> **⚠️ この決定は「トークン走査だけで Contract 全体が復元できる」という前提に乗っており、その前提は一度もコンパイルされていない。**

下記§却下理由が無効化された経緯 の主張である。このプロジェクトはマクロの能力について両方向に間違えた実績があり（RK-003 / RK-004、T-M1-01 の `E0428`）、`CLAUDE.md` は「コンパイラの振る舞いに関する主張は必ずコンパイルして確かめる」と定めている。

**T-M1-07 として Phase 1 に spike を追加した。** [`handler-rules.md`](./handler-rules.md) の実例に対して測る:

1. `ctx.users().set_name(..)` → `Mutate<User, user::Name>` に復元できるか
2. `ctx.when::<C>(.., async |..| { .. })` → クロージャ内を `When<C, ..>` に入れられるか
3. `ctx.after_commit(|ctx| ..)` → スコープを区別できるか
4. `AuditLog::user_updated(&user)` → **見えないことを確認**する（規約依存であることの実証）
5. `User::from_repr(..)` → **escape として検出できるか**

5 は台帳 path 21 に直接効く。**生成は path 21 を塞がない**（`from_repr` は `ctx.` を通らないので Effect としては現れない）が、**トークン走査は `handle` 内の `from_repr` を見つけられる**ので、塞げなくても**可視化はできる**。#33 が閉じ方を見つけられなかった場合の最低限の担保になる。

### 却下した案と理由

| 案 | 却下理由 |
|---|---|
| **型強制のみ（現状維持）** | 「嘘を作らせない」という目標の後半を達成しないことを明示的に認めることになる。`type_checked` を禁止語にして誤読を防いでいるのは、**嘘を防ぐ代わりに読めることを諦めている**状態であり、目標と整合しない |
| **生成のみ** | 閉ループ（AI が違反したまま前進できず、人間のレビューゲートを介さずに自己修正する）を失う。`forbidden`（事前の意図表明）は生成では表現できない。生成は**記述であって防止ではない** |
| **生成を主にし型は最小核だけ** | 概念数（約40）が減り Q-B の token 収支は改善するが、`mutates` の閉ループを失う。**T-M1-07 と Q-B の実測後**に再検討する余地として残す — 今この方向に倒す根拠がない |
| **crate 横断の呼び出しグラフ解析** | MIR / `rustc_private` 依存。本ファイルがそれを書かずに済むことを利点として挙げており、nightly 依存は MSRV 方針と衝突する |
| **差分を報告のみにする** | Contract 緩和バイアス（下記§型では解決しない問題）に対して何もしない |

---

## Inferenceが真に必要になる範囲（型強制方式を維持する場合）

1. **Escape Hatchを通った箇所**
2. **生SQL**（Repository実装内部）
3. **自由関数コンストラクタの副作用**

いずれも `handle` のトークン走査では見えない。ただし範囲が限定されるため後付けできる。

---

## 信頼境界としての Repository 実装

```text
Endpoint / Service 層  → 型で保証される（rustcが照合）
Repository 実装        → 信頼境界（レビュー・監査の対象）
DB                     → 対象外
```

> **⚠️ 上の図の1行目は現時点では成立していない**（T-M1-01 / #13 で実測）。台帳 **path 21** が開いている間、Endpoint / Service 層の普通のコードが Capability も Repository も SQL も `unsafe` も無しに `User::from_repr(UserRepr { .. })` で Domain を捏造できる。図は path 21 を閉じたあとの姿である。詳細は [`persistence.md`](./persistence.md) §判定。

境界を狭める手段は [`persistence.md`](./persistence.md) を参照。すべての未検査境界は [`unverified-boundaries.md`](./unverified-boundaries.md) に列挙し、AI Contextに出力する。

---

## 型では解決しない問題: Contract緩和バイアス

AIはコンパイルエラーに対して**実装を直すよりContractを1行広げる方を選ぶ**。

```text
error: undeclared mutation `User::status`
  help: add `User::status` to the contract, or remove this call
        ↑ AIはこちらを選びやすい
```

Contractが「実装を拘束する契約」ではなく「実装に追従して広がるラベル」になった時点で、型検査は無意味になる。しかも**「型で保証されている」という安心感がレビュアの注意をそらす**ため、Axumで同じバグを書いた場合より発見しにくくなる逆転が起こりうる。

これは型システムの問題ではなく運用の問題である。対処は [`unverified-boundaries.md`](./unverified-boundaries.md) を参照（CIでContract拡大差分を検出する等）。

---

## 保証手段の優先順位

```text
Type System → AST → Static Analyzer → Code Generator → Compiler
```

目標:

> AIが間違ったコードを書いても、実行前に契約違反として検出できる。

ただし**「すべての違反を検出できる」とは主張しない**。検出範囲を [`unverified-boundaries.md`](./unverified-boundaries.md) で明示する。

---

## 優先度

First PoCでは Inference を実装しない。Capability方式でどこまでカバーできるかを実測し、残った隙間に対してのみ設計する。

[`../roadmap/roadmap.md`](../roadmap/roadmap.md) を参照。
