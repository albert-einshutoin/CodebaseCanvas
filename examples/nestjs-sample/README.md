# NestJS 解析 fixture

Analyzer 実装前の独立した期待値です。source の静的意味を手で決めた [expected-graph.json](expected-graph.json) を正本とし、Analyzer の出力で上書きしません。ID のエンコードは [v0.1 契約](../../docs/DATA_MODEL.md) に従います。型検査はできますが、起動するアプリではありません。重複 route や未対応 DI を意図的に含みます。

```text
AppModule
├─ AuthModule
│  ├─ AuthController → requests AuthService
│  ├─ AuthService → requests TokenRepository
│  ├─ TokenRepository
│  └─ UsersService (shared)
└─ UsersModule
   ├─ UsersController → requests UsersService
   ├─ UsersService → requests UserRepository
   └─ UserRepository
```

## 検証

リポジトリのルートで実行します。

```sh
pnpm install --frozen-lockfile --ignore-scripts
pnpm fixture:typecheck
pnpm --filter @codebasecanvas/web test src/fixture.test.ts
cargo test --workspace --locked --test fixture
```

Fixture は pnpm workspace 内の private package です。単独で読む入口は `src/app.module.ts` と `tsconfig.json`。NestJS の実型を使用し、型定義 stub は置きません。`reflect-metadata` と `rxjs` は NestJS peer、Node 型は NestJS 型定義のために使用します。サーバー、NestFactory、Nest CLI、Prisma Client、DB 接続、認証処理は追加していません。`schema.prisma` は model 抽出用で、datasource/client 生成設定はありません。

Rust/Web の本番契約 validator が期待 graph を受理することを検証します。Web の fixture check は evidence/diagnostic の相対パスと行を実 source に照合し、下記の重要な期待値を固定します。現時点では Analyzer による抽出精度をテストしたことにはなりません。

## Source と期待値

| Source | 期待する宣言・関係 |
|---|---|
| `src/app.module.ts` | AppModule。AuthModule / UsersModule へ depends_on。Module import は contains にしない |
| `src/auth/*.ts` | AuthModule、AuthController、AuthService、TokenRepository。4 membership、2 requested-token DI、POST /auth/login |
| `src/users/*.ts` | UsersModule、UsersController、UsersService、UserRepository。3 membership、2 requested-token DI、GET /users を別 handler で2件、PATCH /users/:id、DELETE /users/:id |
| `src/regressions/override.module.ts` | OverrideModule、OverrideController、MockUsersService。Controller membership のみ。Controller は UsersService を要求。useClass は診断対象で、Mock/UsersService への provider membership や実行 calls を推測しない |
| `src/regressions/ports.ts` | Port interface と TypeOnlyToken class。いずれも DI token edge の対象にしない |
| `src/regressions/unsupported.module.ts` | UnsupportedConsumer service、DynamicFeature / UnsupportedModule。未対応 entries と同居する UnsupportedConsumer の membership は保持 |
| `src/regressions/identity.ts` | Alpha.Same / Beta.Same は異なる class ID。ExternalBindings.inspect から rxjs と rxjs/operators の map へ imports。package root だけで統合しない |
| `prisma/schema.prisma` | User / Token の database_model 2件。DB 接続や reads/writes はない |

同じ UsersService 宣言を AuthModule と UsersModule に登録しているため、node は1件、membership は2件、parentId はありません。単独登録された Controller/Service/Repository は登録 Module を parent とします。MockUsersService は Injectable ではないため generic class のままです。Repository role は Injectable と名前末尾の補助規則による `best_effort` evidence、宣言自体と静的 membership は `confirmed` です。

Module exports は resolver findings に留め、追加の contains/imports edge にしません。import edge は実際に binding を使う class/method に付けます。Decorator は装飾対象、constructor type は class、method decorator/return type は method が consumer です。`sharedProviders` の file-level 初期化だけから UnsupportedModule → UsersService を推測しません。forwardRef 内の UsersModule binding は imports の根拠ですが、depends_on の根拠にはしません。

Scoped declaration は #3 の ID 規則に従う期待値です。#7 の能力 gate で名前・scope の識別を実証します。対応できない場合は gate を失敗させ、範囲変更をレビューしてから期待値を改訂します。異なる宣言を同じ node にまとめてテストを通してはいけません。Constructor、arrow function、namespace 自体は method/class node として追加しません。

## 期待件数

**57 nodes / 90 edges / 15 diagnostics**。すべての node/edge に source evidence があります。

