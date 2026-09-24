# Analysis pipeline (#17)

リポジトリルートから実行する例:

```sh
cargo run --locked -p codebasecanvas-analyzer -- analyze /path/to/disposable-repository
```

成功時の出力は選択rootの `.codebasecanvas/graph.json`。source、graph、diagnosticを外部へ送信しない。CLIは `output::Repository::open` でrootを一度捕捉し、その `RepositoryRoot` をlibraryの `pipeline::analyze` と安全な `Repository::save` の双方へ渡す。別の文字列pathを解析側で再解決しない。

## 接続順・診断の所有

1. `ImportResolver::analyze(root)` がTS/TSX sourceを読み、同じsource snapshotと探索・parse・import診断を保持する。
2. 各sourceを `nestjs_roles::extract_file` で一度抽出し、宣言・method・roleをBuilderへ投入する。generic抽出を重ねない。
3. Module、route、DI、callsをこの順に解析・適用する。callsは0件でも一度だけbatch適用する。
4. `resolver.apply_imports` がResolverの診断を一度適用し、import edgeを追加する。
5. `prisma::analyze(root)` が代表schemaを選択して読み、一度適用する。同じBuilderに適用したResolver診断と完全一致する共有 `DISCOVERY_*` のみ省き、Prisma固有や異なる診断は保持する。
6. `builder.finish()` が矛盾・graph invariantを検証し、`Repository::save` が正規JSONを再検証・serializationしてから安全に置換する。保存成功後だけCLI summaryを表示する。

Resolverが保持するTS source以外をpipelineは再読しない。Prismaの選択sourceはPrisma入口で一度読む。ResolverとPrismaの探索は別実行であり、repository全体の原子的snapshotを保証しない。

## 結果と失敗

Graphは既存schemaVersion `0.1`。`analyzerVersion` はRust package version、`analyzedAt` は実行ごとに一度取得したUTC秒精度 `YYYY-MM-DDTHH:mm:ssZ`、`rootName` は選択rootの最終要素（安全な名前がなければ `repository`）。日時は標準ライブラリの `SystemTime` を小さなUTC暦変換で表す。時刻取得・変換失敗はエラーであり、固定fixture時刻や1970年には代替しない。テストは明示時刻をlibrary入口へ渡す。絶対root pathや解析時間はGraphへ入れない。

CLI summaryの「TypeScript/TSX source files」はResolverが**読んだ**件数で、parse失敗fileも含む。Prisma schemaは選択・読込済みの0または1件で、未選択候補は含まない。node/edgeとinfo/warning/errorは保存Graphの件数。error診断を含む場合は部分解析の注意を表示する。解析経過時間が必要ならlibrary呼出し前後で測れるが、Graphへ混ぜない。

対象sourceが0件なら空Graphと0件summaryで成功し得る。sourceがあっても宣言0件なら読込件数を残す。純粋なTS parse/semantic errorは既存のerror診断を持つ部分Graphとして保存でき、全TSがparse不能でも「完全解析済み」とは表示しない。正常fileとparse不能fileが混在すれば正常側の結果を保持する。PrismaのみのrepoはTS 0件・schema 1件を表示する。CLI成功は対応範囲の処理と保存の完了であり、全sourceの正しさや完全解析の保証ではない。

root/discovery/source read等のError、安全な入力・出力境界違反、内部処理・Builder・validation・serialization・writeの失敗はfatalで終了コード1。空Graphや診断だけの成功へ変換しない。個別import未解決、未対応構文、動的route、未対応DI、回復可能なPrisma block/fieldは診断として継続する。error診断の存在だけではfatalにしないが、Graph invariant違反をedge削除で通すこともしない。helpは0、引数不正は2。

保存は既存の固定出力先とUnix directory-relative I/Oを再利用する。検証・serialization前に出力directoryを作らず、root identity、出力directoryとtargetのsymlink/通常file検査、排他的temp、sync、同directory rename、失敗時temp cleanupと旧snapshot保護を維持する。file置換の原子性はrepository snapshot、同時writerの完全隔離、電源断後のdirectory entry永続性を意味しない。非Unixの安全な保存は未提供。

使い捨てsource copyのfixture統合試験は57 nodes・90 edges・15 diagnostics、calls 9/2/7、Prisma 2 modelを手定義oracleのID・構造projectionおよび独立したfields/endLine期待値と照合する。実processのCLIも同じ使い捨てcopyを2回解析し、source変更後の再生成、parse errorとfatal、旧出力保護を確認する。追加の正当なendLine・export metadata等があるため、node全文と旧oracleの完全一致は主張しない。#18の詳細構造回帰、#26のブラウザE2E、実repo評価、性能評価とは別の証拠である。
