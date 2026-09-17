use codebasecanvas_analyzer::{
    CallAnalysis, CallMode, CallScope, Confidence, EdgeKind, EvidenceSource, GraphBuilder,
    GraphMetadata, NodeKind, SystemGraph, discovery::RepositoryRoot, nestjs_roles,
    resolver::ImportResolver,
};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_REPO: AtomicU64 = AtomicU64::new(0);

struct Repo(PathBuf);
impl Repo {
    fn new(source: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "canvas-routes-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_REPO.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("main.ts"), source).unwrap();
        Self(path)
    }
    fn resolver(&self) -> ImportResolver {
        ImportResolver::analyze(&RepositoryRoot::open(&self.0).unwrap()).unwrap()
    }
}
impl Drop for Repo {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn builder() -> GraphBuilder {
    // Declaration/role/import projection only; these counters do not claim call coverage.
    GraphBuilder::new(GraphMetadata {
        analyzer_version: "roles-test".into(),
        analyzed_at: "2026-09-17T00:00:00Z".into(),
        root_name: None,
        call_analysis: CallAnalysis {
            scope: CallScope::ParsedNamedClassMethods,
            mode: CallMode::SameClassOnly,
            examined_calls: 0,
            emitted_calls: 0,
            skipped_calls: 0,
        },
    })
}
fn graph(r: &ImportResolver, roles: bool) -> SystemGraph {
    let mut b = builder();
    for (file, source) in r.sources() {
        if roles {
            nestjs_roles::extract_file(file, r, &mut b).unwrap();
        } else {
            codebasecanvas_analyzer::typescript::extract_file(file, source, &mut b).unwrap();
        }
    }
    if roles {
        codebasecanvas_analyzer::nestjs_modules::analyze(r, &b)
            .unwrap()
            .apply(&mut b)
            .unwrap();
    }
    if roles {
        codebasecanvas_analyzer::nestjs_routes::analyze(r, &b)
            .unwrap()
            .apply(&mut b)
            .unwrap();
    }
    r.apply_imports(&mut b).unwrap();
    b.finish().unwrap()
}
#[test]
fn fixture_routes_and_existing_structure_match_oracle() {
    let root = RepositoryRoot::open(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/nestjs-sample"),
    )
    .unwrap();
    let r = ImportResolver::analyze(&root).unwrap();
    let actual = graph(&r, true);
    let expected = SystemGraph::from_json(include_str!(
        "../../../examples/nestjs-sample/expected-graph.json"
    ))
    .unwrap();
    let composition = |g: &SystemGraph| {
        g.edges
            .iter()
            .filter(|e| {
                let from = g.nodes.iter().find(|n| n.id == e.from).unwrap();
                let to = g.nodes.iter().find(|n| n.id == e.to).unwrap();
                from.kind == NodeKind::Module
                    && (e.kind == EdgeKind::DependsOn
                        || e.kind == EdgeKind::Contains && to.kind != NodeKind::Method)
            })
            .map(|e| {
                (
                    (e.from.clone(), format!("{:?}", e.kind), e.to.clone()),
                    e.evidence.clone(),
                )
            })
            .collect::<BTreeMap<_, _>>()
    };
    assert_eq!(composition(&actual).len(), 11);
    assert_eq!(composition(&actual), composition(&expected));
    for n in actual.nodes.iter().filter(|n| {
        matches!(
            n.kind,
            NodeKind::Module
                | NodeKind::Controller
                | NodeKind::Service
                | NodeKind::Repository
                | NodeKind::Class
                | NodeKind::Method
        )
    }) {
        let oracle = expected.nodes.iter().find(|o| o.id == n.id).unwrap();
        assert_eq!(n.kind, oracle.kind);
        assert_eq!(n.parent_id, oracle.parent_id, "{}", n.name);
    }
    let diags = |g: &SystemGraph| {
        g.diagnostics
            .iter()
            .filter(|d| d.code.starts_with("unsupported_module_"))
            .map(|d| {
                (
                    d.code.clone(),
                    d.file.clone(),
                    d.line,
                    d.related_node_id.clone(),
                    d.skipped_count,
                )
            })
            .collect::<std::collections::BTreeSet<_>>()
    };
    assert_eq!(diags(&actual).len(), 5);
    assert_eq!(diags(&actual), diags(&expected));
    let edges_of = |g: &SystemGraph, imports: bool| {
        g.edges
            .iter()
            .filter(|e| {
                if imports {
                    e.kind == EdgeKind::Imports
                } else {
                    e.kind == EdgeKind::Contains
                        && g.nodes
                            .iter()
                            .any(|n| n.id == e.to && n.kind == NodeKind::Method)
                }
            })
            .map(|e| (e.from.clone(), e.to.clone()))
            .collect::<std::collections::BTreeSet<_>>()
    };
    assert_eq!(edges_of(&actual, true).len(), 43);
    assert_eq!(edges_of(&actual, true), edges_of(&expected, true));
    assert_eq!(edges_of(&actual, false).len(), 17);
    assert_eq!(edges_of(&actual, false), edges_of(&expected, false));

    let endpoints = |g: &SystemGraph| {
        g.nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Endpoint)
            .map(|n| (n.id.clone(), n.clone()))
            .collect::<BTreeMap<_, _>>()
    };
    let route_edges = |g: &SystemGraph| {
        g.edges
            .iter()
            .filter(|e| {
                e.kind == EdgeKind::Exposes
                    || (e.kind == EdgeKind::DependsOn
                        && g.nodes
                            .iter()
                            .any(|n| n.id == e.from && n.kind == NodeKind::Endpoint))
            })
            .map(|e| (e.id.clone(), e.clone()))
            .collect::<BTreeMap<_, _>>()
    };
    assert_eq!(endpoints(&actual).len(), 6);
    assert_eq!(endpoints(&actual), endpoints(&expected));
    assert_eq!(route_edges(&actual).len(), 12);
    assert_eq!(route_edges(&actual), route_edges(&expected));
    let before = graph(&r, false);
    for old in &before.nodes {
        let new = actual.nodes.iter().find(|n| n.id == old.id).unwrap();
        assert!(old.evidence.iter().all(|e| new.evidence.contains(e)));
    }
    assert_eq!(actual, graph(&r, true));
    let mut reversed = builder();
    for (file, _) in r.sources().collect::<Vec<_>>().into_iter().rev() {
        nestjs_roles::extract_file(file, &r, &mut reversed).unwrap();
    }
    let mut modules = codebasecanvas_analyzer::nestjs_modules::analyze(&r, &reversed).unwrap();
    modules.modules.reverse();
    modules.apply(&mut reversed).unwrap();
    let mut findings = codebasecanvas_analyzer::nestjs_routes::analyze(&r, &reversed).unwrap();
    findings.routes.reverse();
    findings.diagnostics.reverse();
    findings.apply(&mut reversed).unwrap();
    findings.apply(&mut reversed).unwrap();
    r.apply_imports(&mut reversed).unwrap();
    assert_eq!(actual, reversed.finish().unwrap());
}

