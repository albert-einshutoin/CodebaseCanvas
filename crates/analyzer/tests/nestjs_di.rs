use codebasecanvas_analyzer::{
    CallAnalysis, CallMode, CallScope, EdgeKind, GraphBuilder, GraphMetadata, NodeKind,
    SystemGraph, discovery::RepositoryRoot, nestjs_di, nestjs_modules, nestjs_roles, nestjs_routes,
    resolver::ImportResolver,
};
use std::collections::BTreeMap;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Repo(PathBuf);
impl Repo {
    fn new(source: &str) -> Self {
        let path = loop {
            let path = std::env::temp_dir().join(format!(
                "canvas-di-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => break path,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("{e}"),
            }
        };
        let repo = Self(path);
        repo.write("main.ts", source);
        repo
    }
    fn write(&self, file: &str, source: &str) {
        let path = self.0.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, source).unwrap();
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
fn builder(r: &ImportResolver) -> GraphBuilder {
    let mut b = GraphBuilder::new(GraphMetadata {
        analyzer_version: "di-test".into(),
        analyzed_at: "2026-09-18T00:00:00Z".into(),
        root_name: None,
        call_analysis: CallAnalysis {
            scope: CallScope::ParsedNamedClassMethods,
            mode: CallMode::SameClassOnly,
            examined_calls: 0,
            emitted_calls: 0,
            skipped_calls: 0,
        },
    });
    for (file, _) in r.sources() {
        nestjs_roles::extract_file(file, r, &mut b).unwrap();
    }
    b
}
fn graph(r: &ImportResolver) -> SystemGraph {
    let mut b = builder(r);
    nestjs_modules::analyze(r, &b)
        .unwrap()
        .apply(&mut b)
        .unwrap();
    nestjs_routes::analyze(r, &b)
        .unwrap()
        .apply(&mut b)
        .unwrap();
    nestjs_di::analyze(r, &b).unwrap().apply(&mut b).unwrap();
    r.apply_imports(&mut b).unwrap();
    b.finish().unwrap()
}
fn injects(g: &SystemGraph) -> Vec<&codebasecanvas_analyzer::GraphEdge> {
    g.edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Injects)
        .collect()
}
#[test]
fn local_parameter_properties_and_dedupe() {
    let repo = Repo::new(
        "import {Injectable} from '@nestjs/common';\nabstract class Token {}\n@Injectable() class Consumer {\n constructor(a: Token,\n private readonly b: Token) {}\n}",
    );
    let g = graph(&repo.resolver());
    let edges = injects(&g);
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].from, id("main.ts", &[], "Consumer"));
    assert_eq!(edges[0].to, id("main.ts", &[], "Token"));
    assert_eq!(edges[0].evidence.len(), 2);
    assert_eq!(
        edges[0].metadata.as_ref().unwrap()["semantics"],
        "requested_token"
    );
}

