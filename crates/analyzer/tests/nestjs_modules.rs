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
            "canvas-modules-{}-{}-{}",
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
    r.apply_imports(&mut b).unwrap();
    if roles {
        codebasecanvas_analyzer::nestjs_modules::analyze(r, &b)
            .unwrap()
            .apply(&mut b)
            .unwrap();
    }
    b.finish().unwrap()
}
fn kind(g: &SystemGraph, name: &str) -> NodeKind {
    g.nodes.iter().find(|n| n.name == name).unwrap().kind
}

#[test]
fn fixture_composition_matches_oracle() {
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
    let before = graph(&r, false);
    for old in &before.nodes {
        let new = actual.nodes.iter().find(|n| n.id == old.id).unwrap();
        assert!(old.evidence.iter().all(|e| new.evidence.contains(e)));
    }
    assert_eq!(actual, graph(&r, true));
}

fn node<'a>(g: &'a SystemGraph, name: &str) -> &'a codebasecanvas_analyzer::GraphNode {
    g.nodes.iter().find(|n| n.name == name).unwrap()
}
fn composition_names(g: &SystemGraph) -> std::collections::BTreeSet<(String, String)> {
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
                g.nodes
                    .iter()
                    .find(|n| n.id == e.from)
                    .unwrap()
                    .name
                    .clone(),
                g.nodes.iter().find(|n| n.id == e.to).unwrap().name.clone(),
            )
        })
        .collect()
}
#[test]
fn local_membership_exports_duplicates_and_order_are_static() {
    let repo = Repo::new(
        "import {Module as Mod, Controller, Injectable} from '@nestjs/common';\n@Injectable() class Shared { run(){} } class Solo {} class ExportOnly {}\n@Controller() class Api {}\n@Mod({}) class Dependency {}\n@Mod({imports:[Dependency],controllers:[Api],providers:[Shared,Shared,Solo],exports:[Shared,ExportOnly]}) class A {}\n@Mod({providers:[Shared]}) class B {}\n@Mod({imports:[],controllers:[],providers:[],exports:[]}) class Empty {}",
    );
    let r = repo.resolver();
    let mut b = builder();
    nestjs_roles::extract_file("main.ts", &r, &mut b).unwrap();
    r.apply_imports(&mut b).unwrap();
    let findings = codebasecanvas_analyzer::nestjs_modules::analyze(&r, &b).unwrap();
    let a = findings
        .modules
        .iter()
        .find(|f| {
            f.module_id == GraphBuilder::node_id(NodeKind::Module, "main.ts", &[], "A").unwrap()
        })
        .unwrap();
    assert_eq!(
        a.entries
            .iter()
            .filter(
                |e| e.field == codebasecanvas_analyzer::nestjs_modules::ModuleField::Exports
                    && e.target.is_ok()
            )
            .count(),
        2
    );
    findings.apply(&mut b).unwrap();
    findings.apply(&mut b).unwrap();
    let g = b.finish().unwrap();
    assert_eq!(
        composition_names(&g),
        [
            ("A", "Dependency"),
            ("A", "Api"),
            ("A", "Shared"),
            ("A", "Solo"),
            ("B", "Shared")
        ]
        .into_iter()
        .map(|(a, b)| (a.into(), b.into()))
        .collect()
    );
    assert_eq!(node(&g, "Shared").parent_id, None);
    assert_eq!(node(&g, "Solo").parent_id, Some(node(&g, "A").id.clone()));
    assert_eq!(node(&g, "ExportOnly").parent_id, None);
    assert_eq!(node(&g, "Dependency").parent_id, None);
    assert_eq!(
        node(&g, "run").parent_id,
        Some(node(&g, "Shared").id.clone())
    );
    assert_eq!(kind(&g, "Solo"), NodeKind::Class);
    assert!(g.diagnostics.is_empty());
    let mut b = builder();
    nestjs_roles::extract_file("main.ts", &r, &mut b).unwrap();
    r.apply_imports(&mut b).unwrap();
    let mut reversed = findings.clone();
    reversed.modules.reverse();
    for f in &mut reversed.modules {
        f.entries.reverse();
    }
    reversed.apply(&mut b).unwrap();
    assert_eq!(b.finish().unwrap(), g);
}
#[test]
fn array_partial_success_differs_from_uncertain_object_overwrites() {
    let repo = Repo::new(
        "import {Module,Controller,forwardRef} from '@nestjs/common';\nclass A {} @Controller() class C {}\n@Module({providers:[A,...rest,{provide:A,useClass:A},missing],imports:[forwardRef(()=>M),M.register()]}) class Partial {}\n@Module({providers:[A],...metadata}) class Spread {}\n@Module({providers:[A],[key]:[]}) class Computed {}\n@Module({providers:[A],providers:[],controllers:[C]}) class Duplicate {}\n@Module({providers:[A]}) @Module({}) class Multiple {}\n@Module(metadata) class Variable {}\n@Module(makeMetadata()) class Factory {}\n@Module({providers: makeProviders(), controllers:[C]}) class FieldFactory {}\n@Module({}) class M { static register(){} }",
    );
    let g = graph(&repo.resolver(), true);
    assert_eq!(
        composition_names(&g),
        [("Partial", "A"), ("Duplicate", "C"), ("FieldFactory", "C")]
            .into_iter()
            .map(|(a, b)| (a.into(), b.into()))
            .collect()
    );
    for name in ["Spread", "Computed", "Multiple", "Variable", "Factory"] {
        assert!(
            g.diagnostics
                .iter()
                .any(|d| d.code == "unsupported_module_metadata"
                    && d.related_node_id.as_ref() == Some(&node(&g, name).id))
        );
    }
    for code in [
        "unsupported_module_spread",
        "unsupported_module_provider",
        "unsupported_module_reference",
        "unsupported_module_forward_ref",
        "unsupported_module_dynamic",
    ] {
        assert!(g.diagnostics.iter().any(|d| d.code == code));
    }
    assert!(
        g.diagnostics
            .iter()
            .filter(|d| d.code.starts_with("unsupported_module_"))
            .all(|d| d.skipped_count.is_none()
                && d.file.as_deref() == Some("main.ts")
                && d.line.is_some())
    );
}
#[test]
fn binding_scope_types_and_aliases_never_guess() {
    let repo = Repo::new(
        "import {Module} from '@nestjs/common';\nimport {Same as Left} from './a'; import {Same as Right} from './b'; import type {Same as Typed} from './a'; import {External} from 'pkg';\nclass Local {} interface Port {} class Merged {} interface Merged {} const Alias=Local;\n@Module({providers:[Left,Right,Typed,External,Port,Merged,Alias,unknown],imports:[Local],controllers:[Local]}) class Root {}\nfunction inner(){ class Local {} @Module({providers:[Local]}) class Inner {} }\nfunction shadow(Left:any){ @Module({providers:[Left]}) class Shadow {} }",
    );
    fs::write(repo.0.join("a.ts"), "export class Same {}").unwrap();
    fs::write(repo.0.join("b.ts"), "export class Same {}").unwrap();
    let g = graph(&repo.resolver(), true);
    let root = node(&g, "Root");
    let edges: Vec<_> = g
        .edges
        .iter()
        .filter(|e| e.from == root.id && e.kind == EdgeKind::Contains)
        .collect();
    assert_eq!(edges.len(), 2);
    let targets: std::collections::BTreeSet<_> = edges.iter().map(|e| e.to.clone()).collect();
    assert_eq!(
        targets,
        ["a.ts", "b.ts"]
            .into_iter()
            .map(|f| GraphBuilder::node_id(NodeKind::Class, f, &[], "Same").unwrap())
            .collect()
    );
    let inner = node(&g, "Inner");
    let e = g
        .edges
        .iter()
        .find(|e| e.from == inner.id && e.kind == EdgeKind::Contains)
        .unwrap();
    assert_ne!(
        e.to,
        GraphBuilder::node_id(NodeKind::Class, "main.ts", &[], "Local").unwrap()
    );
    assert!(
        !g.edges
            .iter()
            .any(|e| e.from == node(&g, "Shadow").id && e.kind == EdgeKind::Contains)
    );
    assert!(
        g.diagnostics
            .iter()
            .any(|d| d.code == "unsupported_module_kind")
    );
}
#[test]
fn unknown_decorators_and_conflicting_roles_do_not_generate_composition_warnings() {
    let repo = Repo::new(
        "import {Module,Injectable} from '@nestjs/common';import {Module as Fake} from 'other'; class A {}\n@Fake({providers:[A]}) class Other {}\n@Module({providers:[A]}) @Injectable() class Mixed {}\nfunction Custom(x:any){} @Custom({providers:[A]}) class Local {}",
    );
    let g = graph(&repo.resolver(), true);
    assert!(composition_names(&g).is_empty());
    assert!(
        !g.diagnostics
            .iter()
            .any(|d| d.code.starts_with("unsupported_module_"))
    );
    fs::write(
        repo.0.join("tsconfig.json"),
        r#"{"compilerOptions":{"baseUrl":"."}}"#,
    )
    .unwrap();
    let g = graph(&repo.resolver(), true);
    assert!(composition_names(&g).is_empty());
}
#[test]
fn builder_rejects_bad_composition_and_keeps_normal_conflict_detection() {
    use codebasecanvas_analyzer::{GraphEdge, typescript};
    for (from, to, kind) in [
        ("Missing", "Target", EdgeKind::Contains),
        ("M", "Missing", EdgeKind::Contains),
        ("M", "M", EdgeKind::Contains),
        ("M", "Target", EdgeKind::DependsOn),
        ("Target", "M", EdgeKind::Contains),
        ("M", "Target", EdgeKind::Imports),
    ] {
        let repo = Repo::new(
            "import {Module} from '@nestjs/common'; @Module({}) class M {} class Target {}",
        );
        let r = repo.resolver();
        let mut b = builder();
        nestjs_roles::extract_file("main.ts", &r, &mut b).unwrap();
        let from = GraphBuilder::node_id(NodeKind::Class, "main.ts", &[], from).unwrap();
        let to = GraphBuilder::node_id(NodeKind::Class, "main.ts", &[], to).unwrap();
        assert!(
            b.apply_module_composition(vec![GraphEdge {
                id: GraphBuilder::edge_id(&from, kind, &to),
                from,
                to,
                kind,
                evidence: vec![codebasecanvas_analyzer::Evidence {
                    source: EvidenceSource::Nestjs,
                    file: "main.ts".into(),
                    line: Some(1),
                    end_line: None,
                    confidence: Confidence::Confirmed
                }],
                metadata: None
            }])
            .is_err()
        );
        assert!(b.finish().is_err());
    }
    let repo = Repo::new(
        "import {Module} from '@nestjs/common'; class Target {} @Module({providers:[Target]}) class M {}",
    );
    let r = repo.resolver();
    let mut b = builder();
    nestjs_roles::extract_file("main.ts", &r, &mut b).unwrap();
    codebasecanvas_analyzer::nestjs_modules::analyze(&r, &b)
        .unwrap()
        .apply(&mut b)
        .unwrap();
    assert!(typescript::extract_file("main.ts", r.source("main.ts").unwrap(), &mut b).is_err());
    assert!(b.finish().is_err());
}