fn endpoints(g: &SystemGraph) -> Vec<&codebasecanvas_analyzer::GraphNode> {
    g.nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Endpoint)
        .collect()
}
fn routes(g: &SystemGraph) -> std::collections::BTreeSet<String> {
    endpoints(g).iter().map(|n| n.name.clone()).collect()
}
fn route_diagnostics(g: &SystemGraph) -> Vec<&codebasecanvas_analyzer::Diagnostic> {
    g.diagnostics
        .iter()
        .filter(|d| d.code.starts_with("unsupported_route_"))
        .collect()
}
fn check_diagnostics(g: &SystemGraph) {
    for d in route_diagnostics(g) {
        assert!(d.file.is_some() && d.line.is_some());
        assert!(
            g.nodes
                .iter()
                .any(|n| Some(&n.id) == d.related_node_id.as_ref()
                    && matches!(
                        n.kind,
                        NodeKind::Class | NodeKind::Controller | NodeKind::Method
                    ))
        );
        assert_eq!(d.skipped_count, None);
    }
}

#[test]
fn literal_paths_methods_and_unicode_positions() {
    let repo = Repo::new(
        "import {Controller, Get, Post, Put, Patch, Delete} from '@nestjs/common';
         // 日本語
         @Controller('//日本語///') class C {
         @Get() async list() {}
         @Post('/追加//') add() {}
         @Put('') update() {}
         @Patch(':id') patch() {}
         @Delete('/:id/') remove() {}
         }
         @Controller() class Empty { @Get() root() {} @Post('users') users() {} }
         @Controller('') class Blank { @Get('') root() {} }
         @Controller('/auth/') class Auth { @Post('/login/') login() {} }",
    );
    let r = repo.resolver();
    let g = graph(&r, true);
    assert_eq!(endpoints(&g).len(), 9);
    assert_eq!(
        routes(&g),
        [
            "GET /日本語",
            "POST /日本語/追加",
            "PUT /日本語",
            "PATCH /日本語/:id",
            "DELETE /日本語/:id",
            "GET /",
            "POST /users",
            "POST /auth/login"
        ]
        .into_iter()
        .map(String::from)
        .collect()
    );
    let first = endpoints(&g)
        .into_iter()
        .find(|n| n.name == "GET /日本語")
        .unwrap();
    assert_eq!(first.line, Some(4));
    assert!(
        first
            .evidence
            .iter()
            .all(|e| e.source == EvidenceSource::Nestjs && e.confidence == Confidence::Confirmed)
    );
    assert!(route_diagnostics(&g).is_empty());
    assert_eq!(g, graph(&r, true));
}

