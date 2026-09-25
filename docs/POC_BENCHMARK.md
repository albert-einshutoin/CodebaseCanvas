# PoC performance baseline — Issue #27

## 事前計画（v1、2026-09-25 UTC、結果観測前に固定）

対象は CodebaseCanvas `6e5b974dc01bead04f93cff06c8ecab15613d8d1` に、この Issue の計測差分だけを加えた worktree とする。計測差分の hash、release binary の hash、Web build 条件を結果に記録する。解析・表示の高速化やレイアウト変更は含めない。

入力は (A) `examples/nestjs-sample` の source、Prisma、tsconfig だけを使い捨て root にコピーしたものと、(B) `https://github.com/lujakob/nestjs-realworld-example-app` の `c1c2cc4e448b279ff083272df1ac50d20c3304fa` を別の clean clone から使い捨て root にコピーしたもの。B の依存 install、script、server、DB、submodule、LFS は実行しない。各入力の source 数と物理 LOC（空行・コメントを含み、末尾改行だけの空行は数えない）、選択 Prisma schema 数、入力 manifest を記録する。既存 probe の B の 35 source と production discovery を照合する。

実行機器は Apple M4 / 32 GiB / Mac16,10、macOS 27.0 (26A428) arm64、AC 電源。Node v24.2.0、pnpm 11.8.0、Rust 1.96.0。Playwright 1.63.0 の Chromium を headless、viewport 1440×900、deviceScaleFactor 1、zoom 100%、通常の初期 kind filter・method 非展開・無選択で用いる。Chromium 実 version、他負荷、build 日時は結果に追記する。OS cache は強制 purge しない。clone・hash 確認で温まる可能性があるため初回を cold cache と呼ばない。

Analyzer は `cargo build --release --locked -p codebasecanvas-analyzer --bin codebasecanvas --message-format=json` を測定前に一度実行し、Cargo artifact JSON から binary を特定する。各 profile は同じ使い捨て root に対し、新規 process の `codebasecanvas analyze <root>` を初回 1 回と反復 5 回実行する。`cli_wall_ms` は process 起動直前から終了までで、discovery、解析、構築、validation、serialization、保存を含む。`peak_rss_bytes` は macOS `/usr/bin/time -l` の対象 process の最大 RSS（bytes）から得て、MiB は 1,048,576 bytes で割る。Linux では GNU time の KiB を 1024 倍する。Node/Cargo/browser の RSS は含めない。1 試行の上限 120 秒。初回は出力 directory 作成、反復は file 置換を含む。出力確認・Graph 集計は計測区間外。各 Graph の JSON bytes/hash、nodes/edges/diagnostics 内訳、callAnalysis、analyzerVersion/analyzedAt、CLI の TS/TSX・Prisma 件数、fatal/部分解析を記録する。analyzedAt を除く Graph 内容の同一性を確認する。

Web は各 profile の **最初の成功した Analyzer 試行** の Graph を固定し、同じ file/hash を 6 回使う。成功 Graph がなければ Web は未実施。production Web build、preview、browser 起動、依存取得は時間外。Chromium browser process は再利用し、試行ごとに新規 context/page を作る。実 File input を通し、browser の User Timing で次を計る: `read_decode_ms` は arrayBuffer 開始から UTF-8 decode 終了、`json_parse_ms` は JSON.parse、`validation_ms` は SystemGraphSchema.safeParse、`layout_ms` は初期 layoutCanvas 呼出し（Fit を含む）、`initial_canvas_ms` は初期要素適用開始から同世代の Cytoscape render、`import_to_render_ms` は File change handler 開始から同 render。対象 render は layout/style の同期効果後、現在 generation と実要素の存在を確認する。renderer event は物理 display 提示時刻の保証ではない。表示 nodes/edges/parent frames は既存 read-only DOM 属性から数える。Web 上限は 1 試行 60 秒。初回 1 回と反復 5 回、再試行・人工 sleep はしない。

