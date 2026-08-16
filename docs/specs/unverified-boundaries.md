# Unverified Boundaries

型検査が届かない経路の台帳。**埋め残しをゼロにするためのファイル。**

関連: [`capability-system.md`](./capability-system.md) / [`ai-context.md`](./ai-context.md) / [`persistence.md`](./persistence.md)

---

## なぜこのファイルが必要か

Verumの核心的リスクは「型が弱いこと」ではない。

> **型で塞いだ経路の隣に、より楽な未検査経路が並んでいること。**

型の壁が高いほど、AIは壁を越えずに**回り込む**。コンパイルエラーに詰まったAIは「Contractを緩める」「Service層でやる」「イベントで別の場所に投げる」「生SQLで書く」という第3の選択肢を常に持っている。

したがって目標は「すべて型で塞ぐ」ではない。**すべての経路について、塞ぐか明示するかを決めること**である。

```text
知らない埋め残し   → 危険。AIと人間の両方が「保証されている」と誤解する
明示された境界     → 管理できる。レビュー対象として特定できる
```

このファイルは全経路を列挙し、それぞれの状態を追跡する。

---

## 経路が生まれる3つの構造的原因

個別に塞ぐとモグラ叩きになる。原因は3つに集約される。

| 原因 | 該当経路 | 構造的な対処 |
|---|---|---|
| **1. Domain Modelが普通のRust structとして公開されている** | 直接代入 / `into_owned` / Debug漏れ / 内部可変性 / **`Repr` 経由の構築と読出し（path 21）** | Domainを不透明化し、Capability付きアクセサ経由のみにする。**ただし不透明化だけでは足りない** — 永続化のために生成される `Repr` が横に開く（path 21）。閉じ方は #18 |
| **2. Capabilityを運べる型に寿命・経路の制約がない** | spawn / テストのgod-mode / `when`漏出 / `dyn Repository` / Endpointに`PgPool` | `Ctx<'req, E>`でリクエスト寿命に縛り、構築経路をsealedにする |
| **3. Contractを要求していない場所でEffectが起きる** | `emits`の先 / Middleware / Repository実装 / 自由関数コンストラクタ / `Condition::holds` | Contractを要求する場所を増やす（段階的） |

---

## 全経路台帳

### 原因1: Domain Modelの公開形式

| # | 経路 | 対処 | 状態 |
|---|---|---|---|
| 1 | `user.email = v` 直接代入 | Domain不透明化（privateフィールド） | **First PoCで塞ぐ** |
| 2 | `*user = other_user`（`find`で2件取って入れ替え） | 構築経路の制限だけでは塞げない | **明示**（下記） |
| 3 | `into_owned()` でProjectionを解除 | **提供しない** | **First PoCで塞ぐ**。⚠️ ただし `Repr` に `Clone` を derive すると復活する（path 21 参照） |
| 4 | `Debug` / `Serialize` 経由のデータ漏れ | 宣言Fieldのみを出す独自実装をderive生成 | **First PoCで塞ぐ**。⚠️ 対処は**Domain 側にしか課されていない** — `Repr` に `Debug` を derive すると別クレートからも漏れる（path 21 参照） |
| 5 | 内部可変性（`RefCell` / `Mutex` / `Cell`）経由のMutation | Domainフィールド型を制限（`Freeze`が安定するまでホワイトリスト） | **First PoCで塞ぐ** |
| 21 | **`User::from_repr(UserRepr { .. })` / `as_repr()` がDomainのクレート内のどこからでも到達可能** | 未定（#17 / #18 で決定） | ⚠️ **開いている**（T-M1-01 / #13 で実測。下記） |

> **番号は追記のみで、振り直さない。** path 12 / 13 / 14 は [`../rules/api-surface.md`](../rules/api-surface.md)・[`../rules/proc-macro.md`](../rules/proc-macro.md)・知見バンクから参照されており、振り直すと全ての相互参照が無言で壊れる。原因ごとのグルーピングより番号の安定を優先する。

#### path 21 — `Repr` は「Repository 実装だけ」には閉じられない（コンパイル検証済み）

