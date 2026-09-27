# TypeORM declaration contract spike — PROPOSED / NOT_IMPLEMENTED

対象は Issue #30 の固定監査で `unknown` だった14 source項目だけである。全体の状態は **PROPOSED / NOT_IMPLEMENTED**、**SystemGraph v0.2 candidate** であり、現行の正式契約は [DATA_MODEL.md](../DATA_MODEL.md) の **v0.1** のまま。`typeorm_repository_request` に限る[実験的な契約層](../DATA_MODEL_V02_DRAFT.md)には専用Rust/Web validatorと共有wireケースがあるが、公開Graph・実Analyzer抽出・本番File入力には適用していない。[手書き受入ケース](typeorm-declaration-cases.json)も実 Analyzer の抽出結果ではない。既存技術監査は **COMPLETED / EXCEEDED**（Module 5/21、DI 9/14、Endpoint 0/21）のまま、#30 Decision は NOT_EVALUATED、人間 UX / source-first は NOT_RUN である。

## 境界と選択

| 案 | 意味と互換性 | 判断 |
|---|---|---|
| A: v0.1維持 + 独立した補助報告 | 宣言の事実を Graph 外の監査 sidecar に記録する。既存 `injects` / `contains` / `depends_on` と unknown は不変。旧 viewer は sidecar を読まず、現行 validator への影響はない。UI / Context に出すには別の明示的 reader と契約が要る。 | 監査記録には安全だが、利用者向け宣言表示の共有契約にならない。 |
| **B: v0.2 の型付き `frameworkDeclarations`** | Graph 最上位へ `repository_request`、`for_feature`、`for_root`、`external_class_request` を追加する。source事実とversion規則による導出を別fieldにする。既存edgeの意味は変えない。Rust wire / Web validator / import / UI / Context の同時移行が必要。旧 viewer は v0.2 を拒否する。 | **推奨**。open `metadata` や既存 `external_dependency` ID へ新しい意味を隠さず、誤関係を防げる。 |

提案 JSON は `schemaVersion: "0.2"` の **新しい明示field** とする。v0.1の `nodes`、`edges`、`diagnostics` はそのまま残し、ここに挙げる宣言から v0.1 の `injects`、Module `depends_on` / `contains`、`calls`、`reads` / `writes` を合成しない。特に `@InjectRepository(User)` は consumer→User の `injects` ではない。`Repository<User>` の型引数だけ、`forFeature([User])` の登録候補だけ、Entityの `@Entity` だけでもこれらのedgeを作らない。TypeORM Entityは Prisma `database_model` ではない。v0.1で保存・配信すれば新fieldはvalidatorに拒否されるべきであり、open `metadata` への迂回はしない。

## 固定根拠と上流規則

