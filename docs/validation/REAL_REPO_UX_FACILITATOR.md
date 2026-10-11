# 実 repo 評価 — 進行者専用

版 B-prep-1 / 実施承認 PENDING / 人間評価 NOT_RUN。参加者にこの文書を配布しない。
進行・割当・失敗扱いは [計画](REAL_REPO_UX_PLAN.md)、環境は [準備手順](REAL_REPO_UX_ENVIRONMENT.md)。結果・承認はこの空票を repo 外へ copy して記入する。本文の空欄を予想回答で埋めない。

## 独立した正解の来歴

入力 commit `c1c2cc4e448b279ff083272df1ac50d20c3304fa` の `git show COMMIT:path` / archive 内の source を直接確認した。下記は repo 相対、1-based 行。定義の先頭に decorator がある method は decorator 先頭と method名の行を区別する。課題の正しい静的事実を Analyzer 出力から逆生成していない。

| 固定 source | raw bytes SHA-256 |
|---|---|
| src/article/article.module.ts | `abec4cdb1fa1a737ff4e377b6ed9c27b15333d530a6899ea1cbd51ad18a27ee4` |
| src/article/article.controller.ts | `934332b068082f30a02fe3e48f7b40d59be998b16468c6c5e5293a0c83f48dfb` |
| src/user/auth.middleware.ts | `ace80f14badc8e7cc28d3238ddf295a3bfef46a0cf1745bad7021ee07986f371` |
| src/user/user.module.ts | `f0739420f41b4b1b80ad4ee81cc2a8aed4f678c7ad403445df1596a698983cc3` |
| src/profile/profile.service.ts | `74765f590496482e5e01e03bb260771222fb59ed990d1d2ec99ad3f6b8c233a6` |
| src/profile/profile.module.ts | `21b2abc56b4ef91f52af307e64abead9376de91f7a84f25e4652d88d6dc24aab` |

比較はこの限定集合であり全 repo の理解を証明しない。既存台帳 `real-repo-source-v7.json` は source全35件の identity 検査に再利用するが、保存 Actual / UI は人間正解の代用品にしない。U2 の「どういう情報を省略する製品か」は固定**製品 source と契約**が正本。具体的な省略数・元解析時刻は実行時表示の読取り項目で、対象 repo の事実の正解とは別に記録する。

## 正解・採点表

各問の a/b/c 各1点、計3点。正しい項目を根拠付きで答えた場合だけ加点。不明を適切に限定する回答も正答。0件や未表示を根拠に不存在/完全性を断定するなど、正しい断片と矛盾する誤断定を含む回答は問全体0点。hex ID の転記・英語 code の暗記は不要。path と該当範囲内の行で同じ宣言に到達できれば許容する。

