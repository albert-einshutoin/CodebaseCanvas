use codebasecanvas_analyzer::{
    CallAnalysis, CallMode, CallScope, Confidence, EdgeKind, Evidence, EvidenceSource,
    GraphBuilder, GraphEdge, GraphMetadata, GraphNode, NodeKind, SystemGraph,
};
use serde_json::Value;

fn graph() -> SystemGraph {
    let cases: Value = serde_json::from_str(include_str!("../../../contracts/cases.json")).unwrap();
    SystemGraph::from_json(&cases["graph"].to_string()).unwrap()
}

fn add_graph(builder: &mut GraphBuilder, graph: &SystemGraph) {
    for node in &graph.nodes {
        builder.add_node(node.clone()).unwrap();
    }
    for edge in &graph.edges {
        builder.add_edge(edge.clone()).unwrap();
    }
    for diagnostic in &graph.diagnostics {
        builder.add_diagnostic(diagnostic.clone()).unwrap();
    }
}

fn hand_made_findings() -> (GraphMetadata, GraphNode, GraphNode, GraphEdge) {
    let evidence = Evidence {
        source: EvidenceSource::Ast,
        file: "src/a.ts".into(),
        line: Some(1),
        end_line: None,
        confidence: Confidence::Confirmed,
    };
    let module_id = GraphBuilder::node_id(NodeKind::Module, "src/a.ts", &[], "A");
    let service_id = GraphBuilder::node_id(NodeKind::Service, "src/a.ts", &[], "S");
    let module = GraphNode {
        id: module_id.clone(),
        kind: NodeKind::Module,
        name: "A".into(),
        qualified_name: None,
        file: Some("src/a.ts".into()),
        line: Some(1),
        end_line: None,
        parent_id: None,
        evidence: vec![evidence.clone()],
        metadata: None,
    };
    let service = GraphNode {
        id: service_id.clone(),
        kind: NodeKind::Service,
        name: "S".into(),
        qualified_name: None,
        file: Some("src/a.ts".into()),
        line: Some(2),
        end_line: None,
        parent_id: Some(module_id.clone()),
        evidence: vec![evidence.clone()],
        metadata: None,
    };
    let edge = GraphEdge {
        id: GraphBuilder::edge_id(&module_id, EdgeKind::Contains, &service_id),
        from: module_id,
        to: service_id,
        kind: EdgeKind::Contains,
        evidence: vec![evidence],
        metadata: None,
    };
    (
        GraphMetadata {
            analyzer_version: "test".into(),
            analyzed_at: "2026-01-01T00:00:00Z".into(),
            root_name: None,
            call_analysis: CallAnalysis {
                scope: CallScope::ParsedNamedClassMethods,
                mode: CallMode::SameClassOnly,
                examined_calls: 0,
                emitted_calls: 0,
                skipped_calls: 0,
            },
        },
        module,
        service,
        edge,
    )
}

#[test]
fn first_add_failure_also_fails_finish() {
    let (metadata, module, _, _) = hand_made_findings();
    let mut builder = GraphBuilder::new(metadata);
    builder.add_node(module.clone()).unwrap();
    let mut conflict = module;
    conflict.name.push_str("-changed");

    assert!(builder.add_node(conflict).is_err());
}

#[test]
fn merges_duplicate_findings_without_partial_conflict() {
    let (metadata, module, service, edge) = hand_made_findings();
    let mut builder = GraphBuilder::new(metadata);
    builder.add_node(module).unwrap();
    let mut node = service;
    builder.add_edge(edge).unwrap();
    let evidence = node.evidence[0].clone();
    builder.add_node(node.clone()).unwrap();
    node.evidence.push(evidence);
    builder.add_node(node).unwrap();
    let result = builder.finish().unwrap();
    assert_eq!(result.nodes.len(), 2);
    assert_eq!(result.edges.len(), 1);
}

#[test]
fn rejects_conflict_and_dangling_edge_at_finish() {
    let source = graph();
    let mut builder = GraphBuilder::new(source.metadata.clone());
    let node = source.nodes[0].clone();
    builder.add_node(node.clone()).unwrap();
    let mut conflict: GraphNode = node;
    conflict.name.push_str("-changed");
    let first_error = builder.add_node(conflict).unwrap_err();

    let edge = GraphEdge {
        id: GraphBuilder::edge_id(
            "missing",
            codebasecanvas_analyzer::EdgeKind::Imports,
            "also-missing",
        ),
        from: "missing".into(),
        to: "also-missing".into(),
        kind: codebasecanvas_analyzer::EdgeKind::Imports,
        evidence: source.edges[0].evidence.clone(),
        metadata: None,
    };
    assert_eq!(builder.add_edge(edge).unwrap_err(), first_error);
    assert_eq!(
        builder
            .add_diagnostic(source.diagnostics[0].clone())
            .unwrap_err(),
        first_error
    );
    assert_eq!(builder.finish().unwrap_err(), first_error);
}

