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
| Discovery 対象テスト | `cargo test --workspace --locked -p codebasecanvas-analyzer discovery` |

`pnpm web:dev` 後に [開発画面](http://127.0.0.1:5173) を開きます。
preview は先にビルドしてから [確認画面](http://127.0.0.1:4173) を開きます。
ポート使用中は別ポートへ自動変更せずエラーになります。終了は Ctrl+C です。

Rust と Web は共通 JSON ケースで契約を検証します。解析器の抽出精度を検証するテストは後続 Issue です。
`codebasecanvas --help` は成功し、`codebasecanvas analyze <repo>` は引数と repository を検証します。実解析は #17 で接続するため、現在は未実装エラーで終了コード 1 を返し、graph を生成しません。
`web:e2e` は未実装です。成功する仮コマンドは用意していません。

Issue #6 の discovery API (`codebasecanvas_analyzer::discovery::discover`) は、選択した root を canonicalize し、`.ts`/`.tsx`（`.d.ts`を除く）を root 相対 `/` 区切りで決定論的に列挙します。`.git`、`.codebasecanvas`、`node_modules`、`dist`、`build`、`coverage`、`.next`、generated directory は除外し、root 外・loop・除外先への symlink alias は取り込みません。`tsconfig.json` は root 直下だけ検出します。解析 pipeline にはまだ接続せず、解析未実装の CLI は非0・無書込のままです。

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
- [Oxc capability probe (#7)](docs/OXC_CAPABILITIES.md)

現在の着手順と完了条件は [Epic #1](https://github.com/albert-einshutoin/CodebaseCanvas/issues/1) と各 Issue を参照してください。
型と検証の入口・後続 Issue の責務は [契約の実装境界](docs/DATA_MODEL.md#wire-validation-and-ownership) を参照してください。

Fixture の意味・期待件数と変更規則は [fixture README](examples/nestjs-sample/README.md) を参照してください。期待 graph は Analyzer 出力の代替ではありません。

## CI の正本と後続の検証

`package.json` の `ci` が独立した完全検証の正本です。`ci:rust` と `ci:web` に分けた既存検証を順に実行し、失敗時に停止します。`ci-pr` は `ci` へ一方向委譲します。変更範囲 selector は未実装です。

GitHub Actions の `Rust / Web quality` は PR と main push で同じ入口を実行します。Ubuntu 24.04 の1環境、Node は `.node-version`、pnpm は `packageManager`、Rust は `rust-toolchain.toml` で固定します。Actions は commit SHA 固定、token は contents read、pnpm store のみ標準 cache、古い同一 PR run は中止します。必須 check に設定する場合は `Rust / Web quality` を選びます（branch protection の設定は別工程）。

#18 の構造回帰は通常の Rust test に、#26 の E2E はこの完全検証入口に接続し、#30 で対象 commit の hosted 結果を確認します。現在 E2E は未実装です。`pnpm audit` は独立した security check で、`ci` の build/test 成功とは分けて確認します。

## CLI 入出力の境界 (#5)

```sh
cargo run --locked -p codebasecanvas-analyzer -- --help
cargo run --locked -p codebasecanvas-analyzer -- analyze ./examples/nestjs-sample
```

help は終了コード 0、引数不正は 2、repository・解析・保存の fatal error は 1 です。現在の analyze は解析未実装として 1 を返します。空 graph や手定義 fixture を解析済みの出力にする経路はありません。#17 で `cli::analyze` に実 pipeline を接続します。成功した pipeline の warning は保存を妨げず、保存先と diagnostics 件数を表示します。fatal error は graph diagnostics と分離します。

保存経路は `SystemGraph::to_json` で検証・正規化してから、固定の `.codebasecanvas/graph.json` に保存します。Unix (macOS/Linux) の directory-relative I/O で開いた root/output directory を使い、既存 output directory/target の symlink・非regular target を拒否します。temp は排他的に作成し権限0600、書込・sync完了後に同じdirectory内でatomic renameします。失敗時は旧 graph を保持しtempを削除し、削除も失敗した場合はそのエラーを報告します。非Unixは安全な保存の未対応エラーです。

symlink参照先への書込は行いません。検査後にtargetがsymlinkへ変わってもrenameはlink自体を置換します。root/output directoryのidentityを照合し、検出した差替えは失敗にします。ただし同一ユーザーが保存中にdirectory自体を移動し続ける状況や、同時writer同士の競合を隔離するsandbox/lockではありません。保存中はrepository/output directoryを移動・変更しないでください。atomic置換は途中JSONの公開を防ぐ保証であり、停電後のdirectory entryの永続性まで保証しません。

成功・再保存・warning・途中write失敗・symlink差替え・旧graph保護は、使い捨てdirectoryの合成graphで検証します。これは現行Analyzerの解析成功やE2Eの証拠ではありません。依存は引数処理の `clap`（独自parserを避ける）と、unsafe自作syscallを避ける `rustix`（Unix filesystemのみ）に限定します。
