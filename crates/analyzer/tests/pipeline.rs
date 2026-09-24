use codebasecanvas_analyzer::{
    Diagnostic, EdgeKind, NodeKind, Severity, SystemGraph, discovery::RepositoryRoot, pipeline,
};
#[path = "support/fixture_repo.rs"]
mod fixture_repo;
use fixture_repo::Repo;
use std::{collections::BTreeMap, fs};

#[test]
fn fixture_pipeline_assembles_all_recognizers() {
    let repo = Repo::new();
    repo.copy_fixture();
    let root = RepositoryRoot::open(&repo.0).unwrap();
    let result = pipeline::analyze(&root, "2026-09-24T00:00:00Z").unwrap();
    assert_eq!(
        (
            result.graph.nodes.len(),
            result.graph.edges.len(),
            result.graph.diagnostics.len()
        ),
        (57, 90, 15)
    );
    assert_eq!(
        result
            .graph
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::DatabaseModel)
            .count(),
        2
    );
    assert_eq!(result.prisma_schemas, 1);
    assert!(result.typescript_sources > 0);
    assert_eq!(
        (
            result.graph.metadata.call_analysis.examined_calls,
            result.graph.metadata.call_analysis.emitted_calls,
            result.graph.metadata.call_analysis.skipped_calls
        ),
        (9, 2, 7)
    );
    assert_eq!(result.typescript_sources, 13);
    assert_eq!(
        result.graph.metadata.analyzer_version,
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(result.graph.metadata.analyzed_at, "2026-09-24T00:00:00Z");
    assert_eq!(
        result.graph.metadata.root_name.as_deref(),
        repo.0.file_name().unwrap().to_str()
    );
    let oracle = SystemGraph::from_json(include_str!(
        "../../../examples/nestjs-sample/expected-graph.json"
    ))
    .unwrap();
    let nodes = |graph: &SystemGraph| {
        graph.nodes.iter().map(|node| (
            node.id.clone(),
            serde_json::json!({"kind":node.kind,"name":node.name,"file":node.file,"parentId":node.parent_id}),
        )).collect::<BTreeMap<_, _>>()
    };
    let edges = |graph: &SystemGraph| {
        graph
            .edges
            .iter()
            .map(|edge| {
                (
                    edge.id.clone(),
                    serde_json::json!({"from":edge.from,"to":edge.to,"kind":edge.kind}),
                )
            })
            .collect::<BTreeMap<_, _>>()
    };
    assert_eq!(nodes(&result.graph), nodes(&oracle));
    assert_eq!(edges(&result.graph), edges(&oracle));
    for expected in &oracle.nodes {
        if let Some(metadata) = &expected.metadata {
            let actual = result
                .graph
                .nodes
                .iter()
                .find(|n| n.id == expected.id)
                .unwrap();
            for (key, value) in metadata {
                assert_eq!(
                    actual.metadata.as_ref().and_then(|m| m.get(key)),
                    Some(value),
                    "{} {key}",
                    expected.id
                );
            }
        }
    }
    for expected in &oracle.edges {
        if let Some(metadata) = &expected.metadata {
            let actual = result
                .graph
                .edges
                .iter()
                .find(|e| e.id == expected.id)
                .unwrap();
            for (key, value) in metadata {
                assert_eq!(
                    actual.metadata.as_ref().and_then(|m| m.get(key)),
                    Some(value),
                    "{} {key}",
                    expected.id
                );
            }
        }
    }
    assert_eq!(
        diagnostic_projection(&result.graph.diagnostics),
        diagnostic_projection(&oracle.diagnostics)
    );
    assert_eq!(
        result
            .graph
            .diagnostics
            .iter()
            .filter(|d| d.code.starts_with("unsupported_call_"))
            .map(|d| d.skipped_count.unwrap())
            .sum::<u64>(),
        7
    );
    assert!(
        result
            .graph
            .edges
            .iter()
            .all(|e| !matches!(e.kind, EdgeKind::Reads | EdgeKind::Writes))
    );
    let service = result
        .graph
        .nodes
        .iter()
        .find(|n| n.name == "UsersService" && n.kind == NodeKind::Service)
        .unwrap();
    assert_eq!(
        result
            .graph
            .nodes
            .iter()
            .filter(|n| n.id == service.id)
            .count(),
        1
    );
    assert!(service.parent_id.is_none());
    assert_eq!(
        result
            .graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Contains && e.to == service.id)
            .count(),
        2
    );
    let get_users = result
        .graph
        .nodes
        .iter()
        .filter(|n| {
            n.kind == NodeKind::Endpoint
                && n.metadata
                    .as_ref()
                    .and_then(|m| m.get("httpMethod"))
                    .and_then(|v| v.as_str())
                    == Some("GET")
                && n.metadata
                    .as_ref()
                    .and_then(|m| m.get("path"))
                    .and_then(|v| v.as_str())
                    == Some("/users")
        })
        .collect::<Vec<_>>();
    assert_eq!(get_users.len(), 2);
    assert_ne!(
        get_users[0].metadata.as_ref().unwrap()["controllerMethodId"],
        get_users[1].metadata.as_ref().unwrap()["controllerMethodId"]
    );
    for (name, line, end, fields) in [
        (
            "User",
            2,
            5,
            serde_json::json!([{"name":"id","type":"String"},{"name":"name","type":"String"}]),
        ),
        (
            "Token",
            7,
            10,
            serde_json::json!([{"name":"id","type":"String"},{"name":"value","type":"String"}]),
        ),
    ] {
        let node = result
            .graph
            .nodes
            .iter()
            .find(|n| n.name == name && n.kind == NodeKind::DatabaseModel)
            .unwrap();
        assert_eq!((node.line, node.end_line), (Some(line), Some(end)));
        assert_eq!(node.metadata.as_ref().unwrap()["fields"], fields);
        assert_eq!(node.evidence[0].file, "prisma/schema.prisma");
        assert_eq!(
            (node.evidence[0].line, node.evidence[0].end_line),
            (Some(line), Some(end))
        );
    }
    let json = result.graph.to_json().unwrap();
    assert!(!json.contains(repo.0.to_str().unwrap()));
    assert_eq!(json, repo.analyze().graph.to_json().unwrap());
}

