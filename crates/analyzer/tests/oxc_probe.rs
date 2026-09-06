use codebasecanvas_analyzer::discovery::{RepositoryRoot, discover};
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Class, Expression, ImportDeclarationSpecifier, ImportOrExportKind, MethodDefinitionKind,
    ModuleExportName, TSTypeName,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_resolver::{FileMetadata, FileSystem, ResolveError, ResolveOptions, ResolverGeneric};
use oxc_semantic::{ReferenceId, ScopeId, Scoping, SemanticBuilder, SymbolId};
use oxc_span::{GetSpan, SourceType, Span};
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

const REAL_REPOSITORY_SHA: &str = "c1c2cc4e448b279ff083272df1ac50d20c3304fa";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Binding {
    symbol_id: SymbolId,
    scope_id: ScopeId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReferenceFact {
    name: String,
    reference_id: ReferenceId,
    symbol_id: Option<SymbolId>,
    scope_id: ScopeId,
    line: usize,
    column: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ImportFact {
    source: String,
    imported: String,
    local: String,
    type_only: bool,
    binding: Binding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecoratorFact {
    text: String,
    callee: Option<ReferenceFact>,
}

#[derive(Default)]
struct Probe {
    classes: BTreeSet<String>,
    interfaces: BTreeSet<String>,
    decorators: Vec<DecoratorFact>,
    imports: Vec<ImportFact>,
    constructor_types: Vec<ReferenceFact>,
    references: Vec<ReferenceFact>,
    bindings: Vec<(String, Binding)>,
    static_methods: BTreeSet<String>,
    instance_methods: BTreeSet<String>,
    class_locations: Vec<(String, usize, usize)>,
    root_scope_id: Option<ScopeId>,
}

struct Collector<'a, 's> {
    source: &'a str,
    scoping: &'s Scoping,
    probe: Probe,
    in_constructor: bool,
}

impl Collector<'_, '_> {
    fn text(&self, span: Span) -> String {
        span.source_text(self.source).to_owned()
    }

    fn reference(&self, ident: &oxc_ast::ast::IdentifierReference<'_>) -> ReferenceFact {
        let reference_id = ident.reference_id.get().expect("semantic reference id");
        let reference = self.scoping.get_reference(reference_id);
        ReferenceFact {
            name: ident.name.to_string(),
            reference_id,
            symbol_id: reference.symbol_id(),
            scope_id: reference.scope_id(),
            line: line(self.source, ident.span.start),
            column: column(self.source, ident.span.start),
        }
    }

    fn binding(&self, ident: &oxc_ast::ast::BindingIdentifier<'_>) -> Binding {
        let symbol_id = ident.symbol_id.get().expect("semantic symbol id");
        Binding {
            symbol_id,
            scope_id: self.scoping.symbol_scope_id(symbol_id),
        }
    }
}

impl<'a> Visit<'a> for Collector<'a, '_> {
    fn visit_class(&mut self, class: &Class<'a>) {
        if let Some(id) = &class.id {
            self.probe.classes.insert(id.name.to_string());
            self.probe.class_locations.push((
                id.name.to_string(),
                line(self.source, class.span.start),
                column(self.source, class.span.start),
            ));
        }
        walk::walk_class(self, class);
    }

    fn visit_binding_identifier(&mut self, ident: &oxc_ast::ast::BindingIdentifier<'a>) {
        self.probe
            .bindings
            .push((ident.name.to_string(), self.binding(ident)));
        walk::walk_binding_identifier(self, ident);
    }

    fn visit_identifier_reference(&mut self, ident: &oxc_ast::ast::IdentifierReference<'a>) {
        self.probe.references.push(self.reference(ident));
        walk::walk_identifier_reference(self, ident);
    }

    fn visit_ts_interface_declaration(
        &mut self,
        interface: &oxc_ast::ast::TSInterfaceDeclaration<'a>,
    ) {
        self.probe.interfaces.insert(interface.id.name.to_string());
        walk::walk_ts_interface_declaration(self, interface);
    }

    fn visit_decorator(&mut self, decorator: &oxc_ast::ast::Decorator<'a>) {
        let callee = match &decorator.expression {
            Expression::CallExpression(call) => call
                .callee
                .get_identifier_reference()
                .map(|ident| self.reference(ident)),
            expression => expression
                .get_identifier_reference()
                .map(|ident| self.reference(ident)),
        };
        self.probe.decorators.push(DecoratorFact {
            text: self.text(decorator.span),
            callee,
        });
        walk::walk_decorator(self, decorator);
    }

    fn visit_import_declaration(&mut self, import: &oxc_ast::ast::ImportDeclaration<'a>) {
        if let Some(specifiers) = &import.specifiers {
            for specifier in specifiers {
                if let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier {
                    let imported = match &specifier.imported {
                        ModuleExportName::IdentifierName(name) => name.name.to_string(),
                        ModuleExportName::IdentifierReference(name) => name.name.to_string(),
                        ModuleExportName::StringLiteral(name) => name.value.to_string(),
                    };
                    self.probe.imports.push(ImportFact {
                        source: import.source.value.to_string(),
                        imported,
                        local: specifier.local.name.to_string(),
                        type_only: import.import_kind == ImportOrExportKind::Type
                            || specifier.import_kind == ImportOrExportKind::Type,
                        binding: self.binding(&specifier.local),
                    });
                }
            }
        }
        walk::walk_import_declaration(self, import);
    }

    fn visit_method_definition(&mut self, method: &oxc_ast::ast::MethodDefinition<'a>) {
        let name = self.text(method.key.span());
        if method.kind == MethodDefinitionKind::Constructor {
            self.in_constructor = true;
            walk::walk_method_definition(self, method);
            self.in_constructor = false;
        } else {
            if method.r#static {
                self.probe.static_methods.insert(name);
            } else {
                self.probe.instance_methods.insert(name);
            }
            walk::walk_method_definition(self, method);
        }
    }

    fn visit_ts_type_reference(&mut self, reference: &oxc_ast::ast::TSTypeReference<'a>) {
        if self.in_constructor
            && let TSTypeName::IdentifierReference(id) = &reference.type_name
        {
            let fact = self.reference(id);
            self.probe.constructor_types.push(fact);
        }
        walk::walk_ts_type_reference(self, reference);
    }
}

fn parse(path: &Path, source: &str) -> Probe {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::ts().with_module(true)).parse();
    assert!(
        !parsed.panicked,
        "Oxc parser panicked for {}",
        path.display()
    );
    assert!(
        parsed.diagnostics.is_empty(),
        "Oxc parse diagnostics for {}: {:?}",
        path.display(),
        parsed.diagnostics
    );
    let semantic = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    assert!(
        semantic.diagnostics.is_empty(),
        "Oxc semantic diagnostics for {}: {:?}",
        path.display(),
        semantic.diagnostics
    );
    let scoping = semantic.semantic.scoping();
    let mut collector = Collector {
        source,
        scoping,
        probe: Probe {
            root_scope_id: Some(scoping.root_scope_id()),
            ..Probe::default()
        },
        in_constructor: false,
    };
    collector.visit_program(&parsed.program);
    collector.probe
}

fn line(source: &str, offset: u32) -> usize {
    source[..offset as usize]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

fn column(source: &str, offset: u32) -> usize {
    source[..offset as usize]
        .rfind('\n')
        .map_or(offset as usize, |newline| offset as usize - newline - 1)
        + 1
}

fn fixture_root() -> RepositoryRoot {
    RepositoryRoot::open(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/nestjs-sample")
            .as_path(),
    )
    .unwrap()
}

fn import<'a>(probe: &'a Probe, source: &str, imported: &str, local: &str) -> &'a ImportFact {
    probe
        .imports
        .iter()
        .find(|fact| fact.source == source && fact.imported == imported && fact.local == local)
        .unwrap_or_else(|| panic!("missing import {source} {imported} as {local}"))
}

