# Analyzer structural regression (#18)

`cargo test --workspace --locked --test structural_regression` で実行する。通常の `cargo test --workspace --locked` にもintegration testとして含まれ、既存CIのRust test入口から実行される。Hosted実行の確認はPR納品時の工程であり、ローカル成功をHosted成功とは扱わない。

## ActualとExpected

Actualは、使い捨てrootへ `examples/nestjs-sample` の `src`、`prisma`、`tsconfig.json` **だけ**をコピーし、そのrootに `pipeline::analyze` を呼んだ `SystemGraph`。Analyzerに手定義oracleや旧 `.codebasecanvas` を入力しない。Expectedは#4でsourceと独立に手定義した `expected-graph.json` と、下記の限定されたsource・v0.1契約由来の補助期待値。両Graphを既存 `SystemGraph::validate` で検証してから比較する。比較helperはtest専用で、Graphを補正・書換えたり、第二のproduction validatorを作ったりしない。#17の実process CLI・保存・fatal試験はそのまま残す。

## 比較範囲

- Nodeはcanonical IDで照合し、集合の不足・余分とduplicate IDを別に報告する。同名でもIDが違えば別node。kind、name、parentId、file、開始line、qualifiedName、metadataのkey/value、各node自身のEvidenceを比較する。
- EdgeはIDで照合し、from→kind→toの向きと集合、metadataのkey/value、各edge自身のEvidenceを比較する。`requested_token` 等の必須metadataとconfidenceを対象edgeに帰属させる。nodeのconfidenceでedgeを代用しない。
- Evidenceは配列順を無視し、source・repo相対file・開始line・confidenceと重複件数をentityごとに比較する。oracleにendLineがある場合はそれも比較する。診断はcode+file+lineを位置のkeyにし、severity・relatedNodeId・skippedCountを比較するmultiset。同一診断の追加適用をSet化で隠さない。
- Graph metadataはschemaVersion、実package由来analyzerVersion、注入したanalyzedAt、選択rootの末尾名rootName、callAnalysisのscope/mode/3件数を独立に確認する。oracle専用の `hand-authored-fixture-v1`、固定時刻・rootNameを本番に要求しない。node kind、edge kind、診断codeごとの期待件数もfixture READMEから別assertする。
- oracle向け比較から診断messageは除く。Graph validatorの非空条件に加えて、source rootの絶対pathがmessageに混入しないことをassertする。反復解析ではmessageも含む全内容を比較するため、同一入力での文言変化を見逃さない。oracleとの診断全文一致は主張しない。

oracle記述と現行sourceの既知の差は、任意のignore設定ではなく個別に扱う。decorator付き6 methodの開始lineは旧oracleが宣言行、現行source上の開始位置はdecorator行であるため、その6つのnodeと対応するAST/contains Evidenceだけを明示したsource行で比較する。`AuthController.login` は7、`OverrideController.list` は10、`UsersController.list/alias/update/remove` は7/10/12/14行。`MockUsersService.list`（同じfile/name）は対象に含めない。2 Repositoryはdecorator行3のconfirmed根拠と宣言行4のbest_effort根拠を両方要求する。旧oracleにない `AuthModule→AuthService` のexports使用位置10行、`UsersModule→UsersService` のexports使用位置9行も2つのimports edgeだけへ追加して比較する。これらのsource行はfixture sourceから別assertする。

旧oracleにないmethod qualifiedNameは、手定義されたowner qualifiedNameとmethod名から期待値を作る。fixtureのclass-like/interface宣言に付く `exported: true` とPrisma fieldsを明示補助期待値として要求する。Prisma Userは2–5行・`id:String/name:String`、Tokenは7–10行・`id:String/value:String`。field順、nodeとPrisma EvidenceのendLineを独立assertする。TSのendLineは旧oracleにないため、`UsersService.choose` とそのEvidenceの10–15行、UsersService classの16行をsource由来の代表値としてassertし、残りのTS endLineは現時点でoracle値との完全比較対象外とする。残る値には既存validatorの範囲条件と、同じrootの反復解析での全内容一致を適用する。oracle比較を全Graph全文一致とは呼ばない。

## 守る関係と禁止する関係

fixtureは57 nodes / 90 edges / 15 warnings、callsのexamined/emitted/skippedが9/2/7、deduplicate後のcalls edgeが2、Prisma modelが2。IDと関係を用いて、別scopeのAlpha.Same/Beta.Same、static/instance find、別subpathのmap、UsersServiceの1 node・2 membership・parentなし、2つのGET /usersの別handlerを確認する。AppModuleのModule importはdepends_onであってcontainsではない。forwardRefのUsersModule binding使用によるimportsは保持するが、未確認のdepends_onは作らない。UnsupportedModuleの安全なUnsupportedConsumer membershipは保持し、spread/provider objectからUsersService membershipを作らない。

DIはOverrideController→**要求token** UsersServiceのinjectsを保持し、interface、type-only、custom Injectやprovider overrideの実装MockUsersServiceへ付け替えない。AuthService.login→normalize、UsersService.choose→instance findのcallsだけを確認し、injected receiver5件・computed1件・nested1件をmethod/reason/代表位置付きunknownとして残す。Prismaへreads/writesを作らず、Prisma診断をcallAnalysis・skippedCountに混ぜない。禁止edgeごとにconsumer/token/handlerや正常な兄弟関係の存在をpositive controlとし、空Graphでの偽合格を防ぐ。

fixtureにない4種類のprovider object（useClass/useValue/useExisting/useFactory）、動的routeと正常な兄弟route、同一caller/calleeの複数call siteから1 edgeへの統合、同method/reasonでskippedCountが2になるケースは、最小の独立sourceを同じ本番pipelineへ通して確認する。新しいrecognizerや解析文法は追加しない。

## 比較器と決定論性

比較器の小さな手定義Graphにnode/edge欠落・余分・duplicate ID、同件数のedge接続先変更、kind/parent、Evidence source/line/confidence、metadata、診断の欠落・重複・severity/relatedNodeId/skippedCountを一つずつ加え、失敗を確認する。この意図的に壊したGraphをproduction validatorの正常入力と扱わない。差分はIDと分かるname/fileまたはfrom→kind→toを含み、`missing node`、`unexpected edge`、`changed ... expected ..., actual ...`、`extra occurrence diagnostic` 等を区別する。順序は決定論的で、先頭24件まで表示する場合は総件数と省略件数も残す。巨大なJSON二つをdumpしない。

同じrootを異なる**有効**時刻で2回本番解析し、`analyzedAt`以外のnode/edge/metadata/Evidence/診断messageを含む内容を比較する。比較器への配列順を変えても同じ結果とし、元Graphが不変であることを確認する。固定時刻では正規JSON全文一致も維持する。sleep・retry・別root間でrootName差を消す正規化は使わない。

oracleや補助期待値を変更するときは、変更理由、product meaning、scope expansionかbug fixかを説明し、source・README・契約と一緒にレビューする。Analyzer出力をexpectedへコピーしたり、snapshotを自動更新したりすることは正しさの根拠にならない。