fn diagnostic_projection(diagnostics: &[Diagnostic]) -> Vec<String> {
    let mut items = diagnostics
        .iter()
        .map(|d| {
            serde_json::json!({
                "code":d.code,"severity":d.severity,"file":d.file,"line":d.line,
                "relatedNodeId":d.related_node_id,"skippedCount":d.skipped_count,
            })
            .to_string()
        })
        .collect::<Vec<_>>();
    items.sort();
    items
}

#[test]
fn empty_no_declarations_parse_errors_and_prisma_only_are_distinct() {
    let repo = Repo::new();
    let empty = repo.analyze();
    assert_eq!(
        (
            empty.typescript_sources,
            empty.prisma_schemas,
            empty.graph.nodes.len()
        ),
        (0, 0, 0)
    );
    assert!(empty.graph.diagnostics.is_empty());

    repo.write("src/empty.ts", "export const value = 1;\n");
    let no_declarations = repo.analyze();
    assert_eq!(
        (
            no_declarations.typescript_sources,
            no_declarations.graph.nodes.len()
        ),
        (1, 0)
    );
    assert!(no_declarations.graph.diagnostics.is_empty());

    repo.write("src/bad.ts", "class Broken {\n");
    let mixed = repo.analyze();
    assert_eq!(mixed.typescript_sources, 2);
    assert!(
        mixed
            .graph
            .diagnostics
            .iter()
            .any(|d| d.code == "TS_PARSE_ERROR"
                && d.severity == Severity::Error
                && d.file.as_deref() == Some("src/bad.ts"))
    );

    fs::remove_file(repo.0.join("src/empty.ts")).unwrap();
    let all_invalid = repo.analyze();
    assert_eq!(
        (
            all_invalid.typescript_sources,
            all_invalid.graph.nodes.len()
        ),
        (1, 0)
    );
    assert!(
        all_invalid
            .graph
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    );
    assert_eq!(all_invalid.graph.metadata.call_analysis.examined_calls, 0);

    fs::remove_file(repo.0.join("src/bad.ts")).unwrap();
    repo.write("prisma/schema.prisma", "model Only { id String }\n");
    let prisma_only = repo.analyze();
    assert_eq!(
        (prisma_only.typescript_sources, prisma_only.prisma_schemas),
        (0, 1)
    );
    assert_eq!(
        prisma_only
            .graph
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::DatabaseModel)
            .count(),
        1
    );
}

#[test]
fn valid_source_survives_a_parse_error() {
    let repo = Repo::new();
    repo.write("good.ts", "export class Good { method() {} }\n");
    repo.write("bad.ts", "class Broken {\n");
    let result = repo.analyze();
    assert_eq!(result.typescript_sources, 2);
    assert!(result.graph.nodes.iter().any(|n| n.name == "Good"));
    assert!(
        result
            .graph
            .diagnostics
            .iter()
            .any(|d| d.code == "TS_PARSE_ERROR" && d.file.as_deref() == Some("bad.ts"))
    );
}

#[cfg(unix)]
#[test]
fn shared_discovery_diagnostics_are_not_doubled_or_used_to_hide_prisma_findings() {
    use std::os::unix::fs::symlink;
    let repo = Repo::new();
    repo.write("real/source.ts", "export class Good {}\n");
    repo.write(
        "real/schema.prisma",
        "model User {\n broken Unsupported(\"X\")\n id Int\n}\n",
    );
    symlink("real", repo.0.join("alias")).unwrap();
    symlink(".", repo.0.join("loop")).unwrap();
    let result = repo.analyze();
    assert_eq!((result.typescript_sources, result.prisma_schemas), (1, 1));
    assert!(result.graph.nodes.iter().any(|n| n.name == "Good"));
    assert!(result.graph.nodes.iter().any(|n| n.name == "User"));
    let shared = result
        .graph
        .diagnostics
        .iter()
        .filter(|d| d.code.starts_with("DISCOVERY_"))
        .collect::<Vec<_>>();
    assert!(!shared.is_empty());
    assert_eq!(
        diagnostic_projection(&shared.into_iter().cloned().collect::<Vec<_>>()).len(),
        result
            .graph
            .diagnostics
            .iter()
            .filter(|d| d.code.starts_with("DISCOVERY_"))
            .map(|d| serde_json::to_string(d).unwrap())
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    );
    assert_eq!(
        result
            .graph
            .diagnostics
            .iter()
            .filter(|d| d.code == "PRISMA_UNSUPPORTED_FIELD")
            .count(),
        1
    );
}