fn decorator<'a>(probe: &'a Probe, text: &str) -> &'a DecoratorFact {
    probe
        .decorators
        .iter()
        .find(|fact| fact.text == text)
        .unwrap()
}

fn constructor_type<'a>(probe: &'a Probe, name: &str) -> &'a ReferenceFact {
    probe
        .constructor_types
        .iter()
        .find(|fact| fact.name == name)
        .unwrap()
}

fn runtime_class_token_matches(
    probe: &Probe,
    decorator_text: &str,
    decorator_source: &str,
    decorator_name: &str,
    token_source: &str,
    token_name: &str,
) -> bool {
    let decorator_import = probe.imports.iter().find(|fact| {
        fact.source == decorator_source
            && fact.imported == decorator_name
            && fact.local == decorator_name
    });
    let decorator_reference = probe
        .decorators
        .iter()
        .find(|fact| fact.text == decorator_text)
        .and_then(|fact| fact.callee.as_ref());
    let token_import = probe.imports.iter().find(|fact| {
        fact.source == token_source && fact.imported == token_name && fact.local == token_name
    });
    let token_reference = probe
        .constructor_types
        .iter()
        .find(|fact| fact.name == token_name);
    match (
        decorator_import,
        decorator_reference,
        token_import,
        token_reference,
    ) {
        (
            Some(decorator_import),
            Some(decorator_reference),
            Some(token_import),
            Some(token_reference),
        ) => {
            !decorator_import.type_only
                && !token_import.type_only
                && decorator_reference.symbol_id == Some(decorator_import.binding.symbol_id)
                && token_reference.symbol_id == Some(token_import.binding.symbol_id)
                && decorator_import.binding.scope_id == probe.root_scope_id.unwrap()
                && token_import.binding.scope_id == probe.root_scope_id.unwrap()
        }
        _ => false,
    }
}

