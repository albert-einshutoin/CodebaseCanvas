# CodebaseCanvas

AI が生成・変更するコードを、人間が理解できる構造の地図にするプロジェクトです。
最初の対象は、既存 TypeScript/NestJS を引き継ぐバックエンドエンジニアです。

## 現在の状態

Issue #2 の開発基盤です。Rust のビルドと React/Vite の起動画面が利用できます。
リポジトリ解析、graph 生成、ファイル選択、Canvas はまだ実装していません。
内蔵 LLM、バックエンド、認証、データベースはありません。

目標とするデータの流れは次のとおりです。

```text
ローカル NestJS リポジトリ
  → Rust/Oxc CLI
  → .codebasecanvas/graph.json
  → ブラウザでファイル選択
  → Canvas・根拠確認・Context コピー
```

解析と graph データはローカルに留め、サーバーへアップロードしない設計です。
Cloudflare による静的配信は #28 で実装します。

## セットアップ

必要環境: Node.js 24.2.0 以上の 24.x、pnpm 11.8.0、rustup、Git。
確認環境は macOS / Node.js 24.2.0。Rust は `rust-toolchain.toml` の 1.96.0 を使用します。

```sh
git clone https://github.com/albert-einshutoin/CodebaseCanvas.git
cd CodebaseCanvas
npm install --global pnpm@11.8.0
pnpm install --frozen-lockfile --ignore-scripts
cargo build --workspace --locked
pnpm web:typecheck
pnpm web:build
```

初回の Cargo 実行時に rustup が指定 toolchain と rustfmt/clippy を取得します。
依存のライフサイクルスクリプトを実行せずインストールできます。
`Cargo.lock` と `pnpm-lock.yaml` をリポジトリで管理します。

## 開発コマンド

すべてリポジトリのルートから実行します。実コマンドの正本は [AGENTS.md](AGENTS.md) です。

| 目的 | コマンド |
|---|---|
| Web 開発サーバー | `pnpm web:dev` |
| Web 型検査 | `pnpm web:typecheck` |
| Web 本番ビルド | `pnpm web:build` |
| ビルド済み Web の確認 | `pnpm web:preview` |
| Rust ビルド | `cargo build --workspace --locked` |
| Rust フォーマット確認 | `cargo fmt --all --check` |
| Rust lint | `cargo clippy --workspace --all-targets --locked -- -D warnings` |
| Rust test runner | `cargo test --workspace --locked` |

`pnpm web:dev` 後に [開発画面](http://127.0.0.1:5173) を開きます。
preview は先にビルドしてから [確認画面](http://127.0.0.1:4173) を開きます。
ポート使用中は別ポートへ自動変更せずエラーになります。終了は Ctrl+C です。

現時点の Rust test は 0 件です。解析品質を検証したことにはなりません。
`codebasecanvas` バイナリは未実装の説明を出して終了コード 1 を返し、ファイルを生成しません。
`web:test`、`web:e2e`、`ci`、`ci-pr` の成功する仮コマンドは用意していません。

## 構成と後続作業

| 場所 | 責務 |
|---|---|
| `crates/analyzer/` | Rust crate `codebasecanvas-analyzer` / binary `codebasecanvas`。CLI は #5、Oxc 能力確認は #7 |
| `apps/web/` | React/Vite SPA。graph 選択は #19、Canvas は #20 |
| `examples/nestjs-sample/` | #4 で作成予定。まだ存在しません |
| `docs/` | 製品・設計・graph 契約の文書 |

#3 で graph 型・validator・関連 test、#26 で `pnpm web:e2e`、#31 で CI を導入します。
Rust と Web はソースコードを共有せず、正規 JSON graph を境界とします。

解析パイプライン完成後は `codebasecanvas analyze <repo>` で
`<repo>/.codebasecanvas/graph.json` を生成し、Web の「Choose graph.json」で選択する予定です。
graph は解析時点の snapshot なので、ソース変更後には再解析と再選択が必要です。
現時点でこの手順を実行して graph を得ることはできません。

## 設計文書

- [製品概要](docs/README.md)
- [PRD](docs/PRD.md)
- [PoC スコープ](docs/MVP_SCOPE.md)
- [アーキテクチャ](docs/ARCHITECTURE.md)
- [Graph 契約](docs/DATA_MODEL.md)
- [PoC 実装計画](docs/POC_IMPLEMENTATION.md)
- [Agent 向け説明](docs/AI_AGENT_PROMPT.md)
- [将来のロードマップ](docs/FUTURE_ROADMAP.md)

現在の着手順と完了条件は [Epic #1](https://github.com/albert-einshutoin/CodebaseCanvas/issues/1) と各 Issue を参照してください。
既存文書と整理済み Issue の契約整合は #3 の担当です。