両 profile とも生値、初回単独値、反復 5 回の中央値・最小・最大を記録する。P95/P99 は求めない。timeout、非 0 終了、欠測、不正 Graph、異なる Graph、ブラウザ失敗は理由を記録し 0 や前回値で埋めず、成功値だけを選び直して全体合格としない。途中中止も理由と未実施回数を残す。

暫定 guardrail（各 profile ごと）: Analyzer 反復 wall 中央値 ≤10,000 ms、初回＋反復の peak RSS 最大 ≤1,024 MiB、Web 反復 validation 中央値 ≤1,000 ms、初期 Canvas 中央値 ≤5,000 ms、File handler→render 中央値 ≤10,000 ms。全 6 試行で必要値が揃わなければ判定不能。超過は `EXCEEDED` とし、閾値を変更・最適化しない。これはこの機器と小規模 2 profile の暫定予算であり、製品 SLA、UX 合格、#30 release GO ではない。

実行順: 計測器の対象テストと pilot → 専門レビュー → Companion Gate → 必要な計測器修正・限定再確認 → 固定条件の本測定 → `env -u RUST_TEST_THREADS pnpm run ci` と `git diff --check`。pilot と本測定は混ぜない。結果には環境、build、入力/Graph hash、計測差分、再現 command、失敗も含む全試行、解釈の限界を記録する。

## 結果

### 固定記録と対象

事前計画 v1 は **2026-09-25T05:25:25Z** に結果欄が空の状態で固定した。対象は当時の `docs/POC_BENCHMARK.md` 全体で、固定時の SHA-256 は `45ffd1dfe9d4d49f88eb81c6f10a48e741e3398ca3a3a846ef11e14f4466f82d`。同時に別保存した [測定前 snapshot](benchmark/issue27-preplan-v1.md) はこの hash と一致し、その事前計画部分は本書でも同一。結果追記後の本書の hash とは区別する。この PR に含む snapshot と記載日時だけで、固定日時の独立した外部証明とは扱わない。

本測定は 2026-09-25T05:39:00.759Z–05:39:08.384Z、base `6e5b974dc01bead04f93cff06c8ecab15613d8d1` の専用 worktree で実施。計測 source manifest SHA-256 は `8431c582afafe41a620ce04c05bb59aef3f87db637eb0b047eaaa9ff1f99e30e`（対象 file 一覧は [raw record](benchmark/issue27-raw.json)）。未 commit の観測差分を含む production Web build と、同実行の Cargo `--release --locked` artifact を使用した。Analyzer binary SHA-256 は `ea7bfd78a425b713b379d0b7f3410f502b91237480d8c7119f4cf890e9a90f58`。runner は当該 worktree の `target/release/codebasecanvas` だけを受理する。pilot は [別記録](benchmark/issue27-pilot.json) であり、本測定の中央値には含めない。

実機は Mac16,10 / Apple M4 / 32 GiB / macOS 27.0 (26A428) arm64、AC 電源。Node v24.2.0、pnpm 11.8.0、rustc/cargo 1.96.0、Playwright 1.63.0、Chromium 153.0.8010.12。headless Chromium、viewport 1440×900、deviceScaleFactor 1、zoom 100%。特別な負荷隔離・thermal 制御は行っていない。Cargo 依存と browser は既存 cache を利用し、実 repo clone・使い捨て解析 root・Web context/page は新規作成。Chromium process は Web 12 試行で再利用し、OS cache は purge していない。

実 repo は `https://github.com/lujakob/nestjs-realworld-example-app` の `c1c2cc4e448b279ff083272df1ac50d20c3304fa`。隔離 clone の origin・HEAD・clean 状態を確認した。代表 source は `src/app.module.ts`、`src/article/article.controller.ts`、`src/article/article.service.ts`、`src/user/user.module.ts` など。依存 install/script/server/DB、submodule/LFS、source 改変は行っていない。production discovery は 35 TS/TSX files で既存 probe の 35 と一致した。Prisma schema は 0。fixture は 13 TS/TSX files と選択 Prisma schema 1、Graph の database_model は 2。

