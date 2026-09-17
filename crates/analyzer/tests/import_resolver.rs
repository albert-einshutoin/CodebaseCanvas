use codebasecanvas_analyzer::{
    discovery::RepositoryRoot,
    resolver::{ImportResolver, Resolution},
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_REPO: AtomicU64 = AtomicU64::new(0);

struct Repo(PathBuf);
impl Repo {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "canvas-resolver-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_REPO.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn put(&self, name: &str, text: &str) {
        let p = self.0.join(name);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }
    fn resolve(&self) -> ImportResolver {
        ImportResolver::analyze(&RepositoryRoot::open(&self.0).unwrap()).unwrap()
    }
}
impl Drop for Repo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn binding_identity_not_spelling_controls_resolution_and_consumers() {
    let repo = Repo::new();
    repo.put(
        "a.ts",
        "export class Same {}\nfunction inner() { class Same {} }",
    );
    repo.put("b.ts", "export class Same {}");
    repo.put("main.ts", "import { Same as First } from './a';\nimport { Same as Second } from './b';\nimport { Same as Unused } from './a';\nclass Consumer { run() { const x = First; const y = Second; { const First = 1; return First; } } }");
    let r = repo.resolve();
    let uses: Vec<_> = r
        .references()
        .filter(|r| r.site.file == "main.ts")
        .collect();
    assert_eq!(uses.len(), 2);
    assert!(uses.iter().all(|u| u.consumer_id.is_some()));
    let targets: Vec<_> = uses.iter().map(|u| &r.import_for(u).resolution).collect();
    assert!(matches!(targets[0],Resolution::LocalSymbol { file, .. } if file=="a.ts"));
    assert!(matches!(targets[1],Resolution::LocalSymbol { file, .. } if file=="b.ts"));
    assert_eq!(r, repo.resolve());
}

#[test]
fn preserves_type_only_and_external_subpath_identity() {
    let repo = Repo::new();
    repo.put("ports.ts", "export class Token {} export interface Port {}");
    repo.put("main.ts", "import type {Token} from './ports'; import {type Port} from './ports'; import {Client as A} from 'pkg/a'; import {Client as B} from 'pkg/b'; import {Client as Scoped} from '@scope/pkg/subpath'; class C { constructor(t: Token, p: Port) {} run() { return [A,B,Scoped]; } }");
    let r = repo.resolve();
    let imports = r.imports();
    assert!(
        imports
            .iter()
            .filter(|i| matches!(i.local_name.as_deref(), Some("Token" | "Port")))
            .all(|i| i.type_only)
    );
    let ids: Vec<_> = imports
        .iter()
        .filter_map(|i| match &i.resolution {
            Resolution::ExternalSymbol { id, .. } => Some(id),
            _ => None,
        })
        .collect();
    assert_eq!(ids.len(), 3);
    assert!(imports.iter().any(|i| matches!(&i.resolution,Resolution::ExternalSymbol{specifier,package_root,..} if specifier=="@scope/pkg/subpath" && package_root=="@scope/pkg")));
    assert_ne!(ids[0], ids[1]);
}

#[test]
fn missing_import_never_uses_an_unrelated_same_name() {
    let repo = Repo::new();
    repo.put("a.ts", "export class Same {}");
    repo.put(
        "main.ts",
        "import {Same} from './missing'; class C { x: Same; }",
    );
    let r = repo.resolve();
    assert!(
        matches!(
            r.imports()[0].resolution,
            Resolution::Unresolved {
                reason: codebasecanvas_analyzer::resolver::UnresolvedReason::MissingModule
            }
        ),
        "{:?}",
        r.imports()[0].resolution
    );
    assert!(!r.diagnostics().is_empty());
}