`#[derive(Domain)]` は**利用者のクレートで展開される**ので、生成される `pub(crate) struct UserRepr` と `pub(crate) fn from_repr` / `as_repr` の可視性は**そのアプリクレート全体**である。derive は Repository がどのモジュールに書かれるかを知らないため `pub(in ...)` を出せない。結果、読み方が2つあって**どちらでも成立しない**。

| Repository の置き場所 | 実測 |
|---|---|
| Domain と同一クレート（単一クレートアプリ） | クレート内のあらゆるハンドラが `User::from_repr(UserRepr { email: 任意, .. })` を書ける。Capability も Repository も SQL も `unsafe` も不要 |
| 別クレート | `Repr` が全く見えない（`E0603`）。設計が機能しない |

**path 2 との比較は軸で分かれる。「厳密に上位互換」ではない。**

| 軸 | どちらが悪いか |
|---|---|
| 値の自由度 | **21**。path 2（`*user = other_user`）は `find` が実際に返した値しか入れられないが、21 は値を**発明できる**。前提も軽い（Capability も `find` の結果も不要） |
| 到達範囲と恒久性 | **2**。path 2 は `&mut D` があればクレート境界を越えて成立し、**21 を閉じても残る**（本ファイルの「原理的に埋まらないもの」に分類済み）。21 は crate-local で、閉じれば消える |

偽造した `User` の getter が実行時にも通ることは確認済み（spike の P8）。ただし**ロードした `User` と直接比較したわけではない**ので、「区別できない」は getter の挙動についての観察である。

**フィールドの private 化自体は効いている。ただし保証範囲は「定義モジュールの外から」であって、型の境界ではない**（実測）。定義モジュールとその子モジュールからは `u.0.email = v` が通り、**マクロは利用者の `struct User` と同じモジュールに展開される**ので、利用者がその横に書く `impl` やヘルパは緩い側に立つ。`E0616` に詰まった AI の最短の回避策は「そのコードを Domain 定義ファイルに移す」ことである（ARK-002 の教科書例）。

エラーコードは形で変わる（実測。当初この行は両方を `E0616` と書いていたが、`u.email = v` にはプローブが存在しなかった）。

| 形 | 実際に出るコード |
|---|---|
| newtype + `email()` getter あり（**実際の設計の形**） | **`E0615`**（メソッドの値を取ろうとした） |
| newtype + getter なし | `E0609`（そんなフィールドは無い） |
| フラットな private 名前付きフィールド、**モジュール外**から | `E0616`（フィールドが private） |
| newtype の `u.0.email`、モジュール外から | `E0616` |
| 内側フィールドを `pub(crate)` にした場合 | **通る** — だから derive は private を出さなければならない |

**`E0615` / `E0609` は `#[diagnostic::…]` で文言を差し替えられない**（3層防御の外）。したがって path 21 をどう閉じても、この形の誤りに対して Contract への誘導は出せない。閉じ方の設計は診断の設計と同時に決める必要がある。

**`Repr` は path 21 だけでなく path 3 / path 4 も開ける。** `Repr` に `Debug` を derive すると宣言 Field 外を含む全フィールドが `format!("{:?}")` で出て（path 4 の対処は Domain 側にしか課されていない）、`Clone` を derive すると完全所有コピーが取れる（path 3 の `into_owned` 相当）。**仕様形では同一クレート内に限る** — 外部クレートは `as_repr` に到達できず `E0624`（実測。当初ここに「別クレートからも漏れる」と書いたが誤りだった）。

生成コードの制約に「`Repr` に `Debug` / `Clone` / `Serialize` / **`Deserialize`** を derive しない」が加わる。**一般形で覚えること — 定義側モジュール内で構造体を組む derive 由来のコンストラクタは、何であれ偽造経路になる。** `FromRow` も `Deserialize` も同じ機構であり、禁止リストの列挙方式では derive を1つ足すだけで穴が開く。

再現とプローブ表: `spikes/domain-opacity-sqlx/`（`bash run.sh`）。仕様側の記述は [`persistence.md`](./persistence.md) §判定。**閉じ方を spike で決めなかったのは意図的** — この経路は Domain の公開形式そのものなので、選択が M2 の derive タスク群の形を決めてしまう（ARK-002: 代替を用意せずに塞ぐと検査されない経路へ人を押し出す）。

