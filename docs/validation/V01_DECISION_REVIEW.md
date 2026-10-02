# v0.1 継続判断レビュー案 — Issue #30

| 項目 | 状態 |
|---|---|
| 文書状態 | **DRAFT_FOR_REVIEW** |
| 推奨する継続判断 | **REVISE** — 現行 v0.1 を完成・GO と認定せず、対象課題と評価前提を見直す |
| 判断対象 | 現行 v0.1 の継続・評価方針。製品価値の最終判定ではない |
| 人間の利用価値 | **NOT_EVALUATED**。初見参加者 0 名 |
| 正式な採否承認 | 未取得 |
| GitHub [Issue #30](https://github.com/albert-einshutoin/CodebaseCanvas/issues/30) | **OPEN / 正式 Decision NOT_EVALUATED**。今回は更新しない |
| 既存技術監査 | **COMPLETED / EXCEEDED** |
| 人間 UX・source-first 比較 | **NOT_RUN / 初見参加者 0 名** |

## 判断の対象と証拠の時点

- この判断資料の base は `c97bccb816da289544c155d6bb94da934133371d`、tree は `6f7663eaec05d7a38f6009ac5c6d24316f9f9aa6`。[PR #67](https://github.com/albert-einshutoin/CodebaseCanvas/pull/67) 統合後の [main CI run 36384430444](https://github.com/albert-einshutoin/CodebaseCanvas/actions/runs/36384430444) はこの SHA で成功した。
- [実 repo 技術監査](REAL_REPO_ACCURACY.md)の製品と、[公開先](https://codebasecanvas.einstein-4s-1110.workers.dev/)の asset を照合して操作した製品は `434bf36a815232f88fc157dd1c4a0df5915030da`。対象入力は `lujakob/nestjs-realworld-example-app` の `c1c2cc4e448b279ff083272df1ac50d20c3304fa`（35 TS/TSX）。公開記録の指定 Version ID は `3ef69354-93bb-488f-965a-3a18b1350e13`。公開 asset の hash と固定製品 build は一致したが、Version ID 自体は公開応答から独立取得されていない。
- [PR #65](https://github.com/albert-einshutoin/CodebaseCanvas/pull/65) は既知 profile の監査、[PR #66](https://github.com/albert-einshutoin/CodebaseCanvas/pull/66) は TypeORM 宣言の設計、PR #67 は `typeorm_repository_request` の**実験的 v0.2 契約検証層**を統合した。本番 Graph 契約・CLI 出力・File 入力・Canvas は [v0.1](../DATA_MODEL.md) のまま。v0.2 の validation 成功は実 source からの抽出や利用価値の証拠ではない。
- 最新 main の CI 成功は最新 main での実 repo 再監査、公開先の自動更新、公開先での再操作を示さない。本レビューでは再解析・再計測・公開ブラウザ試験を行っていない。

## 結果表

分類はこの判断資料での証拠状態である。「確認済み」も、列挙した対象と観測範囲に限る。edge の分母と source 項目の分母を混ぜない。

| 分類 | 対象 commit / profile | 根拠 | 示す範囲／示さない範囲 |
|---|---|---|---|
| 確認済み | v0.1、製品 `434bf36a…`、固定実 repo `c1c2cc4e…` | [監査の実行と照合](REAL_REPO_ACCURACY.md#実行と照合結果)、[PR #65](https://github.com/albert-einshutoin/CodebaseCanvas/pull/65) | CLI が Graph を保存し、正規 schema・semantic validator を通過。164 nodes / 385 edges / warning 116。runtime 配線や全構文の解析完了は示さない。 |
| 確認済み | v0.1、現行 Rust build と fresh fixture | [#26 E2E 記録](../WEB_E2E.md)、[main CI run 36384430444](https://github.com/albert-einshutoin/CodebaseCanvas/actions/runs/36384430444) | 現行 Rust → fresh fixture → production File 入力 → Canvas・検索・Details・focus・unknown・Context の回帰経路は通る。固定実 repo の再監査や人間の理解は示さない。 |
| 確認済み | v0.1、公開 asset と固定実 repo Graph、製品 `434bf36a…` | [公開 UI 操作記録](REAL_REPO_ACCURACY.md#公開-canvas--clipboard-context)、[UI summary](real-repo-ui-summary.json) | Static Assets の asset hash 一致、実 File 入力、Canvas・検索・Details・unknown・Clipboard Context の技術操作、入力後 HTTP request 0 を観測。公開実 repo での focus 操作、観測期間外の通信、初見 UX、最新 main の公開は示さない。 |
| 確認済み | v0.1、既知の固定実 repo | [監査結果](REAL_REPO_ACCURACY.md#実行と照合結果)、[結果 JSON](real-repo-result.json) | Module supported edge **16/16 correct**、missing / false 0。未解決 source 項目の 5/21 とは別の分母。全 repo の Module 対応率ではない。 |
| 確認済み | v0.1、同 profile | [監査結果](REAL_REPO_ACCURACY.md#実行と照合結果) | DI supported edge **5/5 correct**、missing / false 0。要求 token の静的事実であり、provider 実装や runtime 注入先は示さない。 |
| 確認済み | v0.1、同 profile | [監査結果](REAL_REPO_ACCURACY.md#実行と照合結果) | Endpoint supported edge **42/42 correct**、未解決 source 項目 **0/21**。宣言 route と handler の関係であり、実行時到達や request 経路ではない。 |
| 確認済み | v0.1、同 profile | [監査結果](REAL_REPO_ACCURACY.md#実行と照合結果) | 評価集合で Module / DI / Endpoint の missing・false・silent omission は未検出。source-first 台帳・固定 scope 内の結果であり、独立 holdout や全 repo の完全性ではない。 |
| 確認済み | v0.1、同 profile | [監査結果](REAL_REPO_ACCURACY.md#実行と照合結果) | calls **141 site examined / 7 emitted / 134 skipped**。7 site / 7 edge は期待と一致し、134 は scoped unknown。7/141 を正解率、warning 116 件を誤り 116 件とは呼ばない。 |
| **基準超過・制約を確認済み** | v0.1、同 profile | [監査の暫定基準と結果](REAL_REPO_ACCURACY.md#source-first-台帳と基準) | Module 未解決 source 項目 **5/21 = 23.8095%**、DI **9/14 = 64.2857%**。各 family の暫定 20% 上限を超え、技術監査は **COMPLETED / EXCEEDED**。14 項目は局所診断済みで、14 個の独立バグや false relation を意味しない。 |
| **基準超過・制約を確認済み** | v0.1、公開 UI での `ArticleController` | [Clipboard Context 記録](REAL_REPO_ACCURACY.md#公開-canvas--clipboard-context) | 関係 item は outgoing 27/27・incoming 2/2・direct method 11/11 を保持。一方、詳細 edge evidence **29/29** と局所 Diagnostic **11/11** の本文は 32,000 code points 上限内で省略。省略数・scope / unknown 計数は残る。利用者が省略を理解し source へ進めるかは示さない。 |
| 確認済み | #27 の fixture と同じ固定実 repo、別 commit・Apple M4 | [性能記録](../POC_BENCHMARK.md#判定解釈再実行) | 小規模 2 profile の Analyzer / Web は事前の暫定 guardrail 内。大規模 repo、別環境、長時間操作、一般的な試用性能は保証しない。 |
| 未実施／未確認 | [Phase A](../POC_VALIDATION.md#phase-a-fixture予備ux)、固定アプリ `e0a139a6…` / A-draft-1 | [計画・空の結果欄](../POC_VALIDATION.md#known-limitationsデータ管理結果)、[進行者記録](FIXTURE_UX_FACILITATOR.md) | 承認 **PENDING**、実評価 **NOT_RUN**、初見参加者 **0**。リハーサルと正解表は人間の回答・得点・理解時間ではない。 |
| 未実施／未確認 | v0.1、固定実 repo と初見対象者 | [#30 受入条件](https://github.com/albert-einshutoin/CodebaseCanvas/issues/30)、[PoC 計画](../POC_VALIDATION.md#phase-b-実repo精度比較性能e2erelease-gate) | 実 repo の人間理解課題、source-first 比較、正答率と時間、誤った経路理解、Context 省略の理解は未評価。利用者価値の成功・失敗は判定不能。 |
| 未実施／未確認 | 最新 main `c97bccb…` と公開製品 | [main CI run 36384430444](https://github.com/albert-einshutoin/CodebaseCanvas/actions/runs/36384430444)、[公開 UI 記録](REAL_REPO_ACCURACY.md#公開-canvas--clipboard-context) | main の build/test は成功。最新 main での実 repo 監査、公開 version の変更、公開先の再操作は未確認。 |
| 仮説・提案 | [Issue #68](https://github.com/albert-einshutoin/CodebaseCanvas/issues/68)、将来の view | #68 本文 | I/O Flow View が可読性・理解時間を改善するという仮説。画面案、比較、評価は今回行わず、現行 Canvas の問題が確定したことも示さない。 |
| 仮説・提案 | PR #66–#67、実験的 v0.2 層 | [設計](../design/TYPEORM_DECLARATION_CONTRACT.md)、[契約草案](../DATA_MODEL_V02_DRAFT.md) | `repository_request` の wire/validator は後続の材料。producer・依存 profile reader・本番 import / Canvas / Context は未接続で、旧監査の超過を解消した証拠ではない。 |

## 暫定監査基準と継続判断

20% はこの**既知 profile に限る暫定技術監査**の source 項目未解決率上限である。詳細 Actual 照合前に source-first 台帳 v6 と基準を固定してレビューした履歴はある。一方、repo は #7 / #27 で既知で、未観測 holdout ではない。Actual 後に v6→v7 の method 位置訂正と判定文言の明確化があり、root `tsconfig.json` の入力同一性も post-Actual に追補された。これらの来歴を残したまま、分母・family 分類・上限・技術判定は変わっていない。承認済みの**最終 release 基準**として 20% が記録されたわけではないため、「最終試験に不合格」とは言わない。暫定基準だからといって超過を合格扱いにもしない。

技術監査の結論は **EXCEEDED**。現在 **GO** と v0.1 完成を認定する証拠は揃っていない。人間の価値評価は **NOT_EVALUATED**。以上から、継続方針の第一案は **REVISE** とする。未評価でも、現行の課題設定・評価前提のまま自動継続しないという判断案は作れる。正式な #30 Decision は承認と未実施評価を経るまで更新しない。**GO WITH FIXES** は「価値は確認済み」という #30 の条件を現状では満たさない。

## 何を維持し、何を止めるか

決定論的な事実抽出、推測 edge を作らない局所 unknown、ローカル Graph、根拠付き Canvas / bounded Copy Context、構造回帰、fixture から production File 入力までの E2E、Static Assets 配信は維持する技術成果である。今回の判断でその価値をゼロと見なさない。

現状では **v0.1 完成、M2 / M3 exit、利用者価値の実証、GO、完全な request→DB 経路、一般的な NestJS 対応率**を認定しない。特に supported edge の正確さと、未解決 source 項目が多いことは両立する。Module の dynamic entry と DI の TypeORM / 外部 token が利用者の仕事でどれだけ重要かも、人間評価なしでは決められない。

TypeORM producer や別構文を、未解決件数を減らすことだけを目的に自動継続しない。#68 の I/O Flow View も、現行 Canvas の問題が確定した改善策として自動開始しない。新契約の異なる意味・分母を使って旧監査を合格化しない。まず、初期利用者が変更前に必要とする構造、許容できる gap、unknown から source へ進む方法を確かめ、**表示上の問題・解析範囲の不足・課題自体の価値不足**を分ける。この段階で解析拡張と UI 変更の優先順位は選び切れない。

この結果だけから、製品価値がない、TypeORM が唯一の原因、I/O 表示で解決する、別 repo なら合格する、とは言えない。現行 v0.1 の能力を実行時呼出しや DB I/O の保証にも拡張しない。

## #1 DoD と #30 受入条件への対応

ここでの「確認済み」は上の対象 commit / profile の証拠であり、GitHub の checkbox を更新するものではない。#1 の古い未チェック欄だけで実装未完了とも判断しない。

| 要件群 | 現在の対応 | 残る境界 |
|---|---|---|
| [#1 DoD](https://github.com/albert-einshutoin/CodebaseCanvas/issues/1): 実 repo CLI、Graph 保存、version 検証 | **確認済み**。固定実 repo の v0.1 Graph と正規 validator 通過 | 他 repo / 新 main での再実行は未確認 |
| #1: node 種別、requested DI / import、Diagnostic、source evidence、Canvas 操作、Details、Copy Context | **確認済み**。fixture E2E の focus と固定実 repo の公開 UI での検索・Details・Copy Context を含む技術操作の範囲 | 公開実 repo での focus 操作、unknown と省略の人間理解、runtime 注入は未確認 |
| #1: fixture 構造回帰、current Rust → fresh fixture E2E、Rust / Web / E2E CI | **確認済み**。#26 経路と対象 main CI 成功 | CI を実 repo 監査や公開再確認へ拡張しない |
| #1: 実 repo 性能、Static Assets 配信 | **確認済み**。#27 の 2 profile と固定製品の公開操作 | 性能一般化・最新 main の配信は未確認 |
| #1: real repository validation で PoC 継続判断 | **未達**。本書は REVISE のレビュー案 | #30 の正式 Decision、承認、利用者評価が未了 |
| [#30 受入条件](https://github.com/albert-einshutoin/CodebaseCanvas/issues/30): 観測前固定、実 repo 解析、false / missing / unsupported 分類 | **一部確認済み**。既知 profile の詳細 Actual 前に v6 を固定し監査。post-Actual 訂正も記録 | 未観測 holdout ではなく、最終 release・人間評価の事前固定は未完了 |
| #30: Canvas の人間課題、source 確認、usability、正答・所要時間・source-first 比較 | **未達**。Phase A も Phase B も人間実評価 NOT_RUN | 初見参加者 0、課題結果なし |
| #30: critical false 0 / 欠落・unknown 上限判定、性能、文書 | **一部確認済み**。固定集合の false / missing 0、性能暫定 guardrail 内、計画と監査文書あり | Module / DI は暫定未解決上限 **EXCEEDED**、最終基準未承認 |
| #30: GO 等の最終判断、blocker 整理、最終対象 CI、#1 DoD 最終確認 | **一部確認済み**。本書で blocker と REVISE 案を整理し、`c97bccb…` の main CI 成功を確認 | 正式判断・release 対象の人間評価・DoD 最終認定は未達。PR #67 の実験層を製品合格に算入しない |

## 再判断に必要な最小の証拠

最初の問いは、**初見の対象エンジニアが、現行 v0.1 の確認済み関係と unknown を正しく区別し、変更前に必要な source へ進めるか**である。失敗時は、次の区別を回答・操作・根拠から行う。

ここで「source へ進める」は、現行の Details / Context にある `file:line` 等の根拠から必要な source を特定し、利用者のローカル editor 等で確認することを指す。アプリからの自動ジャンプ、IDE 連携やコード展開の実装を評価の前提にしない。

| 観察したい区別 | 再判断に必要な記録 |
|---|---|
| 表示された情報を正しく読めない | 関係の方向、requested token、unknown をどう回答したか。誤った実行経路理解も残す |
| 読めるが、必要な関係が不足する | 質問に対する独立 source 正答と、Graph の既知関係・局所 unknown・欠落を対照する |
| 根拠や unknown に到達しづらい | 検索・Details・source location への経路、到達可否、時間と介助を記録する |
| Context 省略を誤解する | `ArticleController` 等で省略通知・scope 計数と詳細本文の差を説明できるかを確認する |
| 対象課題に十分な価値がない | 正答率と所要時間を、比較可能な source-first 課題とともに記録する |

既存の Phase A A-draft-1 は、固定 fixture 上で関係・unknown・Copy Context を読めるかの**予備**課題と進行者正解を用意している。承認 PENDING のままで、結果欄に回答・点数・時間はない。fixture の予備結果を実 repo の理解や source-first 優位へ転用しない。実 repo では対象者の仕事に沿う課題と独立 source 正答が別途必要である。今回、新課題集の作成や参加者なしの予想採点は行わない。

正式判断・比較の前に、評価対象 commit/profile、質問と正答、採点・制限時間・適格性、比較方式と順序を承認・固定する。実装者・既知回答者と初見参加者を分け、正答率と時間の両方、誤った経路理解、人数不足・未回答・時間切れ・介助の扱いを記録する。技術側も旧監査を改変せず、新しい評価版・対象範囲・分母・上限を観測前に明示する。その結果で、現行 view の表示を直すのか、抽出範囲を変えるのか、対象課題を改めるのかを再判断する。進行を止める理由は**暫定監査の超過と、この分岐を選ぶための人間証拠の欠如**であり、無期限の保留を提案するものではない。

#68 を後に検討する場合は、#30 の REVISE 理由と検証する課題を先に対応付け、現行 v0.1 の immutable な view projection を優先する。TypeORM v0.2 を無条件の前提にせず、requested token を call、Module 所属を runtime 順序へ変えず、欠けた区間を推測 edge で埋めない。現行 Canvas との比較前に質問と正答を固定する。#68 の設計・試作・比較は今回の判断資料に含めない。

## 文書の時点と残る不整合

[PoC Validation](../POC_VALIDATION.md) の Phase A にある「#17 未接続」「本番 analyze 非0」は、固定アプリ `e0a139a6…` を対象に計画した**当時の開始条件**である。最新 main の仕様説明として読まない。評価対象を新 main へ自動差替えしない。[Deployment](../DEPLOYMENT.md) の「実 deploy / 公開後確認 NOT_RUN」も #28 作業当時の記録であり、後の固定製品の公開操作は監査記録にある。これらの履歴本文は今回書き換えない。

本書は既存監査・CI・公開記録の読み取りに基づく文書レビューである。技術監査の分類・分母・閾値・結果、Phase A の計画・承認、GitHub Issue の Decision、公開製品を変更しない。