fn assert_runtime_class_token(
    probe: &Probe,
    decorator_text: &str,
    decorator_source: &str,
    decorator_name: &str,
    token_source: &str,
    token_name: &str,
) {
    assert!(runtime_class_token_matches(
        probe,
        decorator_text,
        decorator_source,
        decorator_name,
        token_source,
        token_name,
    ));
    let decorator_import = import(probe, decorator_source, decorator_name, decorator_name);
    let decorator_reference = decorator(probe, decorator_text).callee.as_ref().unwrap();
    let token_import = import(probe, token_source, token_name, token_name);
    let token_reference = constructor_type(probe, token_name);
    assert!(!decorator_import.type_only);
    assert!(
        !token_import.type_only,
        "runtime class token cannot use import type"
    );
    assert_eq!(
        decorator_reference.symbol_id,
        Some(decorator_import.binding.symbol_id)
    );
    assert_eq!(
        token_reference.symbol_id,
        Some(token_import.binding.symbol_id)
    );
    assert_eq!(
        decorator_import.binding.scope_id,
        probe.root_scope_id.unwrap()
    );
    assert_eq!(token_import.binding.scope_id, probe.root_scope_id.unwrap());
    assert_ne!(token_reference.scope_id, probe.root_scope_id.unwrap());
    assert_ne!(
        decorator_reference.reference_id,
        token_reference.reference_id
    );
}

#[derive(Clone)]
struct RootFileSystem {
    root: PathBuf,
}

impl RootFileSystem {
    fn checked(&self, path: &Path) -> io::Result<PathBuf> {
        let resolved = fs::canonicalize(path)?;
        if resolved.starts_with(&self.root) {
            Ok(resolved)
        } else {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "resolver path is outside repository",
            ))
        }
    }
}

impl FileSystem for RootFileSystem {
    fn new() -> Self {
        Self {
            root: PathBuf::new(),
        }
    }
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        fs::read(self.checked(path)?)
    }
    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        fs::read_to_string(self.checked(path)?)
    }
    fn metadata(&self, path: &Path) -> io::Result<FileMetadata> {
        Ok(fs::metadata(self.checked(path)?)?.into())
    }
    fn symlink_metadata(&self, path: &Path) -> io::Result<FileMetadata> {
        Ok(fs::symlink_metadata(self.checked(path)?)?.into())
    }
    fn read_link(&self, path: &Path) -> Result<PathBuf, ResolveError> {
        let path = self.checked(path).map_err(ResolveError::from)?;
        fs::read_link(path).map_err(ResolveError::from)
    }
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        self.checked(path)
    }
}

