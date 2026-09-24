# Prisma schema recognizer (#15)

## 責務と利用例

Prisma DSL は Oxc の TypeScript 入力にしない。追加依存なしの byte offset を保持する字句走査と top-level block 認識を使い、基本 model の存在一覧だけを得る。完全な Prisma parser / validator は不要なため導入しない。

```rust,ignore
use codebasecanvas_analyzer::{discovery::RepositoryRoot, prisma};
// root は呼出側が明示した repository。builder は既存 GraphBuilder。
let root = RepositoryRoot::open(repository_path)?;
let findings = prisma::analyze(&root)?;
findings.apply(&mut builder, &[])?; // 単独利用。Resolver併用時は下記参照。
let graph = builder.finish()?;
```

`analyze` は共有 discovery で候補を選択し、1 source を一度読み、純粋関数 `parse_schema` で findings を収集する。`PrismaFindings` の candidates / selected_schema / nodes / diagnostics は検証可能。`apply(builder, owned_discovery_diagnostics)` は既存 `add_node` / `add_diagnostic` だけを使い、再読・再解析・calls batch 再適用を行わない。Builder の矛盾検出と finish validation はそのまま。CLI #17 の pipeline はこの入口を calls batch の後に一度だけ適用する。

## 探索・選択・安全な read

TS discovery と同じ walker / RepositoryRoot / 除外directory / canonical containment / symlink処理を使う専用入口。TS files と tsconfig / Resolver の契約は変更しない。canonical target のファイル名が `schema.prisma` の通常fileだけが候補。論理名だけが `schema.prisma` の別拡張子aliasは候補にしない。

1. symlinkを正規化し、canonical repo相対pathでsort / dedupする。同一targetへのsymlinkを複数schemaとして数えない（hard linkのinode同一性は統合しない）。
2. **正規化後**の `prisma/schema.prisma` を優先し、なければ辞書順先頭。標準pathが他pathへのaliasならtarget pathとして判定する。
3. 複数候補は1つだけ解析し、選択fileと未解析候補の存在を `PRISMA_MULTIPLE_SCHEMAS` で示す。候補一覧はfindingsに保持。選択schemaが壊れても別候補へfallbackしない。
4. 0候補は空findings。正常な0 modelとともに、repository全体にDBがないことは意味しない。

除外directoryを直接走査しない。除外先alias、dangling / looping symlink、既訪問directory aliasは既存 `DISCOVERY_*` 診断を保持し、正常なschemaなしに隠さない。外向き参照はPrisma入口ではErrorとし、そのtargetの本文を読まない（TS入口の既存warning方針は維持）。非UTF-8 path、通常fileでないschema候補、予期しない探索I/O失敗はError。

選択後、root内canonical pathであること・選択pathから変わっていないこと・除外先でないことを再確認。open後に通常fileを確認してから読む。UnixではNOFOLLOW / NONBLOCKにより最終componentのsymlinkへの差替えやFIFO等を拒否する。読込み不能と不正UTF-8 contentは区別した固定Error。生OS error、絶対root、本文をPrisma診断やread errorへコピーしない。

これは全repositoryの原子的snapshotではない。同一ユーザーによるancestor directoryの同時rename / 差替えの完全隔離は保証しない。収集後はそのsource由来findingsだけを適用する。直接 `parse_schema` を呼ぶ場合、呼出側がsource取得を担当する（pathは相対path検証のみ）。

## 対応文法と記録範囲

- `model Name { ... }` のtop-level宣言。`model`、名前、`{` の3 tokenが元sourceの同一行にある場合だけ確定する。同一行の空白・コメントは許すが、改行や改行を含むblock comment越しのheaderは `PRISMA_INVALID_BLOCK` とし、閉じたblockを読み飛ばす。
- ASCII識別子のmodel / field / type名。型は宣言textとして `String`、`Int`、任意識別子、直結した `?` または `[]` を保持。型参照や実DB column / foreign keyとの対応は解決しない。
- fieldは改行区切り。単一fieldを同一行のblockに書くことも可能。field名と型は同一行。fields配列は対応fieldの宣言順。
- `//` / `///` / `/* ... */` コメント、escapeを含む二重引用文字列、括弧 / 配列 / brace の境界を区別。文字列・コメント中の偽modelやbraceは構文扱いしない。元textを置換せず、UTF-8 byte位置でtokenを走査し1-based lineを数える。LF / CRLFに対応。
- `@id`、`@default(...)`、`@relation(...)`、`@map(...)`、`@@map(...)` 等の属性を安全に読み飛ばす。引数は複数行、文字列、配列、入れ子括弧を許す。属性の意味、default評価、mappingは取得しない。
- `generator` / `datasource` / `enum` / `type` / `view` の5 keywordと完全一致する、同一行headerの閉じた非model blockだけを無診断で読み飛ばす。未知keyword（`modle` 等）は `PRISMA_UNSUPPORTED_TOP_LEVEL` を残す。どちらもblock内部のmodel風文字列・tokenを拾い直さない。

