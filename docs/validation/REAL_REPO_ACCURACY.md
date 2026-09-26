# Issue #30 第1工程: 固定実 repo の技術監査

実施日: 2026-09-27 JST（記録中の `2026-09-26T...Z` は UTC）。技術判定: **EXCEEDED**。これは既知 profile に対する暫定的な accuracy / unknown / Copy Context 監査であり、#30 の最終 Decision は **NOT_EVALUATED**。初見参加者の UX 評価、source-first 比較、release 判断は実施していない。

## 固定対象と既知性

| 対象 | 固定値・確認 |
|---|---|
| 製品 repository | `albert-einshutoin/CodebaseCanvas` |
| Analyzer / Web commit | `434bf36a815232f88fc157dd1c4a0df5915030da` |
| 製品 tree | `894b669e1beb8cb1a21c8b78c8c3e7e5fb005f71` |
| 評価資料 branch | `codex/issue-30-realrepo-accuracy` の専用 worktree。製品 source は固定 HEAD のまま評価資料と helper のみ追加 |
| 保護した元 checkout | HEAD `cd04fc78c894175937fa9831901a0195b5141a0f`。checkout / reset / commit なし |
| 入力 repository | `https://github.com/lujakob/nestjs-realworld-example-app.git` |
| 入力 commit | `c1c2cc4e448b279ff083272df1ac50d20c3304fa`。隔離 clone は detached HEAD・clean。入力の依存 install / script / server / DB / submodule は実行していない |
| 入力 snapshot | 上記 commit の `git archive` から隔離解析 copy を作成。対象 TS/TSX 35 file の SHA を台帳と照合。source manifest SHA-256 `47b4d73861771f4bae81502be1d541dd09991cddf23e04dc738d558ddf45c405` |
| 公開 Web | [CodebaseCanvas](https://codebasecanvas.einstein-4s-1110.workers.dev/)；指定 Version ID `3ef69354-93bb-488f-965a-3a18b1350e13`。この ID 自体は公開 asset 応答から独立に取得していない |

これは未観測の holdout / 盲検ではない。[#7 固定 profile と source probe](../OXC_CAPABILITIES.md)で 35 TS/TSX を把握し、[#27 の既存計測](../POC_BENCHMARK.md)で 164 nodes / 385 edges / warning 116、calls examined / emitted / skipped = 141 / 7 / 134 を観測していた。#7 の担当者は source 構文を、#27 の担当者は集計と raw record を確認済み。今回の担当者もこれらの既存文書を台帳作成前に知っていた。一方、今回の詳細 expected relation / unknown 台帳は固定 source と固定 commit の契約から作成し、今回の Actual Graph や #27 raw Graph を expected 値の生成元にしていない。今回の照合だけで #30 の「観測前固定」や独立評価を全面達成したとは扱わない。

## source-first 台帳と基準

正本は [source-v7](real-repo-source-v7.json)、固定記録は [freeze](real-repo-freeze.json)。全 35 file の path / SHA / scope と、class・interface 36、名前付き method 59、Module metadata 22、constructor parameter 14、route-bearing handler 21、named class method 本体の call site 141 を source 位置・canonical ID・期待 relation / Diagnostic に結び付けた。Module の `exports` 1 entry は記録のみで採点対象外。top-level test call は既存 calls scope 外。Prisma schema は 0 件で **N/A**；TypeORM の利用を Prisma `database_model` とみなさない。[#15 / #18 の fixture](../OXC_CAPABILITIES.md)は別証拠である。

詳細出力を見る前の専門レビューで source-v6 は **ALLOW**。固定日時 `2026-09-26T15:25:39.474830Z`、台帳 SHA-256 `78d2ae4de12c11bb943f6cfe2654fb3a23b280f6364ce96f9d95b4fd361d20bd`、入力 manifest SHA は上表の値。レビュー対象は固定 source と契約、source 由来の分母・call group で、Actual / #27 raw Graph は参照していない。v1–v5 はこのレビュー前の作業草案で、method identity の追記、call reason group / 分母式 / pass-fail 条件の明文化、一意な call target 数と edge 数の訂正を経た。納品する観測前版はレビュー済み v6 とし、草案全文は重複するため同梱しない。hash と記録日時は独立した時刻証明ではない。

Actual 照合後、method 23 件の行位置に**台帳側の source 解釈誤り**を発見した。v6 は method 名の位置を開始位置としたが、契約上は method 宣言全体の開始位置であり、decorator があればその先頭行になる。固定 source に対する TypeScript parser の `method.getStart()` と、固定 Rust の `method.span.start` を独立再確認した。v7 では `METHOD-028–086` の59項目に `nameLine` / `nameByte` を明示し、58項目の開始 byte、23項目の開始行と期待 node / evidence 行を訂正した。残る同一行の byte 訂正も隠さない。さらに Actual に同一 code/file/line の import 診断があったため、call 診断の重複禁止と、別名 import が同じ行にある場合は非call診断の同一キーだけで重複 emission と断定しない扱いを criteria 文に明記した。**この文言明確化も Actual 後の変更**であり、観測前固定事項に含めない。旧 [source-v6](real-repo-source-v6.json)、[freeze-v6](real-repo-freeze-v6.json)、[v6 照合結果](real-repo-result-v6.json)を残す。v6 結果の node 行不一致 23 件は訂正後 0 件。family 分類・relation target・分母・数値閾値・Actual Graph は不変で、Module / DI の EXCEEDED は両版で同じ。v7 は **post-Actual source 再確認版**であり、観測前レビュー済み版と同一視しない。v7 の SHA-256 は `01c834c700dceff889a1458e46c32586ef0d01bdcacdad7187ee892ab47ecc20`、記録日時は `2026-09-26T15:28:18.422519Z`。行が変わった23項目の旧→新は次表。全59項目の byte / name 位置は両版の同じ `itemId` で照合できる。

| file | 項目と旧→新の開始行 |
|---|---|
| `src/app.controller.ts` | `METHOD-028` 6→5 |
| `src/article/article.controller.ts` | `METHOD-029` 25→22、`030` 34→30、`031` 39→38、`032` 44→43、`033` 52→48、`034` 60→56、`035` 69→65、`036` 77→73、`037` 85→81、`038` 94→90、`039` 102→98 |
| `src/article/article.entity.ts` | `METHOD-040` 30→29 |
| `src/profile/profile.controller.ts` | `METHOD-054` 19→18、`055` 24→23、`056` 29→28 |
| `src/tag/tag.controller.ts` | `METHOD-067` 18→17 |
| `src/user/user.controller.ts` | `METHOD-071` 22→21、`072` 27→26、`073` 33→31、`074` 38→37、`075` 44→42 |
| `src/user/user.entity.ts` | `METHOD-076` 29→28 |

暫定基準は台帳の `criteria` / `rules` に固定した。Module は `imports` / `controllers` / `providers` の各 entry、DI は constructor parameter、Endpoint は route decorator を持つ handler を未解決率の source 単位にする。動的式は展開数を推定せず 1 単位。同じ path の異なる handler は別単位。supported の欠落率は各 family の**一意な意味 relation**を分母とし、Endpoint は 1 handler 当たり Controller→Endpoint と Endpoint→handler の 2 edge を数える。false は source owner から余分に出た対象 relation を数え、正 target 欠落と誤 target 接続は missing と false の両方に計上する。`imports`、lexical method ownership、Module 所属から派生した display parent は別概念として検査する。

各 family を独立判定する: false relation 0、supported missing ≤5%、unsupported の scoped Diagnostic による説明 100%・silent omission 0、source 単位の未解決 ≤20%。unsupported も未解決分子に含める。分母 0 は N/A、未判定は NOT_ASSESSABLE。閾値はこの既知 profile の暫定監査用で、承認済み release 基準ではない。calls はこの 20% に混ぜず、static target の false 0、source site / edge / unknown group / evidence を個別に検査する。

## 実行と照合結果

固定 HEAD の Rust source から `cargo build --locked -p codebasecanvas-analyzer --bin codebasecanvas`（exit 0）を行った。`target/debug/codebasecanvas` の SHA-256 は `2b7bc205047f327d7d1bfd7eef098c971eea14323604cf7aa28cda7b0c907e82`。製品 source に評価 helper の変更はなく、binary は固定 commit/tree に対応する。新規隔離 snapshot を `target/debug/codebasecanvas analyze <analysis-root>` で解析し exit 0。出力は入力 snapshot 内の `.codebasecanvas/graph.json` にのみ保存し、公開 asset / 製品 `dist` へ置かなかった。

| 実出力 | 今回の値 |
|---|---|
| Graph SHA-256 | `b1d58bd695d34bf15c296e172781664492369a4bd7a99194c2fd7b7cf76e55f3` |
| `analyzerVersion` / `analyzedAt` | `0.1.0` / `2026-09-26T15:26:51Z` |
| CLI / Graph | TS/TSX 35、Prisma 0、164 nodes、385 edges、diagnostics info 0 / warning 116 / error 0 |
| calls metadata | `parsed_named_class_methods` / `same_class_only`、141 examined / 7 emitted / 134 skipped |
| fatal / 部分解析 | CLI 成功、Graph 正規 `SystemGraphSchema.parse` 成功、error 0。既存契約上の fatal / partial error は観測せず。warning / unknown は残る |

[評価 helper](evaluate-real-repo.mjs)は台帳 hash・35 source SHA を検証し、正規 validator を通した Graph の canonical ID / kind / direction / 意味 metadata / source line / evidence source / confidence、Diagnostic code・file・line・`relatedNodeId` と集計を照合する。余分な source 宣言 node と全 `calls` edge も失敗条件に含める。[機械可読の全結果](real-repo-result.json) の `provenance` に評価台帳・入力 manifest・Actual Graph の SHA と Graph metadata を記録し、`correctKeys`、`missingKeys`、`falseKeys`、`unresolvedItems` と台帳 `itemId` から各結果へ追える。

| family | source 項目 | supported 期待 edge | correct | missing | false | unsupported | 診断済み | silent | 保留 | 未解決 source 単位 | 判定 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| Module | 21 | 16 | 16 | 0 | 0 | 5 | 5 | 0 | 0 | 5/21 = **23.8095%** | **EXCEEDED** |
| DI | 14 | 5 | 5 | 0 | 0 | 9 | 9 | 0 | 0 | 9/14 = **64.2857%** | **EXCEEDED** |
| Endpoint | 21 | 42 | 42 | 0 | 0 | 0 | N/A（対象 0） | 0 | 0 | 0/21 = 0% | WITHIN |

supported missing は各 family 0%、unsupported 説明率は Module / DI とも 100%。declaration 36、named method 59、Endpoint node 21 の identity / metadata / source / evidence / display parent 照合エラーと、余分な宣言 node は 0。Module / DI / Endpoint の余分な source-owned relation、誤 scope、想定外 scoped Diagnostic、edge provenance エラーはいずれも 0。Graph の総 warning 116 を説明率として流用していない。

calls は source の 141 site を 7 supported / 134 unknown に分類。7 site は 7 **一意な edge**（一意な target 数とは別）として全件一致し、missing site / target・false target・evidence mismatch は 0。134 unknown は method＋reason の 76 group に集約され、group ごとの code / file / 代表行 / related method / skippedCount と合計 134 が一致した。内訳は injected receiver 67、higher order 10、unknown receiver 52、wrapped target 3、nested function 2。requested token を runtime call target へ変換していない。`TS_IMPORT_UNSUPPORTEDEXPORT` の同一 code/file/line 行は 3 group で複数出るが、それぞれ同一行に別の named DTO import が 2 / 3 / 3 個ある。wire 診断に specifier が無いため完全な一対一帰属はできず、これは候補として結果 JSON に残した。重複 emission と断定しない。call 診断の重複 group は 0。

### 閾値超過の source 根拠

- **Module 5/21**: `MODULE-096` (`src/app.module.ts:12`) の `TypeOrmModule.forRoot()` と `MODULE-102/106/110/114`（各 feature Module の `TypeOrmModule.forFeature(...)`）は固定 scope では動的 Module import。各 entry は `unsupported_module_dynamic` で Module ID に局所帰属し、推測した依存 edge はない。特に `src/profile/profile.module.ts:11` は `ProfileModule` の source 構造が部分的にしか表現されない例。暫定 20% を超えるため **blocker 候補**。原因として確定できるのは固定対応範囲とこの repo の TypeORM dynamic entry の組合せであり、runtime provider 構成への影響は未評価。将来 scope を変更するなら、この 5 entry の方向・target・unknown 診断と全 Module 分母を再検証する必要がある。
- **DI 9/14**: `DI-118` (`src/app.module.ts:24`) の外部 `Connection` と、`DI-120–123/125–126/128/131` の `@InjectRepository` parameter 8 件は固定 scope 外。例 `src/profile/profile.service.ts:13,15` の decorator は `@nestjs/typeorm` 由来で `unsupported_di_decorator_origin` として consumer class に帰属する。実装/provider/runtime 注入先は推測していない。暫定 20% を大幅に超えるため **blocker 候補**。将来対応するなら token の宣言・import 起源・decorator 起源・scope を独立に検証し、誤 `injects` と silent omission の回帰を防ぐ。

| 台帳項目 | owner | 固定入力の source 位置 | 必要最小限の source 識別子 | Actual Diagnostic code |
|---|---|---|---|---|
| `MODULE-096` | `ApplicationModule` | `src/app.module.ts:12` | `TypeOrmModule.forRoot()` | `unsupported_module_dynamic` |
| `MODULE-102` | `ArticleModule` | `src/article/article.module.ts:13` | `TypeOrmModule.forFeature(...)` | `unsupported_module_dynamic` |
| `MODULE-106` | `ProfileModule` | `src/profile/profile.module.ts:11` | `TypeOrmModule.forFeature(...)` | `unsupported_module_dynamic` |
| `MODULE-110` | `TagModule` | `src/tag/tag.module.ts:9` | `TypeOrmModule.forFeature(...)` | `unsupported_module_dynamic` |
| `MODULE-114` | `UserModule` | `src/user/user.module.ts:9` | `TypeOrmModule.forFeature(...)` | `unsupported_module_dynamic` |
| `DI-118` | `ApplicationModule` | `src/app.module.ts:24` | `connection: Connection` | `unsupported_di_external` |
| `DI-120` | `ArticleService` | `src/article/article.service.ts:16` | `@InjectRepository(ArticleEntity)` | `unsupported_di_decorator_origin` |
| `DI-121` | `ArticleService` | `src/article/article.service.ts:18` | `@InjectRepository(Comment)` | `unsupported_di_decorator_origin` |
| `DI-122` | `ArticleService` | `src/article/article.service.ts:20` | `@InjectRepository(UserEntity)` | `unsupported_di_decorator_origin` |
| `DI-123` | `ArticleService` | `src/article/article.service.ts:22` | `@InjectRepository(FollowsEntity)` | `unsupported_di_decorator_origin` |
| `DI-125` | `ProfileService` | `src/profile/profile.service.ts:13` | `@InjectRepository(UserEntity)` | `unsupported_di_decorator_origin` |
| `DI-126` | `ProfileService` | `src/profile/profile.service.ts:15` | `@InjectRepository(FollowsEntity)` | `unsupported_di_decorator_origin` |
| `DI-128` | `TagService` | `src/tag/tag.service.ts:9` | `@InjectRepository(TagEntity)` | `unsupported_di_decorator_origin` |
| `DI-131` | `UserService` | `src/user/user.service.ts:17` | `@InjectRepository(UserEntity)` | `unsupported_di_decorator_origin` |

全14行は観測前 v6 から `unsupported` で、v7 でも分類不変。実際の Diagnostic の code / file / line / `relatedNodeId` は [結果 JSON](real-repo-result.json) の各 family `diagnosedItems` と台帳の同じ `itemId` で照合でき、scope は表の owner の canonical class ID。Module については各 owner を起点とする `depends_on` / 非method `contains`、DI については `injects` の Actual 全集合を supported 期待集合と比較し、これら unsupported 項目から推測した余分な edge は 0。unsupported の target は未確定であり、Entity名や外部型名を resolved provider target に読み替えない。14件は主に同じ固定対応範囲に由来する source 項目で、14個の独立した実装不具合という意味ではない。

これらは誤関係や黙殺ではない。固定仕様どおりの unknown でも、利用者に提供できる主要構造量が暫定水準を満たさないという判定である。

## 公開 Canvas / Clipboard Context

固定 worktree の `pnpm web:build`（exit 0）後、公開応答と local build の SHA-256 を照合した。`index.html` は `0a1b7476…f29c215dda0`、`index-CCKFgRU0.js` は `9d8a3bc6…f34cc0f17d1`、`index-BGioPoxO.css` は `95728b91…45fc11db14` で、それぞれ全文 hash が一致する。公開 Version ID との対応はユーザー指定値であり、独立した Cloudflare API 照会はしていない。固定 commit の GitHub `CI` run `36226275264` は `success`（commit SHA を read-only 確認）。#28 は closed、#30 は open。CI green / 配信一致は accuracy 判定とは別証拠である。

公開 URL に対しローカル Playwright Chromium headless・1440×900・実 File input を用い、上記 Graph を選択した。`Graph loaded and validated`、Analysis 164/385/116 と calls 141/7/134、Canvas generation 1 / layout ready / 既定 projection 105 nodes・155 edges を確認。入力後の HTTP request 0、browser console error 0。Graph は browser の File API でローカルから読み込まれ、観測した通信に Graph upload はない。外部 LLM は使用していない。[再実行 script](check-real-repo-ui.cjs)は [固定 UI 記録](real-repo-ui-summary.json) と ID / Context hash・長さ / Canvas・Analysis 数 / 通信・error を照合し、不一致では非 0 終了する。今回の保存 Graph を使う厳密モードでは全 SHA を比較する。新規解析は `analyzedAt` で Graph / Context SHA が変わるため、明示 `--fresh-snapshot` ではその時刻依存 SHA だけ比較対象から外し、source/Graph は直前の評価 helper で別途固定照合する。

対象 3 件は Actual を見る前に source 台帳 `uiSelection` で固定した。検索結果から選択した canonical ID と Details の ID が一致し、node Evidence / edge Evidence、局所 Diagnostic / graph 全体 callAnalysis、元 `analyzedAt` と scope の区別を確認。各 Context は Copy ボタンから**実 Clipboard**を読み直した。以下の SHA は Clipboard 文字列そのもの。詳細 ID は [台帳](real-repo-source-v7.json) の `DECLARATION-003/020/013` と [UI 記録](real-repo-ui-summary.json) に保存した。

| source-first 対象 | 確認した意味 | Clipboard Context SHA-256 / 長さ | 省略と unknown |
|---|---|---|---|
| `ArticleController` (`src/article/article.controller.ts:18`) | `ArticleModule` 所属、宣言 route `GET /articles` 等、handler。Context の route と Details の node/edge evidence を確認 | `9d2b4bad2f11d2169d48d0ca67a1292cb92bbdd6cdee798954045ea17f583b38` / 28,638 code points | 32,000 上限の下で候補 edge evidence 29/29 と局所診断 11/11 の**詳細行**を省略。`Truncation` は candidates/included/omitted を明記し、Analysis scope / unknown の 11 件と skipped site 11 件は残る。Context 上で個別根拠をすべて渡せない制約であり、Analyzer の relation missing と二重計上しない |
| `AuthMiddleware` (`src/user/auth.middleware.ts:10`) | local `UserService` class token の **要求**を表示。runtime 実装先とは断定しない。`unsupported_call_injected_receiver` が局所 unknown として残る | `f2d7c7c795530de7550ef463a442a4efc8c33a20180e42821d8db14dad837b80` / 13,814 code points | 局所診断 9/9、edge evidence 8/8 を掲載。省略 0 |
| `ProfileService` (`src/profile/profile.service.ts:11`) | `ProfileModule` 所属、外部 `@InjectRepository` の `unsupported_di_decorator_origin` を Details / Context で確認 | `92f4515b4e6b8cb79d6b73c1e22e0a574a50fb3a7bdcae941f78ea5bfc40f068` / 25,343 code points | 局所診断 7/7、edge evidence 21/21 を掲載。省略 0 |

追加で `ENDPOINT-133` の宣言 `GET /articles` を検索し、endpoint ID / route / `ArticleController` / `findAll` と “Declared endpoint” 表示を確認した。これは runtime URL 到達や handler 実行の証明ではない。Context は対象・元解析日時・関係方向・掲載された根拠と confidence の帰属・unknown・省略理由を保持するが、長い対象では個別詳細が落ちる。`ArticleController` の省略は **非 blocking な Context 制約**として記録し、初見利用者が十分理解できるかは未評価。ブラウザ操作所要時間を人間の理解時間へ換算しない。

`ArticleController` の Clipboard 全文（SHA は上表）を追加で読んだ。関係 item は outgoing 27/27、incoming 2/2、direct method 11/11 を掲載し、related node ID は27/200。一方、個別の node evidence は41候補中4掲載・37省略、edge evidence は29中0・29省略、局所 Diagnostic は11中0・11省略、同一file noticeは3中0・3省略。いずれも Context の `## Truncation` で文字数予算による省略数を明示し、別に field shortening 62件を記録する。本文は28,638 code points、契約上限は32,000。関係 item の省略0、fieldのprefix短縮62、詳細根拠/診断行の省略は別の数である。`## Analysis scope / unknown` は selected node＋included methods の diagnostics 11 / skipped call sites 11、node/edge evidence confidence の掲載候補集計 confirmed 41/29 を保持するが、個別 edge evidence の source位置は Context からは読めない。省略通知・unknown保持は [契約](../DATA_MODEL.md)の上限と優先順位に沿い、現時点で契約違反は確認していない。省略された原因説明やsource位置の到達性が十分かは人間評価まで未確認とする。

## 再実行と確認範囲

Node 24.x（24.2.0 以上）、pnpm 11.8.0、Rust 1.96.0、Playwright Chromium を使用。以下は専用の空の一時 directory を `mktemp -d` で作る。入力 clone の `git status --porcelain` は空であることを確認し、製品 worktree は固定 HEAD/tree で実行する。source 台帳の全 SHA と manifest は helper が再確認する。入力の依存・script は不要。製品側の依存と Chromium は既存の `pnpm install --frozen-lockfile --ignore-scripts` / `pnpm web:e2e:install` で用意できる。

```sh
# 入力 clone / 新しい解析 snapshot。入力内の script は実行しない
audit_dir="$(mktemp -d)"
input_clone="$audit_dir/input"
analysis_root="$audit_dir/analysis"
ui_evidence_dir="$audit_dir/ui-evidence"
git clone --no-checkout https://github.com/lujakob/nestjs-realworld-example-app.git "$input_clone"
git -C "$input_clone" checkout --detach c1c2cc4e448b279ff083272df1ac50d20c3304fa
git -C "$input_clone" status --porcelain
mkdir "$analysis_root"
git -C "$input_clone" archive c1c2cc4e448b279ff083272df1ac50d20c3304fa | tar -x -C "$analysis_root"

# 以降は固定製品 worktree ルート
git rev-parse HEAD HEAD^{tree}
cargo build --locked -p codebasecanvas-analyzer --bin codebasecanvas
shasum -a 256 target/debug/codebasecanvas
target/debug/codebasecanvas analyze "$analysis_root"
shasum -a 256 "$analysis_root/.codebasecanvas/graph.json"
node --experimental-strip-types docs/validation/evaluate-real-repo.mjs \
  docs/validation/real-repo-source-v7.json docs/validation/real-repo-freeze.json \
  "$analysis_root/.codebasecanvas/graph.json" \
  "$analysis_root" "$audit_dir/recomputed-result.json"
pnpm validation:test
pnpm web:build
node docs/validation/check-real-repo-ui.cjs \
  "$analysis_root/.codebasecanvas/graph.json" \
  "$ui_evidence_dir" --fresh-snapshot
git diff --check
```

対象 helper test 13 件は欠落と別 target false の同時計上、freeze 後の unsupported 再分類拒否、Diagnostic 重複 / scope、分母 0・未確認、source 台帳からの集計、unknown group 位置 / 件数、余分な node / calls、入力 SHA 拒否、provenance、UI 不一致時の非 0 終了・fresh mode の比較範囲を対象にし、今回すべて成功。UI check は指定された公開 URL にのみアクセスし、Graph の実 File 入力と実 Clipboard readback を行い、full Context と画面記録を指定された一時 directory 側へ保存する。公開 asset の SHA 比較を先に実施すること。今回の技術操作を人間 UX 試験または hosted CI の追加試験と呼ばない。[#27 性能](../POC_BENCHMARK.md)は別 commit / 別測定の既存結果であり、今回の debug build / UI 操作から性能値を再計測していない。

専門レビューは二段階。source-v6 の観測前 oracle / 対応範囲 / 分母は Actual を見ないレビューで **ALLOW**。評価 helper の初回レビューは余分な node / calls、UI の非 0 判定、Graph provenance と回帰不足で **BLOCK**。これらを helper/test のみ修正し、影響範囲の再照合後、同じ指摘範囲の最終レビューは **ALLOW**。納品時の `validation:test` 接続・診断項目・報告訂正の限定レビューも **ALLOW**。製品 Analyzer / Web の source は変更していない。`git diff --check` は成功。Codex Companion の **Stop Review Gate は NOT_RUN**。この Codex desktop 作業では Claude Code `Stop` hook が呼び出されず、対象 worktree の Companion `stopReviewGate` 設定も false だった。2026-09-26 UTC に正規の手動 `codex:review --wait --scope working-tree` を同じ作業差分で一度起動したが、設定モデル `gpt-6-sol` が ChatGPT アカウントの Codex review に非対応という 400 エラーで終了し、判定は得られなかった。設定変更や代替モデル実行はしていない。専門レビューと失敗した手動起動を Stop Gate 実施と読み替えない。

PR #65 の初回 HEAD `c252102a8e717ed84b6a6cf18b81623eafb1c5f6` に対する GitHub の Codex 自動レビューは、評価 helper に P2 を 3 件指摘した。固定入力の `src` 外の TypeScript / Prisma schema 候補の見逃し、一般の error 診断を技術判定に反映しない点、台帳 item のない既知宣言 owner からの余分な family edge の見逃しである。3 条件を既存 13 test の回帰チェックに加え、修正前は 3 件失敗、修正後は 13/13 成功。helper は固定 Analyzer と同じ除外 directory / 対象拡張子で root 全体を inventory 照合し、symlink は推測せず失敗させる。error 診断を `errorDiagnostics` に保存して EXCEEDED とし、宣言台帳の family 別既知 owner からの余分な relation を false に数える。**台帳・閾値・分母・製品 source は不変**。保存済みの固定 Graph を再照合した結果は error 0、family と calls の集計・EXCEEDED は不変。これら 3 件の限定専門レビューは **ALLOW**。この修正は初回 HEAD の Hosted CI 後であり、最終 HEAD の CI を別途確認する。

納品差分で `pnpm run ci` は成功。Rust build / format / clippy / test（189 pass、1 ignored）、Web unit 163、評価 helper 13、Web typecheck / build、fixture typecheck、E2E 準備 helper 2、公開設定検査 1、Chromium E2E 5 を実ログで確認した。`pnpm audit` は独立して実行し、既知の脆弱性 0。これは納品前のローカル検証であり、固定製品 commit の過去 CI や PR 最終 HEAD の Hosted CI とは別の証拠である。通常 CI は外部実 repo の取得・技術監査・公開 URL へのアクセスを実行しない。

未確認: 独立の新 profile による観測前固定、初見人間参加者の正答 / 理解 / 時間、source-first 比較、実 runtime 配線、自然な Clipboard 拒否、巨大 repo / 長時間操作、公開 Version ID の API による独立照合。これらを 0 件や成功として埋めない。今回の超過と未確認が残るため、#30 を GO / close にしない。