### 原因2: Capabilityの寿命と経路

| # | 経路 | 対処 | 状態 |
|---|---|---|---|
| 6 | `tokio::spawn` で `Ctx` を持ち出す | `Ctx<'req, E>`（`'static`でなくする。`Send`は保つ） | **First PoCで塞ぐ** |
| 7 | `static Sender<Ctx<E>>` へ譲渡 | 同上 | **First PoCで塞ぐ** |
| 8 | `when` スコープから `Ok(ctx)` で漏出 | クロージャ戻り型を `Result<()>` に固定 | **First PoCで塞ぐ** |
| 9 | `Ctx::for_test()` がgod-modeコンストラクタになる | sealedな`Runtime`トークンを要求。テストはEndpoint型を固定したAPI経由 | **First PoCで塞ぐ** |
| 10 | Endpoint構造体に `PgPool` を持つ | `#[endpoint]` がunit struct以外を拒否 | **First PoCで塞ぐ** |
| 11 | Serviceに `dyn Repository` を渡す（型パラメータが消える） | `dyn Repository` を公開しない。Serviceも Capability でパラメタライズ | **First PoCで塞ぐ** |
| 12 | 手書き `impl Endpoint` で任意のCapabilityを宣言 | `Endpoint` を sealed trait 化 | **First PoCで塞ぐ** |
| 13 | `impl Includes<Order> for User`（ローカル型なのでorphan ruleを通る） | `Includes` を sealed trait 化 | ⚠️ **暫定閉鎖**（T-M0-06 / #6。下記の再検証条件つき） |
| 14 | `impl Field<...>` の偽装（`Field::NAME` を偽ると生成 SQL の列名を偽れる） | sealed trait 化 | **First PoCで塞ぐ**（`Field` 未実装） |
> ### ⚠️ 14a〜14e は **M2 で再オープンしうる — ただし seal を分割したので、しない**（#9 のレビューで検出）
>
> 下の path 13 の注記が「M2 は `#[doc(hidden)] pub mod __private` を導入せざるを得ない」「そこが seal の強度が下がる瞬間」と書いている。**その警告は 13 にしか付いていなかった。** 全 seal が1モジュールにあれば、その1回の変更で**すべての seal が名指し可能**になり、14a〜14e が同時に再オープンする — 実際にその変更を加えて downstream から偽の所属がコンパイルすることを確認した。
>
> 対処として seal を**2モジュールに分割**した（#9）。`private` は**構造的** seal（`SealedConsList` / `SealedIndex` / `SealedHas` / `SealedAppend` / `SealedLookup`）を持ち、verum が自分でタプルに実装するだけで derive は関与しないので **`pub(crate)` を恒久的に維持**する。`derive_facing` は derive が満たす必要のある seal（現状 `SealedIncludes`）だけを持ち、M2 で公開されるのはこちらだけになる。
>
> したがって **14a〜14e の ✅ は M2 を越えて有効**であり、13 の ⚠️ 暫定閉鎖は `derive_facing` の側の話として残る。`compile_fail/sealed_derive_facing_module_is_private.rs` が現状を固定しており、M2 が開けた瞬間に `.stderr` の diff として現れる。

