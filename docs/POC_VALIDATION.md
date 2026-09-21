# PoC Validation — #30早期準備

版: A-draft-1 / 2026-09-21。文書レビュー用。実施承認: PENDING（承認者・日時未記入）。
Kit status: READY_FOR_REVIEW / Execution approval: PENDING。
Phase A実評価: NOT_RUN / 初見参加者数: 0。#30最終Decision: **NOT_EVALUATED**。
資料作成・実装担当リハーサルは初見UX合格、M2 exit達成、PoC完成、release GOの証拠ではない。

## 正本・配布境界

- [Epic #1](https://github.com/albert-einshutoin/CodebaseCanvas/issues/1)のM2 exitに向けた予備準備。[#30](https://github.com/albert-einshutoin/CodebaseCanvas/issues/30)のDepends onは最終close条件であり、Phase Aの準備を止めない。
- [参加者用5課題](validation/FIXTURE_UX_TASKS.md)だけを配布する。source、fixture README、oracle本文、採点表、進行者資料、本計画は参加者に同梱しない。JSONはアプリのFile入力用にのみ渡す。文書分離だけで事前閲覧を防げたとは扱わず、正解資料/READMEの事前閲覧、実装・レビュー参加、fixture知識を確認し、初見参加者と区別する。
- [進行者用正解・根拠・記録票・リハーサル](validation/FIXTURE_UX_FACILITATOR.md)は進行者専用。回答の独力確定後まで答え合わせをしない。
- [契約](DATA_MODEL.md)・[Canvas仕様](CANVAS.md)を意味の正本にする。[リリース準備](RELEASE_READINESS.md)の過去判定と本計画を混同しない。

## Phase A: fixture予備UX

### Repository profile・固定対象

| 項目 | 固定値 |
|---|---|
| repository | albert-einshutoin/CodebaseCanvas |
| アプリcommit / 作業base | `e0a139a6c498ddccc7fb0ba18c74c69beb73935f` |
| アプリtree | `b4d0bae6e3145d0f3092e2bcddef92ac4620d0f8` |
| 入力 | `examples/nestjs-sample/expected-graph.json` |
| 入力SHA-256 | `7f378e14cfc7b733cb43f3d5bdbf2ef10d74816187ad15f03e0594ec80b2f046` |
| schemaVersion | `0.1` |
| analyzerVersion | `hand-authored-fixture-v1` |
| 元analyzedAt | `2026-09-05T00:00:00Z` |
| 資料branch | `codex/issue-30-fixture-ux-kit` |
| 資料commit | 本文への自己参照SHAは埋め込まず、資料PR本文に最終HEAD/treeを記録する。上記アプリcommitとは別に、配布承認時の資料commitまたは3文書のhashを記録する |

選定理由は共有登録、同一route別handler、provider override、局所unknown、mixed evidenceが小さい独立fixtureに共存すること。実repo代表性やAnalyzer精度は主張しない。最新mainが進んでも評価対象を自動更新せず、再固定と資料整合確認を先に行う。

### 開始条件・環境

実施前に承認者が版・対象・課題・正解・採点・介助・人数・時間・重大誤認規則を承認し、資料hashと日時を記録する。変更は参加者結果を観測する前に理由付きで改版。現在は承認待ちであり、本番評価を開始しない。

実施前の承認・確定対象:

- 資料版/hash、アプリcommit/tree、fixture、課題・正解・最終採点・重大誤認規則。
- 人数・適格性/事前閲覧確認、時間、介助、利用環境、記録・同意・保管方法。
- 比較の経験対応・方式割当・順序、時間切れ/不正解/介助/欠測/除外の扱い。

初見3名を予定。製品/fixtureの実装・レビュー未参加で、既存NestJSの引継ぎ/変更前調査を行うバックエンドエンジニア。TS/NestJS経験、AI Coding利用、fixture事前知識を匿名記録。実装者・依頼者・既知回答者は補助/リハーサルのみ、初見人数に含めない。

進行者が固定commitのWebとローカル貼付け先を用意し、参加者は手定義JSONを実File選択する。CLI生成を開始条件にしない（#17未接続、本番analyze非0/無書込）。これはCLI導入体験の評価ではない。

実行は専用worktreeルートで次のとおり。既存checkoutを書き換えるcheckout/resetはしない。

```sh
git rev-parse HEAD HEAD^{tree}
shasum -a 256 examples/nestjs-sample/expected-graph.json
pnpm install --frozen-lockfile --ignore-scripts
pnpm web:build
pnpm --filter @codebasecanvas/web exec vite preview --host 127.0.0.1 --port 5190 --strictPort
```

Node24.x（24.2.0以上）、pnpm11.8.0。実施環境案: Chrome stable/macOS、viewport1440×900、zoom100%、日本語キーボード。実際のbrowser version/OS/viewport/zoom/入力方式を実施前に記入し、リハーサルのIABとの違いを残す。port競合時は実行を止め、空きportを事前に決め記録する。参加者結果を環境修復時間と混ぜない。

### Canvas usability・採点・暫定の次工程基準

5課題をT1→T5で提示、各5分。課題提示時から独力回答確定までを計測し、5分で未完回答を凍結する。課題間は同じfixtureを再入力して表示状態をリセット。参加者の学習は残るため、これは独立な5実験やsource比較ではない。

各2点、10点満点。2=必要事項と根拠を介助なしで回答、1=正しい一部、0=誤答/不能/時間内に対象へ到達不能。具体条件は進行者表に固定。意味の誤断定を部分正解で相殺しない。介助後の答えで独力点を上書きせず、追加時間・介助文言・変化を別記。

暫定基準案: 各人8/10以上、独力回答に「DIは実装保証」「unknownは不存在」「0件は完全解析」の重大誤認なし、全員が対象Contextを取得。自動copyと手動fallbackは別記し、fallback取得だけでも取得条件は満たすが自動copy成功率には含めない。3名未満、欠測、環境中断は基準達成とはしない。未達は予備課題を報告し、無断のUI改善や基準緩和・Issue作成をしない。

これは本プロジェクトの予備基準で、普遍的UX標準や統計的価値実証ではない。結果は全員/全課題分を残し、成功者だけ選ばない。現在の得点・時間・判定欄は空欄。

### source-first比較計画（今回はNOT_RUN）

正式比較を省略しない。Phase Aの同じ参加者に同じ課題を再回答させた時間差をCanvas効果にしない。Phase A参加者は同じfixtureの比較対象から除外する。

比較設計案: 新規の初見参加者6名を経験（TS/NestJS年数、引継ぎ経験、AI Coding利用）で3対に組み、各対をsource-first/Canvas-firstに事前割当する参加者間比較。全員に同じ、双方から解けるQ1所属（UsersServiceの静的登録全集合）、Q2route/handler（GET /usersの全一致集合）、Q3宣言token（OverrideControllerのconstructor要求）だけを提示。5分/問、順序Q1→Q2→Q3固定、1人は一方の方式のみ。source側はローカルeditorの検索/ファイル閲覧、Canvas側は本UI、外部LLM/インターネット回答探索は双方禁止。graph独自skipped集計とCopyは時間比較から除外。

実施前に、比較人数・対の作り方・割当seed/結果・最終質問/正解/採点・使用editorと検索機能・環境・記録方法を固定し承認する（未承認）。この案はPhase B実repo課題へ自動転用せず、独立正解表と難度対応を別に作る。難度差、事前知識、方式間の経験差、順序内学習を交絡として記録する。6名でも統計的製品価値を証明したとは扱わない。

正答率（全割当回答を分母）と全問の経過時間/完了状態を併記。タイムアウトは300秒打切り・未成功、誤答も経過時間と不正解、介助ありも独力結果と別記し除外しない。中央値等だけで速度改善を宣言せず、全件の散布/表と正答差を見る。欠測・中断・除外の定義、理由記録、分母と時間集計での扱いは観測前の承認事項とし、未確定のまま比較を開始しない。比較人数・課題対応が揃わなければ予備観測に留め、速度優位/製品価値実証を主張しない。

## Phase B: 実repo精度・比較・性能・E2E・release gate

Status: NOT_RUN。#30最終Decision: NOT_EVALUATED。Phase A結果から数値を後付けしない。

| 項目 | 測定前に固定するもの | 現状 |
|---|---|---|
| Repository profile | #7のpinned実repo/commit/capability matrix参照。変更なら理由/pattern差分。公開可能なsourceと独立重要関係集合 | 未固定 |
| Analyzer result | 現行Rust commit、入力commit、実analyzeコマンド、生成日時/graph hash、成功/失敗、files/nodes/edges/diagnostics | 未実施 |
| Accuracy findings | Module/DI/Endpoint false relation=0必須。supported total/correct/missing/false、unsupported正診断/黙殺を別集計。calls falseも問題とする | missing/unknown許容上限の具体値は未固定、測定開始不可 |
| Unsupported findings | scope別unknown理由/件数、#14、provider overrideのfalse calls禁止 | 未実施 |
| Canvas usability / source比較 | 未知の実repo課題、対応正解/難度、人数/経験/割当/順序、時間/正答の基準 | 未固定 |
| Performance | #27の機器/入力規模/測定方法、時間/peak memory/graphサイズ/UI loadの数値上限 | 未固定、測定開始不可 |
| Copy Context | 独立正解に対する構造・evidence・confidence帰属・unknown・日時・上限の保持 | 実repo未実施 |
| E2E / release | #17統合、#18構造回帰、#26現行Rust→fresh graph→production File API→Canvas/Context、#27、#28配信、最終commit #31 hosted CI | 未完了。fixture JSON代替不可 |
| Decision | 全証拠でGO / GO WITH FIXES / STOPを判定。証拠不足はNOT_EVALUATED | NOT_EVALUATED |

#14/#15/#17を含む必須scopeを除外してv0.1完成にしない。#29は任意。GO WITH FIXESも修正/再検証前の完成を意味しない。#1 DoD最終確認、release/公開判断は別工程。新たな追跡Issue作成は今回行わない。

## Known limitations・データ管理・結果

手定義fixtureはAnalyzer出力/精度、Rust→browser E2E、runtimeを証明しない。自然なClipboard拒否、実IME/pinch、初見UX、正式性能、layout回数、全callbackは未確認。chunk警告、長いfieldのprefix省略、escapingがprompt injection/秘密除去の完全保証でない点を維持する。

匿名P01等を使用。本人同意なしに録画・発言引用・共有をしない。対応表/録画/私的コードは公開repoへ保存しない。回答は進行者のアクセス制限されたローカル領域に保管し、保管期限/共有先/撤回方法を実施前同意票へ記載。自動upload、analytics、backend、新規調査サービスは導入しない。

| 結果欄 | 状態 |
|---|---|
| 初見参加者数 | 0 |
| Phase A実評価 | NOT_RUN |
| 点数・時間・発言・参加者回答 | 未記入 |
| source-first実験 | NOT_RUN |
| Phase B | NOT_RUN |
| #30最終Decision | NOT_EVALUATED |

リハーサルの実施結果と資料差分レビューは進行者資料にのみ記録。LLM仮想参加者や予想回答で結果を埋めない。