#[test]
fn import_provenance_aliases_and_unrelated_decorators() {
    let repo = Repo::new(
        "import {Controller as Ctrl, Get as Fetch} from '@nestjs/common';
         import {Get as Foreign} from 'other';
         import {Roles as Get} from './roles';
         function Public() { return () => {}; }
         const guards = { check() { return () => {}; } };
         @Ctrl('a') class C {
           @Fetch() @Get() @Public() @guards.check() ok() {}
           @Foreign() foreign() {}
           @Get() custom() {}
         }
         function inner() { const Fetch = Public; @Ctrl('b') class C { @Fetch() shadowed() {} } }",
    );
    fs::write(
        repo.0.join("roles.ts"),
        "export function Roles() { return () => {}; }",
    )
    .unwrap();
    let g = graph(&repo.resolver(), true);
    assert_eq!(routes(&g), ["GET /a".into()].into());
    assert!(route_diagnostics(&g).is_empty());
    assert!(
        g.diagnostics
            .iter()
            .any(|d| d.file.as_deref() == Some("main.ts"))
    ); // Resolver's unsupported function export remains.
}

#[test]
fn type_only_and_unresolved_candidates_remain_unknown() {
    let repo = Repo::new(
        "import {Controller, Get} from '@nestjs/common';
         import type {Post as Send} from '@nestjs/common';
         import {Get as Indirect} from './barrel';
         @Controller() class C { @Send() typeOnly() {} @Indirect() indirect() {} @Get() safe() {} }",
    );
    fs::write(
        repo.0.join("barrel.ts"),
        "export {Get} from '@nestjs/common';",
    )
    .unwrap();
    let g = graph(&repo.resolver(), true);
    assert_eq!(routes(&g), ["GET /".into()].into());
    assert_eq!(route_diagnostics(&g).len(), 2);
    assert!(
        route_diagnostics(&g)
            .iter()
            .all(|d| d.code == "unsupported_route_origin")
    );
    assert!(
        g.diagnostics
            .iter()
            .any(|d| !d.code.starts_with("unsupported_route_"))
    );
    check_diagnostics(&g);

    let repo = Repo::new(
        "import type {Controller as Ctrl} from '@nestjs/common';
         import {Get} from '@nestjs/common'; @Ctrl() class C { @Get() run() {} }",
    );
    let g = graph(&repo.resolver(), true);
    assert!(endpoints(&g).is_empty());
    assert!(
        route_diagnostics(&g)
            .iter()
            .any(|d| d.code == "unsupported_route_origin")
    );
    check_diagnostics(&g);

    let repo = Repo::new(
        "import {Controller,Get} from '@nestjs/common'; @Controller() class C { @Get() run() {} }",
    );
    fs::write(
        repo.0.join("tsconfig.json"),
        r#"{"compilerOptions":{"moduleSuffixes":[".native",""]}}"#,
    )
    .unwrap();
    let g = graph(&repo.resolver(), true);
    assert!(endpoints(&g).is_empty());
    assert!(!route_diagnostics(&g).is_empty());
    check_diagnostics(&g);
}