| 問 | 独立 source による正答・各1点の条件 | 誤答・重大誤認 |
|---|---|---|
| Q1 | **a** 直接 Controller=`ArticleController`、Service=`ArticleService` の両方と根拠。**b** `ArticleModule` が `UserModule` を静的 import する関係を、直接登録/lexical所属と区別。`UserService` は `UserModule` の provider/export で、ArticleModule の直接 provider と加算しない。**c** 所属の確認先 `src/article/article.module.ts:12–18`（provider14、controller15–17）、import13、追加確認 `src/user/user.module.ts:8–15` を同定。 | 画面の近接から所属を推測。UserService を ArticleModule の直接登録と断定。TypeOrm動的呼出しを確定Moduleとして加算。 |
| Q2 | **a** Controller=`ArticleController`、instance handler=`findAll` の両方。**b** `@Controller('articles')` と `@Get()` から宣言 `GET /articles`、実行時到達/実行/Service呼出しの保証ではない。**c** `src/article/article.controller.ts:17` と `:22–26`（Get24、method名25、宣言全体開始22）を確認先に示す。 | feedやslugを混同。route/handlerを実行済み・DB到達済みと断定（重大 M4）。 |
| Q3 | **a** `AuthMiddleware` constructor が local class token `UserService` を要求、確認先 `src/user/auth.middleware.ts:7–11`（constructor11）。**b** requested token は container が返す実装/instanceや実行経路の保証ではない。固定 source は `UserService` の通常登録を示すが、この静的調査で実行時注入を証明していない。**c** 登録を追加確認する先 `src/user/user.module.ts:8–15`（providers10、exports14）を示す。Canvas は UserService を選択→membership の UserModule→source location から同定できる。 | DI edge から実装解決・runtime保証と断定（重大 M1）。AuthMiddleware 自体を Module provider と断定。 |
| U1 | **a** 外部 origin の `@InjectRepository` が v0.1 で未解決と説明、ProfileService の constructor位置を示す（code `unsupported_di_decorator_origin` の意味を言葉で説明すれば可）。**b** 未表示/unknown は依存不存在ではないと説明。**c** `src/profile/profile.service.ts:12–17`（decorator13/15）と `src/profile/profile.module.ts:10–17`（TypeOrm forFeature11）を追加確認先とする。固定 source は UserEntity/FollowsEntity の decorator 引数と Repository 型 parameter の2つを宣言。Canvasだけでentity名を言えなくても減点しない。 | 「DIがない」「Repositoryが存在しない」と断定（重大 M2）。Entityをresolved class provider tokenと断定（M1）。 |
| U2 | **a** ArticleController の Context を独力取得しローカル貼付けで確認、対象・元 analyzedAt を読める。**b** 関係item保持と個別根拠/診断省略を区別し、Truncationの該当数を読める。全sourceの確認資料としては不足、Detailsの根拠/固定sourceへ戻る必要を説明。**c** graph-wide scope=`parsed_named_class_methods` / mode=`same_class_only`、局所と全体のunknown計数を区別し、calls0/errors0を完全性としない。確認先 `src/article/article.controller.ts:17–26` 等を示す。 | 省略を不存在と解釈（M2）、0 error/7 emittedを完全解析・正答率と解釈（重大 M3）、省略なしの全source/実行経路保証（M4）。 |

Q1: 固定 source の ArticleModule providers/controllers は上記各1、imports は動的 TypeOrm と静的 UserModule。TypeOrm 展開先を module の確定関係へ推測しない。
Q2: source の findAll 本体26行には `this.articleService.findAll(query)` があるが、v0.1 の requested DI/calls はこの receiver の実行先を保証しない。
Q3: 固定 source 内の useClass override 差はこの例にはない。「実装先と違う」という架空の override を作らず、要求の静的事実と実装解決の証明の差を評価する。
U1: 型 `Repository<UserEntity>` の字面を class token と扱わない。TypeORM の実 token命名やruntime DB接続は本キットの採点範囲外。