| 14a | `impl Has<Elem, Idx> for <集合>` — 所属判定そのものの偽装。**先頭位置（`Here`）と先頭以外（`There<_>`）は別経路** | `Has` を sealed trait 化し、**seal の再帰 impl も条件付きにする** | ✅ **閉鎖済み**（T-M0-08 / #8。`has_cannot_be_forged.rs` + `has_cannot_be_forged_at_depth.rs`）— 下記の注記を読むこと |
| 14b | `impl ConsList for MyType` — 形の証明を偽装し、壊れた集合を well-formed に見せる | `ConsList` を sealed trait 化 | ✅ **閉鎖済み**（T-M0-07 / #7。`cons_list_cannot_be_forged.rs`）。タプル形は orphan rule（E0117）でも塞がれている（T-M0-08 で追試） |
| 14c | `impl Index for MyIdx` — 所属位置を偽装する | `Index` を sealed trait 化 | ✅ **閉鎖済み**（T-M0-07 / #7。`index_cannot_be_forged.rs`）。`There<MyIdx>` / `There<There<MyIdx>>` も orphan rule で塞がれている（T-M0-08 で追試） |
| 14d | `impl Append<B> for <集合>` — 連結結果の偽装。`type Out` を持つので**合成後の Capability 集合そのものを名指しできる** | `Append` の seal を trait と**一致**させる（base impl の `B: ConsList` を含む） | ✅ **閉鎖済み**（T-M0-09 / #9。`append_cannot_be_forged_at_base.rs` + `_at_depth.rs`）。**いちど空振りで閉鎖していた** — 下記注記 |
| 14f | `impl Has<H, Idx> for (H, <非 cons list>)` — **壊れた集合を capability 検査に通す**。**先頭位置も深い位置も同様**（`impl Has<Other, There<There<Here>>> for (Decl, (Elem, (Other, Junk)))` は通る）。所属自体は真なので capability の増加は無い | なし（`Has` の seal は診断のため意図的に `ConsList` を落としている — `SEAL-DIFF`） | ⚠️ **明示**（T-M2-09 まで）。`ConsList` の「壊れた集合は fail closed」は downstream から無効化できる。ただし**要素が素のローカル型のときだけ** — 実効果型 `Mutate<User, Email>` は verum のジェネリクスがローカル型を包むので orphan rule（E0117）で塞がれる（実測）。つまり**effect 集合では到達不能で、domain 形の要素に限る**。`has_forged_membership_on_malformed_set.rs`（先頭）と `has_forged_membership_at_depth_on_malformed_set.rs`（深さ）が*偽の*所属が拒否される側を両位置で固定している。**「恒久的」ではない** — T-M2-09 が宣言箇所で形をアサートするか、条件付き `on_unimplemented` が stable 化すれば `SEAL-DIFF` の正当化は失効し、bound を戻せる |
| 14e | `impl Lookup<K, Idx> for <map>` — 「その鍵に対応するエントリはこれだ」の偽装。条件付きスコープを任意に差し替えられる | `Lookup` の seal を trait と**一致**させる（head impl の `T: ConsList` を含む） | ✅ **閉鎖済み**（T-M0-09 / #9。`lookup_cannot_be_forged_at_head.rs` + `_at_depth.rs`）。**いちど空振りで閉鎖していた** — 下記注記 |

> ### ⚠️ 14d / 14e も空振りで閉鎖されていた（#9 のレビューで検出、14a に続いて2度目）
>
> #9 は 14a の教訓に従って**最深位置**のフィクスチャを両方に付けた。開いていたのは**最浅位置**だった — `Append` の `for ()`（base）と `Lookup` の head である。`Append` の base は**全ての連結が bottom out する床**なので、`impl Append<Local> for ()` の1行が**プログラム中の全ての連結結果を書き換えた**。
>
> 原因は seal が形の bound を落としていたこと。「verum の impl に `B: ConsList` が付いているから守られている」と読んだのが誤りで、**verum の impl に付けた bound は外部 impl には課されない**。
>
> 台帳の運用として: **閉鎖の根拠は「全 impl 位置を覆っている」であり、最深だけでも最浅だけでも足りない。** [api-surface.md](../rules/api-surface.md) §2 の表で空欄（`—`）を許さないこと — #9 ではその空欄が穴の所在をそのまま指していた。

> ### ⚠️ 14a はいちど**空振りで閉鎖されていた**（T-M0-08 のレビューで検出）
>
> #8 は当初 `has_cannot_be_forged.rs`（`Here` + 非タプル `Self`）だけで 14a を閉じたと記録した。**どちらも seal の head impl が既に閉じていた経路**で、実際に開いていた `There<_>` 経路は覆われていなかった — 行の記述自体が `impl Has<Elem, Here> for MyList` と**浅い側だけを書いていた**ため、フィクスチャは行に一致し行はフィクスチャに一致して、どちらも本当の穴を外した。
>
> 教訓は台帳の運用そのものに関わる: **閉鎖の根拠は「フィクスチャが1つある」ではなく「その trait の impl 位置すべてを覆っている」** である。再帰 impl を持つ trait は最浅と最深の両方を固定すること（[api-surface.md](../rules/api-surface.md) §2 に規則化）。
>
> **型引数を持つ sealed trait ほど露出が大きい。** 14b/14c が実際には無傷だったのは、`ConsList` / `Index` が型引数を持たず、ローカル型が入れる位置が `Self` しかないためである（タプルや `There<_>` は `Self` ではローカル型にならないので orphan rule が先に弾く）。`Has<T, Idx>` は `T` にローカル要素型を置けるので orphan rule を通ってしまい、seal が唯一の防御になる。**新しい sealed trait を審査するときは、まず型引数の数を見ること。**

