# CodebaseCanvas 開発ガイド

日本語で回答する。明示された 1 Issue の範囲を守る。
既存の未コミット変更を保護し、commit・push・PR・merge・deploy・Issue close は依頼された工程だけ実施する。

## 構成

- `crates/analyzer`: Rust。crate は `codebasecanvas-analyzer`、binary は `codebasecanvas`。
- `apps/web`: React / TypeScript / Vite。pnpm workspace package は `@codebasecanvas/web`。
- Analyzer と Web の境界は `docs/DATA_MODEL.md` の正規 JSON。ソースコードや AST を共有しない。
- `examples/nestjs-sample` は #4、graph 契約は #3、CLI は #5、file import は #19、Canvas は #20、CI は #31。
- 現在はビルド、起動画面、SystemGraph v0.1 契約型・検証・共有テスト。未実装の解析・test を成功する stub にしない。

## 実コマンド（ルートから実行）

| 目的 | コマンド |
|---|---|
| 依存インストール | `pnpm install --frozen-lockfile --ignore-scripts` |
| Web 開発 | `pnpm web:dev` |
| Web 契約テスト | `pnpm web:test` |
| Web 型検査 | `pnpm web:typecheck` |
| Web ビルド | `pnpm web:build` |
| Web preview | `pnpm web:preview` |
| Rust ビルド | `cargo build --workspace --locked` |
| Rust format | `cargo fmt --all --check` |
| Rust lint | `cargo clippy --workspace --all-targets --locked -- -D warnings` |
| Rust test runner | `cargo test --workspace --locked` |
| JS 依存監査 | `pnpm audit` |

Node 24.2.0 以上の 24.x、pnpm 11.8.0、Rust 1.96.0 を使用する。
Rust/Web 契約テストは `contracts/cases.json` を共用する。E2E/CI は各担当 Issue で追加する。

## 実装と検証

- Ponytail を使用し、標準機能・既存実装を優先する。不要な依存・抽象化を追加しない。
- 通常は単一 Agent と影響範囲の検証。build/依存/CI/security 境界変更では事前計画、専門レビュー 1 件、完全検証と関連 security check を行う。
- 専門レビューは高リスクのため Sol medium。それ以外の Subagent は明示依頼時のみ。
- 対象検証→レビュー→指摘修正→最終検証の順に行い、成功後は新しい懸念がなければ繰り返さない。
- 今回のような build 基盤変更の完全検証は上表の Rust build/format/lint/test と Web test/typecheck/build。加えて clean copy の依存インストールと起動を確認する。
- Codex Companion Stop Review Gate を完了確認に使用する。利用不能な場合は未実施を明示し、成功と主張しない。
- 未対応構文は最小範囲の unknown と診断にし、推測 edge・暗黙 fallback を作らない。
- ソースや graph の外部送信、backend、内蔵 LLM、Cloudflare binding を追加しない。
- `node_modules`、`target`、`dist`、解析出力、秘密情報を納品しない。lockfile は管理する。
