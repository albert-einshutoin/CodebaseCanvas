# Oxc capability probe (#7)

この文書は、recognizer の実装前に Oxc で取得できる構文・semantic・module 情報を固定した accuracy gate です。production recognizer、Graph 出力、TypeScript compiler fallback、実repoの依存 install は含みません。

## 再実行

通常のfixture probe（ネットワーク不要）:

```sh
cargo test --locked -p codebasecanvas-analyzer --test oxc_probe
```

実repo probe は `#[ignore]` です。通常suiteで `ignored` と表示されてもreal-repo gateの成功証拠にはなりません。次の手順で対象を別途cloneし、detached HEADへ固定します（sourceはこのrepositoryへ再配布しません）。

```sh
git clone --no-checkout https://github.com/lujakob/nestjs-realworld-example-app.git \
  /private/tmp/codebasecanvas-issue7-realrepo
git -C /private/tmp/codebasecanvas-issue7-realrepo \
  checkout --detach c1c2cc4e448b279ff083272df1ac50d20c3304fa
git -C /private/tmp/codebasecanvas-issue7-realrepo status --porcelain

OXC_REAL_REPO=/private/tmp/codebasecanvas-issue7-realrepo \
  cargo test --locked -p codebasecanvas-analyzer --test oxc_probe -- \
  --ignored --exact real_repository_probe_is_pinned_and_never_runs_scripts
```

testはorigin、exact HEAD、clean worktree、35件のsourceと代表pathを検査します。dirtyなら、その内容を固定SHAの証拠に含めないため失敗します。package script、Node、依存installは呼びません。

probe は #6 の `RepositoryRoot` / `discover` / `resolve` を使用し、root内だけを許可するfilesystemをOxc resolverへ渡します。resolver の拡張子は `.ts`, `.tsx`, `.js`, `.json` に限定し、解決結果も `RepositoryRoot::relative_path` でroot内を確認します。実行testでroot外sourceのresolver/readとroot外configのreadが `PermissionDenied` / resolve errorになることを確認しました。`tsconfig` の外部参照やroot外sourceはfallbackで読みません。

## 固定したstackと公式API

取得したcrate source（2026-09-06）でAPIを確認し、次のexact versionを `Cargo.toml` と `Cargo.lock` に固定しました。

| crate | version | probeで確認したAPI |
|---|---:|---|
| `oxc_parser` | `0.148.0` | `Parser::new(..., SourceType::ts().with_module(true)).parse()` |
| `oxc_ast` / `oxc_ast_visit` | `0.148.0` | class/interface/decorator/import/method/type-reference AST visitor |
| `oxc_semantic` | `0.148.0` | `SemanticBuilder::with_check_syntax_error`、`SymbolId` / `ReferenceId` / `ScopeId` |
| `oxc_span` | `0.148.0` | `Span::source_text`、byte spanからline/column |
| `oxc_resolver` | `11.24.3` | `ResolverGeneric::new_with_file_system`、`resolve_file` |