> **path 14 は3分割された。** 当初 `Has` と `Field` を1行にまとめていたが、閉鎖の根拠は**その trait が seal を supertrait に持つこと**であり、trait ごとに時期が違う。まとめたままにすると `Has` が閉じた時点で `Field` も閉じたと誤読される。#7 で `ConsList` / `Index` を分けたのと同じ理由。

> 注: #14 について、`impl Has<Mutate<User, Password>> for ()` は `Has` も `()` も外部型であり、`Mutate<User, ..>` は型引数にローカル型を含むだけでlocal typeではないため、**orphan ruleで防がれる可能性が高い**（当初未検証）。一方 #13 は `User` がローカル型なので確実に通る。sealed化はどちらにも有効なので、区別せず適用する。
>
> **T-M0-06 で実測した（上の推測は「この形については」正しく、一般則としては誤り）**:
> ```text
> impl verum::Includes<Order>      for ()  ->  E0277（orphan は通る。seal だけが止めている）
> impl verum::Includes<Vec<Order>> for ()  ->  E0117（外部ジェネリクスに包むと local 扱いされない）
> ```
> **ローカル型が trait の型引数に直接現れれば orphan rule は通る。** `Mutate<User, ..>` のように外部ジェネリクスの内側にあると通らない。つまり #14 の推測は `Has<Mutate<..>>` の形に限れば正しいが、「型引数にローカル型を含むだけなら防がれる」と一般化すると誤りである。**どちらにせよ orphan rule に依存してはならず、seal が唯一の防御**という結論は変わらない。

> **#13 の閉鎖について（T-M0-06 / #6）**: seal の基盤と `Includes<D>: SealedIncludes<D>` の sealed 化が入り、UI テストが `impl verum::Includes<Order> for User {}` の失敗を `.stderr` ごと固定した。
>
> **閉鎖の根拠は「`Sealed` が存在すること」ではなく「その trait が `Sealed` を supertrait に持つこと」**なので、#12（`Endpoint`）と #14（`Field`）は当該 trait が実装される M2 まで開いたままとする（`Has` は 14a として分離され T-M0-08 で閉鎖）。基盤ができた時点でまとめて閉じたことにしない。
>
> **⚠️ 暫定閉鎖である理由（T-M0-07 のレビューで判明）**: 現在 path 13 が閉じているのは、**`SealedIncludes<D>` を誰も満たせない**からである — `verum-macros` は macro を1つも出していない。かつ [`../rules/api-surface.md`](../rules/api-surface.md) §2 が記録するとおり、**proc-macro の出力は呼び出し側クレートで解決されるため `pub(crate) mod private` に到達できない**（E0603 を実測）。M2 は `#[doc(hidden)] pub mod __private` を導入せざるを得ず、§2 自身が「そこが seal の強度が下がる瞬間」と書いている。
>
> **したがって今日の緑は M2 の緑の証拠ではない。** 再検証条件: `__private` 導入後に、derive が1ドメイン分の seal を出した状態で `impl Includes<未宣言>` が **E0277 になること**と、宣言済みが**通ること**を双方向で確認する（T-M0-06 で実施した手順と同一）。
>
> **derive が入っても閉じたままであること**が、この閉鎖の要点である。当初 seal を `Sealed`（`Self` のみ）で書いたところ、Tier-2 レビューが「derive が `Sealed` を1つ生成した瞬間、`impl Includes<未宣言>` が通る」ことを実測で示した。seal を `SealedIncludes<D>` に変えて**関係そのものを封じ**、偽装が E0277 になることと宣言済みが通ることを両方向で確認済み。詳細は [`../rules/api-surface.md`](../rules/api-surface.md) §2「seal は対象 trait の型引数を持たなければならない」。