| Profile | 入力 files | TS/TSX files | 物理 LOC | 入力 manifest SHA-256 |
|---|---:|---:|---:|---|
| Fixture | 15 | 13 | 150 | `70cdb78acde1622a0855db1a1f5275d7a2bf32c3eed533281499eb804d9db462` |
| 実 repo | 49 | 35 | 1,191 | `dabaac389c11718b4cd926ada4cd3ca8a06fd302ec7a64e16ccf24201ce77eb3` |

LOC は Analyzer と同じ除外 directory と `.d.ts` 除外を適用した `.ts`/`.tsx` の物理行で、空行・コメントを含む。末尾改行だけで生じる空行は数えない。入力 manifest は相対 path と各 file の SHA-256 から算出した。生成 Graph や依存は入力に含めていない。

### Analyzer — release CLI 実 process

`cli_wall_ms` は Node の monotonic clock で `/usr/bin/time -l` を起動する直前から終了まで測り、time wrapper のわずかな起動・終了 overhead を含む。discovery から安全な保存までを含み、parser 単独時間ではない。peak RSS は macOS `/usr/bin/time -l` の `maximum resident set size` **bytes** を MiB に換算した対象 child process の値。初回は output directory 作成を、反復は atomic file 置換を含む。Graph の読取り・契約検証・集計は計測時間外。

| Profile | 初回 wall (ms) | 反復 wall 中央値 (ms) | 反復 min–max (ms) | 6回の peak RSS 最大 (MiB) | JSON (bytes) | nodes | edges | diagnostics |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Fixture | 269.59 | 13.99 | 12.53–15.01 | 5.44 | 150,732 | 57 | 90 | 15 |
| 実 repo | 32.73 | 29.47 | 28.90–30.16 | 8.05 | 646,330 | 164 | 385 | 116 |

| Trial | Fixture wall (ms) | Fixture peak RSS (MiB) | 実 repo wall (ms) | 実 repo peak RSS (MiB) |
|---:|---:|---:|---:|---:|
| 初回 0 | 269.59 | 5.31 | 32.73 | 7.97 |
| 反復 1 | 15.01 | 5.30 | 29.47 | 7.97 |
| 反復 2 | 12.53 | 5.44 | 28.90 | 7.95 |
| 反復 3 | 13.28 | 5.30 | 30.16 | 7.91 |
| 反復 4 | 14.26 | 5.30 | 29.01 | 8.05 |
| 反復 5 | 13.99 | 5.34 | 29.68 | 7.94 |

全 12 試行は終了 code 0、RSS と保存 Graph の取得・SystemGraph 契約検証に成功。各 profile の `analyzedAt` を除いた Graph は 6 回で一致した。選択した初回 Graph SHA-256 は Fixture `fef2048faf6ee2f0d70f277b7bc890a6a9b8716b812c622104ed398e5bef6d84`、実 repo `22970b836c8bd1277af85603b5190bed65a3a39df66e82bd6a8a62394a6c7415`。両 profile とも全 6 回で file hash 自体も一致した。Fixture の初回時間差を cold cache の純粋な効果とは断定しない。

Fixture の diagnostics は warning 15/error 0、callAnalysis は examined/emitted/skipped = **9/2/7**。実 repo は warning 116/error 0、**141/7/134**。実 repo には `TS_IMPORT_MISSINGMODULE` 4、`TS_IMPORT_UNSUPPORTEDEXPORT` 17、`TS_IMPORT_UNSUPPORTEDIMPORT` 5、`unsupported_call_injected_receiver` 45、`unsupported_call_unknown_receiver` 22 などがあり、Graph が検証可能でも抽出範囲に局所 unknown が多い。node/edge kind と diagnostic code の全内訳は raw record に残した。性能だけから実 repo を完全解析した、あるいは関係が正しいとは判断しない。

### Web — production build / 実 File input / Chromium

