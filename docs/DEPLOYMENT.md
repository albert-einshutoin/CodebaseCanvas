# Static Assets 配信手順（Issue #28）

今回の成果物はローカル build・preview・deploy dry-run までです。Cloudflare への実deploy、公開URL、公開先でのFile入力と非送信確認は未実施です。

## 固定環境と成果物

Node 24.2.0 以上の24.x、pnpm 11.8.0、Rust 1.96.0を使います。WebはVite 8.2.2、`@cloudflare/vite-plugin` 1.60.1、Wrangler 4.140.0をexact固定しています。pluginのpeer条件はVite `^6.1.0 || ^7.0.0 || ^8.0.0`、Wrangler `^4.140.0`、WranglerのNode条件は`>=22.0.0`です。

以下はrepository rootから実行します。

```sh
pnpm install --frozen-lockfile --ignore-scripts
pnpm web:build
pnpm web:preview
pnpm web:deploy:check
```

`web:build` はVite build後に生成設定と公開assetを検査します。入力は `apps/web/wrangler.jsonc`、生成設定は `apps/web/dist/wrangler.json` です。今回確認した生成設定の `assets.directory` は生成設定からの `.` で、`apps/web/dist/` の `index.html`、`assets/` 内のJS/CSS、`_headers`を指します。生成 `.assetsignore` は `wrangler.json` と `.dev.vars` を公開assetから除外します。入力設定には `assets.directory` を重ねません。アプリ用Workerの `main`、API、storage/service/secret binding、route、custom domainはありません。`codebasecanvas` は希望するWorker名で、Cloudflare上の存在・利用可能性は未確認です。`compatibility_date` は `2026-09-24`、Wranglerのツールtelemetryは `send_metrics: false` です。

`web:preview` は新しいbuild後、`http://127.0.0.1:4173/` のloopbackだけでCloudflare Vite pluginのpreviewを起動します。strict portで、既存serverがあれば失敗します。`web:deploy:check` はbuildと検査の後、`apps/web`をcwdとして `wrangler deploy --config dist/wrangler.json --dry-run --outdir .wrangler/dry-run` を実行します。dry-runの一時出力はclient assets外でgit管理対象外です。`--dry-run` は実deployや認証・公開URLの成功を意味しません。これらのコマンドはGraphや解析対象sourceを引数やupload対象に渡しません。

## 後の明示的な公開工程

実deployの許可を得た後にのみ、対象accountと既存WorkerをCloudflare Dashboardで確認します。`pnpm --filter @codebasecanvas/web exec wrangler whoami` で認証先を確認し、必要な場合だけWranglerのログインまたは権限を限定したAPI tokenを準備します。環境変数を使う場合の名前は `CLOUDFLARE_ACCOUNT_ID` と `CLOUDFLARE_API_TOKEN` です。値をrepository、Wrangler設定、`VITE_*` に記録しません。既存の `codebasecanvas` Workerがある場合は、上書き対象と設定を確認するまで実deployしません。

deploy直前にcheckoutのcommit (`git rev-parse HEAD`) と変更状態 (`git status --porcelain=v1`) を記録し、cleanな対象commitで `pnpm web:build` と検査を完了します。生成 `index.html`、JS/CSS、`dist/wrangler.json` のhashとasset一覧を記録し、同じbuildを使用します。**実deploy用のコマンド**は `apps/web` をcwdとして次のとおりです。今回は実行しません。

```sh
cd apps/web
pnpm exec wrangler deploy --config dist/wrangler.json
```

実行後は実URL、deployment/version ID、対象commit、生成設定とasset hashを同じ記録へ残します。URLやaccount名を推測で埋めません。公開先ではHTTPSの `/` と `/review` の直接navigation/reload、JS/CSS、security headers、ローカルGraphのFile選択・Canvas・Copy Contextを確認します。DevToolsまたはPlaywrightでGraph読込前から通信を監視し、Graph内容を送信するリクエストがないことと、Cloudflare側にWorker API・禁止bindingがないことを確認します。これらの公開後確認は今回 **NOT_RUN** です。

Cloudflareが配るのは製品のHTML/JS/CSS等です。利用者の `graph.json` はFile APIでブラウザ内に読み込み、アプリにはGraph upload/API/storageを追加していません。静的ページとassetの通常requestは発生します。ローカルChromium E2Eの非送信観測は、その期間・環境に限る証拠であり、公開先の確認を代替しません。