#[test]
fn unknown_paths_are_scoped_and_never_become_empty_segments() {
    let repo = Repo::new(
        "import {Controller,Get} from '@nestjs/common'; const PREFIX='x'; const PATHS={user:'x'};
         @Controller(PREFIX) class Dynamic { @Get() lost() {} }
         @Controller({path:'x'}) class Options { @Get() lost() {} }
         @Controller('safe') class C {
           @Get('ok') ok() {}
           @Get(PREFIX) constant() {}
           @Get(PATHS.user) member() {}
           @Get(['a','b']) array() {}
           @Get(`template`) template() {}
           @Get('a' + 'b') concat() {}
           @Get(String('x')) call() {}
           @Get(' x ') whitespace() {}
           @Get('../x') dot() {}
           @Get('x?y') query() {}
           @Get('x#y') fragment() {}
           @Get('x\\\\y') backslash() {}
         }",
    );
    let g = graph(&repo.resolver(), true);
    assert_eq!(routes(&g), ["GET /safe/ok".into()].into());
    assert_eq!(route_diagnostics(&g).len(), 13);
    assert!(
        route_diagnostics(&g)
            .iter()
            .all(|d| d.code == "unsupported_route_path")
    );
    check_diagnostics(&g);
}

#[test]
fn multiple_decorators_and_unsupported_configuration_are_order_independent() {
    for reverse in [false, true] {
        let pair = if reverse {
            "@Post() @Get()"
        } else {
            "@Get() @Post()"
        };
        let controllers = if reverse {
            "@Controller('b') @Controller('a')"
        } else {
            "@Controller('a') @Controller('b')"
        };
        let repo = Repo::new(&format!(
            "import {{Controller,Get,Post,Version,All,Head,Options,RequestMapping}} from '@nestjs/common';
             {controllers} class Multiple {{ @Get() run() {{}} }}
             @Controller() @Version('1') class Versioned {{ @Get() run() {{}} }}
             @Controller('ok') class C {{
               {pair} conflict() {{}}
               @Get() @Version('1') versioned() {{}}
               @All() all() {{}} @Head() head() {{}} @Options() options() {{}}
               @RequestMapping({{path:'x'}}) mapped() {{}}
               @Get() safe() {{}}
             }}"
        ));
        let g = graph(&repo.resolver(), true);
        assert_eq!(routes(&g), ["GET /ok".into()].into());
        assert_eq!(route_diagnostics(&g).len(), 8);
        check_diagnostics(&g);
    }
}

#[test]
fn scope_handler_identity_and_unsupported_member_kinds() {
    let repo = Repo::new(
        "import {Controller,Get} from '@nestjs/common'; const name='computed';
         @Controller('same') class C {
           @Get() first() {} @Get() second() {}
           @Get() static stat() {}
           @Get() get getter() {return 1;}
           @Get() field = () => {};
           @Get() [name]() {}
           outer() { class Inner { @Get() nested() {} } }
         }
         namespace Scope { @Controller('same') export class C { @Get() first() {} } }
         @Controller() class Outer { @Get() run() { @Controller('nested') class Nested { @Get() run() {} } } }",
    );
    fs::write(repo.0.join("other.ts"), "import {Controller,Get} from '@nestjs/common'; @Controller('same') class C { @Get() first() {} }").unwrap();
    let g = graph(&repo.resolver(), true);
    assert_eq!(endpoints(&g).len(), 6);
    assert_eq!(
        endpoints(&g)
            .iter()
            .filter(|n| n.name == "GET /same")
            .count(),
        4
    );
    assert_eq!(route_diagnostics(&g).len(), 4);
    check_diagnostics(&g);
    for endpoint in endpoints(&g) {
        let metadata = endpoint.metadata.as_ref().unwrap();
        let handler = g
            .nodes
            .iter()
            .find(|n| n.id == metadata["controllerMethodId"].as_str().unwrap())
            .unwrap();
        assert_eq!(endpoint.parent_id, handler.parent_id);
        assert_eq!(
            g.edges
                .iter()
                .filter(|e| e.to == endpoint.id && e.kind == EdgeKind::Exposes)
                .count(),
            1
        );
        assert!(
            g.edges.iter().any(|e| e.from == endpoint.id
                && e.to == handler.id
                && e.kind == EdgeKind::DependsOn)
        );
    }
}

