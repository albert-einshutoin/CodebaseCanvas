use codebasecanvas_analyzer::typescript::extract_file;
use codebasecanvas_analyzer::{
    CallAnalysis, CallMode, CallScope, GraphBuilder, GraphMetadata, NodeKind, SystemGraph,
    discovery::{RepositoryRoot, discover},
};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

fn fixture_root() -> RepositoryRoot {
    RepositoryRoot::open(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/nestjs-sample")
            .as_path(),
    )
    .unwrap()
}

fn metadata() -> GraphMetadata {
    GraphMetadata {
        analyzer_version: "typescript-extractor-test".to_owned(),
        analyzed_at: "2026-09-05T00:00:00Z".to_owned(),
        root_name: Some("nestjs-sample".to_owned()),
        call_analysis: CallAnalysis {
            scope: CallScope::ParsedNamedClassMethods,
            mode: CallMode::SameClassOnly,
            examined_calls: 0,
            emitted_calls: 0,
            skipped_calls: 0,
        },
    }
}

fn extract_fixture() -> SystemGraph {
    let root = fixture_root();
    let discovered = discover(&root).unwrap();
    let mut builder = GraphBuilder::new(metadata());
    for file in discovered.files {
        let source = fs::read_to_string(root.resolve(&file).unwrap()).unwrap();
        extract_file(&file, &source, &mut builder).unwrap();
    }
    builder.finish().unwrap()
}

fn node<'a>(
    graph: &'a SystemGraph,
    kind: NodeKind,
    name: &str,
    file: &str,
) -> &'a codebasecanvas_analyzer::GraphNode {
    graph
        .nodes
        .iter()
        .find(|node| node.kind == kind && node.name == name && node.file.as_deref() == Some(file))
        .unwrap_or_else(|| panic!("missing {kind:?} {name} in {file}"))
}

#[test]
fn extracts_fixture_declarations_methods_scopes_and_exports() {
    let graph = extract_fixture();
    let expected = SystemGraph::from_json(include_str!(
        "../../../examples/nestjs-sample/expected-graph.json"
    ))
    .unwrap();
    let expected_ids: BTreeSet<_> = expected
        .nodes
        .iter()
        .filter(|node| {
            matches!(
                node.kind,
                NodeKind::Module
                    | NodeKind::Controller
                    | NodeKind::Service
                    | NodeKind::Repository
                    | NodeKind::Class
                    | NodeKind::Interface
                    | NodeKind::Method
            )
        })
        .map(|node| node.id.as_str())
        .collect();
    let actual_ids: BTreeSet<_> = graph.nodes.iter().map(|node| node.id.as_str()).collect();
    assert_eq!(actual_ids, expected_ids);
    let expected_contains: BTreeSet<_> = expected
        .edges
        .iter()
        .filter(|edge| {
            edge.kind == codebasecanvas_analyzer::EdgeKind::Contains
                && expected
                    .nodes
                    .iter()
                    .any(|node| node.id == edge.to && node.kind == NodeKind::Method)
        })
        .map(|edge| edge.id.as_str())
        .collect();
    let actual_contains: BTreeSet<_> = graph
        .edges
        .iter()
        .filter(|edge| edge.kind == codebasecanvas_analyzer::EdgeKind::Contains)
        .map(|edge| edge.id.as_str())
        .collect();
    assert_eq!(actual_contains, expected_contains);
    assert_eq!(graph.nodes.len(), 37);
    assert_eq!(graph.edges.len(), 17);
    assert_eq!(
        graph
            .nodes
            .iter()
            .filter(|node| node.kind == NodeKind::Class)
            .count(),
        19
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .filter(|node| node.kind == NodeKind::Method)
            .count(),
        17
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .filter(|node| node.kind == NodeKind::Interface)
            .count(),
        1
    );
    assert!(graph.nodes.iter().any(|node| node.kind == NodeKind::Class));
    assert!(graph.nodes.iter().any(|node| node.kind == NodeKind::Method));
    assert!(
        graph
            .edges
            .iter()
            .any(|edge| edge.kind == codebasecanvas_analyzer::EdgeKind::Contains)
    );

    let controller = node(
        &graph,
        NodeKind::Class,
        "AuthController",
        "src/auth/auth.controller.ts",
    );
    assert_eq!(controller.qualified_name.as_deref(), Some("AuthController"));
    assert_eq!(controller.line, Some(5));
    assert_eq!(
        controller.metadata.as_ref().and_then(|m| m.get("exported")),
        Some(&Value::Bool(true))
    );

    let interface = node(
        &graph,
        NodeKind::Interface,
        "Port",
        "src/regressions/ports.ts",
    );
    assert_eq!(interface.line, Some(1));
    let outer = node(
        &graph,
        NodeKind::Class,
        "Same",
        "src/regressions/identity.ts",
    );
    let same_ids: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Class && node.name == "Same")
        .map(|node| node.id.as_str())
        .collect();
    assert_eq!(same_ids.len(), 2);
    assert!(same_ids.iter().any(|id| id != &outer.id));

    let methods: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.kind == NodeKind::Method
                && node.name == "find"
                && node.file.as_deref() == Some("src/users/users.service.ts")
        })
        .collect();
    assert_eq!(methods.len(), 2);
    assert_ne!(methods[0].id, methods[1].id);
}