U2 の独立意味正本は製品 commit `b48df42…` の `apps/web/src/contextMarkdown.ts`、`apps/web/src/CopyContext.tsx`、[契約/仕様](../CANVAS.md#copy-context-24)。input source の ArticleController は11 methodの宣言を持つ（22/30/38/43/48/56/65/73/81/90/98行から開始）。Context は source全文ではなく、選択と固定した最大depth2の三経路だけ。上限は各section50 items、関連200 ID、文書32,000 Unicode code points。node/edge evidenceは別、confidenceは各根拠の値、解析日時はexport日時ではない。

初回準備記録 U2 の表示参照値（人間の回答ではなく、原本ログは現在消失）: outgoing 27/27、incoming 2/2、direct methods 11/11、node evidence 41中4掲載/37省略、edge evidence29中0掲載/29省略、局所diagnostic11中0掲載/11省略、file notices3中0掲載/3省略、field shortening62。全体141 examined/7 emitted/134 skipped、局所11 diagnostics/11 skipped、時刻 `2026-10-10T16:13:01Z`。再生成した場合はそのsnapshotの時刻/hashを先に封印し、表示参照値を別の準備記録へ保存する。意味・採点・閾値は変更しない。予期しない構造差なら開始せず診断を残す。

新しい技術確認の時刻・graph/Context識別子と永続証拠は [納品時の再確認](REAL_REPO_UX_ENVIRONMENT.md#納品時の再確認) に記録する。旧時刻やhashを新しい生成snapshotへ転記しない。質問・採点条件は不変。

## 重大誤認と採点後の記録

M1=要求DIから実装/runtime保証。M2=unknown/省略/未表示から不存在。M3=0件から完全解析、call-site計数から全repo正答率。M4=宣言route/static relationから実行済み経路/DB到達保証。各問のflagと根拠発言を記録し、他問の点で相殺しない。根拠のない経路理解、介助、UI不達、source位置不達、解析範囲不足、主観的価値不足を別の観察分類で残す。

## 承認票テンプレート

以下を repo 外の access 制限された記録先へ copy。現在の承認者・日時・hash欄は空欄。承認対象の全文を再生成/改変せず、次を記入する。

```text
kit_version: B-prep-1
kit_manifest_sha256:
approver:
approved_at_JST:
delivery_commit:
product_commit: b48df42d61d43b7389c191346282ecfdde52529a
input_commit: c1c2cc4e448b279ff083272df1ac50d20c3304fa
graph_sha256:
analyzedAt:
plan_and_scoring_approved:
allocation_and_order_approved:
limits_missing_assistance_approved:
environment_and_training_approved:
consent_storage_retention_sharing_approved:
known_technical_EXCEEDED_acknowledged:
record_directory:
```

承認対象は4文書・POC参照・固定契約・使用helperとsource/config台帳の [manifest](REAL_REPO_UX_SHA256SUMS.txt)。ルートで `shasum -a 256 -c docs/validation/REAL_REPO_UX_SHA256SUMS.txt`、次に manifest 自体のSHAを承認票へ記録。資料の納品SHAはPR最終HEADまたはmerge commitから取得し、承認票のdelivery_commitへ記録する。評価対象product_commitとは区別し、文書へ自己参照SHAを埋め込まない。

## 空の記録票 — NOT_RUN

人間参加者数0。以下の列定義・空行はテンプレート。P番号や得点・秒・発言をリハーサルで埋めない。

参加者票（1人1行、同意/氏名対応は別file）:

```csv
anonymous_id,TS_years,NestJS_years,handover_count,AI_days_per_week,graph_experience,prior_knowledge,eligibility,consent_ref,pair,mode,question_order,environment_ref
,,,,,,,,,,,,
```

共通Q票（開始後は各人3枠・全18行）、U票（Canvas各人2枠・全6行）:

```csv
anonymous_id,question_id,mode,started_at,elapsed_s,answer_frozen,evidence_path_line,unknown_statement,point_a,point_b,point_c,total,critical_flags,critical_quote,timeout,assisted,missing_reason,success_time_s,copy_mode,observation_category
,,,,,,,,,,,,,,,,,,,
```

追加票（独力結果とは別）:

```csv
anonymous_id,question_id,assistance_level,assistance_words,extra_seconds,answer_after_assistance,feedback,recording_consent,sharing_consent
,,,,,,,,
```

集計票（数字・判定空欄を維持）:

| 欄 | 値 |
|---|---|
| execution_status | NOT_RUN |
| participants / valid pairs / missing slots | |
| Canvas common points /27, full correct /9 | |
| source common points /27, full correct /9 | |
| Canvas individual common /9 and U /6 | |
| critical flags /独力Context取得/自動copy/手動取得 | |
| all raw elapsed / all success_time / pair reductions / median | |
| consent withdrawal / excluded reasons / unavailable slots | |
| proposed UX criteria / comparison criteria verdict | |
| interpretation / limitations / required next decision | |

復唱・介助後の正答で空欄/独力回答を上書きしない。未実施 NOT_RUN、観測不足 NOT_ASSESSABLE、基準未達を分ける。集計結果を #30 の正式 Decision として自動転記しない。
