# CodebaseCanvas 実 repo 評価 — 参加者用

版 B-prep-1。実施承認後に、進行者が説明と各問を一問ずつ渡します。
既存の NestJS を引き継ぎ、変更前に構造と確認すべき source を調べる場面です。あなたの能力ではなく、調査の道具と情報の分かりやすさを確認します。入力は固定された source snapshot です。

各問 **5分**。分かったこと、見た根拠、不明な範囲、次に確認すべき source の相対 path と行番号を回答してください。最初の最終回答または時間切れで回答を保存します。助けを求められますが、その時点の独力回答を保存し、案内はすべての独力課題の後です。辞退・中断できます。

録画は既定では行いません。回答は合意したローカル領域へ保管し、外部サービスへ送らないでください。録画・発言引用・追加共有には別途同意を確認します。

## 方式の説明（割り当てられた方だけ読む）

Canvas: 進行者の graph を実 File 選択してください。Canvas、検索、Details、Analysis、Copy Context の表示を使えます。source、JSON の直接閲覧、repo README/監査/正解資料、外部検索/LLM は回答確定前に開かないでください。

source-first: 進行者の固定 source copy を editor で読み、ローカル文字検索を使えます。実行・DB接続・依存install、graph/Canvas/監査/正解資料、外部検索/LLM は使わないでください。

## Q1 Module と component の所属

ArticleModule に直接登録された Controller と Service をすべて挙げてください。UserModule との関係も説明し、UserModule 内の component と直接登録を区別してください。所属と接続の根拠を確認する source の path と行を示してください。

## Q2 宣言 Endpoint と handler

宣言上の `GET /articles`（`/articles/feed` や `/:slug` は含めない）に対応する Controller と handler を調べてください。route の宣言位置と handler の source 位置を示してください。この情報で実行時の到達・実行をどこまで確認できますか。

## Q3 要求 token と実装先

AuthMiddleware の constructor が要求する token は何ですか。要求している token、source 上の登録、実行時に注入・実行される実装を区別し、この調査だけで言える範囲を説明してください。constructor を確認する source と、登録を追加確認する source の path と行を示してください。

## U1 未解決と「存在しない」（Canvas のみ）

ProfileService の constructor DI を調べてください。表示にある未解決の理由・場所を挙げ、関係が表示されないことから依存が存在しないと言えるか説明してください。source で確認を続けるならどこを読みますか。

## U2 Copy Context（Canvas のみ）

ArticleController の Context を取得し、進行者が用意したローカル貼付け先で確認してください。対象・元の解析日時、解析範囲と unknown を説明してください。関係の情報と個別の根拠/診断の詳細について、省略や短縮があるか、渡した相手がこの Context だけで全 source を確認できるか説明してください。省略通知と、確認を続ける source の場所を示してください。

## 全回答の確定後

分かりにくかった表示・言葉・操作を教えてください。回答をやり直す必要はありません。