#[test]
fn extraction_is_deterministic_for_the_same_fixture() {
    let first = extract_fixture().to_json().unwrap();
    let second = extract_fixture().to_json().unwrap();
    assert_eq!(first, second);
}

#[test]
fn parse_failure_is_reported_as_a_file_diagnostic() {
    let mut builder = GraphBuilder::new(metadata());
    let summary = extract_file("src/broken.ts", "export class Broken {", &mut builder).unwrap();
    assert_eq!(summary.nodes, 0);
    assert_eq!(summary.diagnostics, 1);
    let graph = builder.finish().unwrap();
    assert_eq!(graph.diagnostics[0].code, "TS_PARSE_ERROR");
    assert_eq!(graph.diagnostics[0].file.as_deref(), Some("src/broken.ts"));
}

#[test]
fn unsupported_namedness_is_reported_without_guessing_ids() {
    let mut builder = GraphBuilder::new(metadata());
    let summary = extract_file(
        "src/unsupported.ts",
        "class Example { [name]() {} }\nconst value = class {};",
        &mut builder,
    )
    .unwrap();
    assert_eq!(summary.diagnostics, 2);
    let graph = builder.finish().unwrap();
    let codes: Vec<_> = graph
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect();
    assert!(codes.contains(&"TS_ANONYMOUS_CLASS"));
    assert!(codes.contains(&"TS_UNSUPPORTED_METHOD_NAME"));
    assert!(!graph.nodes.iter().any(|node| node.name == "name"));
}

#[test]
fn lexical_blocks_keep_same_names_distinct() {
    let mut builder = GraphBuilder::new(metadata());
    let summary = extract_file(
        "src/scopes.ts",
        "function first() { class Same {} }\nfunction second() { class Same {} }",
        &mut builder,
    )
    .unwrap();
    let graph = builder.finish().unwrap();
    assert_eq!(summary.nodes, 2);
    let ids: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Class && node.name == "Same")
        .map(|node| node.id.as_str())
        .collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
}

#[test]
fn static_blocks_keep_same_names_distinct() {
    let mut builder = GraphBuilder::new(metadata());
    let summary = extract_file(
        "src/static-scopes.ts",
        "class Outer { static { class Same {} } static { class Same {} } }",
        &mut builder,
    )
    .unwrap();
    let graph = builder.finish().unwrap();
    assert_eq!(summary.nodes, 3);
    let ids: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Class && node.name == "Same")
        .map(|node| node.id.as_str())
        .collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
}

#[test]
fn switch_scopes_keep_same_names_distinct() {
    let mut builder = GraphBuilder::new(metadata());
    let summary = extract_file(
        "src/switch-scopes.ts",
        "switch (first) { case 1: class Same {} }\nswitch (second) { case 2: class Same {} }",
        &mut builder,
    )
    .unwrap();
    let graph = builder.finish().unwrap();
    assert_eq!(summary.nodes, 2);
    let ids: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Class && node.name == "Same")
        .map(|node| node.id.as_str())
        .collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
}

#[test]
fn source_evidence_handles_all_line_terminators() {
    for separator in ["\n", "\r", "\r\n", "\u{2028}", "\u{2029}"] {
        let mut builder = GraphBuilder::new(metadata());
        let source = format!("class First {{}}{separator}class Second {{}} ");
        extract_file("src/lines.ts", &source, &mut builder).unwrap();
        let graph = builder.finish().unwrap();
        let second = node(&graph, NodeKind::Class, "Second", "src/lines.ts");
        assert_eq!(second.line, Some(2), "separator: {separator:?}");
        assert_eq!(second.end_line, Some(2), "separator: {separator:?}");
    }
}

#[test]
fn re_exports_do_not_mark_local_declarations_as_exported() {
    let mut builder = GraphBuilder::new(metadata());
    extract_file(
        "src/reexport.ts",
        "class Local {}\nexport { Local } from './other';",
        &mut builder,
    )
    .unwrap();
    let graph = builder.finish().unwrap();
    let local = node(&graph, NodeKind::Class, "Local", "src/reexport.ts");
    assert!(local.metadata.is_none());
}

#[test]
fn local_exports_mark_local_declarations_as_exported() {
    let mut builder = GraphBuilder::new(metadata());
    extract_file(
        "src/export.ts",
        "class Local {}\nexport { Local };",
        &mut builder,
    )
    .unwrap();
    let graph = builder.finish().unwrap();
    let local = node(&graph, NodeKind::Class, "Local", "src/export.ts");
    assert_eq!(
        local
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get("exported")),
        Some(&Value::Bool(true))
    );
}

#[test]
fn rejects_non_repository_or_non_typescript_paths() {
    let mut builder = GraphBuilder::new(metadata());
    assert!(extract_file("/tmp/broken.ts", "", &mut builder).is_err());
    assert!(extract_file("src/broken.js", "", &mut builder).is_err());
}
