# CodebaseCanvas

AI が生成・変更するコードを、人間が理解できる構造の地図にするプロジェクトです。
最初の対象は、既存 TypeScript/NestJS を引き継ぐバックエンドエンジニアです。

## 現在の状態

Issue #2 の開発基盤に、#3 の SystemGraph v0.1 型・検証・契約テストを追加しています。
Rust のビルドと React/Vite の起動画面、#4 の NestJS fixture と手定義の期待 graph が利用できます。
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
| 完全検証（依存 install + Rust/Web/fixture） | `pnpm run ci` |
| PR 検証（現在は完全検証へ委譲） | `pnpm run ci-pr` |
| Web 開発サーバー | `pnpm web:dev` |
| Fixture 型検査 | `pnpm fixture:typecheck` |
| Web 契約テスト | `pnpm web:test` |
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

Rust と Web は共通 JSON ケースで契約を検証します。解析器の抽出精度を検証するテストは後続 Issue です。
`codebasecanvas` バイナリは未実装の説明を出して終了コード 1 を返し、ファイルを生成しません。
`web:e2e` は未実装です。成功する仮コマンドは用意していません。

## 構成と後続作業

| 場所 | 責務 |
|---|---|
| `crates/analyzer/` | Rust crate `codebasecanvas-analyzer` / binary `codebasecanvas`。CLI は #5、Oxc 能力確認は #7 |
| `apps/web/` | React/Vite SPA。graph 選択は #19、Canvas は #20 |
| `examples/nestjs-sample/` | #4 の NestJS source・手定義 expected graph・根拠 README |
| `docs/` | 製品・設計・graph 契約の文書 |

#3 の契約は [DATA_MODEL.md](docs/DATA_MODEL.md) と `contracts/cases.json` に定義しています。
#31 の基本 CI は導入済みです。#26 で `pnpm web:e2e` を同じ完全検証入口へ接続します。
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
型と検証の入口・後続 Issue の責務は [契約の実装境界](docs/DATA_MODEL.md#wire-validation-and-ownership) を参照してください。

Fixture の意味・期待件数と変更規則は [fixture README](examples/nestjs-sample/README.md) を参照してください。期待 graph は Analyzer 出力の代替ではありません。

## CI の正本と後続の検証

`package.json` の `ci` が独立した完全検証の正本です。`ci:rust` と `ci:web` に分けた既存検証を順に実行し、失敗時に停止します。`ci-pr` は `ci` へ一方向委譲します。変更範囲 selector は未実装です。

GitHub Actions の `Rust / Web quality` は PR と main push で同じ入口を実行します。Ubuntu 24.04 の1環境、Node は `.node-version`、pnpm は `packageManager`、Rust は `rust-toolchain.toml` で固定します。Actions は commit SHA 固定、token は contents read、pnpm store のみ標準 cache、古い同一 PR run は中止します。必須 check に設定する場合は `Rust / Web quality` を選びます（branch protection の設定は別工程）。

#18 の構造回帰は通常の Rust test に、#26 の E2E はこの完全検証入口に接続し、#30 で対象 commit の hosted 結果を確認します。現在 E2E は未実装です。`pnpm audit` は独立した security check で、`ci` の build/test 成功とは分けて確認します。