#[test]
fn merged_controllers_are_diagnosed_in_both_orders() {
    for first in [true, false] {
        let interface = "interface C { marker?: string; }";
        let class = "@Controller() class C { @Get() run() {} }";
        let repo = Repo::new(&format!(
            "import {{Controller,Get}} from '@nestjs/common';
{}
{}
@Controller('safe') class Safe {{ @Get() run() {{}} }}",
            if first { interface } else { class },
            if first { class } else { interface }
        ));
        let g = graph(&repo.resolver(), true);
        assert_eq!(routes(&g), ["GET /safe".into()].into());
        assert_eq!(route_diagnostics(&g).len(), 1);
        assert_eq!(route_diagnostics(&g)[0].code, "unsupported_route_ambiguous");
        assert_eq!(
            route_diagnostics(&g)[0].line,
            Some(if first { 3 } else { 2 })
        );
        check_diagnostics(&g);
    }
}

#[test]
fn overload_implementation_only_snapshot_and_reapplication() {
    let source = "import {Controller,Get} from '@nestjs/common';
@Controller('old') class C {
run(x: string): string;
@Get() run(x: string) { return x; }
}";
    let repo = Repo::new(source);
    let r = repo.resolver();
    fs::write(repo.0.join("main.ts"), source.replace("old", "new")).unwrap();
    let mut b = builder();
    for (file, _) in r.sources() {
        nestjs_roles::extract_file(file, &r, &mut b).unwrap();
    }
    codebasecanvas_analyzer::nestjs_modules::analyze(&r, &b)
        .unwrap()
        .apply(&mut b)
        .unwrap();
    let findings = codebasecanvas_analyzer::nestjs_routes::analyze(&r, &b).unwrap();
    assert_eq!(findings.routes.len(), 1);
    let finding = &findings.routes[0];
    assert_eq!(
        &source[finding.site.start as usize..finding.site.end as usize],
        "Get()"
    );
    assert_eq!(
        &source[finding.controller_site.start as usize..finding.controller_site.end as usize],
        "Controller('old')"
    );
    assert!(
        source[finding.handler_site.start as usize..finding.handler_site.end as usize]
            .contains("return x")
    );
    findings.apply(&mut b).unwrap();
    findings.apply(&mut b).unwrap();
    r.apply_imports(&mut b).unwrap();
    let g = b.finish().unwrap();
    assert_eq!(routes(&g), ["GET /old".into()].into());
    assert_eq!(g, graph(&r, true));
    assert!(!g.to_json().unwrap().contains("return x"));
    assert_eq!(
        routes(&graph(&repo.resolver(), true)),
        ["GET /new".into()].into()
    );

    let mut empty = builder();
    assert!(findings.apply(&mut empty).is_err());
}

#[test]
fn type_only_unknown_does_not_capture_shadowed_or_foreign_decorators() {
    let repo = Repo::new(
        "import {Controller,Get} from '@nestjs/common';\n\
         import type {Get as X} from '@nestjs/common';\n\
         import type {Get as Foreign} from 'other';\n\
         function inner() { const X = () => () => {};\n\
           @Controller('shadow') class C { @X() @Get() run() {} }\n\
         }\n\
         @Controller('foreign') class ForeignC { @Foreign() @Get() run() {} }",
    );
    let g = graph(&repo.resolver(), true);
    assert_eq!(
        routes(&g),
        ["GET /shadow".into(), "GET /foreign".into()].into()
    );
    assert!(route_diagnostics(&g).is_empty());
}