入力は `lujakob/nestjs-realworld-example-app` の commit [`c1c2cc4`](https://github.com/lujakob/nestjs-realworld-example-app/tree/c1c2cc4e448b279ff083272df1ac50d20c3304fa)。root [`package.json`](https://github.com/lujakob/nestjs-realworld-example-app/blob/c1c2cc4e448b279ff083272df1ac50d20c3304fa/package.json) の raw SHA-256 は `673b7b0e871621f465d76cfdeafa00df6c233f78e35ec636181b71424e1d63d2`、[`package-lock.json`](https://github.com/lujakob/nestjs-realworld-example-app/blob/c1c2cc4e448b279ff083272df1ac50d20c3304fa/package-lock.json)（lockfileVersion 1）は `d5e2bbd08d770652e964b0f540b2c3253f948805eefa1bf6e912ecf6999e16c0`。宣言範囲と解決値は別である。

| package | manifestの宣言 | lockfileの値 |
|---|---|---|
| `@nestjs/typeorm` | `^7.0.0` | `7.0.0` |
| `typeorm` | `^0.2.24` | `0.2.24` |
| `@nestjs/common` / `@nestjs/core` | 各 `^7.0.5` | 各 `7.0.5` |

固定入力のroot `tsconfig.json`（raw SHA-256 `0da1a8198a9dbed9fcc80b412af5b621667aa36ea8b2fa976e9c31b2587f951f`）には `experimentalDecorators: true` と `emitDecoratorMetadata: true` がある。外部 `Connection` のclass-token要求候補には、この設定が確認できることを別の必須条件とする。設定が無効・不明なら、constructorの型参照と局所診断だけを残し、`typeorm_external_class_request` recordも要求token descriptorも作らない。

`@nestjs/typeorm` tag `7.0.0` は commit `ac077cba634b4d19558bb767cb17c04b5ee57936`（annotated tag object `3ebf2de86162f4340c81a2b922af04923e32fbf2`）。次は実装sourceの raw SHA-256であり、現行 `master` の DataSource 実装とは混同しない。

| source（tag固定） | raw SHA-256 | この案で使う根拠 |
|---|---|---|
| [`lib/common/typeorm.decorators.ts`](https://github.com/nestjs/typeorm/blob/ac077cba634b4d19558bb767cb17c04b5ee57936/lib/common/typeorm.decorators.ts) | `3276dc57fd2fd4df4cf1ba94344592afdc65ef05f81eac09ceba0454d3e76560` | `InjectRepository(entity, connection)` → `Inject(getRepositoryToken(...))` |
| [`lib/common/typeorm.utils.ts`](https://github.com/nestjs/typeorm/blob/ac077cba634b4d19558bb767cb17c04b5ee57936/lib/common/typeorm.utils.ts) | `8395dc1f59d3471724312c4053cbd7af0dff38fd354d0720580b72452af633f9` | `getRepositoryToken`、`getConnectionPrefix`、`getConnectionToken` |
| [`lib/typeorm.module.ts`](https://github.com/nestjs/typeorm/blob/ac077cba634b4d19558bb767cb17c04b5ee57936/lib/typeorm.module.ts) | `1bbdd421db525ba40de3af9a04685a537bd3ee1fc51df39dc6ccea19c15a6eca` | `forFeature`のproviders/exportsと`forRoot`のcore import |
| [`lib/typeorm.providers.ts`](https://github.com/nestjs/typeorm/blob/ac077cba634b4d19558bb767cb17c04b5ee57936/lib/typeorm.providers.ts) | `4d30a34548384ff5478d6e63e5e7a768d2c0aac813b3a7d61fe664c349c1de59` | entityごとのprovider宣言、factoryのruntime分岐 |
| [`lib/typeorm-core.module.ts`](https://github.com/nestjs/typeorm/blob/ac077cba634b4d19558bb767cb17c04b5ee57936/lib/typeorm-core.module.ts) | `08c838aac9f91013894564c0e158580b0e5bb97f149211f64aac1bd7f4cfe866` | `forRoot`のConnection provider候補、外部設定と実行時処理 |

TypeORM tag `0.2.24` は commit `4ed79c9cf9ad0f1e51930159dfdab8c1e8549339`。[`src/connection/Connection.ts`](https://github.com/typeorm/typeorm/blob/4ed79c9cf9ad0f1e51930159dfdab8c1e8549339/src/connection/Connection.ts)（SHA-256 `1940377cca8bac4ed35bf6af25cf36349cd1c555aa8a38e4278bdde636a2125d`）は `export class Connection`、[`src/index.ts`](https://github.com/typeorm/typeorm/blob/4ed79c9cf9ad0f1e51930159dfdab8c1e8549339/src/index.ts)（SHA-256 `492f60c5f98d93650d915b238a2cbe92a9bf66643c620b8f2b26bd56970fecbf`）はそれを公開exportする。この固定profileでは外部 `Connection` の class *宣言とexport*を確認できる。一方、一般の external import を一律classとみなさない。後の [TypeORM 0.3.0 `DataSource`](https://github.com/typeorm/typeorm/blob/941b584ba135617e55d6685caef671172ec1dc03/src/data-source/DataSource.ts) の名前・規則をこの `Connection` profileへ移植しない。

7.0.0 の規則を静的な条件として表す。

| 呼出し・条件 | 導出できる descriptor | 導出できないこと |
|---|---|---|
| `InjectRepository(E)`、通常のローカルclass値 `E`、接続省略または文字列 `"default"` | string token `E.name + "Repository"`。`getConnectionPrefix` は両方で空文字。 | provider実在・注入・EntityへのDB操作。 |
| 同じ `E`、名前付き文字列 `"reports"` | string token `"reports_" + E.name + "Repository"`。`getConnectionToken("reports")` は別の string `"reportsConnection"`。 | defaultへのfallbackやModule可視性。 |
| `Repository` / `AbstractRepository` 派生class | `getCustomRepositoryToken` はclass名を返し、通常Entityの `Repository` suffixを付けない。 | 継承関係を確認できなければ通常Entity規則を適用しない。初期対応外。 |
| `forFeature([E], c)` | `createTypeOrmProviders` は各 entry を `getRepositoryToken(E,c)` のprovider候補とし、DynamicModuleはそのprovidersをexportsへ載せる。 | 実際のprovider選択、可視性、factory成功、DB接続。 |
| `forRoot()` | `TypeOrmModule` は `TypeOrmCoreModule.forRoot(undefined)` をimportし、core側の既定options `{}` で `getConnectionToken({})` は class `Connection` を指す。 | `createConnection()` が読む外部設定、認証情報、接続成功、最終構成。任意options objectはGraphに複写しない。 |

`"default"` の省略／明示だけを同じ normalized connection とする。未知の式、type-only/namespace/re-export/wrapper、空文字や評価を要するobjectをdefaultへ落とさない。`package-lock.json` は再現profileの根拠であり、その現場で実際にinstallされたpackage instanceやruntime module解決を証明しない。

## 提案する型付き表現

`SystemGraph` v0.2へ `frameworkDeclarations: FrameworkDeclaration[]` を **必須の配列**として加える。空なら `[]`。v0.1のnode/edge/diagnostic IDは維持し、この配列を既存edgeへ暗黙変換しない。Rust wireとWeb validatorは同じstrict unionを実装する。`kind` は `typeorm_repository_request`、`typeorm_for_feature`、`typeorm_for_root`、`typeorm_external_class_request` の4値。`ownerId` は既存のsource class/module nodeを指す。`site` はroot相対path、0始まりUTF-8 byte span、1始まりlineに加え、requestなら0始まり`parameterIndex`、Moduleなら`metadataField:"imports"`と0始まり`metadataEntryIndex`を持つ。`forFeature.entries[]` は0始まり`entityIndex`と個別source位置を持ち、未知entryと正常な兄弟を混ぜても各境界を保つ。

`origin` は正規package specifier、元export名、使用位置のlocal alias、lock値を分ける。`entityRef` は既存のsource class canonical IDと宣言名を持ち、未確定なら省略して局所診断を残す。`connection` は `omitted` / `literal` / `unknown_expression`、正規化できた場合のみ `normalizedName` を持つ。`token` は、条件を満たす場合だけ `state:"derived_under_profile"`、`kind:"string"`、実文字列と `comparisonKey` を持つ。別の `derivation` はrule ID、固定上流source hashと成立条件を持ち、外部Connectionではexport indexとcompiler設定のraw hashも持つ。未知の式・version・class分岐では `token` に `state:"unknown"` と `reason` だけを持ち、`derivation` と推測文字列を入れない。`forRoot`と外部Connectionの class-token descriptorは `kind:"external_class_export"` と固定 `typeorm@0.2.24/Connection` の *静的export参照*を持つが、runtimeで同一class objectがロードされた保証はない。`evidence[].claim` は `decorator_origin`、`argument_reference`、`metadata_entry`、`constructor_type` のどのsource事実に対する `confirmed` かを指定する。tokenの規則適用を単に `best_effort` edgeにしない。

提案のidentityは3層を独立させる。

1. **Entity source ID**: 現行 `canonical_id("class", [root相対file, lexical scope..., 宣言名])`。同名別fileは別ID。
2. **出現ID**: `canonical_id("typeorm_decl", [root相対file, kind, decimal startByte, slot])`。ここで `kind` はJSONの完全な判別子（例: `typeorm_repository_request`）そのもので、省略形へ変換しない。`slot` は `parameter:<index>` または `imports:<entryIndex>`。`forFeature.entries[]` は `canonical_id("typeorm_entry", [file, callStartByte, entity:<index>])`。同じEntityを複数parameter・Moduleで使っても合算しない。owner IDをrecordで検証し、source上の位置が変わればIDも変わる。
3. **token比較ID**: string tokenは `canonical_id("nest_string", [実際のtoken文字列])`。同じ接続・同名の別file Entityは、source IDが別でも同じstring比較IDとなり、**衝突候補**として双方を残す。同じEntityでも接続名が異なれば別比較ID。class tokenはpackage/version/exportの静的参照で比較候補とするが、runtime class object同一性とは呼ばない。prefixやpackage名をstring token比較IDに追加して衝突を隠さない。

次は v0.2 の frameworkDeclarations 部分だけの具体例であり、完全な SystemGraph JSONではない。UserService requestは監査 item DI-131、UserModule callは MODULE-114 に対応する。出現位置は固定sourceのUTF-8 byte offsetである。

```json
{
  "frameworkDeclarations": [
    {
      "id": "typeorm_decl:7372632f757365722f757365722e736572766963652e7473:747970656f726d5f7265706f7369746f72795f72657175657374:363438:706172616d657465723a30",
      "kind": "typeorm_repository_request",
      "ownerId": "class:7372632f757365722f757365722e736572766963652e7473:5573657253657276696365",
      "site": {
        "file": "src/user/user.service.ts",
        "startByte": 648,
        "endByte": 677,
        "line": 17,
        "parameterIndex": 0
      },
      "origin": {
        "specifier": "@nestjs/typeorm",
        "exportedName": "InjectRepository",
        "localName": "InjectRepository",
        "lockedVersion": "7.0.0"
      },
      "entityRef": {
        "id": "class:7372632f757365722f757365722e656e746974792e7473:55736572456e74697479",
        "name": "UserEntity"
      },
      "connection": {
        "syntax": "omitted",
        "normalizedName": "default"
      },
      "token": {
        "state": "derived_under_profile",
        "kind": "string",
        "value": "UserEntityRepository",
        "comparisonKey": "nest_string:55736572456e746974795265706f7369746f7279"
      },
      "derivation": {
        "rule": "@nestjs/typeorm@7.0.0/getRepositoryToken:ordinary_class",
        "sourceSha256": "8395dc1f59d3471724312c4053cbd7af0dff38fd354d0720580b72452af633f9",
        "conditions": [
          "verified package-lock profile",
          "non-inherited local class value",
          "default connection"
        ]
      },
      "evidence": [
        {
          "claim": "decorator_origin",
          "source": "resolver",
          "file": "src/user/user.service.ts",
          "line": 17,
          "confidence": "confirmed"
        },
        {
          "claim": "argument_reference",
          "source": "resolver",
          "file": "src/user/user.service.ts",
          "line": 17,
          "confidence": "confirmed"
        }
      ],
      "runtime": {
        "providerExistence": "unverified",
        "visibility": "unverified",
        "injection": "unverified"
      }
    },
    {
      "id": "typeorm_decl:7372632f757365722f757365722e6d6f64756c652e7473:747970656f726d5f666f725f66656174757265:333532:696d706f7274733a30",
      "kind": "typeorm_for_feature",
      "ownerId": "class:7372632f757365722f757365722e6d6f64756c652e7473:557365724d6f64756c65",
      "site": {
        "file": "src/user/user.module.ts",
        "startByte": 352,
        "endByte": 390,
        "line": 9,
        "metadataField": "imports",
        "metadataEntryIndex": 0
      },
      "origin": {
        "specifier": "@nestjs/typeorm",
        "exportedName": "TypeOrmModule",
        "localName": "TypeOrmModule",
        "lockedVersion": "7.0.0",
        "method": "forFeature"
      },
      "connection": {
        "syntax": "omitted",
        "normalizedName": "default"
      },
      "entries": [
        {
          "id": "typeorm_entry:7372632f757365722f757365722e6d6f64756c652e7473:333532:656e746974793a30",
          "site": {
            "file": "src/user/user.module.ts",
            "startByte": 378,
            "endByte": 388,
            "line": 9
          },
          "entityIndex": 0,
          "entityRef": {
            "id": "class:7372632f757365722f757365722e656e746974792e7473:55736572456e74697479",
            "name": "UserEntity"
          },
          "token": {
            "state": "derived_under_profile",
            "kind": "string",
            "value": "UserEntityRepository",
            "comparisonKey": "nest_string:55736572456e746974795265706f7369746f7279"
          },
          "derivation": {
            "rule": "@nestjs/typeorm@7.0.0/getRepositoryToken:ordinary_class",
            "sourceSha256": "8395dc1f59d3471724312c4053cbd7af0dff38fd354d0720580b72452af633f9",
            "conditions": [
              "verified package-lock profile",
              "non-inherited local class value",
              "default connection"
            ]
          }
        }
      ],
      "evidence": [
        {
          "claim": "metadata_entry",
          "source": "nestjs",
          "file": "src/user/user.module.ts",
          "line": 9,
          "confidence": "confirmed"
        }
      ],
      "runtime": {
        "providerExistence": "unverified",
        "visibility": "unverified",
        "injection": "unverified"
      }
    }
  ]
}
```

この例は UserModule.forFeature([UserEntity]) の1要素をすべて示す。JSONのsource位置とtoken値は固定source/上流規則に基づく提案期待値で、実 Analyzer が抽出した証拠ではない。tokenのstring比較IDが同じでも、Module可視性や注入成功は未確認である。

## supported / unknown の判定と受入入力

[受入ケースJSON](typeorm-declaration-cases.json)は短い手書きsource、positive control、期待するsource事実・条件付き導出・禁止edge・診断・旧v0.1と新案の期待を1 caseずつ保持する。[追加の提案JSON例](typeorm-declaration-proposal-examples.json)は同じケースのalias、named connection、同名別file、forFeature、unknown式をrecord形で示す。`confirmed` はsource上のorigin / 参照 / 位置にだけ付く。`derived_under_profile` は固定規則に従う比較descriptorで、runtime保証ではない。`entityIndex` はsource配列の位置であり、spread展開後のruntime配列位置を推測しない。

| 境界 | 新契約での結果 |
|---|---|
| exact named value importとalias、local class値、単一の直接call / array literal / string connection | 対象出現のsource事実を記録。通常classの `extends` がなく、固定依存profileが一致する場合だけtoken descriptorを導出。 |
| 同名foreign decorator、local shadow、type-only、re-export、namespace、wrapper、動的配列・spread、未知connection | 該当出現だけ unknown 診断。安全な兄弟recordやownerを残す。名前だけのfallback・全repo走査をしない。 |
| custom Repository / AbstractRepository、EntitySchema、`extends` 未解決 | 出現と参照事実は残せても通常Entity tokenは導出しない。固定7.0.0のcustom分岐を別に設計するまで局所unknown。 |
| 版不明、manifestとlockfileの矛盾、patch未確認 | syntax事実だけ。version規則・token descriptor・candidate matchを作らず、原因を局所診断する。 |
| 外部 `Connection` | fixed `typeorm@0.2.24` のclass宣言/export、正規import、constructor type、必要なtsconfig metadata条件に加え、そのparameterに明示的な`@Inject`や出自不明のdecoratorがない場合だけ class-token **要求候補**。provider・class object同一性・注入成功は不明。 |

[case C18](typeorm-declaration-cases.json) は固定root tsconfigのhashと両compiler optionを明示した入力である。C19はTypeORM版が不明、C22はmetadata設定が不明またはfalseという2変種で、いずれもclass-token要求recordを出さない。C23では正規`@Inject('ALT')`が型メタデータより優先され、出自不明decoratorもtype由来の要求を抑止する。両parameterは型参照と局所診断だけを残し、decoratorのない正常な兄弟parameterだけclass-token要求候補にする。この抑止は型から推定する外部class経路に適用し、正規`@InjectRepository`自体を無効化しない。後続実装は設定やdecoratorの意味が不明な状態を有効と推定しない。

未知なentryを確定した隣のentryへ伝播させない。ただし同一 `forFeature` の connection引数やorigin/版が不明なら、そのcallの全entryについてtoken導出は未知とする。tokenが同じrequestと登録entryを見つけても、表示は「token文字列一致の候補」であり、Graph edgeやruntime選択ではない。同名別fileの衝突時は全候補を列挙し `typeorm_token_name_collision` を付け、単一targetへ勝手に縮約しない。

## 入力証跡・互換性・利用者表示

将来version規則をAnalyzerへ入れるなら、root `package.json` の対象dependency範囲、選択した `package-lock.json` の lockfileVersion / package解決version / integrity、root `tsconfig.json` のdecorator関連設定が新たな解析入力になる。root containment と symlink拒否を既存RepositoryRoot境界で行い、各fileの存在/不存在とraw SHAを入力証跡へ記録する。lockfileがない・複数種ある・manifest/lockが矛盾する・override/patchがある・対象versionが未対応ならsource事実だけ残し、token導出を局所unknownにする。暗黙のnpm install、node_modules実行、外部取得、最新versionへのfallbackはしない。分析中の同時filesystem改変に対する完全隔離は、この案だけでは保証しない。接続URL/password/環境変数や任意 `forRoot` options objectをGraphへ複写しない。許可するのは、source位置、connectionの省略/安全なliteral/unknown、必要なpackage版・hashだけである。

採用時は Rust wire / Web validator と共有contract casesを同じ版で更新し、`canonical_id`とGraphBuilderに出現IDの検証を追加する。Resolverは使用位置のoriginal export・aliasとlocal class参照を証明し、Module/DI recognizerは該当出現だけを収集する。現行 `resolver.at_reference` / `import_for` / `local_reference` とOxcのdecorator、call、array、parameter spanおよび既存 `ModuleEntry.site` / `DiFinding.parameter_index` は材料になる。ただし TypeORM形の抽出自体は未実装・未実行で、namespace/re-export等の現行未対応境界を超えたとは主張しない。初回実装単位は **v0.2の `repository_request` だけを共有contract cases・Rust/Web validator・固定profile付きで通すこと**。forFeature候補matchingやCanvas表示はその単位の成功条件に混ぜない。

旧v0.1 Graphはversion別 readerで従来どおり読む。旧Graphに新しい宣言がないことを「TypeORMを使っていない」と表示しない。v0.2 Graphを旧viewerへ渡すとunsupported versionで拒否する。新viewerのCanvas初期表示は既存node/edgeのまま、owner detailsに「Entity `User` を参照」「framework生成token `UserRepository` を要求する宣言」「登録宣言を発見」「runtime注入成功・実装先は未確認」を分けて表示する。外部 `Connection` は「class token `typeorm.Connection` を要求する候補」とする。searchはEntity名・token文字列から該当ownerへ案内し、token collisionは別source出現を統合せず一覧にする。8要求を一つの汎用Repository nodeへまとめない。必要時だけ宣言を展開し、全宣言を初期Canvasに一括配置しない。bounded Copy Contextは選択ownerの直接の宣言だけを既存の件数・文字数上限内に載せ、省略数・unknown・runtime非保証を残す。DIから2-hop実装・calls・DB read/writeへ拡張しない。

## 旧監査14項目への設計上の見込み

下表は将来の **宣言情報 / 条件付きdescriptor** の見込みであり、実装・再監査の成功数ではない。14項目とも正式な新関係edgeを評価できる段階には進まない。`MODULE-*` は旧 `depends_on`、`DI-*` は旧 `injects` の未解決として残る。source対応は旧台帳の `itemId` をそのまま別評価版へ持ち、新しい宣言metricを追加する。意味が異なる表現を旧未解決率の単純改善にしない。

| 旧item | 固定sourceの出現 | 新案で増やせるもの | 引き続きunknown |
|---|---|---|---|
| MODULE-096 | `ApplicationModule` `forRoot()` | root宣言 + 条件付き `Connection` class-token descriptor | 設定、実接続、Module依存/可視性 |
| MODULE-102 | `ArticleModule` `forFeature([ArticleEntity, Comment, UserEntity, FollowsEntity])` | call + 4登録entry、各通常Entityの条件付きstring token | 実provider/Module関係 |
| MODULE-106 | `ProfileModule` `forFeature([UserEntity, FollowsEntity])` | call + 2登録entry、条件付きstring token | 同上 |
| MODULE-110 | `TagModule` `forFeature([TagEntity])` | call + 1登録entry、条件付きstring token | 同上 |
| MODULE-114 | `UserModule` `forFeature([UserEntity])` | call + 1登録entry、条件付きstring token | 同上 |
| DI-118 | `ApplicationModule` `connection: Connection` | 外部class宣言根拠付きの要求候補 + 条件付きclass-token descriptor | compiler emit / runtime class object / 注入成功 |
| DI-120 | `ArticleService` `@InjectRepository(ArticleEntity)` | 独立したparameter request + 条件付き `ArticleEntityRepository` | provider選択・注入成功 |
| DI-121 | `ArticleService` `@InjectRepository(Comment)` | 独立したparameter request + 条件付き `CommentRepository` | 同上 |
| DI-122 | `ArticleService` `@InjectRepository(UserEntity)` | 独立したparameter request + 条件付き `UserEntityRepository` | 同上 |
| DI-123 | `ArticleService` `@InjectRepository(FollowsEntity)` | 独立したparameter request + 条件付き `FollowsEntityRepository` | 同上 |
| DI-125 | `ProfileService` `@InjectRepository(UserEntity)` | 独立したparameter request + 条件付き `UserEntityRepository` | 同上 |
| DI-126 | `ProfileService` `@InjectRepository(FollowsEntity)` | 独立したparameter request + 条件付き `FollowsEntityRepository` | 同上 |
| DI-128 | `TagService` `@InjectRepository(TagEntity)` | 独立したparameter request + 条件付き `TagEntityRepository` | 同上 |
| DI-131 | `UserService` `@InjectRepository(UserEntity)` | 独立したparameter request + 条件付き `UserEntityRepository` | 同上 |

この14項目は固定Graphで `unsupported` と診断された妥当な旧結果である。supported期待関係のmissing / false / silent omissionは旧評価集合で未検出だが、宣言追加で必要な構造量が足りると主張しない。固定repo向け過学習の可能性があり、手書き境界ケースは外部妥当性の試験ではない。新契約実装後の別版評価と、未観測実repoでの確認が必要になる。

## Spikeの検証・レビュー記録

最初の専門レビューはC18の入力に `emitDecoratorMetadata` の前提が欠けていると **BLOCK** した。欠落したままでは、`Connection` のconstructor型参照だけから `typeorm_external_class_request` とclass-token descriptorを導出し得た。修正ではC18に固定root `tsconfig.json` のraw SHA-256と `experimentalDecorators: true` / `emitDecoratorMetadata: true` を入力として明示し、[提案JSON例](typeorm-declaration-proposal-examples.json)の導出根拠にもcompiler設定とTypeORMの公開export indexのraw hashを加えた。C19はTypeORM版不明、C22はmetadata設定が不明またはfalseとして、型参照と局所診断を残し、要求recordを作らない。固定設定が存在することは確認したが、compilerがその設定を使用した実行結果やruntime metadataの存在は確認していない。TypeORM意味論・Graph契約・誤接続防止を対象とした同じ専門レビュー担当による指摘範囲の読み取り再確認は **ALLOW** だった。これをCompanion Gateの結果とは扱わない。

設計PRの初回HEADに対する追加レビューは、出現IDの `kind` がJSON判別子の省略形になっていた点と、明示的 `@Inject` がある外部 `Connection` parameterに型由来tokenを付け得る点をP2として指摘した。出現IDはすべて完全な `kind` で再計算し、C23で `@Inject('ALT')` と出自不明decoratorをそれぞれ局所抑止する期待を追加した。既存の `explicit_inject_precedes_resolvable_or_invalid_type` は明示tokenの優先順位を示すが、このSpikeのC23をAnalyzerで実行した証拠ではない。

`python3 -m json.tool` で受入ケースと提案JSON例の構文を確認した。一時Pythonチェックで23 case ID、9 example参照、12 declaration record、2 feature entry、および固定sourceの2 recordについて、UTF-8 byte位置・行・完全な `kind` を含む `canonical_id`・string token比較ID・unknown条件を照合してPASSした。C23の明示token・未知decorator・正常な兄弟parameterの期待も照合した。固定入力と上流sourceのraw hashは `shasum -a 256` で照合し、修正後の `git diff --check` もPASSした。計算用Pythonは一時コマンドで、常設の自動testではない。同じ照合は両JSONを読み、case `files` と固定入力の原文をUTF-8 bytesで切り出し、上記3種類のID規則を適用して再確認できる。実Analyzer抽出probe、v0.2正式validator、Canvas/Context表示、ローカル完全CI、実repo再監査は未実施。Companion Gateも **NOT_RUN**（既知のモデル非対応HTTP 400を同じ設定で再試行していない）。
