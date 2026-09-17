use codebasecanvas_analyzer::{
    CallAnalysis, CallMode, CallScope, Confidence, EdgeKind, EvidenceSource, GraphBuilder,
    GraphMetadata, NodeKind, SystemGraph, discovery::RepositoryRoot, nestjs_roles,
    resolver::ImportResolver,
};
use std::{
    collections::BTreeMap,
    fs, io,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_REPO: AtomicU64 = AtomicU64::new(0);

struct Repo(PathBuf);
impl Repo {
    fn new(source: &str) -> Self {
        Self::at_time(
            source,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        )
    }
    fn at_time(source: &str, timestamp: u128) -> Self {
        let path = std::env::temp_dir().join(format!(
            "canvas-roles-{}-{timestamp}-{}",
            std::process::id(),
            NEXT_REPO.fetch_add(1, Ordering::Relaxed)
        ));
        Self::create(path, source).unwrap()
    }
    fn create(path: PathBuf, source: &str) -> io::Result<Self> {
        // Acquire ownership exclusively; never overwrite or remove a pre-existing directory.
        fs::create_dir(&path)?;
        let repo = Self(path);
        fs::write(repo.0.join("main.ts"), source)?;
        Ok(repo)
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
    b.finish().unwrap()
}
fn kind(g: &SystemGraph, name: &str) -> NodeKind {
    g.nodes.iter().find(|n| n.name == name).unwrap().kind
}
#[test]
fn basic_alias_evidence_and_identity() {
    let repo = Repo::new(
        "import {Module, Controller, Injectable as Managed} from '@nestjs/common';\n@Module({providers: anything()}) class App {}\n@Controller(dynamic()) class Api {}\n@Managed() class AuthService { run() {} }\n@Managed() class UserRepository {}\nclass PlainService {} class PlainRepository {}",
    );
    let r = repo.resolver();
    let before = graph(&r, false);
    let after = graph(&r, true);
    for (name, expected) in [
        ("App", NodeKind::Module),
        ("Api", NodeKind::Controller),
        ("AuthService", NodeKind::Service),
        ("UserRepository", NodeKind::Repository),
        ("PlainService", NodeKind::Class),
        ("PlainRepository", NodeKind::Class),
    ] {
        assert_eq!(kind(&after, name), expected);
    }
    assert_eq!(before.nodes.len(), after.nodes.len());
    assert_eq!(before.edges, after.edges);
    for old in &before.nodes {
        let new = after.nodes.iter().find(|n| n.id == old.id).unwrap();
        let mut unchanged = new.clone();
        unchanged.kind = old.kind;
        unchanged.evidence = old.evidence.clone();
        assert_eq!(&unchanged, old);
        assert!(old.evidence.iter().all(|e| new.evidence.contains(e)));
    }
    let repository = after
        .nodes
        .iter()
        .find(|n| n.name == "UserRepository")
        .unwrap();
    assert!(
        repository
            .evidence
            .iter()
            .any(|e| e.source == EvidenceSource::Nestjs && e.confidence == Confidence::BestEffort)
    );
    assert!(
        repository
            .evidence
            .iter()
            .any(|e| e.source == EvidenceSource::Nestjs && e.confidence == Confidence::Confirmed)
    );
    assert!(after.diagnostics.is_empty());
    assert_eq!(after, graph(&r, true));
}
#[test]
fn binding_and_unsupported_forms_do_not_guess() {
    let repo = Repo::new(
        "import {Injectable, Controller as Real, InjectableFactory as Similar} from '@nestjs/common';\nimport {Controller} from 'other';\nimport type {Module} from '@nestjs/common';\nimport * as nest from '@nestjs/common';\nimport {Injectable as Reexported} from './barrel';\n@Controller() class Other {}\n@Module() class TypeOnly {}\n@Similar() class SimilarExport {}\n@nest.Injectable() class Namespace {}\n@wrap(Injectable()) class Wrapped {}\n@Reexported() class Barrel {}\nfunction local(Injectable: any) { @Injectable() class Shadow {} }\nfunction Controller2() {}\n@Controller2() class UserDefined {}\n@Real() @Controller2() class Valid {}\n@Injectable() class Same { run(){} }\nfunction inner(){ @Injectable() class Same { run(){} } }",
    );
    fs::write(
        repo.0.join("barrel.ts"),
        "export {Injectable} from '@nestjs/common';",
    )
    .unwrap();
    fs::write(
        repo.0.join("other.ts"),
        "import {Injectable} from '@nestjs/common'; @Injectable() class Same { run(){} }",
    )
    .unwrap();
    let g = graph(&repo.resolver(), true);
    for name in [
        "Other",
        "TypeOnly",
        "SimilarExport",
        "Namespace",
        "Wrapped",
        "Barrel",
        "Shadow",
        "UserDefined",
    ] {
        assert_eq!(kind(&g, name), NodeKind::Class, "{name}");
    }
    assert_eq!(kind(&g, "Valid"), NodeKind::Controller);
    assert_eq!(
        g.nodes
            .iter()
            .filter(|n| n.name == "Same" && n.kind == NodeKind::Service)
            .count(),
        3
    );
    assert!(
        g.diagnostics
            .iter()
            .any(|d| d.code == "NESTJS_ROLE_TYPE_ONLY" || d.code == "NESTJS_ROLE_ORIGIN_UNKNOWN")
    );
    assert!(
        g.diagnostics
            .iter()
            .filter(|d| d.code.starts_with("NESTJS_ROLE_"))
            .all(|d| d.file.as_deref() == Some("main.ts")
                && d.line.is_some()
                && d.related_node_id.is_some())
    );
}
#[test]
fn conflicts_are_order_independent_and_keep_static_evidence() {
    for decorators in ["@Module() @Injectable()", "@Injectable() @Module()"] {
        let repo = Repo::new(&format!(
            "import {{Module, Injectable}} from '@nestjs/common';\n{decorators}\nclass Mixed {{}}"
        ));
        let g = graph(&repo.resolver(), true);
        assert_eq!(kind(&g, "Mixed"), NodeKind::Class);
        assert_eq!(
            g.diagnostics
                .iter()
                .filter(|d| d.code == "NESTJS_ROLE_CONFLICT")
                .count(),
            1
        );
        let n = g.nodes.iter().find(|n| n.name == "Mixed").unwrap();
        assert!(n.evidence.iter().any(|e| e.source == EvidenceSource::Ast));
        assert!(
            n.evidence
                .iter()
                .any(|e| e.source == EvidenceSource::Nestjs)
        );
    }
}
#[test]
fn unresolved_config_and_snapshot_stay_fail_closed() {
    let repo = Repo::new("import {Injectable} from '@nestjs/common';\n@Injectable() class A {}");
    fs::write(
        repo.0.join("tsconfig.json"),
        r#"{"compilerOptions":{"baseUrl":"."}}"#,
    )
    .unwrap();
    let r = repo.resolver();
    fs::write(repo.0.join("main.ts"), "class Changed {}").unwrap();
    let g = graph(&r, true);
    assert_eq!(kind(&g, "A"), NodeKind::Class);
    assert!(
        g.diagnostics
            .iter()
            .any(|d| d.code == "TS_IMPORT_UNSUPPORTEDCONFIG")
    );
    assert!(
        g.diagnostics
            .iter()
            .any(|d| d.code == "NESTJS_ROLE_UNRESOLVED" && d.related_node_id.is_some())
    );
}
#[test]
fn fixture_exact_id_kind_and_43_imports_projection() {
    let root = RepositoryRoot::open(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/nestjs-sample"),
    )
    .unwrap();
    let r = ImportResolver::analyze(&root).unwrap();
    let g = graph(&r, true);
    let oracle = SystemGraph::from_json(include_str!(
        "../../../examples/nestjs-sample/expected-graph.json"
    ))
    .unwrap();
    let roles = |g: &SystemGraph| {
        g.nodes
            .iter()
            .filter(|n| {
                matches!(
                    n.kind,
                    NodeKind::Class
                        | NodeKind::Module
                        | NodeKind::Controller
                        | NodeKind::Service
                        | NodeKind::Repository
                )
            })
            .map(|n| (n.id.clone(), n.kind))
            .collect::<BTreeMap<_, _>>()
    };
    assert_eq!(roles(&g).len(), 19);
    assert_eq!(roles(&g), roles(&oracle));
    let imports = |g: &SystemGraph| {
        g.edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Imports)
            .map(|e| (e.from.clone(), e.kind, e.to.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(imports(&g).len(), 43);
    assert_eq!(imports(&g), imports(&oracle));
    let before = graph(&r, false);
    assert_eq!(before.edges, g.edges);
    assert_eq!(before.nodes.len(), g.nodes.len());
    assert_eq!(g, graph(&r, true));
}

#[test]
fn repeated_classification_is_idempotent_and_method_decorators_do_not_classify_owner() {
    let repo = Repo::new(
        "import {Injectable as Managed, Controller} from '@nestjs/common';\n// 日本語 changes byte offsets\n@Managed() @Managed() class ARepository { run() {} }\nclass Plain { @Controller() run(){} }",
    );
    let r = repo.resolver();
    let expected = graph(&r, true);
    assert_eq!(kind(&expected, "Plain"), NodeKind::Class);
    let mut b = builder();
    for _ in 0..2 {
        nestjs_roles::extract_file("main.ts", &r, &mut b).unwrap();
        r.apply_imports(&mut b).unwrap();
    }
    assert_eq!(b.finish().unwrap(), expected);
    let mut b = builder();
    assert!(nestjs_roles::extract_file("missing.ts", &r, &mut b).is_err());
}

#[test]
fn type_only_aliases_have_class_scoped_unknowns_without_spelling_fallback() {
    let repo = Repo::new(
        "import type {Injectable as Managed} from '@nestjs/common';\nimport {type Controller as Handler} from '@nestjs/common';\n@Managed() class A {}\n@Handler() class B {}",
    );
    let g = graph(&repo.resolver(), true);
    for name in ["A", "B"] {
        let node = g.nodes.iter().find(|n| n.name == name).unwrap();
        assert_eq!(node.kind, NodeKind::Class);
        assert!(
            node.evidence
                .iter()
                .all(|e| e.source == EvidenceSource::Ast)
        );
        assert!(
            g.diagnostics
                .iter()
                .any(|d| d.related_node_id.as_ref() == Some(&node.id)
                    && matches!(
                        d.code.as_str(),
                        "NESTJS_ROLE_TYPE_ONLY" | "NESTJS_ROLE_ORIGIN_UNKNOWN"
                    ))
        );
    }
}

#[test]
fn unresolved_custom_decorator_exports_do_not_add_role_diagnostics() {
    for (exported, local) in [
        ("Roles", "AccessRoles"),
        ("Roles", "Injectable"),
        ("Public", "Public"),
    ] {
        let repo = Repo::new(&format!(
            "import {{ Injectable as Managed }} from '@nestjs/common';\nimport {{ {exported} as {local} }} from './roles';\n@Managed()\n@{local}()\nexport class AuthService {{}}"
        ));
        fs::write(
            repo.0.join("roles.ts"),
            format!("export function {exported}() {{ return (_target: Function) => {{}}; }}"),
        )
        .unwrap();
        let r = repo.resolver();
        let binding = r
            .imports()
            .iter()
            .find(|i| i.specifier == "./roles")
            .unwrap();
        assert_eq!(binding.exported_name, exported);
        assert!(matches!(
            binding.resolution,
            codebasecanvas_analyzer::resolver::Resolution::Unresolved { .. }
        ));
        let g = graph(&r, true);
        let node = g.nodes.iter().find(|n| n.name == "AuthService").unwrap();
        assert_eq!(node.kind, NodeKind::Service);
        assert!(
            node.evidence
                .iter()
                .any(|e| e.source == EvidenceSource::Ast)
        );
        assert!(
            node.evidence
                .iter()
                .any(|e| e.source == EvidenceSource::Nestjs
                    && e.confidence == Confidence::Confirmed
                    && e.line == Some(3))
        );
        assert!(
            r.diagnostics()
                .iter()
                .any(|d| d.code == "TS_IMPORT_UNSUPPORTEDEXPORT")
        );
        assert!(r.diagnostics().iter().all(|d| g.diagnostics.contains(d)));
        assert!(
            !g.diagnostics
                .iter()
                .any(|d| d.code == "NESTJS_ROLE_UNRESOLVED"),
            "{exported} as {local}: {:?}",
            g.diagnostics
        );
    }
}

#[test]
fn unresolved_reexported_role_keeps_class_scoped_diagnostic() {
    let repo = Repo::new(
        "import { Injectable as Managed } from './barrel';\n@Managed() export class AuthService {}",
    );
    fs::write(
        repo.0.join("barrel.ts"),
        "export { Injectable } from '@nestjs/common';",
    )
    .unwrap();
    let r = repo.resolver();
    let g = graph(&r, true);
    let node = g.nodes.iter().find(|n| n.name == "AuthService").unwrap();
    assert_eq!(node.kind, NodeKind::Class);
    assert!(
        node.evidence
            .iter()
            .all(|e| e.source == EvidenceSource::Ast)
    );
    assert!(
        g.diagnostics
            .iter()
            .any(|d| d.code == "NESTJS_ROLE_UNRESOLVED"
                && d.related_node_id.as_ref() == Some(&node.id)
                && d.file.as_deref() == Some("main.ts")
                && d.line == Some(2))
    );
    assert!(!r.diagnostics().is_empty());
    assert!(r.diagnostics().iter().all(|d| g.diagnostics.contains(d)));
}

#[test]
fn repo_same_timestamp_keeps_sources_and_cleanup_independent() {
    let first = Repo::at_time("class First {}", 42);
    let second = Repo::at_time("class Second {}", 42);
    assert_ne!(first.0, second.0);
    assert_eq!(
        fs::read_to_string(first.0.join("main.ts")).unwrap(),
        "class First {}"
    );
    assert_eq!(
        fs::read_to_string(second.0.join("main.ts")).unwrap(),
        "class Second {}"
    );
    let first_path = first.0.clone();
    drop(first);
    assert!(!first_path.exists());
    assert_eq!(
        fs::read_to_string(second.0.join("main.ts")).unwrap(),
        "class Second {}"
    );
    let second_path = second.0.clone();
    drop(second);
    assert!(!second_path.exists());
}

#[test]
fn repo_existing_directory_is_rejected_without_mutation() {
    let owner = Repo::at_time("class Original {}", 42);
    let error = Repo::create(owner.0.clone(), "class Overwritten {}")
        .err()
        .unwrap();
    assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(
        fs::read_to_string(owner.0.join("main.ts")).unwrap(),
        "class Original {}"
    );
}