公式のParser docsは [oxc.rs Parser](https://oxc.rs/docs/guide/usage/parser)、module resolver docsは [oxc.rs Resolver](https://oxc.rs/docs/guide/usage/resolver) と [docs.rs oxc_resolver](https://docs.rs/oxc_resolver/11.24.3/oxc_resolver/) を参照しました。semanticの `SemanticBuilder` は [docs.rs](https://docs.rs/oxc_semantic/0.148.0/oxc_semantic/struct.SemanticBuilder.html) の現行crate sourceで確認しました。

## Capability matrix

| Capability | Result | Confidence | Expected / observed |
|---|---|---|---|
| fixture全 `.ts` parse | Supported | confirmed | discoveryの13ファイルを全てparse、diagnosticなし |
| class declaration name | Supported | confirmed | `AppModule`, `AuthController`, `UsersService` を取得 |
| interface declaration name | Supported | confirmed | `Port` を取得 |
| decorator name / arguments | Supported | confirmed | `@Controller('auth')`, `@Get('user')`, `@Module({...})` をAST spanから取得 |
| constructor parameters | Supported | confirmed | `auth: AuthService` などのformal parameter/type annotationを取得 |
| constructor type reference | Supported | confirmed | `AuthService` / `UserService` / `ArticleService`を`TSTypeReference`として取得し、referenceの`SymbolId`をvalue import宣言の`SymbolId`と照合 |
| import declaration / source | Supported | confirmed | named specifierのsource / imported / local / type-onlyを取得。`Controller`、`AuthService`等をexact assert |
| local binding/reference | Supported | confirmed | decorator calleeとconstructor typeの`ReferenceId`から宣言`SymbolId`を取得し、対応するimport bindingと一致をassert |
| normal relative module resolution | Supported | confirmed | `./auth.service` -> `src/auth/auth.service.ts` |
| span -> line/column | Supported | confirmed | `AuthController` class span -> line 5, column 8 |
| static / instance distinction | Supported | confirmed | `UsersService.find` のstatic/instanceを別々に取得 |
| lexical scope | Supported | confirmed | 宣言とreferenceの`ScopeId`を取得。同名のfunction-local `Controller` はroot importと別symbol/scopeになり、decorator originとして誤採用しない |
| dynamic dispatch / runtime provider | Unsupported | confirmed boundary | `this[flag ? ...]`, `useFactory`、runtime-generated provider targetは解決しない |
| arbitrary factory return / deep conditional generic reasoning | Unsupported | confirmed boundary | full TypeScript type checkerの代替にしない |
| alias resolution with complex tsconfig | Best effort | scoped | このprobeは通常relative importだけを確認 |

同じclass内の静的なmethod名はsyntax/semanticから取得できますが、注入receiverの動的dispatch targetは取得しません。後続の `calls` 解析は同一class・名前付きmethodだけのbest effortに限定します。

## 固定実repoとsource照合

選定repoは [lujakob/nestjs-realworld-example-app](https://github.com/lujakob/nestjs-realworld-example-app) です。複数Module、Controller、class-token constructor DI、local relative import、route decoratorが同時にあり、fixtureにない実NestJS形状を確認できるため選定しました。

| Item | Fixed value |
|---|---|
| commit SHA | `c1c2cc4e448b279ff083272df1ac50d20c3304fa` |
| profile | `nestjs-realworld-example-app` v2.0.0、NestJS 7、CommonJS、TypeScript 3.8系 |
| license / redistribution | `package.json` declared `ISC`。このprobeは外部cloneを参照し、sourceを再配布しない |
| config | root `tsconfig.json`、`experimentalDecorators: true`、`emitDecoratorMetadata: true`、`src/**/*` include、`node_modules`/spec exclude |
| representative source | `src/app.module.ts`, `src/user/user.module.ts`, `src/user/user.controller.ts`, `src/article/article.module.ts`, `src/article/article.controller.ts` |
| local source probe | 35 `.ts` filesをOxcでparse、依存 installなし、package script実行なし |

sourceとの手照合結果:

- `src/app.module.ts`: expectedの`ApplicationModule` line 23 column 8、`@Module({...})` calleeから`@nestjs/common`のvalue import宣言へのbinding、`ArticleModule` / `UserModule` / `ProfileModule` / `TagModule` referenceから各local import宣言へのbinding、constructorの`Connection` referenceから`typeorm` value import宣言へのbindingがobservedと一致。
- `src/user/user.controller.ts`: expectedの`UserController` line 17 column 8、`@Controller()` calleeから`@nestjs/common` importへのbinding、`UserService` type reference line 19 column 45から`./user.service` value importへのbinding、`@Get('user')` line 21、`./user.service`から`src/user/user.service.ts`へのrelative resolutionがobservedと一致。
- `src/article/article.controller.ts`: expectedの`@Controller('articles')` callee binding、constructor `ArticleService`から`./article.service` value importへのbinding、`@Post(':slug/comments')` line 76がobservedと一致。
- `src/user/user.module.ts` / `src/article/article.module.ts` は35 sourceのparse対象かつ手確認path。動的なNest runtime wiring、`@ApiResponse({...})`の任意object意味解析、runtime route生成はこのprobeでは取得しない。

実repo testはorigin / HEAD / clean、profile・license・TypeScript/NestJS config、同じcommitの全35 source parse、上記exact assertionsをpassしました。実repoのコードはこのrepositoryへコピー・納品していません。

## Gate判定と制約

fixtureと固定commitの構文・semantic・通常relative resolver能力は取得できました。runtime class tokenはconstructor type syntaxとvalue-import bindingとしてのみ確認し、runtime instanceやprovider実装を推論しません。これは解析精度、Canvas理解速度、production Graphの合格証明ではありません。

dynamic dispatch、runtime-generated provider、factory return、deep TypeScript type reasoningはunknown/unsupportedとして後続で扱い、TypeScript fallbackは追加しません。今後、必要能力・root内情報・Oxc APIのいずれかを取得できなくなった場合は広域fallbackで成功扱いせずgate未完了とし、recognizer拡張を止めてscope/技術選定を再検討します。
