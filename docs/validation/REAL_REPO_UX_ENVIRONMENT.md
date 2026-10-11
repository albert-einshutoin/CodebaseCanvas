# 実 repo 評価 — 環境準備と技術リハーサル

版 B-prep-1 / 2026-10-11 JST。**TECHNICAL_REHEARSAL_COMPLETED**。
人間評価・source-first 比較 **NOT_RUN**。以下の技術操作から人間の回答・得点・理解時間を推定しない。

## 前提と分離

元 checkout `/Volumes/Satechi/Developer/codebasecanvas` は #20 の `cd04fc78c894175937fa9831901a0195b5141a0f`、開始時 clean。read-only確認と origin/main fetch の後、Codex管理の専用 worktree `/Users/shutoide/.codex/worktrees/issue-30-real-repo-ux-kit/codebasecanvas` を最新main `b48df42d61d43b7389c191346282ecfdde52529a` から作成した。元checkoutに checkout/reset/編集なし。以後製品commitは固定。[計画](REAL_REPO_UX_PLAN.md)に製品/入力treeも記録。

Node24.x（24.2.0以上）・pnpm11.8.0・Rust1.96.0、git、tar、shasum、製品lockfileが必要。入力repoの依存install、script、NestJS server、DB、migration、submoduleは一切起動しない。製品のVite previewだけを127.0.0.1へ起動する。既存のPlaywrightを使い、追加のbrowser stackは導入しない。agent-browser CLIはこの環境に無く、既存ライブラリへ切り替えた。