#[test]
fn rejects_dangling_edge_at_finish() {
    let source = graph();
    let mut builder = GraphBuilder::new(source.metadata);
    builder.add_node(source.nodes[0].clone()).unwrap();
    let edge = GraphEdge {
        id: GraphBuilder::edge_id("missing", EdgeKind::Imports, "also-missing"),
        from: "missing".into(),
        to: "also-missing".into(),
        kind: EdgeKind::Imports,
        evidence: source.edges[0].evidence.clone(),
        metadata: None,
    };
    builder.add_edge(edge).unwrap();
    assert_eq!(builder.finish().unwrap_err(), "Dangling edge");
}

#[test]
fn merges_duplicate_edges_and_rejects_absolute_evidence() {
    let source = graph();
    let mut builder = GraphBuilder::new(source.metadata.clone());
    add_graph(&mut builder, &source);
    let mut edge = source.edges[0].clone();
    builder.add_edge(edge.clone()).unwrap();
    edge.evidence.push(source.edges[0].evidence[0].clone());
    builder.add_edge(edge).unwrap();
    let result = builder.finish().unwrap();
    assert_eq!(result.edges.len(), source.edges.len());
    assert_eq!(
        result.edges[0].evidence.len(),
        source.edges[0].evidence.len()
    );

    let mut invalid = source.nodes[0].clone();
    invalid.evidence = vec![Evidence {
        source: EvidenceSource::Ast,
        file: "/absolute.ts".into(),
        line: Some(1),
        end_line: None,
        confidence: Confidence::Confirmed,
    }];
    let mut builder = GraphBuilder::new(source.metadata);
    assert!(builder.add_node(invalid).is_err());
}

#[test]
fn finish_is_deterministically_ordered() {
    let source = graph();
    let mut a = GraphBuilder::new(source.metadata.clone());
    let mut b = GraphBuilder::new(GraphMetadata {
        ..source.metadata.clone()
    });
    for node in &source.nodes {
        a.add_node(node.clone()).unwrap();
    }
    for edge in &source.edges {
        a.add_edge(edge.clone()).unwrap();
    }
    for diagnostic in &source.diagnostics {
        a.add_diagnostic(diagnostic.clone()).unwrap();
    }
    for node in source.nodes.iter().rev() {
        b.add_node(node.clone()).unwrap();
    }
    for edge in source.edges.iter().rev() {
        b.add_edge(edge.clone()).unwrap();
    }
    for diagnostic in source.diagnostics.iter().rev() {
        b.add_diagnostic(diagnostic.clone()).unwrap();
    }
    assert_eq!(a.finish().unwrap(), b.finish().unwrap());
}

#[test]
fn preserves_multi_module_membership_without_display_parent() {
    let (metadata, module, mut service, edge) = hand_made_findings();
    let second_id = GraphBuilder::node_id(NodeKind::Module, "src/b.ts", &[], "B");
    let second = GraphNode {
        id: second_id.clone(),
        kind: NodeKind::Module,
        name: "B".into(),
        qualified_name: None,
        file: Some("src/b.ts".into()),
        line: Some(1),
        end_line: None,
        parent_id: None,
        evidence: service.evidence.clone(),
        metadata: None,
    };
    service.parent_id = None;
    let service_id = service.id.clone();
    let second_edge = GraphEdge {
        id: GraphBuilder::edge_id(&second_id, EdgeKind::Contains, &service_id),
        from: second_id,
        to: service_id,
        kind: EdgeKind::Contains,
        evidence: service.evidence.clone(),
        metadata: None,
    };
    let mut builder = GraphBuilder::new(metadata);
    for node in [module, second, service] {
        builder.add_node(node).unwrap();
    }
    builder.add_edge(edge).unwrap();
    builder.add_edge(second_edge).unwrap();
    let result = builder.finish().unwrap();
    let service = result.nodes.iter().find(|n| n.name == "S").unwrap();
    assert_eq!(service.parent_id, None);
    assert_eq!(
        result
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Contains && e.to == service.id)
            .count(),
        2
    );
}