Web は各 profile の最初の成功 Graph を固定し、各 6 回とも同じ SHA-256 を File input に指定した。毎回新しい context/page の初期状態で、File read/UTF-8 decode、JSON.parse、`SystemGraphSchema.safeParse`、初期要素適用・layout/Fit、同 generation の Cytoscape render を browser 内の User Timing で測った。対象 render は同期 style/layout 効果、generation、実要素を確認したイベントであり、物理画面提示時刻の保証ではない。初期表示は全 Graph ではなく既定 kind projection で、method は展開していない。

| Profile | Graph JSON (bytes) | 全 nodes/edges | 表示 nodes/edges | parent frames | 初回→render (ms) | 反復→render 中央値 (ms) |
|---|---:|---:|---:|---:|---:|---:|
| Fixture | 150,732 | 57 / 90 | 40 / 56 | 7 | 113.70 | 84.20 |
| 実 repo | 646,330 | 164 / 385 | 105 / 155 | 10 | 150.80 | 150.80 |

以下の min/max は反復 5 回だけから算出。各試行の丸め前の値は raw JSON に記録した。`initial_canvas_ms` は `layout_ms` を内包し、各列を足して全体時間にはしない。

| Profile / metric (ms) | 初回 | 反復中央値 | 反復 min | 反復 max |
|---|---:|---:|---:|---:|
| Fixture read/decode | 3.50 | 0.90 | 0.80 | 1.40 |
| Fixture JSON.parse | 0.50 | 0.20 | 0.00 | 0.20 |
| Fixture validation | 13.50 | 6.80 | 6.10 | 6.90 |
| Fixture layout/Fit | 40.90 | 41.40 | 36.50 | 43.00 |
| Fixture initial Canvas→render | 72.90 | 64.80 | 60.20 | 67.50 |
| Fixture File handler→render | 113.70 | 84.20 | 78.20 | 87.80 |
| 実 repo read/decode | 1.40 | 1.10 | 0.90 | 1.50 |
| 実 repo JSON.parse | 0.40 | 0.40 | 0.30 | 0.40 |
| 実 repo validation | 15.20 | 15.80 | 14.80 | 17.10 |
| 実 repo layout/Fit | 85.90 | 85.40 | 79.20 | 90.10 |
| 実 repo initial Canvas→render | 117.70 | 117.70 | 111.30 | 120.50 |
| 実 repo File handler→render | 150.80 | 150.80 | 142.80 | 154.00 |

全 12 試行で対象 generation の render、既定表示数、取込成功を確認し、timeout・欠測・失敗は 0。Fixture の反復 JSON.parse `0.00 ms` はブラウザ時計の分解能で 0 と報告された生値であり、処理時間が物理的にゼロとの意味ではない。browser 全体や JS heap の memory は測っていない。

### 判定・解釈・再実行

| 暫定 guardrail | Fixture 実測 | 実 repo 実測 | 判定 |
|---|---:|---:|---|
| Analyzer 反復 wall 中央値 ≤10,000 ms | 13.99 ms | 29.47 ms | 両方 WITHIN |
| Analyzer peak RSS 最大 ≤1,024 MiB | 5.44 MiB | 8.05 MiB | 両方 WITHIN |
| Web 反復 validation 中央値 ≤1,000 ms | 6.80 ms | 15.80 ms | 両方 WITHIN |
| Web 反復 initial Canvas 中央値 ≤5,000 ms | 64.80 ms | 117.70 ms | 両方 WITHIN |
| Web 反復 File handler→render 中央値 ≤10,000 ms | 84.20 ms | 150.80 ms | 両方 WITHIN |

本測定の完備状態は `OK`、guardrail 超過は 0。小規模 2 profile では PoC 性能の明確な blocker を観測せず、今回の性能だけを理由とした v0.1 前の修正や新 Issue は不要と判断する。Web では測定した区間のうち layout/Fit が反復中央値で Fixture 41.40 ms、実 repo 85.40 ms と最長だが、未 profiling の内部関数や依存ライブラリを原因と断定しない。数万～10万 LOC の repository、method 展開、長時間の反復、初見 UX、精度は測っていない。incremental analysis の必要性はこの 2 件からは判定できない。#30 の Phase B 全体と release 判定は引き続き未評価。

