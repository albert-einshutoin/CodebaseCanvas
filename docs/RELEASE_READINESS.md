# v0.1 リリースまでの進め方

Issue #30 の準備文書。最終評価の正本は [#30](https://github.com/albert-einshutoin/CodebaseCanvas/issues/30)、必須スコープと依存順は [Epic #1](https://github.com/albert-einshutoin/CodebaseCanvas/issues/1) と各 Issue とする。本書の追加では #30 を完了しない。

## 現在の判定

2026-09-08、main `6e36caf` のソースと GitHub の Issue/PR を確認した時点で、v0.1 の公開判定は保留。CLI は #17 の解析接続待ちで非0終了し、graph を生成しない。Web は起動画面の段階で、import、Canvas、Copy Context、Rust-to-browser E2E は未実装。実利用者評価、性能測定、静的配信の証拠も本書では取得していない。

開発基盤・契約・fixture・基本CI・CLI保存境界・discovery・Oxc能力確認・GraphBuilder・汎用TypeScript抽出は main に実装がある。Issue が open のままの項目もあり、実装の存在、受入条件の達成、Issue close は個別に確認する。過去PRの成功を最終リリース対象commitの検証結果に転用しない。

## ローカル main の dirty を扱う

調査時のローカル main は初期commit `bf3176e` で、追跡済み9ファイルと未追跡21ファイルがあった。内容は主に #2/#3 の段階の開発基盤・契約。例えば `docs/DATA_MODEL.md` は #3 の `7447503` と一致する一方、ローカルのパスvalidatorには main にある C1 制御文字拒否が含まれない。全ファイルが既存commitと同一であることは未確認。

1. 作業中ファイルはそのまま保護し、最新 origin/main から隔離worktreeでPRを作る。
2. dirtyを取り込む必要がある場合は、各ファイルを最新mainおよび対応する過去commitと比較する。既に反映済みの内容と後続修正前の旧版は再投入しない。
3. 未反映の有効な変更だけが見つかった場合、その目的に対応する1 IssueのPRで扱う。
4. 元worktreeの整理は別工程。バックアップの内容と復元可能性を確認し、変更破棄の明示許可後に同期する。

本PRは元worktreeの変更破棄・同期を行わない。

## 実装と検証の順序

各行は成果の区切りであり、一度に実装する作業単位ではない。各Issueの依存を確認し、1 taskで1 Issueを進める。

| 段階 | 関連Issue | 次へ進む証拠 |
|---|---|---|
| 初回の利用フロー | #19 → #20 → #21 → #22 → #23 → #25 → #24 | 手定義fixtureを選択し、全体像・検索・根拠・unknown・Contextを確認できる。fixture UX評価として記録する |
| NestJS解析 | #13 → #9 → #10 → #11 → #12（#8はmainに実装あり） | 対象構文の抽出と未対応診断を各Issueの対象テストで確認する |
| 完全なgraph出力 | #14、#15 → #17 → #18 | callsのunknown件数とPrismaを含む現行必須スコープを統合し、独立した期待値で構造回帰を確認する |
| 配信と品質 | #26、#27、#28 | 現行Rustから生成したgraphのproduction import/E2E、性能測定、静的配信の証拠を揃える |
| 最終判定 | #30 | 事前基準、実repo精度、利用課題、性能、最終commitのhosted CIと配信を照合する |

次の実装候補はEpicの推奨順に沿った #19。#29 Healthは任意で、v0.1の公開条件には加えない。

## 観測前に固定する評価

#30 に従い、実評価前に `docs/POC_VALIDATION.md` へ対象repo/commitと選定理由、重要関係の正解表、質問・正答・時間制限、source-first比較方法、欠落/unknown上限、#27の性能上限を具体的な数値で固定する。#7 の [能力確認](OXC_CAPABILITIES.md) の対象を引き継ぐか、変更理由を残す。

本書は評価対象や数値基準の確定を代行していない。この準備が終わるまで本評価を開始しない。少人数の初見利用者について人数と経験を記録し、学習効果を抑えた比較で正答率と所要時間を測る。Module/DI/Endpointの評価集合でfalse relationは0必須。supportedの欠落とunsupported/unknownは別集計し、結果を見て集合や閾値を緩めない。

## 公開判断時に残すもの

- リリース候補のcommit、対象repo/commit、解析日時、再現コマンド、既知の未対応範囲。
- #18の構造回帰、#26の現行Rust buildからfresh graphを生成するE2E、最終commitに対する #31 hosted CIのURLと結果。`pnpm run ci` が完全検証入口で、E2E接続は #26 の責務。
- #27の時間・peak memory・graphサイズ・UI load、#30の精度・理解課題・Copy Context評価。
- #28の配信URLと対象commit、初回利用フロー、graph/sourceがアップロードされないことの動作確認。
- GO / GO WITH FIXES / STOPまたは証拠不足の判定と、未達条件。GO WITH FIXESは修正・再検証前のv0.1完成を認めない。

証拠が揃ってGOになった対象のみ、公開工程の依頼に従ってmerge後のmain CI、配信対象との対応、READMEのコマンド・制約・配信URLを再確認する。タグやGitHub Releaseを作成する場合も対象commitと公開内容を明示する。本PRではmerge、tag、release、deployは実施しない。