fn resolver(root: &RepositoryRoot) -> ResolverGeneric<RootFileSystem> {
    ResolverGeneric::new_with_file_system(
        RootFileSystem {
            root: root.path().to_owned(),
        },
        ResolveOptions {
            extensions: vec![".ts".into(), ".tsx".into(), ".js".into(), ".json".into()],
            ..ResolveOptions::default()
        },
    )
}

#[test]
fn fixture_all_typescript_files_and_required_oxc_capabilities() {
    let root = fixture_root();
    let discovered = discover(&root).unwrap();
    assert_eq!(discovered.files.len(), 13);

    let mut all_classes = BTreeSet::new();
    let mut all_interfaces = BTreeSet::new();
    let mut all_sources = BTreeSet::new();
    for relative in &discovered.files {
        let source = fs::read_to_string(root.resolve(relative).unwrap()).unwrap();
        let probe = parse(Path::new(relative), &source);
        all_classes.extend(probe.classes);
        all_interfaces.extend(probe.interfaces);
        all_sources.extend(probe.imports.into_iter().map(|fact| fact.source));
    }
    assert!(all_classes.contains("AppModule"));
    assert!(all_classes.contains("UsersService"));
    assert!(all_interfaces.contains("Port"));
    assert!(all_sources.contains("./auth/auth.module"));
    assert!(all_sources.contains("../users/users.service"));
}

#[test]
fn fixture_probe_proves_exact_import_decorator_di_and_scope_bindings() {
    let root = fixture_root();
    let source = fs::read_to_string(root.resolve("src/auth/auth.controller.ts").unwrap()).unwrap();
    let probe = parse(Path::new("src/auth/auth.controller.ts"), &source);
    assert!(probe.classes.contains("AuthController"));
    assert_eq!(
        probe
            .class_locations
            .iter()
            .find(|(name, _, _)| name == "AuthController"),
        Some(&("AuthController".to_owned(), 5, 8))
    );
    assert_runtime_class_token(
        &probe,
        "@Controller('auth')",
        "@nestjs/common",
        "Controller",
        "./auth.service",
        "AuthService",
    );
    let controller_import = import(&probe, "@nestjs/common", "Controller", "Controller");
    let controller_ref = decorator(&probe, "@Controller('auth')")
        .callee
        .as_ref()
        .unwrap();
    assert_eq!(controller_ref.line, 4);
    assert_eq!(controller_ref.column, 2);
    assert_eq!(controller_ref.scope_id, probe.root_scope_id.unwrap());
    assert_eq!(
        probe
            .bindings
            .iter()
            .find(|(name, binding)| name == "Controller" && *binding == controller_import.binding)
            .map(|(_, binding)| *binding),
        Some(controller_import.binding)
    );

    let users = fs::read_to_string(root.resolve("src/users/users.service.ts").unwrap()).unwrap();
    let users_probe = parse(Path::new("src/users/users.service.ts"), &users);
    assert!(users_probe.static_methods.contains("find"));
    assert!(users_probe.instance_methods.contains("find"));
    assert!(users_probe.instance_methods.contains("choose"));
}

#[test]
fn semantic_gate_rejects_fake_decorator_type_only_token_and_shadow_binding() {
    let fake = "import { Controller } from './fake';\nimport { AuthService } from './auth.service';\n@Controller() class AuthController { constructor(auth: AuthService) {} }";
    let fake_probe = parse(Path::new("fake.ts"), fake);
    assert!(!runtime_class_token_matches(
        &fake_probe,
        "@Controller()",
        "@nestjs/common",
        "Controller",
        "./auth.service",
        "AuthService",
    ));

    let type_only = "import { Controller } from '@nestjs/common';\nimport type { AuthService } from './auth.service';\n@Controller() class AuthController { constructor(auth: AuthService) {} }";
    let type_probe = parse(Path::new("type-only.ts"), type_only);
    assert!(!runtime_class_token_matches(
        &type_probe,
        "@Controller()",
        "@nestjs/common",
        "Controller",
        "./auth.service",
        "AuthService",
    ));

    let shadow = "import { Controller as NestController } from '@nestjs/common';\nimport { AuthService } from './auth.service';\nfunction make() { const Controller = () => () => {}; @Controller() class AuthController { constructor(auth: AuthService) {} } return AuthController; }";
    let shadow_probe = parse(Path::new("shadow.ts"), shadow);
    let local_binding = shadow_probe
        .bindings
        .iter()
        .find(|(name, _)| name == "Controller")
        .unwrap()
        .1;
    let shadow_ref = decorator(&shadow_probe, "@Controller()")
        .callee
        .as_ref()
        .unwrap();
    assert_eq!(shadow_ref.symbol_id, Some(local_binding.symbol_id));
    assert_eq!(shadow_ref.scope_id, local_binding.scope_id);
    assert_ne!(local_binding.scope_id, shadow_probe.root_scope_id.unwrap());
    assert!(!runtime_class_token_matches(
        &shadow_probe,
        "@Controller()",
        "@nestjs/common",
        "Controller",
        "./auth.service",
        "AuthService",
    ));
}