fn graph(root: &RepositoryRoot) -> codebasecanvas_analyzer::SystemGraph {
    use codebasecanvas_analyzer::{
        CallAnalysis, CallMode, CallScope, GraphBuilder, GraphMetadata, discovery::discover,
    };
    // Test-only declaration/import projection, not completed call analysis or #17 output.
    let mut builder = GraphBuilder::new(GraphMetadata {
        analyzer_version: "resolver-test".into(),
        analyzed_at: "2026-09-17T00:00:00Z".into(),
        root_name: None,
        call_analysis: CallAnalysis {
            scope: CallScope::ParsedNamedClassMethods,
            mode: CallMode::SameClassOnly,
            examined_calls: 0,
            emitted_calls: 0,
            skipped_calls: 0,
        },
    });
    for file in discover(root).unwrap().files {
        let source = fs::read_to_string(root.resolve(&file).unwrap()).unwrap();
        codebasecanvas_analyzer::typescript::extract_file(&file, &source, &mut builder).unwrap();
    }
    ImportResolver::analyze(root)
        .unwrap()
        .apply_imports(&mut builder)
        .unwrap();
    builder.finish().unwrap()
}

#[test]
fn fixture_import_edges_match_hand_defined_oracle() {
    use codebasecanvas_analyzer::{EdgeKind, SystemGraph};
    let root = RepositoryRoot::open(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/nestjs-sample"),
    )
    .unwrap();
    let actual = graph(&root);
    let expected = SystemGraph::from_json(include_str!(
        "../../../examples/nestjs-sample/expected-graph.json"
    ))
    .unwrap();
    let edges = |g: &SystemGraph| {
        g.edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Imports)
            .map(|e| e.id.clone())
            .collect::<std::collections::BTreeSet<_>>()
    };
    assert_eq!(edges(&actual), edges(&expected));
}

#[test]
fn consumers_are_actual_declarations_and_static_instance_methods() {
    use codebasecanvas_analyzer::{EdgeKind, GraphBuilder, NodeKind};
    let repo = Repo::new();
    repo.put("main.ts", "import {Client} from 'pkg/a'; import {Unused} from 'pkg/b'; function top(){return Client;} class Empty{} class C { static run(){return Client;} run(){return Client;} } function outer(){class C {run(){return Client;}}}");
    let root = RepositoryRoot::open(&repo.0).unwrap();
    let r = repo.resolve();
    assert_eq!(
        r.references().filter(|u| u.consumer_id.is_none()).count(),
        1
    );
    let g = graph(&root);
    let edges: Vec<_> = g
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Imports)
        .collect();
    assert_eq!(edges.len(), 3);
    let owner = GraphBuilder::node_id(NodeKind::Class, "main.ts", &[], "C").unwrap();
    assert!(
        edges
            .iter()
            .any(|e| e.from == GraphBuilder::method_id(&owner, "static", "run"))
    );
    assert!(
        edges
            .iter()
            .any(|e| e.from == GraphBuilder::method_id(&owner, "instance", "run"))
    );
    assert!(
        edges
            .iter()
            .all(|e| e.to == GraphBuilder::external_id("pkg/a", "Client"))
    );
    assert!(
        !g.nodes
            .iter()
            .any(|n| n.id == GraphBuilder::external_id("pkg/b", "Unused"))
    );
}

#[test]
fn export_alias_and_export_type_retain_declaration_identity() {
    use codebasecanvas_analyzer::NodeKind;
    let repo = Repo::new();
    repo.put(
        "a.ts",
        "class Original{} interface Port{} export {Original as Alias}; export type {Port as API};",
    );
    repo.put(
        "main.ts",
        "import {Alias as Renamed,API} from './a'; class C { x: API; run(){return Renamed;} }",
    );
    let r = repo.resolve();
    assert!(matches!(
        &r.imports()[0].resolution,
        Resolution::LocalSymbol {
            kind: NodeKind::Class,
            ..
        }
    ));
    assert!(matches!(
        &r.imports()[1].resolution,
        Resolution::LocalSymbol {
            kind: NodeKind::Interface,
            export_type_only: true,
            ..
        }
    ));
    assert!(r.imports()[1].type_only);
    for reference in r.references() {
        assert_eq!(
            r.at_reference(&reference.site.file, reference.site.start),
            Some(reference)
        );
    }
}