### 原因3: Contractの外で起きるEffect

| # | 経路 | 対処 | 状態 |
|---|---|---|---|
| 15 | `emits` の購読側が任意のEffectを起こす | 購読側にContract必須化 + 推移閉包をAI Contextに出力 | **後回し（明示）** |
| 16 | MiddlewareのEffectがContractに現れない | MiddlewareにContract必須化 + Routerが合成 | **後回し（明示）** |
| 17 | Repository実装内部の生SQL | derive生成で境界を移す / SQL Lint | **後回し（明示）** |
| 18 | 自由関数コンストラクタ内の副作用（`AuditLog::user_updated()` 等） | コンストラクタをderive生成して手書きを消す | **後回し（明示）** |
| 22 | **`observed_effects` の走査が Service 本体に届かない**（Q-A の決定で `handle` のみを走査すると決めたことの帰結） | Effect を持つアイテムを全て注釈しビルド時に推移閉包を取る（将来形）。当面は `scope: "handle_only"` と `deferred` で明示 | **明示**（Q-A / 2026-08-15） |
| 19 | `creates` + `deletes` でField粒度を迂回（upsert） | deriveが同一Domainの併記を拒否 / `create`は新規IDのみ | **後回し** |
| 20 | `Condition::holds` が `true` を返すだけで全解錠 | **原理的に不可能** | **恒久的に明示** |

---

## 原理的に埋まらないもの

### `Condition::holds` の中身

```rust
impl Condition<User, UpdateUserRequest> for EmailChanged {
    const NAME: &'static str = "EmailChanged";
    fn holds(user: &User, req: &UpdateUserRequest) -> bool {
        true    // ← これで条件付きEffectが全件無条件化する
    }
}
```

利用者が書いたbool値を型で検証することはできない。しかも**AI Contextは依然として `"conditional": [...]` と出力するため、メタデータが能動的に嘘をつく**。

対処:

- AI Contextに `condition_verified: false` を必ず出力する
- `Condition` の実装は純関数であることを規約化する（外部I/O・時刻・乱数を禁止）
- 条件をnamed typeとして1箇所に定義させることで、レビュー・テストの対象として特定可能にする

### 行レベル権限（IDOR）

`Mutate<User, user::Email>` は「User型のemail列を書ける」であって「**この** Userを書ける」ではない。

```rust,ignore   // fragment, not a complete item
let victim = ctx.users().find(attacker_supplied_id).await?;
ctx.users().set_email(&mut victim, attacker_email)?;   // Capabilityは満たされる
```

1行の更新と全行の更新がContract上で同じに見える。**認可は必ず別途必要**であり、Capabilityは認可の代替ではない。[`capability-system.md`](./capability-system.md) の「静的Capabilityと動的Authorizationの区別」を参照。

### `*user = other_user`

Domainを不透明化しても、`&mut User` を持っていれば全体置換は可能。

```rust,ignore   // fragment, not a complete item
let mut a = ctx.users().find(id_a).await?;
let b = ctx.users().find(id_b).await?;
*a = b;    // 型検査を通る
```

`find` の戻り値をID型でブランド化すれば防げるが、ergonomicsのコストが大きい。現段階では明示に留める。

---

## Contract緩和バイアス — 型では解決しない問題

AIはコンパイルエラーに対して、**実装を直すより Contract を1行広げる方を選ぶ**。これは経済的に合理的な選択であり、型では防げない。

```text
error: undeclared mutation `User::status`
  help: add `User::status` to the contract, or remove this call
        ↑ AIはこちらを選ぶ                ↑ 本来はこちらが正しい場合も多い
```

[`diagnostics.md`](./diagnostics.md) の「helpは必ず2方向を示す」は文言レベルの対策であり、選択そのものは制約できない。

対処（型の外）:

| 手段 | 内容 |
|---|---|
| CI | `mutates` / `reads` / `domains` を**広げる**差分を検出し、別ラベル・追加レビュー必須にする |
| コミット規約 | Contractを緩める変更は理由を1行以上明記する |
| AI向け指示 | Contract緩和は最後の手段であることをCLAUDE.md相当に明記 |