source copy・生成graph・Context全文・画像は使い捨ての repo 外へ保存する。検証ログとhash・時刻・計数などの識別情報は、消失を避けるため別の repo 外の永続領域へ保存する。個人情報・回答は今回存在しない。製品 source/依存/lockfile/CI/helperの変更は不要。port5190が使用中なら開始を止め、事前に空きportを決め、全origin指定と環境記録を合わせる。実参加者を開始するには [承認](REAL_REPO_UX_PLAN.md#同意保存承認)が別途必要。

## 再実行（資料 checkout と固定製品 worktree を分離）

資料を含む納品commit（PRの最終HEADまたはmerge commit）から開始し、そのSHAを承認票の `delivery_commit` に記録する。評価対象は常に `b48df42…`、資料commitとは別である。以下はbash/zshで実行する。資料checkoutのHEADを製品SHAと等しいと要求せず、別のdetached製品worktreeを作る。shallow cloneで固定SHAがない場合は、そのSHAだけを `git fetch origin b48df42d61d43b7389c191346282ecfdde52529a` で取得してから始める。製品を最新mainへ差し替えない。input cloneは取得とsource閲覧だけ。

```sh
set -euo pipefail
kit_root="$(git rev-parse --show-toplevel)"
git -C "$kit_root" rev-parse HEAD # 資料納品SHA。承認票へ記録
shasum -a 256 -c docs/validation/REAL_REPO_UX_SHA256SUMS.txt
node --version
pnpm --version
rustc --version

ux_run_dir="$(mktemp -d /tmp/codebasecanvas-real-ux.XXXXXX)"
printf '%s\n' "$ux_run_dir" # このpathを記録・以後の端末へ引き継ぐ
evidence_dir="$HOME/Library/Application Support/CodebaseCanvas-evaluation/technical/$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "$evidence_dir"
chmod 700 "$evidence_dir"
product_commit=b48df42d61d43b7389c191346282ecfdde52529a
git -C "$kit_root" worktree add --detach "$ux_run_dir/product" "$product_commit"
product_root="$ux_run_dir/product"
cd "$product_root"
test "$(git rev-parse HEAD)" = "$product_commit"
test -z "$(git status --porcelain)"
input_commit=c1c2cc4e448b279ff083272df1ac50d20c3304fa
git clone --no-checkout https://github.com/lujakob/nestjs-realworld-example-app.git "$ux_run_dir/input"
git -C "$ux_run_dir/input" checkout --detach "$input_commit"
test -z "$(git -C "$ux_run_dir/input" status --porcelain)"
git -C "$ux_run_dir/input" rev-parse HEAD HEAD^{tree}
mkdir "$ux_run_dir/analysis"
git -C "$ux_run_dir/input" archive "$input_commit" | tar -x -C "$ux_run_dir/analysis"
# root configの期待hashは固定Git commitから取得。copy自身から期待値を採用しない
git -C "$ux_run_dir/input" show "$input_commit:tsconfig.json" | shasum -a 256
shasum -a 256 "$ux_run_dir/analysis/tsconfig.json"

pnpm install --frozen-lockfile --ignore-scripts > "$evidence_dir/product-install.log" 2>&1
cargo build --locked -p codebasecanvas-analyzer --bin codebasecanvas > "$evidence_dir/product-rust-build.log" 2>&1
shasum -a 256 target/debug/codebasecanvas
target/debug/codebasecanvas analyze "$ux_run_dir/analysis" > "$evidence_dir/analyze.log" 2>&1
node --experimental-strip-types docs/validation/evaluate-real-repo.mjs \
  docs/validation/real-repo-source-v7.json docs/validation/real-repo-freeze.json \
  docs/validation/real-repo-input-config.json docs/validation/real-repo-input-config-freeze.json \
  "$ux_run_dir/analysis/.codebasecanvas/graph.json" "$ux_run_dir/analysis" \
  "$evidence_dir/recomputed-result.json" > "$evidence_dir/evaluate.log" 2>&1
shasum -a 256 "$ux_run_dir/analysis/.codebasecanvas/graph.json"
pnpm validation:test > "$evidence_dir/validation-test.log" 2>&1
pnpm web:build > "$evidence_dir/product-web-build.log" 2>&1
pnpm web:e2e:install > "$evidence_dir/browser-prepare.log" 2>&1
shasum -a 256 apps/web/dist/index.html apps/web/dist/assets/*
```

helperは35 sourceのinventory/SHA、root tsconfig manifest、正規v0.1 validator、固定relation/diagnostic scopeを検査する。終了0は照合処理成功で、`technicalVerdict: EXCEEDED` を合格に読み替えない。nodeErrors/false/missing等に新しい差があれば開始を止め、旧監査を修正せず新しい再現記録を保存する。graphは生成ごとにanalyzedAtが変わるため全文hashも変わる。

別端末で上の `product_root`（資料checkoutではない）へ移動してpreviewを起動し、終了時はCtrl-C。`evidence_dir`も同じ値を引き継ぐ:

```sh
pnpm --filter @codebasecanvas/web exec vite preview --host 127.0.0.1 --port 5190 --strictPort > "$evidence_dir/preview.log" 2>&1
```

Viteの既存Cloudflare pluginはlocal static previewを用意する。target NestJSやDBを起動するものではない。追加binding/公開deploy/Explorer API操作は行わない。

最初の端末へ戻り、**既存**UI helperをローカルだけで実行。保存された旧UI baselineは上書きしない。URLだけをtemp baselineでローカルへ変更する。fresh modeはanalyzedAtとgraph/Context全文SHAだけを比較から外し、canonical ID・計数・省略・scope・通信/errorなどは固定baselineと照合する。新graphのsource/provenanceは直前の評価helperで検査済み。

```sh
node - "$ux_run_dir/local-baseline.json" <<'JS'
const fs = require('fs');
const baseline = JSON.parse(fs.readFileSync('docs/validation/real-repo-ui-summary.json', 'utf8'));
baseline.url = 'http://127.0.0.1:5190/';
fs.writeFileSync(process.argv[2], JSON.stringify(baseline, null, 2) + '\n');
JS
node docs/validation/check-real-repo-ui.cjs \
  "$ux_run_dir/analysis/.codebasecanvas/graph.json" "$ux_run_dir/ui-evidence" \
  http://127.0.0.1:5190 "$ux_run_dir/local-baseline.json" --fresh-snapshot \
  > "$evidence_dir/ui.log" 2>&1
cat "$evidence_dir/ui.log"
```

このhelperは実File API入力、ArticleController/AuthMiddleware/ProfileServiceの検索結果選択、選択ID/Details、unknown、元時刻、実Clipboard write/readback、GET /articlesの宣言handlerを確認する。権限は使い捨てbrowser contextのclipboard-read/writeだけ。自動copyを実Clipboardで確認したが、人間の実OSキーボード操作・自然な権限拒否・手動取得は未検証。

追加のsource location確認は次の小さい一時scriptを作り実行する。Node/Edge evidenceの折りたたみを**実クリック**で開き、要求されたsource位置がDOMの表示本文にあることを確認する。UIのsource locationはpath:line表示でありeditorを自動起動するリンクではない。

```sh
cat > "$ux_run_dir/locations.cjs" <<'JS'
const fs = require('fs');
const assert = require('node:assert/strict');
const { chromium } = require(process.cwd() + '/apps/web/node_modules/@playwright/test');
async function run() {
  const [dir, origin = 'http://127.0.0.1:5190'] = process.argv.slice(2);
  const graphPath = dir + '/analysis/.codebasecanvas/graph.json';
  const graph = JSON.parse(fs.readFileSync(graphPath));
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
    const requests = [], errors = [];
    page.on('request', r => requests.push(r.url()));
    page.on('pageerror', e => errors.push(e.message));
    await page.goto(origin, { waitUntil: 'networkidle' });
    const before = requests.length;
    await page.locator('#graph-file').setInputFiles(graphPath);
    await page.getByRole('heading', { name: 'Graph loaded and validated' }).waitFor();
    const details = page.locator('aside[aria-label="Node details"]');
    const records = [];
    for (const [name, kind, required] of [
      ['ArticleModule', 'module', ['ArticleController', 'ArticleService', 'UserModule', 'src/article/article.module.ts:19']],
      ['UserModule', 'module', ['UserService', 'src/user/user.module.ts:16']],
      ['AuthMiddleware', 'service', ['Requests token', 'UserService', 'src/user/auth.middleware.ts:11']],
      ['ProfileService', 'service', ['unsupported_di_decorator_origin', 'src/profile/profile.service.ts:13', 'src/profile/profile.service.ts:15']],
      ['GET /articles', 'endpoint', ['Declared endpoint', 'findAll', 'src/article/article.controller.ts:24']],
    ]) {
      const nodes = graph.nodes.filter(n => n.name === name && n.kind === kind);
      assert.equal(nodes.length, 1);
      await page.getByRole('searchbox', { name: 'Search symbols' }).fill(name);
      await page.getByRole('list', { name: 'Search results' }).getByRole('button').filter({ hasText: nodes[0].id }).click();
      await details.locator('summary').filter({ hasText: /^Node evidence/ }).click();
      for (const summary of await details.locator('summary').all()) {
        if (await summary.isVisible() && !(await summary.evaluate(el => el.parentElement.open))) await summary.click();
      }
      const text = await details.innerText();
      for (const value of required) assert.ok(text.includes(value), name + ': ' + value);
      records.push({ name, kind, id: nodes[0].id, required, checked: true });
      fs.writeFileSync(dir + '/' + name.replaceAll('/', '_') + '-details.txt', text);
    }
    assert.equal(requests.length - before, 0);
    assert.deepEqual(errors, []);
    fs.writeFileSync(dir + '/locations.json', JSON.stringify({ records, requestsAfterImport: 0, pageErrors: errors }, null, 2) + '\n');
    console.log('PASS: 5 source-location selections; no post-import HTTP requests or page errors');
  } finally { await browser.close(); }
}
run().catch(e => { console.error(e); process.exitCode = 1; });
JS
node "$ux_run_dir/locations.cjs" "$ux_run_dir" > "$evidence_dir/locations.log" 2>&1
cp "$ux_run_dir/locations.json" "$evidence_dir/locations.json"
cat "$evidence_dir/locations.log"
```

人間実施時のsource確認は全独力回答確定後、`nl -ba "$ux_run_dir/input/src/article/article.controller.ts"` 等で行う。input copyを起動しない。source-first群のeditorにも同commitのcopyを用意し、README/監査/正解の閲覧を許可しない。

Contextの人間貼付け先は既存fixture進行者資料と同じローカルtextareaを再利用する。新しい端末で、repo外の専用directoryに置き、終了時Ctrl-C。空き5191を事前確認する:

```sh
paste_dir="$(mktemp -d /tmp/codebasecanvas-ux-paste.XXXXXX)"
printf '%s\n' '<!doctype html><meta charset="utf-8"><title>Local paste</title><label>Context paste<textarea rows="25" cols="100"></textarea></label><p>Local only. No submit.</p>' > "$paste_dir/index.html"
python3 -m http.server 5191 --bind 127.0.0.1 --directory "$paste_dir"
```

`http://127.0.0.1:5191`のtextareaへOS貼付け。技術リハーサルのreadbackは上記Playwrightで、参加者の実貼付け成否は別記する。今回の補助チェックは保存したArticleController Context全文を `navigator.clipboard.writeText` で再度書き、textareaへ `ControlOrMeta+V` を送り、28,638 code points全文との一致と通常入力を確認した。製品Copyボタン→実readbackの確認とは別の貼付け先preflightで、人間の物理キーボード操作ではない。データ送信・回答保存機能はない。windowを閉じ、Clipboardは所有者が明示操作で消去、必要ログを移した後使い捨て領域を削除する。人間記録は計画で承認した保存先へ分離する。

## 初回準備時の技術記録（文書・チャットの履歴）

以下は初回の保存済み文書とチャット記録に由来する。納品工程で `/private/tmp/codebasecanvas-real-ux.tyQeWs` が不存在であることを直接確認した。原本ログ・graph・Context・画像は再確認不能であり、以下のhash/成功/失敗を今回直接観測した値として扱わない。新しい直接証拠は後の「納品時の再確認」に分ける。

実施日2026-10-11 JST、解析UTC `2026-10-10T16:13:01Z`。macOS27.0 / arm64、Node24.2.0、pnpm11.8.0、Rust1.96.0、Playwright1.63.0、headless Chromium153.0.8010.12、viewport1440×900。再実行のbrowser versionは `browser.version()` で記録する。
当時の使い捨てroot: `/private/tmp/codebasecanvas-real-ux.tyQeWs`（現在不存在）。記録上は入力treeとclean statusを確認、target依存/script/server/DB起動なし。

| 識別子 | 初回の記録上のSHA-256 |
|---|---|
| Rust debug binary | `777b678c1cd997cf3be883ced7ab065e8e9bf45b3c02048252e1ec6490474123` |
| graph.json | `1fea004a412206724b89b67d74eb6cfe971c2a10d7197be863dfdee000db1b77` |
| root tsconfig | `0da1a8198a9dbed9fcc80b412af5b621667aa36ea8b2fa976e9c31b2587f951f` |
| 35 TS/TSX source manifest | `47b4d73861771f4bae81502be1d541dd09991cddf23e04dc738d558ddf45c405` |
| Web index.html | `c3b91647b4a2da2e539edeb30167199d90016cad0a8b40cf26bcaf78ebb6bd55` |
| Web index-CdS2F1FK.js | `4b488b874691357462e69cda7bd000d1e08a555419c27a918d0270e4421aad7b` |
| Web index-BGioPoxO.css | `95728b91909e2c76014242a841cf6982f8da876fb493d2503fa99a45fc11db14` |
| ArticleController Clipboard | `52f6792d39798fba46dade8268eecc288eb177e0ffb773c16ac4e8dfd21f69b7` |
| AuthMiddleware Clipboard | `8f6f010f6dbbf94d74a25e4380436a85d0c0bbbb6a00d5f284d9288bed1a246d` |
| ProfileService Clipboard | `587d2bf67d9510950fcae36d6182dca134c93f7f0b554dc27b3b8ce5cb853b5f` |

binary/build hashは今回の環境での識別値であり、別machineの再buildで同一になる保証ではない。再生成graphの時刻/hashと承認対象を別途記録する。

| 操作・検証 | 実結果と範囲 |
|---|---|
| 固定sourceからのCLI | exit0、35 TS/TSX、Prisma0、164 nodes/385 edges、116warning/0error。analyzerVersion0.1.0 |
| 独立台帳/設定/正規validator照合 | exit0、nodeErrors/unexpectedNodes/provenanceErrors/errorDiagnostics各0。supported Module16/16、DI5/5、Endpoint42/42、missing/false各0。Module5/21、DI9/14未解決で **EXCEEDED** を保持 |
| Web build | exit0、generated Static Assets検査1件pass。884.74kB JS chunkについて500kB警告。最適化へ拡張しない |
| 既存評価helper test | 14/14 pass（UI不一致/fresh modeとsource/config/provenance等）。製品変更はないため全CIは起動しない |
| File入力/UI helper | exit0、WITHIN_STRUCTURAL_BASELINE。generation1、layoutReady=true、projection105nodes/155edges、検索選択ID・Details一致、GET /articles→ArticleController/findAll、unknown保持 |
| Clipboard | 3対象の実readback成功。ArticleController28,638、AuthMiddleware13,814、ProfileService25,343 code points。ArticleControllerのedge evidence29・局所diagnostic11詳細省略、scope/省略数保持 |
| source位置追加確認 | 最終exit0、ArticleModule/UserModule/AuthMiddleware/ProfileService/GET /articlesの5選択。Evidence開示、constructor11、DI13/15、Get24等の表示を確認 |
| 貼付け先preflight | localhost textareaの通常入力と、保存Contextを実Clipboardへ再書込→browser keyboard貼付けの全文一致を確認。製品ボタンのreadbackとは別チェック |
| 通信・browser error | 両UIチェックでFile入力後HTTP request0、UI helper console/page error0、追加チェックpage error0。観測期間外やbrowser自体の通信の保証ではない |
| 人間回答・得点・理解時間・価値 | **NOT_RUN / NOT_EVALUATED**。空票維持 |

当時の追加source位置チェックの初回は AuthMiddleware constructor11行の assertion が失敗（Node process exit1）。記録ではNode evidenceだけを開き、constructor位置を持つ関係のEdge evidenceが閉じていた。temp scriptへ可視summaryの実クリックを追加して最終exit0。これは過去の失敗として保持する。`locations-initial-failure.log` は当時一時rootへ保存されたが現在は消失しており、再取得したとは主張しない。製品の根拠欠落とは判定せず、根拠の開示操作が必要な制約という当時の解釈も残す。

当時の `analyze.log`、評価結果、UI summary、3Context全文/3PNG、locationのDetails全文とログはいずれも上記一時rootとともに消失した。画像を目視したという当時の記録はあるが、現在の画像確認ではない。採点に必要な固定sourceは入力commitから再取得できる。生成graph/Context/画像を公開repoへ納品しない。

## 初回の最終確認とGateの履歴

セルフレビュー: 課題/正解source/採点、比較対象と固有課題、割当・順序、失敗/欠測、空結果、同意/承認、再現手順を照合。UIで所属宣言・constructor・unknownの位置まで辿れることを追加確認した。相対link、固定Git source由来の6 raw hash、空CSV票、NOT_RUN、過去POC本文がそのまま残ること、`git diff --check` の検査は成功。製品source/helper/契約/旧監査・Phase Aに差分なし、元checkoutのHEAD `cd04fc78…` とclean状態も確認。最終manifestの各全文hash照合を納品前に行い、manifest自体のhashを承認票へ記録できる状態にする。

初回のCompanion Stop Review Gateは **NOT_RUN**。desktopはClaude CodeのStop hookを自動実行せず、記録上worktreeの `stopReviewGate:false`。正規の手動 `node <plugin>/scripts/codex-companion.mjs review --wait --scope working-tree` を初回資料差分で一度試みたがexit1。HTTP400 / `invalid_request_error`: `The 'gpt-6.1-sol' model is not supported when using Codex with a ChatGPT account.` で判定を取得できなかったというチャット・文書記録を保持する。一時rootの `companion-review.log` / `companion-review.exit` 原本は現在確認不能。手動起動失敗とセルフレビューをGate成功に読み替えない。

未確認: 初見人間UX・source-first時間/正答、実Chrome stable/VS Codeの参加者操作、自然なClipboard拒否・手動取得・OS貼付け、IME/画面reader、大規模repo/長時間性能、runtime注入/request→DB、最新製品のhosted結果/公開asset。製品問題を見つけた場合はこの固定条件で再現記録だけを追加し、解析拡張・UI改修・他Issueへ自動継続しない。

## 納品時の再確認

納品用branch `codex/issue-30-real-repo-ux-delivery`。製品・入力の固定SHA、課題・採点・比較条件は変更しない。原本のない旧記録を新しい実行で復元したとは呼ばない。

新しい証拠保存先（repo外の永続領域、directory 700）:
`/Users/shutoide/.codex/validation-evidence/codebasecanvas/issue-30/B-prep-1-delivery-20261011T010801Z/`

ここには今回のinstall/build/CLI/評価helper/UI/locationログ、集計JSON、識別子、Companion status、元資料のhash/差分と最終検証、PR/commit/CIの納品メタデータを保存する。source copy・生成graph・Context全文・画像・回答/個人情報はcommitしない。旧原本ログをこの領域へ再作成しない。新規UIの全文・画像は新しいtemp領域だけに置く。

今回の直接確認: 解析UTC **2026-10-11T01:10:47Z**（2026-10-11 JST）。macOS27.0/arm64、Node24.2.0、pnpm11.8.0、Rust1.96.0、Playwright1.63.0、Chromium153.0.8010.12、1440×900。製品・入力commit/treeは計画の固定値と一致。資料の最終SHAとmerge/hosted結果は自己参照を避け、PRとrepo外の `delivery.json` へ記録する。

| 今回の直接証拠 | 識別・結果 |
|---|---|
| graph SHA-256 | `fcb33aed2f7facac61e9d3e5a502010f663c521dd04f3cbc8c71ffadad5100ed` |
| Rust debug binary SHA-256 | `a90b27f8bed1758746ace35c06e9672a813d91aa7a6f8edb5aa4fd6f733b2a49` |
| CLI / validator / source・config照合 | exit0。35 TS/TSX、Prisma0、164 nodes/385 edges/116 warnings/0 errors。入力source manifestとroot configは固定台帳hashと一致。node/provenance/error diagnostics不整合0 |
| 固定関係照合 | Module16/16、DI5/5、Endpoint42/42 correct、missing/false0。Module5/21、DI9/14未解決、**EXCEEDED**。旧監査の閾値・分類を変更しない |
| install / Rust build / Web build | 各exit0。Web Static Assets検査1/1、JS chunk500kB警告継続。index/JS/CSS hashは初回文書の値とも一致するが、今回新しく生成して識別情報へ記録したもの |
| 既存評価helper | 14/14 pass、0 fail。完全ローカルCIは重複実行しない。最終hostedはPRで確認 |
| 実File入力/UI/Clipboard | exit0 / WITHIN_STRUCTURAL_BASELINE、projection105nodes/155edges、3対象の検索・選択・Details・unknown・元時刻・実ClipboardとGET /articlesのhandlerを確認。post-import HTTP request0、console/page error0 |
| source location | 5対象、Node/Edge evidence開示とconstructor11/unknown13・15/Get24等を確認、exit0。今回locationチェックの失敗なし。旧初回失敗は旧記録として保持 |
| ローカル貼付け | 製品Copyボタン→実Clipboard readback→localhost textareaへのbrowser keyboard貼付け、28,638 code points全文一致。人間の物理操作・自然な権限拒否の評価ではない |
| 人間評価 / source-first / 承認 | **NOT_RUN / 0名 / PENDING**。結果票は空。正式Decision NOT_EVALUATED、REVISEは提案のまま |

| 今回のClipboard対象 | SHA-256 / code points / 省略 |
|---|---|
| ArticleController | `86d6c91b67003adfe417ebcfce2119031de493dba849830fb217002679adde16` / 28,638 / edge evidence詳細29、局所diagnostic詳細11省略 |
| AuthMiddleware | `445ef0138f7e9620cd491b2442a821d05f8357953cdf6dfe14ea57719e95e9d3` / 13,814 / edge evidence詳細0、局所diagnostic詳細0省略 |
| ProfileService | `42cdf2a1c6c002305014a0346b1808b131e9847b38961f81f004d78c34e4abbe` / 25,343 / edge evidence詳細0、局所diagnostic詳細0省略 |

画像は新しいtemp領域でProfileServiceの選択/Details/source位置を目視した。旧画像の再確認ではない。原本の新規ログは上の永続領域で直接確認可能。`identifiers.json` はproduct/input/tree・graph時刻/hash・binary/asset hash、`recomputed-result.json` はsource/configとrelation照合、`ui-summary.json` / `locations.json` / `paste-check.json` はUI/Clipboard/位置の結果を保持する。UI summaryからContext/Details抜粋を除き、全文・画像・生成graphは永続証拠へ移していない。

再実行時の識別情報保存（UI helperとlocationチェックの成功後、固定 `product_root` 内から実行）:

```sh
node - "$ux_run_dir" "$evidence_dir" <<'JS'
const fs = require('fs'), crypto = require('crypto'), cp = require('child_process');
const [run, out] = process.argv.slice(2);
const sha = p => crypto.createHash('sha256').update(fs.readFileSync(p)).digest('hex');
const git = (cwd, ...args) => cp.execFileSync('git', ['-C', cwd, ...args], {encoding:'utf8'}).trim();
const graph = JSON.parse(fs.readFileSync(run + '/analysis/.codebasecanvas/graph.json'));
const ui = JSON.parse(fs.readFileSync(run + '/ui-evidence/summary.json'));
for (const selection of ui.selections) {
  delete selection.detailExcerpt;
  delete selection.contextExcerpt;
}
fs.writeFileSync(out + '/ui-summary.json', JSON.stringify(ui, null, 2) + '\n');
fs.writeFileSync(out + '/identifiers.json', JSON.stringify({
  recordedAt: new Date().toISOString(),
  productCommit: git(process.cwd(), 'rev-parse', 'HEAD'),
  productTree: git(process.cwd(), 'rev-parse', 'HEAD^{tree}'),
  inputCommit: git(run + '/input', 'rev-parse', 'HEAD'),
  inputTree: git(run + '/input', 'rev-parse', 'HEAD^{tree}'),
  graphSha256: sha(run + '/analysis/.codebasecanvas/graph.json'),
  graphMetadata: graph.metadata,
  graphCounts: {nodes:graph.nodes.length, edges:graph.edges.length, diagnostics:graph.diagnostics.length},
  binarySha256: sha('target/debug/codebasecanvas'),
  webAssets: Object.fromEntries(['index.html', ...fs.readdirSync('apps/web/dist/assets').map(n=>'assets/'+n)]
    .map(n => [n, sha('apps/web/dist/' + n)])),
}, null, 2) + '\n');
JS
chmod -R go-rwx "$evidence_dir"
```

失敗時は終了値・実command・原因・部分出力を保存して停止し、success JSONや旧成功値で置換しない。新しい成功は前の失敗を消さない。temp製品worktreeの利用終了時は、その専用pathだけを `git -C "$kit_root" worktree remove --force "$product_root"` で除去する（依存/build等の使い捨て生成物を含む）。入力や出力の削除はその実行で作成したtemp pathを確認してから行い、永続証拠は残す。永続ログの移動・削除は所在とhashを別途更新する。

納品工程でもCompanion Gateは **NOT_RUN**。配布pluginの `codex-companion.mjs review` と `hooks/hooks.json` のStop入口が存在し、現行statusは `stopReviewGate:false` / runningなし / runtime direct。desktopにStop hookが自動適用される証拠はなく、初回のgpt-6.1-sol非対応HTTP400を解消した証拠もない。同条件の手動レビューを再試行せず、モデル・hook設定を変更しない。現在確認したstatusは上の永続領域に保存し、失われた前回のstderr原本とは区別する。納品範囲のセルフレビューはGateの代替成功として扱わない。