#[test]
fn keeps_scopes_staticness_and_external_subpaths_distinct() {
    let class_a = GraphBuilder::node_id(NodeKind::Class, "src/a.ts", &["Outer"], "Thing");
    let class_b = GraphBuilder::node_id(NodeKind::Class, "src/a.ts", &["Inner"], "Thing");
    assert_ne!(class_a, class_b);
    assert_eq!(
        GraphBuilder::node_id(NodeKind::Class, "src/a.ts", &[], "Thing"),
        GraphBuilder::node_id(NodeKind::Service, "src/a.ts", &[], "Thing")
    );
    assert_ne!(
        GraphBuilder::method_id(&class_a, "instance", "run"),
        GraphBuilder::method_id(&class_a, "static", "run")
    );
    assert_ne!(
        GraphBuilder::external_id("pkg", "x"),
        GraphBuilder::external_id("pkg/subpath", "x")
    );
}

#[test]
fn deduplicates_call_edges_but_keeps_emitted_sites_and_direct_output_order() {
    let (mut metadata, module, service, contains) = hand_made_findings();
    metadata.call_analysis.examined_calls = 2;
    metadata.call_analysis.emitted_calls = 2;
    let owner = service.id.clone();
    let first_id = GraphBuilder::method_id(&owner, "instance", "first");
    let second_id = GraphBuilder::method_id(&owner, "instance", "second");
    let evidence = |file: &str, line: u64| Evidence {
        source: EvidenceSource::Ast,
        file: file.into(),
        line: Some(line),
        end_line: None,
        confidence: Confidence::Confirmed,
    };
    let method = |id: String, name: &str| GraphNode {
        id,
        kind: NodeKind::Method,
        name: name.into(),
        qualified_name: None,
        file: Some("src/a.ts".into()),
        line: Some(2),
        end_line: None,
        parent_id: Some(owner.clone()),
        evidence: vec![evidence("src/a.ts", 2)],
        metadata: None,
    };
    let call_id = GraphBuilder::edge_id(&first_id, EdgeKind::Calls, &second_id);
    let call = |site| GraphEdge {
        id: call_id.clone(),
        from: first_id.clone(),
        to: second_id.clone(),
        kind: EdgeKind::Calls,
        evidence: vec![site],
        metadata: None,
    };
    let mut builder = GraphBuilder::new(metadata);
    for node in [
        module,
        service,
        method(first_id.clone(), "first"),
        method(second_id.clone(), "second"),
    ] {
        builder.add_node(node).unwrap();
    }
    builder.add_edge(contains).unwrap();
    for (from, to) in [
        (owner.clone(), first_id.clone()),
        (owner, second_id.clone()),
    ] {
        builder
            .add_edge(GraphEdge {
                id: GraphBuilder::edge_id(&from, EdgeKind::Contains, &to),
                from,
                to,
                kind: EdgeKind::Contains,
                evidence: vec![evidence("src/a.ts", 2)],
                metadata: None,
            })
            .unwrap();
    }
    builder.add_edge(call(evidence("src/a.ts", 10))).unwrap();
    builder.add_edge(call(evidence("src/a.ts", 20))).unwrap();
    let result = builder.finish().unwrap();
    let call = result
        .edges
        .iter()
        .find(|e| e.kind == EdgeKind::Calls)
        .unwrap();
    assert_eq!(
        result
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Calls)
            .count(),
        1
    );
    assert_eq!(call.evidence.len(), 2);
    assert_eq!(result.metadata.call_analysis.emitted_calls, 2);
}

#[test]
fn rejects_invalid_parent_and_cycle() {
    let (metadata, _, mut service, _) = hand_made_findings();
    service.parent_id = Some("missing".into());
    let mut builder = GraphBuilder::new(metadata.clone());
    builder.add_node(service.clone()).unwrap();
    assert!(builder.finish().is_err());

    let mut left = service.clone();
    let mut right = service;
    left.id = GraphBuilder::node_id(NodeKind::Class, "src/l.ts", &[], "Left");
    right.id = GraphBuilder::node_id(NodeKind::Class, "src/r.ts", &[], "Right");
    left.parent_id = Some(right.id.clone());
    right.parent_id = Some(left.id.clone());
    let mut builder = GraphBuilder::new(metadata);
    builder.add_node(left).unwrap();
    builder.add_node(right).unwrap();
    assert!(builder.finish().is_err());
}