出力は `canonical_id("database_model", &[file, model_name])`、schema上のname、canonical file、開始line / 閉じbraceのendLine、parentなし。evidenceは `source=prisma, confidence=confirmed` と同じ位置。confirmedはsourceの宣言を確認した意味であり、migration適用、実DB、Client生成成功を示さない。

metadataは `fields: [{"name":"id","type":"String"}]` のみ。全文・コメント・属性引数・URL・configを保存しない。独立field nodeやedgeは作らず、reads / writesやcallAnalysisも変更しない。

## 診断・回復

| 状態 | 結果 |
|---|---|
| schemaなし / 正常な0 model | 空node、存在否定はしない |
| 未知top-level / 不正header | それぞれ `PRISMA_UNSUPPORTED_TOP_LEVEL` / `PRISMA_INVALID_BLOCK`。閉じたblockの境界を確認できれば全体を読み飛ばし、次の独立宣言へ進む |
| 閉じmodel内の未対応field / 属性 | `PRISMA_UNSUPPORTED_FIELD`、当該宣言を除く部分一覧 |
| delimiter不一致（属性引数内の早すぎる `}` 等） | `PRISMA_BLOCK_BOUNDARY`、当該blockはmodel終端を確定できないためnodeなし、残りを推測しない |
| model block未閉鎖 | `PRISMA_UNCLOSED_BLOCK`、当該nodeなし、残りの解析停止 |
| string / comment未閉鎖 | `PRISMA_LEXICAL_ERROR`、安全に完了済みのmodelだけ保持、残りから再同期しない |
| 同名model | `PRISMA_DUPLICATE_MODEL`、収集段階で当該nameの全nodeを抑制（不完全な同名headerも計数） |
| 同名field | `PRISMA_DUPLICATE_FIELD`、当該nameの全fieldを除く。上書き / first winsなし |
| unsafe read / 境界違反 | Error。malformed成功結果として扱わない |

診断はwarning、固定の短いmessage、相対file・位置（探索診断はlineなし）。生成しないIDを参照しないためPrisma診断のrelatedNodeIdは省略する。skippedCountなし、call coverageには加算しない。findingsは探索診断を全て保持する。Resolver併用時は `findings.apply(&mut builder, resolver.diagnostics())` とし、Resolver側が所有・適用する `DISCOVERY_*` との完全一致だけ追加を省略する。単独利用は `&[]`。渡した診断を所有するcomponentは同じBuilderへ適用すること。新規・内容の異なる探索診断とPrisma診断は省略しない。Builderに汎用重複排除を導入しない。

同じfindingsの再適用でnode/evidenceは既存Builderの同一値統合が働くが、通常Diagnosticは追記される。完全冪等ではない。calls batch自身は既存どおり再適用不可。

未対応: 完全なPrisma意味検証、複数schema統合 / 分割.prisma、非ASCII識別子、Unsupported(...)型、複合suffix、同一行の複数field、field型をまたぐ改行、enum / composite / view node、relation graph、query推論、config / 環境変数 / .env読込み、Prisma CLI / DB接続。

## 検証契約

`tests/prisma.rs` はoracleを更新せず、fixture database_model 2件のID集合・kind/name/file/開始line・evidence source/file/開始line/confidence・parentなしをprojection比較する。別の手定義assertでUser (2–5: id String, name String)、Token (7–10: id String, value String)のfields順序とnode/evidence endLineを検証する。余分なDB nodeは集合比較、余分なedgeは適用前後の全edge比較で拒否する。

directory aliasを持つrepoでもResolver併用時に共有探索診断が1件だけ残ることを検証する。部品harnessで非Prisma nodes / 全既存edges / diagnostics / metadataが適用前後で不変、calls 9/2/7・2 edgesを確認する。full CLI、#18、#26 E2Eの完成証拠ではない。

独立テストは候補作成順、標準path優先、no fallback、コメント / 文字列 / CRLF、malformed / 重複 / source位置、alias / exclusion / loop / outside root、UTF-8 content、取得後のsource変更を扱う。private read境界のテストはdiscovery後の削除・外向き/除外先symlink・directory・FIFO・読込み権限を確認する。不正UTF-8 filenameを実際に作る探索試験はLinux限定（macOS filesystemは作成自体を拒否）。同時改変の完全隔離試験ではない。