**これは型システムの問題ではなく運用の問題である**ことを認識し、型で解決しようとしない。

---

## AI Contextへの出力

未検査境界は**必ず**AI Contextに出力する。

```json
{
  "endpoint": "UpdateUser",
  "scope_of_readonly_guarantee": "handler_only",
  "unverified_boundaries": [
    {
      "kind": "condition_body",
      "detail": "EmailChanged::holds は型検証不可",
      "location": "src/conditions/user.rs:12",
      "permanent": true
    },
    {
      "kind": "middleware",
      "detail": "適用される middleware の Effect は未宣言",
      "permanent": false
    },
    {
      "kind": "event_subscriber",
      "detail": "UserUpdated の購読側 Effect は未検査",
      "permanent": false
    },
    {
      "kind": "repository_impl",
      "detail": "Repository 実装内部の SQL は未検査",
      "location": "src/repositories/user.rs",
      "permanent": false
    },
    {
      "kind": "row_scope",
      "detail": "行レベル権限は型検査の対象外。認可は別途必要",
      "permanent": true
    },
    {
      "kind": "domain_swap",
      "detail": "*user = other_user は &mut D があれば成立し、閉じられない（path 2）",
      "permanent": true
    },
    {
      "kind": "domain_repr",
      "detail": "Domain の Repr は同一クレートのどこからでも到達可能。Capability 無しに構築・全フィールド読出しができる（path 21）",
      "location": "src/domain/user.rs",
      "permanent": false
    },
    {
      "kind": "malformed_set",
      "detail": "壊れた effect 集合を capability 検査に通せる（path 14f）。要素が素のローカル型のときに限る",
      "permanent": false
    },
    {
      "kind": "service_body",
      "detail": "observed_effects の走査は handle の中だけ。Service 本体で起きる Effect は下界に現れない（path 22）",
      "permanent": false
    }
  ]
}
```

`permanent: true` は原理的に埋まらないもの、`false` は将来Contractを拡大すれば消えるもの。

**この出力機構はFirst PoCから実装する。** 後から追加すると、それまでのAI Contextが「嘘をついていた」ことになる。

---

## 進捗の測り方

Contractを拡大すると `unverified_boundaries` の項目が減る。これがそのまま進捗指標になる。

```text
First PoC:  permanent 3 件 + non-permanent 6 件
Full PoC:   permanent 3 件 + non-permanent 3 件（middleware / event を対応）
将来:       permanent 3 件 + non-permanent 0 件
```

`permanent` が0になることはない。それを隠さないことがこのファイルの目的である。

> **数え方の定義**（レビューで「数えるたびに違う値になる」と指摘されたので明示する）: **AI Context の `unverified_boundaries` に出る項目と1対1**で数える。permanent 3 = `condition_body`（20）/ `row_scope`（行レベル権限）/ `domain_swap`（2）。non-permanent 6 = `middleware`（16）/ `event_subscriber`（15）/ `repository_impl`（17）/ `domain_repr`（21）/ `malformed_set`（14f）/ `service_body`（22）。
>
> **数に入らないのは path 18 と 19 の2つで、理由は同じ** — どちらも「後回し」ラベルだけで `kind` 名が決まっておらず、どのサンプルにも現れない。以前この定義は 19 だけを除外して 18 を数えており、**定義が数えるものと出力されるものが食い違っていた**（#43 項目8）。両方を決めること。
>
> この9項目は、本ファイルのサンプルと [`ai-context.md`](./ai-context.md) のサンプルの両方と**集合として一致していなければならない**。3箇所が違う値を持っていたのが、この注記が書き直された理由である。

---

## 「GET は read-only」の正確な範囲

MiddlewareがContractを持たない限り、この保証は**ハンドラスコープに限定される**。

```rust,ignore   // fragment, not a complete item
// Auth Middleware が last_login_at を更新する場合
GET /users/{id}
  ハンドラスコープ  : Mutates = () → read-only（真）
  リクエストスコープ: User.last_login_at が更新される（偽）
```

`scope_of_readonly_guarantee: "handler_only"` として明示する。Middleware Contractを導入した時点で `"request"` に昇格させる。

**保証の範囲を正確に名乗ることは、保証を強くすることと同じくらい重要である。**
