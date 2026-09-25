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

未記入。事前計画の file hash と固定日時は結果観測前に外部記録し、ここへ転記する。