再実行はこの専用 worktree で Node 24.2.0 以上の 24.x、pnpm 11.8.0、Rust 1.96.0、Playwright Chromium を用意して `pnpm install --frozen-lockfile --ignore-scripts`、`pnpm web:e2e:install`、`pnpm bench:poc`。runner が固定実 repo を隔離取得し、release binary と production Web を build し、各 profile の 1+5 試行を実行する。出力は [全生値・内訳・hash の JSON](benchmark/issue27-raw.json)（本測定 file SHA-256 `5608679a1f0e6502b0da96fc119ad0304ff8941322172deb3a8d52e28fda94ce`）。pilot は `CBC_BENCH_PILOT=1 pnpm bench:poc` で別 file に保存する。通常の `ci` には公開 repo 取得と実測反復を入れず、計測 helper の安定したテスト・型検査だけを含めた。

### 計測器・最終検証

RSS の macOS bytes/Linux KiB 変換と欠測拒否、反復 5 値、browser mark の順序・generation、失敗/timeout/hash 不一致時の完備判定を単体テストで確認した。既存 Graph import と計測 callback 付き import の結果一致、不正 JSON で render 成功 mark が出ないことも確認した。専門レビューは当初 runner の不完備試行 0 終了を `BLOCK` とし、raw 保存後の非 0 終了へ修正した指摘範囲の再確認は `ALLOW`。Companion Stop Review Gate は stale/debug binary を受理できる build-log override を `BLOCK` とし、この経路を削除して規定 release build と当該 artifact を必須にした 2 行だけの再確認は `ALLOW`。

保存済み pilot は修正前の runner で両 profile 1 回ずつ実施し、build-log override と不完備試行を 0 終了にし得る経路が残っていたため、正式な 6 回測定・guardrail 判定から全件除外した。これは pilot を本測定と分けた事前規則の適用であり、観測後の閾値や成功試行の選別ではない。修正後の正式測定では runner 自身が同 worktree の release build を起動した。

本測定後、`env -u RUST_TEST_THREADS pnpm run ci` は exit 0。frozen install、Chromium 準備、Rust build/format/clippy と workspace tests **189 passed / 0 failed / 1 ignored**、Web typecheck/unit/build、fixture typecheck、既存 current-output Chromium E2E **4/4** が成功した。既存 E2E preparation helper は **2/2**。Web unit は **11 files / 163 tests** 成功し、この中に計測 helper の 4 tests と canonical import の計測有効/無効・不正入力 test を含む。E2E 4件は通常の `*.e2e.ts` のみで、`*.bench.ts` と公開実 repo の取得・反復測定は実行していない。`git diff --check` も成功。hosted CI はこのローカル計測段階では未実施。

| Issue #27 受入条件 | 今回の根拠 |
|---|---|
| fixture release Analyzer | 上記 release binary/hash と Fixture 6 試行 |
| 代表的実 repo | 固定 commit・clean clone・35 files の実 repo 6 試行 |
| wall-clock | 初回/反復の表と raw JSON |
| peak memory | `/usr/bin/time -l` bytes、全試行値と最大値 |
| Graph node/edge/JSON size | Analyzer 表と kind 別 raw 内訳 |
| Web validation/render | 実 File input、各段階・表示数の表と raw JSON |
| environment/method | 本文の固定計画・実機・境界・再実行 command |
| 実測に基づく claim | 2 profile の `WITHIN` と未測定範囲を分離 |
| bottleneck の別 Issue 用情報 | 今回の暫定予算超過はなく作成不要。将来の大規模入力では同 runner の入力規模・wall/RSS・Web各段階・診断を再取得してから判断する |