fn id(file: &str, scope: &[&str], name: &str) -> String {
    GraphBuilder::node_id(codebasecanvas_analyzer::NodeKind::Class, file, scope, name).unwrap()
}
#[test]
fn fixture_di_and_existing_structure_match_oracle() {
    let root = RepositoryRoot::open(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/nestjs-sample"),
    )
    .unwrap();
    let r = ImportResolver::analyze(&root).unwrap();
    let actual = graph(&r);
    let expected = SystemGraph::from_json(include_str!(
        "../../../examples/nestjs-sample/expected-graph.json"
    ))
    .unwrap();

    assert_eq!(
        actual
            .nodes
            .iter()
            .filter(|n| matches!(
                n.kind,
                NodeKind::Module
                    | NodeKind::Controller
                    | NodeKind::Service
                    | NodeKind::Repository
                    | NodeKind::Class
            ))
            .count(),
        19
    );
    assert_eq!(injects(&actual).len(), 5);
    assert_eq!(injects(&actual), injects(&expected));
    let diags_di = |g: &SystemGraph| {
        g.diagnostics
            .iter()
            .filter(|d| d.code.starts_with("unsupported_di_"))
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
    assert_eq!(diags_di(&actual).len(), 3);
    assert_eq!(diags_di(&actual), diags_di(&expected));
    assert!(!actual.edges.iter().any(|e| e.kind == EdgeKind::Calls));
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
    let mut baseline = builder(&r);
    r.apply_imports(&mut baseline).unwrap();
    let before = baseline.finish().unwrap();
    for old in &before.nodes {
        let new = actual.nodes.iter().find(|n| n.id == old.id).unwrap();
        assert!(old.evidence.iter().all(|e| new.evidence.contains(e)));
    }
    assert_eq!(actual, graph(&r));
    let mut reversed = builder(&r);
    let mut modules = codebasecanvas_analyzer::nestjs_modules::analyze(&r, &reversed).unwrap();
    modules.modules.reverse();
    modules.apply(&mut reversed).unwrap();
    let mut findings = codebasecanvas_analyzer::nestjs_routes::analyze(&r, &reversed).unwrap();
    findings.routes.reverse();
    findings.diagnostics.reverse();
    findings.apply(&mut reversed).unwrap();
    findings.apply(&mut reversed).unwrap();
    let mut di = nestjs_di::analyze(&r, &reversed).unwrap();
    di.parameters.reverse();
    di.diagnostics.reverse();
    di.apply(&mut reversed).unwrap();
    di.apply(&mut reversed).unwrap();
    r.apply_imports(&mut reversed).unwrap();
    let mut reapplied = reversed.finish().unwrap();
    // Builder merges edge evidence but deliberately appends diagnostics.
    assert_eq!(
        reapplied
            .diagnostics
            .iter()
            .filter(|d| d.code.starts_with("unsupported_di_"))
            .count(),
        6
    );
    reapplied.diagnostics.dedup();
    assert_eq!(actual, reapplied);
}

fn di_codes(g: &SystemGraph) -> Vec<&str> {
    g.diagnostics
        .iter()
        .filter(|d| d.code.starts_with("unsupported_di_"))
        .map(|d| {
            assert!(d.file.is_some() && d.line.is_some() && d.skipped_count.is_none());
            assert!(
                g.nodes.iter().any(
                    |n| Some(&n.id) == d.related_node_id.as_ref() && n.kind != NodeKind::Method
                )
            );
            d.code.as_str()
        })
        .collect()
}
#[test]
fn import_alias_identity_and_same_name_scopes() {
    let repo = Repo::new(
        "import {Injectable} from '@nestjs/common';\nimport {Same as A} from './a'; import {Same as B} from './b';\n@Injectable() class Consumer { constructor(a: A, b: B) {} }\nnamespace Scope { class Same {} @Injectable() export class Consumer { constructor(a: Same) {} } }\nclass Same {}\n@Injectable() class Outer { constructor(a: Same) {} }",
    );
    repo.write("a.ts", "export class Same {}");
    repo.write("b.ts", "export class Same {}");
    let g = graph(&repo.resolver());
    let edges: std::collections::BTreeSet<_> = injects(&g)
        .iter()
        .map(|e| (e.from.clone(), e.to.clone()))
        .collect();
    assert_eq!(
        edges,
        [
            (id("main.ts", &[], "Consumer"), id("a.ts", &[], "Same")),
            (id("main.ts", &[], "Consumer"), id("b.ts", &[], "Same")),
            (
                id("main.ts", &["Scope"], "Consumer"),
                id("main.ts", &["Scope"], "Same")
            ),
            (id("main.ts", &[], "Outer"), id("main.ts", &[], "Same")),
        ]
        .into_iter()
        .collect()
    );
    assert!(di_codes(&g).is_empty());
}
#[test]
fn unsupported_parameters_keep_safe_siblings() {
    for (decl, param, code) in [
        ("interface Port {}", "bad: Port", "unsupported_di_interface"),
        (
            "type Alias = Good",
            "bad: Alias",
            "unsupported_di_reference",
        ),
        (
            "declare class Ambient {}",
            "bad: Ambient",
            "unsupported_di_class_value",
        ),
        ("", "bad: Missing", "unsupported_di_reference"),
        ("", "bad: Good | string", "unsupported_di_type"),
        ("", "bad: Good & {}", "unsupported_di_type"),
        ("", "bad: Good[]", "unsupported_di_type"),
        ("", "bad: typeof Good", "unsupported_di_type"),
        (
            "namespace N { export class Token {} }",
            "bad: N.Token",
            "unsupported_di_type",
        ),
        ("", "bad: import('./tokens').Token", "unsupported_di_type"),
        (
            "class Generic<T> {}",
            "bad: Generic<Good>",
            "unsupported_di_type",
        ),
        ("", "bad", "unsupported_di_type"),
        ("", "bad: Good = new Good()", "unsupported_di_parameter"),
        ("", "{bad}: {bad: Good}", "unsupported_di_parameter"),
        ("", "[bad]: [Good]", "unsupported_di_parameter"),
        (
            "interface Merged {} class Merged {}",
            "bad: Merged",
            "unsupported_di_reference",
        ),
        (
            "const Alias = Good",
            "bad: Alias",
            "unsupported_di_reference",
        ),
    ] {
        let repo = Repo::new(&format!(
            "import {{Injectable}} from '@nestjs/common'; class Good {{}} {decl}\n@Injectable() class Consumer {{ constructor({param}, safe: Good) {{}} }}"
        ));
        let g = graph(&repo.resolver());
        assert_eq!(injects(&g).len(), 1, "{param}: {g:?}");
        assert_eq!(injects(&g)[0].to, id("main.ts", &[], "Good"));
        assert_eq!(di_codes(&g), [code], "{param}");
        let d = g.diagnostics.iter().find(|d| d.code == code).unwrap();
        assert_eq!(d.line, Some(2));
        assert_eq!(d.related_node_id, Some(id("main.ts", &[], "Consumer")));
    }
}
#[test]
fn type_only_import_export_and_external_are_not_class_proofs() {
    for (import, exported, code) in [
        (
            "import type {Token} from './tokens';",
            "export class Token {}",
            "unsupported_di_type_only",
        ),
        (
            "import {type Token} from './tokens';",
            "export class Token {}",
            "unsupported_di_type_only",
        ),
        (
            "import {Token} from './tokens';",
            "class Token {} export type {Token};",
            "unsupported_di_type_only",
        ),
        (
            "import {Token} from './tokens';",
            "export declare class Token {}",
            "unsupported_di_type_only",
        ),
        (
            "import {Token} from './tokens';",
            "export interface Token {}",
            "unsupported_di_interface",
        ),
        (
            "import {Token} from './tokens';",
            "export type Token = string;",
            "unsupported_di_reference",
        ),
        (
            "import {Token} from './tokens';",
            "export {Token} from './other';",
            "unsupported_di_reference",
        ),
        (
            "import {Token} from 'unverified-package';",
            "",
            "unsupported_di_external",
        ),
    ] {
        let repo = Repo::new(&format!(
            "import {{Injectable}} from '@nestjs/common'; {import}\n@Injectable() class Consumer {{ constructor(token: Token) {{}} }}"
        ));
        repo.write("tokens.ts", exported);
        repo.write("other.ts", "export class Token {}");
        let r = repo.resolver();
        let g = graph(&r);
        assert!(injects(&g).is_empty(), "{import} {exported}");
        assert_eq!(di_codes(&g), [code], "{import} {exported}");
        if code == "unsupported_di_external" {
            assert!(
                g.nodes
                    .iter()
                    .any(|n| n.kind == NodeKind::ExternalDependency && n.name.contains("Token"))
            );
            assert!(
                g.edges.iter().any(
                    |e| e.kind == EdgeKind::Imports && e.from == id("main.ts", &[], "Consumer")
                )
            );
        }
    }
}
#[test]
fn explicit_inject_precedes_resolvable_or_invalid_type() {
    for expr in [
        "Wire('CACHE')",
        "Wire(SYMBOL)",
        "Wire(Good)",
        "Wire(forwardRef(() => Good))",
        "Wire()",
        "Wire",
    ] {
        for annotation in ["Good", "Missing"] {
            let repo = Repo::new(&format!(
                "import {{Injectable, Inject as Wire, forwardRef}} from '@nestjs/common'; const SYMBOL = Symbol(); class Good {{}}\n@Injectable() class Consumer {{ constructor(@{expr} bad: {annotation}, safe: Good) {{}} }}"
            ));
            let g = graph(&repo.resolver());
            assert_eq!(
                di_codes(&g),
                ["unsupported_di_custom_token"],
                "{expr} {annotation}"
            );
            assert_eq!(injects(&g).len(), 1);
            assert_eq!(injects(&g)[0].evidence.len(), 1);
            let r = repo.resolver();
            let b = builder(&r);
            let findings = nestjs_di::analyze(&r, &b).unwrap();
            assert!(findings.parameters[0].target.is_err());
            assert!(findings.parameters[1].target.is_ok());
        }
    }
}
#[test]
fn unsupported_decorator_origins_never_fall_back() {
    for (imports, decorator) in [
        ("import {Inject} from 'foreign';", "Inject('x')"),
        (
            "function Inject(x: unknown) { return () => {}; }",
            "Inject('x')",
        ),
        (
            "import * as nest from '@nestjs/common';",
            "nest.Inject('x')",
        ),
        ("import {Optional} from '@nestjs/common';", "Optional()"),
        (
            "function wrapper(x: unknown) { return x; } import {Inject} from '@nestjs/common';",
            "wrapper(Inject('x'))",
        ),
        ("import type {Inject} from '@nestjs/common';", "Inject('x')"),
    ] {
        let repo = Repo::new(&format!(
            "import {{Injectable}} from '@nestjs/common'; {imports} class Good {{}}\n@Injectable() class Consumer {{ constructor(@{decorator} private bad: Good) {{}} }}"
        ));
        let g = graph(&repo.resolver());
        assert!(injects(&g).is_empty(), "{decorator}");
        assert_eq!(
            di_codes(&g),
            [if decorator == "Optional()" {
                "unsupported_di_decorator"
            } else {
                "unsupported_di_decorator_origin"
            }],
            "{imports}"
        );
    }
    let repo = Repo::new(
        "import {Injectable, Inject} from '@nestjs/common'; class Good {} function scope(Inject: any) { @Injectable() class Consumer { constructor(@Inject('x') bad: Good) {} } }",
    );
    let g = graph(&repo.resolver());
    assert!(injects(&g).is_empty());
    assert_eq!(di_codes(&g), ["unsupported_di_decorator_origin"]);
}
#[test]
fn generic_shadowing_and_ambient_namespace_do_not_resolve_by_name() {
    for source in [
        "class Token {} @Injectable() class Consumer<Token> { constructor(t: Token) {} }",
        "import {Token} from './tokens'; @Injectable() class Consumer<Token> { constructor(t: Token) {} }",
        "declare namespace N { class Token {} @Injectable() class Consumer { constructor(t: Token); } }",
        "declare namespace N { class Token {} } namespace N { @Injectable() class Consumer { constructor(t: Token) {} } }",
    ] {
        let repo = Repo::new(&format!(
            "import {{Injectable}} from '@nestjs/common'; {source}"
        ));
        repo.write("tokens.ts", "export class Token {}");
        let g = graph(&repo.resolver());
        assert!(injects(&g).is_empty(), "{source}");
        assert!(!di_codes(&g).is_empty(), "{source}");
    }
}
#[test]
fn only_direct_role_constructor_implementation_is_consumed() {
    let repo = Repo::new(
        "import {Injectable, Controller, Module, Inject} from '@nestjs/common'; class Good {}\nclass Generic { constructor(t: Good) {} }\n@Module({providers:[Generic]}) class App {constructor(t: Good) {}}\n@Injectable() class Consumer {\n @Inject(Good) property: Good;\n constructor(t: Good);\n constructor(public t: Good) { class Nested { constructor(t: Good) {} } }\n method(other: Good) {}\n}\n@Controller() class Empty { constructor() {} }\n@Injectable() class Child extends Consumer {}\n@Injectable() class Access { constructor(protected a: Good, readonly b: Good) {} }",
    );
    let g = graph(&repo.resolver());
    assert_eq!(injects(&g).len(), 3);
    assert!(di_codes(&g).is_empty());
    assert!(g.nodes.iter().all(|n| n.name != "constructor"));
    assert_eq!(
        injects(&g)
            .iter()
            .filter(|e| e.from == id("main.ts", &[], "Consumer"))
            .count(),
        1
    );
    assert!(
        !injects(&g)
            .iter()
            .any(|e| e.from == id("main.ts", &[], "Generic")
                || e.from == id("main.ts", &[], "Child"))
    );
}
#[test]
fn dependencies_and_merged_consumer_are_scoped() {
    for (source, code) in [
        (
            "@Injectable() @Dependencies(Good) class Consumer { constructor(t: Good) {} }",
            "unsupported_di_dependencies",
        ),
        (
            "interface Consumer {} @Injectable() class Consumer { constructor(t: Good) {} }",
            "unsupported_di_ambiguous",
        ),
    ] {
        let repo = Repo::new(&format!(
            "import {{Injectable, Dependencies}} from '@nestjs/common'; class Good {{}} {source}"
        ));
        let g = graph(&repo.resolver());
        assert!(injects(&g).is_empty());
        assert_eq!(di_codes(&g), [code]);
    }
}
#[test]
fn rest_and_same_line_evidence() {
    let repo = Repo::new(
        "import {Injectable} from '@nestjs/common'; class Good {} @Injectable() class Consumer { constructor(a: Good, b: Good, ...rest: Good[]) {} }",
    );
    let g = graph(&repo.resolver());
    assert_eq!(injects(&g).len(), 1);
    assert_eq!(injects(&g)[0].evidence.len(), 1);
    assert_eq!(di_codes(&g), ["unsupported_di_parameter"]);
}
#[test]
fn provider_overrides_do_not_select_implementation() {
    for provider in [
        "{provide: Good, useClass: Mock}",
        "{provide: Good, useValue: new Mock()}",
        "{provide: Good, useFactory: () => new Mock()}",
        "{provide: Good, useExisting: Mock}",
    ] {
        let repo = Repo::new(&format!(
            "import {{Injectable, Module}} from '@nestjs/common'; class Good {{ run() {{}} }} class Mock {{ run() {{}} }}\n@Injectable() class Consumer {{constructor(t: Good) {{}}}}\n@Module({{providers:[{provider}, Consumer]}}) class App {{}}"
        ));
        let g = graph(&repo.resolver());
        assert_eq!(injects(&g).len(), 1);
        assert_eq!(injects(&g)[0].to, id("main.ts", &[], "Good"));
        assert!(!g.edges.iter().any(|e| e.kind == EdgeKind::Calls));
        assert_eq!(
            g.diagnostics
                .iter()
                .filter(|d| d.code == "unsupported_module_provider")
                .count(),
            1
        );
    }
}
#[test]
fn retained_source_and_application_order_are_stable() {
    let repo = Repo::new(
        "import {Injectable, Module} from '@nestjs/common'; import {Token} from './tokens'; @Injectable() class Consumer {constructor(t: Token, bad: Missing) {}} @Module({providers:[Consumer]}) class App {}",
    );
    repo.write("tokens.ts", "export class Token {}");
    let r = repo.resolver();
    let expected = graph(&r);
    repo.write("main.ts", "invalid !!!");
    repo.write("tokens.ts", "export interface Token {}");
    let mut b = builder(&r);
    let original = builder(&r).finish().unwrap();
    let di = nestjs_di::analyze(&r, &b).unwrap();
    r.apply_imports(&mut b).unwrap();
    di.apply(&mut b).unwrap();
    di.apply(&mut b).unwrap();
    nestjs_routes::analyze(&r, &b)
        .unwrap()
        .apply(&mut b)
        .unwrap();
    nestjs_modules::analyze(&r, &b)
        .unwrap()
        .apply(&mut b)
        .unwrap();
    let mut actual = b.finish().unwrap();
    assert_eq!(di_codes(&actual).len(), 2);
    actual.diagnostics.dedup();
    assert_eq!(actual, expected);
    for n in original.nodes {
        let new = actual.nodes.iter().find(|x| x.id == n.id).unwrap();
        assert!(n.evidence.iter().all(|e| new.evidence.contains(e)));
    }
}

#[test]
fn apply_rechecks_existing_nodes_and_consumer_kind() {
    let repo = Repo::new(
        "import {Injectable} from '@nestjs/common'; class Good {} class Generic {} @Injectable() class Consumer {constructor(t: Good) {}}",
    );
    let r = repo.resolver();
    let mut b = builder(&r);
    let mut findings = nestjs_di::analyze(&r, &b).unwrap();
    findings.parameters[0].target = Ok(id("main.ts", &[], "Missing"));
    assert!(findings.apply(&mut b).is_err());
    findings.parameters[0].target = Ok(id("main.ts", &[], "Good"));
    findings.parameters[0].consumer_id = id("main.ts", &[], "Generic");
    assert!(findings.apply(&mut b).is_err());
    assert!(injects(&b.finish().unwrap()).is_empty());
}

#[test]
fn unresolved_dependencies_alias_blocks_only_affected_consumer() {
    for decorators in [
        "@Injectable()\n@Needs(SelectedToken)",
        "@Needs(SelectedToken)\n@Injectable()",
    ] {
        let source = format!(
            "import {{ Injectable }} from '@nestjs/common';\nimport {{ Dependencies as Needs }} from './barrel';\nclass WrittenType {{}}\nclass SelectedToken {{}}\n{decorators}\nclass Consumer {{ constructor(value: WrittenType) {{}} }}\n@Injectable()\nclass Healthy {{ constructor(value: WrittenType) {{}} }}"
        );
        let repo = Repo::new(&source);
        repo.write(
            "barrel.ts",
            "export { Dependencies } from '@nestjs/common';",
        );
        let r = repo.resolver();
        let binding = r
            .imports()
            .iter()
            .find(|b| b.local_name.as_deref() == Some("Needs"))
            .unwrap();
        assert_eq!(binding.exported_name, "Dependencies");
        assert!(matches!(
            binding.resolution,
            codebasecanvas_analyzer::resolver::Resolution::Unresolved { .. }
        ));
        let findings = nestjs_di::analyze(&r, &builder(&r)).unwrap();
        let g = graph(&r);
        eprintln!(
            "binding={binding:?}\nfindings={findings:?}\nedges={:?}\nresolver diagnostics={:?}\ndi diagnostics={:?}",
            injects(&g),
            r.diagnostics(),
            di_codes(&g)
        );
        let consumer = id("main.ts", &[], "Consumer");
        assert!(!injects(&g).iter().any(|e| e.from == consumer));
        assert!(
            !findings
                .parameters
                .iter()
                .any(|p| p.consumer_id == consumer)
        );
        assert!(
            g.diagnostics
                .iter()
                .any(|d| d.code == "unsupported_di_dependencies_origin"
                    && d.related_node_id.as_ref() == Some(&consumer)
                    && d.file.as_deref() == Some("main.ts")
                    && d.line == Some(5))
        );
        assert_eq!(injects(&g).len(), 1);
        assert_eq!(injects(&g)[0].from, id("main.ts", &[], "Healthy"));
        assert_eq!(injects(&g)[0].to, id("main.ts", &[], "WrittenType"));
        assert!(!r.diagnostics().is_empty());
        assert!(r.diagnostics().iter().all(|d| g.diagnostics.contains(d)));
    }
}

#[test]
fn dependencies_candidates_use_original_export_and_semantic_scope() {
    for (imports, decorator, code) in [
        (
            "import {Dependencies} from '@nestjs/common';",
            "Dependencies",
            Some("unsupported_di_dependencies"),
        ),
        (
            "import {Dependencies as Needs} from '@nestjs/common';",
            "Needs",
            Some("unsupported_di_dependencies"),
        ),
        (
            "import type {Dependencies as Needs} from '@nestjs/common';",
            "Needs",
            Some("unsupported_di_dependencies_origin"),
        ),
        (
            "import {type Dependencies as Needs} from '@nestjs/common';",
            "Needs",
            Some("unsupported_di_dependencies_origin"),
        ),
        (
            "import type {Dependencies as Needs} from './barrel';",
            "Needs",
            Some("unsupported_di_dependencies_origin"),
        ),
        (
            "import {Roles as Dependencies} from './barrel';",
            "Dependencies",
            None,
        ),
        (
            "import {Dependencies} from 'foreign';",
            "Dependencies",
            None,
        ),
        (
            "function Dependencies(...args: any[]) { return (...args: any[]) => {}; }",
            "Dependencies",
            None,
        ),
    ] {
        for reverse in [false, true] {
            let decorators = if reverse {
                format!("@{decorator}(SelectedToken)\n@Injectable()")
            } else {
                format!("@Injectable()\n@{decorator}(SelectedToken)")
            };
            let repo = Repo::new(&format!(
                "import {{Injectable}} from '@nestjs/common'; {imports}\nclass WrittenType {{}} class SelectedToken {{}}\n{decorators}\nclass Consumer {{constructor(value: WrittenType) {{}}}}\n@Injectable() class Healthy {{constructor(value: WrittenType) {{}}}}"
            ));
            repo.write(
                "barrel.ts",
                "export {Dependencies, Roles} from '@nestjs/common';",
            );
            let g = graph(&repo.resolver());
            let consumer = id("main.ts", &[], "Consumer");
            assert_eq!(
                injects(&g).iter().any(|e| e.from == consumer),
                code.is_none(),
                "{imports}"
            );
            assert!(
                !injects(&g)
                    .iter()
                    .any(|e| e.to == id("main.ts", &[], "SelectedToken"))
            );
            assert_eq!(
                di_codes(&g),
                code.into_iter().collect::<Vec<_>>(),
                "{imports}"
            );
            assert!(
                injects(&g)
                    .iter()
                    .any(|e| e.from == id("main.ts", &[], "Healthy")
                        && e.to == id("main.ts", &[], "WrittenType"))
            );
        }
    }
    for import in [
        "import {Dependencies} from '@nestjs/common';",
        "import type {Dependencies} from '@nestjs/common';",
    ] {
        let repo = Repo::new(&format!(
            "import {{Injectable}} from '@nestjs/common'; {import} class WrittenType {{}} function scope(Dependencies: any) {{ @Injectable() @Dependencies() class Consumer {{constructor(value: WrittenType) {{}}}} }}"
        ));
        let g = graph(&repo.resolver());
        assert_eq!(injects(&g).len(), 1);
        assert!(di_codes(&g).is_empty());
    }
}