#[test]
fn merged_module_origin_is_diagnosed_in_both_declaration_orders() {
    for interface_first in [true, false] {
        let interface = "interface FeatureModule { marker?: string; }\n";
        let class = "@Module({ providers: [Service] })\nclass FeatureModule {}\n";
        let source = format!(
            "import {{ Module }} from '@nestjs/common';\nclass Service {{}}\n{}{}@Module({{ providers: [Service] }}) class OrdinaryModule {{}}\n",
            if interface_first { interface } else { class },
            if interface_first { class } else { interface },
        );
        let repo = Repo::new(&source);
        let r = repo.resolver();
        let g = graph(&r, true);
        let module = node(&g, "FeatureModule");
        let edges = composition_names(&g);
        eprintln!(
            "interface_first={interface_first}, node={:?}, edges={edges:?}, diagnostics={:?}",
            module.kind, g.diagnostics
        );
        assert_eq!(module.kind, NodeKind::Module);
        assert_eq!(edges, [("OrdinaryModule".into(), "Service".into())].into());
        let diagnostics: Vec<_> = g
            .diagnostics
            .iter()
            .filter(|d| d.related_node_id.as_deref() == Some(&module.id))
            .collect();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "unsupported_module_ambiguous");
        assert_eq!(diagnostics[0].file.as_deref(), Some("main.ts"));
        assert_eq!(
            diagnostics[0].line,
            Some(if interface_first { 4 } else { 3 })
        );
        assert_eq!(g, graph(&r, true));
    }
}
