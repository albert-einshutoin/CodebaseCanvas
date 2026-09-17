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

## #13 shared import resolver（production部品、pipeline未接続）

`resolver::ImportResolver::analyze(&RepositoryRoot)` はparserのmodule recordとsemanticのSymbolIdを照合する。一致する綴りを他fileから探すfallbackはない。返り値はsnapshot内のimport/reference findingsであり、完成したSystemGraphではない。

| 対象 | #13の範囲 |
|---|---|
| relative named import / `as` alias | root内の `.ts` / `.tsx`、省略拡張子・directory indexをOxc resolverで解決。named class/interfaceまたは同じfileのexport-list aliasへ対応 |
| 同名symbol / shadowing | ReferenceId→SymbolId→import bindingで照合。canonical IDは既存GraphBuilderと宣言抽出のlexical scope規則を共有 |
| 宣言マージ | 同じSymbolIdの複数宣言は一意に選ばず、そのbindingだけUnresolved + export位置のDiagnostic。class/interfaceの順序でtargetを変えない |
| type-only | import type、specifier type、export typeを`ImportFinding.type_only`に保持。LocalSymbolは宣言kindも保持。value importやExternalSymbolであることもruntime classの証明にはしない |
| external named import | `node:fs` / `node:fs/promises`を含む元specifier + export名をcanonical external IDに使用。外部specifier検証をrepository path検証から分離し、Node実行・URL取得はしない。package rootは別フィールドで保持し、subpath同士を統合しない。package source/metadataを読まない |
| default / namespace / side-effect import | 元specifier・bindingを保持しUnresolved + Diagnostic。利用解析は未対応 |
| re-export（単段を含む） | 未対応。`import { A } from "./a"; export { A };`も下流importerの有無によらずexport位置のDiagnosticを保持し、下流はUnresolved。Oxcがnamed importのlocal形式をindirect entryへ変換する場合も、import元ではなくexport entryのspanを使用する。同じfile内の宣言のexport-list aliasは引き続き対応。多段barrelは辿らない |
| tsconfig paths/baseUrl | 解決は未対応。bare specifierはUnresolvedとし、通常relative importは従来どおり解決する |
| tsconfig extends/references/rootDirs/moduleSuffixes | 未評価の設定でrelative lookupが変わり得るため、relative/bareともUnresolved + Diagnostic。通常の.tsへconfirmed edgeを出さない。継承先configは読まず、設定解決を実装しない |
| JS/JSON module、NodeNext `.js`→`.ts`置換、dynamic import、CommonJS、type推論 | 未対応。dynamic import/CommonJSは静的ES import APIの対象外 |

source/configを読む前にRepositoryRootでcanonical containmentを確認する。Oxc resolverのfilesystemにも同じ境界とdiscoveryの除外規則を適用し、root外・node_modulesのreadを拒否する。resolverのpackage.json祖先探索は無効なread（NotFound）として止める。unsupported configの参照先は読まない。snapshot中の他プロセスによるfilesystem変更の隔離は提供しない。

後続Recognizerは同じsource snapshotのAST referenceのbyte offsetから取得する（文字列名だけでは照合しない）:

```rust,ignore
let resolver = ImportResolver::analyze(&root)?;
if let Some(reference) = resolver.at_reference(file, reference_span.start) {
    let binding = resolver.import_for(reference);
    // binding.type_only と resolution のkindを確認してからRecognizer固有の意味を判断。
    // LocalSymbol / ExternalSymbol / Unresolved。外部importだけでruntime tokenと断定しない。
}
```

`references()` はsource byte span/line、import元とtype-only、使用箇所のconsumer IDを保持する。top-level関数など契約にないconsumerはNoneのまま。anonymous classや表現不能なmethodもconsumerを捏造しない。named method内のnested functionにある使用はそのmethodのlexicalな使用として保持し、呼び出し対象の解決は行わない。

`apply_imports(&mut GraphBuilder)` はgeneric宣言投入後に呼ぶ。実際の使用referenceに対してだけ`consumer --imports--> target`を追加し、external nodeもその時点で追加する。class decorator/field/constructorはclass、method内の使用はmethodがconsumerとなる。Evidenceは既存のresolver/confirmedで、静的なimport bindingの事実のみを示す。type-onlyをruntime関係へ変換せず、NestJS意味解析・call counter・正規graph metadataは生成しない。`analyze` CLIへの接続は#17のままであり、未実装エラーを変更していない。

検証は`tests/import_resolver.rs`とresolver内filesystem test。手定義fixtureのimports edge集合と比較し、expected-graph.jsonは変更しない。テスト内のgraphは宣言/import projectionであり、完全解析の証拠ではない。#7の固定実repoprobeと#13 production APIの検証は区別する。

固定版`oxc_resolver 11.24.3`の`TsConfig`はextends/paths/baseUrl/rootDirs等を保持するが、`CompilerOptions`にmoduleSuffixesはなく、未知fieldとして破棄する。そのため同じroot sourceを、Oxcが既に使う`json-strip-comments 3.1.2`とserde_jsonでも構造的に読み、moduleSuffixesの存在をtyped deserialization前に確認する。JSONCのcomment・trailing comma・BOM・escaped keyを扱い、文字列検索で設定の有無を判断しない。同依存を固定versionの直接依存として宣言したが、依存packageの追加・upgradeはない。

回帰testは継承先/directのmoduleSuffixesでUnresolvedとimports edge不存在を確認し、設定なし・paths/baseUrlのみのrelative named/alias解決とfixture 43 importsを維持する。設定warningだけを成功条件にはしない。