#[test]
fn unsupported_forms_and_unparsed_targets_are_explicit() {
    use codebasecanvas_analyzer::resolver::UnresolvedReason;
    let repo = Repo::new();
    repo.put("a.ts", "export class A{} export default A;");
    repo.put("barrel.ts", "export {A} from './a';");
    repo.put("bad.ts", "export class {");
    repo.put("main.ts","import Default from './a'; import * as NS from './a'; import {A} from './barrel'; import {Broken} from './bad'; import 'side-effects'; class C {run(){return [Default,NS,A,Broken];}}");
    let r = repo.resolve();
    let reasons: Vec<_> = r
        .imports()
        .iter()
        .map(|i| match &i.resolution {
            Resolution::Unresolved { reason } => reason.clone(),
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(
        reasons,
        vec![
            UnresolvedReason::UnsupportedImport,
            UnresolvedReason::UnsupportedImport,
            UnresolvedReason::UnsupportedExport,
            UnresolvedReason::ParseIncomplete,
            UnresolvedReason::UnsupportedImport
        ]
    );
}

#[cfg(unix)]
#[test]
fn root_symlink_and_config_boundaries_never_resolve_outside_sources() {
    use codebasecanvas_analyzer::resolver::UnresolvedReason;
    use std::os::unix::fs::symlink;
    let repo = Repo::new();
    let outside = Repo::new();
    outside.put("secret.ts", "export class Secret{}");
    outside.put("config.json", "invalid secret config");
    symlink(&outside.0, repo.0.join("escape")).unwrap();
    symlink(outside.0.join("secret.ts"), repo.0.join("linked.ts")).unwrap();
    repo.put("node_modules/pkg/secret.ts", "export class Secret{}");
    repo.put("barrel.ts", "export {Secret} from './escape/secret';");
    repo.put("main.ts","import {Secret as A} from './escape/secret'; import {Secret as B} from './linked'; import {Secret as C} from '../secret'; import {Secret as D} from './node_modules/pkg/secret'; import {Secret as E} from './escape/missing'; class C { run(){return [A,B,C,D,E];} }");
    let r = repo.resolve();
    assert_eq!(r.imports().len(), 5);
    assert!(
        r.diagnostics()
            .iter()
            .any(|d| d.file.as_deref() == Some("barrel.ts")
                && d.code == "TS_IMPORT_BOUNDARYVIOLATION")
    );
    assert!(r.imports().iter().all(|i| matches!(
        i.resolution,
        Resolution::Unresolved {
            reason: UnresolvedReason::BoundaryViolation
        }
    )));
    for config in [
        r#"{"extends":"./escape/config.json"}"#,
        r#"{"compilerOptions":{"baseUrl":"./escape","paths":{"@/*":["*"]}}}"#,
        r#"{"compilerOptions":{"paths":{"@/*":["./escape/*"]}}}"#,
    ] {
        repo.put("tsconfig.json", config);
        repo.put(
            "main.ts",
            "import {Secret} from '@/secret'; class C { x: Secret; }",
        );
        let r = repo.resolve();
        assert!(matches!(
            r.imports()[0].resolution,
            Resolution::Unresolved {
                reason: UnresolvedReason::UnsupportedConfig
            }
        ));
        assert!(
            r.diagnostics()
                .iter()
                .any(|d| d.code == "TS_IMPORT_BOUNDARYVIOLATION")
        );
        assert!(!format!("{:?}", r.diagnostics()).contains(outside.0.to_str().unwrap()));
    }
}

#[test]
fn repeated_side_effect_occurrences_and_unsupported_exports_stay_visible() {
    use codebasecanvas_analyzer::resolver::UnresolvedReason;
    let repo = Repo::new();
    repo.put(
        "a.ts",
        "export class A{} export function helper(){} export const value=1;",
    );
    repo.put("main.ts","import {A} from './a'; import './a'; import './a'; import {helper,value} from './a'; class C {run(){return A;}}");
    let r = repo.resolve();
    assert_eq!(r.imports().len(), 5);
    let reasons: Vec<_> = r
        .imports()
        .iter()
        .filter_map(|i| match &i.resolution {
            Resolution::Unresolved { reason } => Some(reason),
            _ => None,
        })
        .collect();
    assert_eq!(
        reasons,
        vec![
            &UnresolvedReason::UnsupportedImport,
            &UnresolvedReason::UnsupportedImport,
            &UnresolvedReason::UnsupportedExport,
            &UnresolvedReason::UnsupportedExport
        ]
    );
    assert_eq!(r.references().count(), 1);
}