| Node kind | 件数 |
|---|---:|
| module | 6 |
| controller | 3 |
| service | 3 |
| repository | 2 |
| class | 5 |
| interface | 1 |
| method | 17 |
| endpoint | 6 |
| database_model | 2 |
| external_dependency | 12 |

| Edge kind | 件数 | 内訳 |
|---|---:|---|
| contains | 26 | module membership 9 + lexical method owner 17 |
| depends_on | 8 | module imports 2 + endpoint handlers 6 |
| injects | 5 | requested_token のみ |
| exposes | 6 | route ごとの Controller → Endpoint |
| calls | 2 | 下記2件の同一 class 呼び出し |
| imports | 43 | local binding 18 + framework/type/operator binding 25 |

External は `@nestjs/common` の Module / Injectable / Controller / Post / Get / Patch / Delete / Inject / forwardRef / DynamicModule と、別 module specifier の map 2件です。外部 source は読まず、import の静的根拠だけを表します。

## Call ledger

Scope は `parsed_named_class_methods`、mode は `same_class_only`。**examined=9、emitted=2、skipped=7**。次の9箇所を各1回数えます。method body 外の decorator/metadata 内の call は対象外です。nested function 内の call は数えますが unknown とします。

| File:line / enclosing method | Call | 期待 |
|---|---|---|
| `src/auth/auth.service.ts:8` AuthService.login | this.normalize() | calls → AuthService.normalize |
| `src/auth/auth.service.ts:9` AuthService.login | this.tokens.save() | injected receiver、skipped=1 |
| `src/auth/auth.controller.ts:8` AuthController.login | this.auth.login() | injected receiver、skipped=1 |
| `src/users/users.service.ts:7` UsersService.list | this.users.find() | injected receiver、skipped=1 |
| `src/users/users.service.ts:11` UsersService.choose | this.find() | calls → **instance** UsersService.find |
| `src/users/users.service.ts:12` UsersService.choose | this[flag ? 'find' : 'list']() | computed target、skipped=1 |
| `src/users/users.service.ts:13` UsersService.choose | nested arrow 内の this.find() | nested function、skipped=1 |
| `src/users/users.controller.ts:8` UsersController.list | this.users.list() | injected receiver、skipped=1 |
| `src/regressions/override.module.ts:11` OverrideController.list | this.users.list() | override があっても injected receiver、skipped=1 |

Static find と instance find は別 node。GET /users の list と alias は同じ path でも別 endpoint ID・別 depends_on handler を持ちます。

## Diagnostics

全件 warning。位置と relatedNodeId を保持します。code はこの fixture の期待値として固定し、後続 recognizer で変更する場合は意味レビューが必要です。

| Code | 件数 | 理由 |
|---|---:|---|
| unsupported_call_injected_receiver | 5 | requested token から実装の実行を証明できない。上記各 method に skippedCount=1 |
| unsupported_call_computed_target | 1 | computed target は一意に解決しない。choose に skippedCount=1 |
| unsupported_call_nested_function | 1 | nested arrow 内を直接の同一 class call とみなさない。choose に skippedCount=1 |
| unsupported_di_interface | 1 | Port は runtime class value ではない |
| unsupported_di_type_only | 1 | TypeOnlyToken は明示 import type |
| unsupported_di_custom_token | 1 | @Inject('TOKEN') は未対応 |
| unsupported_module_forward_ref | 1 | forwardRef を展開しない |
| unsupported_module_dynamic | 1 | DynamicFeature.register() の返り値を展開しない |
| unsupported_module_spread | 1 | ...sharedProviders を展開しない |
| unsupported_module_provider | 2 | useFactory / useClass を展開しない |

## 更新ルールと後続利用

Source、期待 graph、件数・call ledger、fixture test を1つの意味変更としてレビューしてください。Analyzer から期待 graph を再生成するコマンドはありません。実装に合わせて期待値を弱めたり、unknown を推測 edge に変えたりしません。外部 import subpath の統合には同一 symbol の証明が必要です。

#7 の能力 probe、#8–#18 の structural tests はこの source と手定義 manifest を利用します。#19/#20 の初期 UI 確認には期待 graph を読み込めますが、解析器の成功とは表示しません。`analyzerVersion=hand-authored-fixture-v1`、固定 analyzedAt は oracle 用のラベルです。#26 E2E は**その時点の Rust Analyzer**で使い捨て source コピーを解析し、新しく生成した graph を File API → 本番 validator → Canvas に通します。この期待 JSON を解析失敗時の代替にしません。

Root 外参照や出力 symlink のケースは #5/#6/#13 の使い捨てディレクトリテストで扱います。fixture に開発者の絶対パス、秘密情報、実データを入れません。