#[test]
fn resolver_stays_inside_repository_for_source_and_config_reads() {
    let root = fixture_root();
    let controller = root.resolve("src/auth/auth.controller.ts").unwrap();
    let resolved = resolver(&root)
        .resolve_file(&controller, "./auth.service")
        .unwrap();
    assert_eq!(
        root.relative_path(resolved.path()).unwrap(),
        "src/auth/auth.service.ts"
    );

    let outside_source = root.path().join("../../crates/analyzer/src/lib.rs");
    let outside_config = root.path().join("../../package.json");
    let filesystem = RootFileSystem {
        root: root.path().to_owned(),
    };
    assert_eq!(
        filesystem.read(&outside_source).unwrap_err().kind(),
        io::ErrorKind::PermissionDenied
    );
    assert_eq!(
        filesystem
            .read_to_string(&outside_config)
            .unwrap_err()
            .kind(),
        io::ErrorKind::PermissionDenied
    );
    assert!(
        resolver(&root)
            .resolve_file(&controller, "../../../../crates/analyzer/src/lib.rs")
            .is_err()
    );
}

#[test]
#[ignore = "requires the separately cloned pinned real repository"]
fn real_repository_probe_is_pinned_and_never_runs_scripts() {
    let path = std::env::var_os("OXC_REAL_REPO")
        .expect("OXC_REAL_REPO must point to the pinned real repository");
    let path = Path::new(&path);
    let head = Command::new("git")
        .args(["-C"])
        .arg(path)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    assert!(head.status.success());
    assert_eq!(
        String::from_utf8(head.stdout).unwrap().trim(),
        REAL_REPOSITORY_SHA
    );
    let status = Command::new("git")
        .args(["-C"])
        .arg(path)
        .args(["status", "--porcelain"])
        .output()
        .unwrap();
    assert!(status.status.success());
    assert_eq!(
        String::from_utf8(status.stdout).unwrap(),
        "",
        "dirty files are not evidence for the pinned commit"
    );
    let origin = Command::new("git")
        .args(["-C"])
        .arg(path)
        .args(["config", "--get", "remote.origin.url"])
        .output()
        .unwrap();
    assert!(origin.status.success());
    assert_eq!(
        String::from_utf8(origin.stdout).unwrap().trim(),
        "https://github.com/lujakob/nestjs-realworld-example-app.git"
    );

    let root = RepositoryRoot::open(path).unwrap();
    let discovered = discover(&root).unwrap();
    assert_eq!(discovered.files.len(), 35);
    for expected in [
        "src/app.module.ts",
        "src/user/user.module.ts",
        "src/user/user.controller.ts",
        "src/article/article.module.ts",
        "src/article/article.controller.ts",
    ] {
        assert!(discovered.files.iter().any(|path| path == expected));
    }
    for relative in &discovered.files {
        let source = fs::read_to_string(root.resolve(relative).unwrap()).unwrap();
        let _ = parse(Path::new(relative), &source);
    }

    let app_source = fs::read_to_string(root.resolve("src/app.module.ts").unwrap()).unwrap();
    let app = parse(Path::new("src/app.module.ts"), &app_source);
    assert!(app.classes.contains("ApplicationModule"));
    assert_eq!(
        app.class_locations
            .iter()
            .find(|(name, _, _)| name == "ApplicationModule"),
        Some(&("ApplicationModule".to_owned(), 23, 8))
    );
    let module_import = import(&app, "@nestjs/common", "Module", "Module");
    let module_text = "@Module({\n  imports: [\n    TypeOrmModule.forRoot(),\n    ArticleModule,\n    UserModule,\n    ProfileModule,\n    TagModule\n  ],\n  controllers: [\n    AppController\n  ],\n  providers: []\n})";
    assert_eq!(
        decorator(&app, module_text)
            .callee
            .as_ref()
            .unwrap()
            .symbol_id,
        Some(module_import.binding.symbol_id)
    );
    assert_runtime_class_token(
        &app,
        module_text,
        "@nestjs/common",
        "Module",
        "typeorm",
        "Connection",
    );
    for (source, name) in [
        ("./article/article.module", "ArticleModule"),
        ("./user/user.module", "UserModule"),
        ("./profile/profile.module", "ProfileModule"),
        ("./tag/tag.module", "TagModule"),
    ] {
        let imported = import(&app, source, name, name);
        assert!(!imported.type_only);
        let reference = app
            .references
            .iter()
            .find(|fact| fact.name == name)
            .unwrap();
        assert_eq!(reference.symbol_id, Some(imported.binding.symbol_id));
    }

    let package: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.resolve("package.json").unwrap()).unwrap())
            .unwrap();
    assert_eq!(package["name"], "nestjs-realworld-example-app");
    assert_eq!(package["version"], "2.0.0");
    assert_eq!(package["license"], "ISC");
    assert_eq!(package["dependencies"]["@nestjs/common"], "^7.0.5");
    assert_eq!(package["dependencies"]["typescript"], "^3.8.3");
    let config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.resolve("tsconfig.json").unwrap()).unwrap())
            .unwrap();
    assert_eq!(config["compilerOptions"]["module"], "commonjs");
    assert_eq!(config["compilerOptions"]["experimentalDecorators"], true);
    assert_eq!(config["compilerOptions"]["emitDecoratorMetadata"], true);
    assert_eq!(config["include"][0], "src/**/*");

    let user_source =
        fs::read_to_string(root.resolve("src/user/user.controller.ts").unwrap()).unwrap();
    let user = parse(Path::new("src/user/user.controller.ts"), &user_source);
    assert_eq!(
        user.class_locations
            .iter()
            .find(|(name, _, _)| name == "UserController"),
        Some(&("UserController".to_owned(), 17, 8))
    );
    assert_runtime_class_token(
        &user,
        "@Controller()",
        "@nestjs/common",
        "Controller",
        "./user.service",
        "UserService",
    );
    let get_reference = decorator(&user, "@Get('user')").callee.as_ref().unwrap();
    assert_eq!(get_reference.line, 21);
    assert_eq!(
        get_reference.symbol_id,
        Some(
            import(&user, "@nestjs/common", "Get", "Get")
                .binding
                .symbol_id
        )
    );
    let user_service = constructor_type(&user, "UserService");
    assert_eq!((user_service.line, user_service.column), (19, 45));
    let resolved = resolver(&root)
        .resolve_file(
            root.resolve("src/user/user.controller.ts").unwrap(),
            "./user.service",
        )
        .unwrap();
    assert_eq!(
        root.relative_path(resolved.path()).unwrap(),
        "src/user/user.service.ts"
    );

    let article_source =
        fs::read_to_string(root.resolve("src/article/article.controller.ts").unwrap()).unwrap();
    let article = parse(
        Path::new("src/article/article.controller.ts"),
        &article_source,
    );
    assert_runtime_class_token(
        &article,
        "@Controller('articles')",
        "@nestjs/common",
        "Controller",
        "./article.service",
        "ArticleService",
    );
    let post_reference = decorator(&article, "@Post(':slug/comments')")
        .callee
        .as_ref()
        .unwrap();
    assert_eq!(post_reference.line, 76);
    assert_eq!(
        post_reference.symbol_id,
        Some(
            import(&article, "@nestjs/common", "Post", "Post")
                .binding
                .symbol_id
        )
    );
}
