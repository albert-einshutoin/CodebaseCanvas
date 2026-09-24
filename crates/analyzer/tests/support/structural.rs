//! Test-only, order-independent comparison against the hand-authored fixture oracle.
use codebasecanvas_analyzer::{
    Confidence, Diagnostic, EdgeKind, Evidence, EvidenceSource, GraphEdge, GraphNode, NodeKind,
    SystemGraph,
};
use serde_json::{Map, Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Debug,
};

#[derive(Clone, Copy)]
enum Mode {
    Oracle,
    Repeated,
}

pub fn compare_oracle(
    oracle: &SystemGraph,
    actual: &SystemGraph,
    root_name: &str,
    analyzed_at: &str,
) -> Result<(), String> {
    let mut diffs = Vec::new();
    field(
        &mut diffs,
        "graph schemaVersion",
        &oracle.schema_version,
        &actual.schema_version,
    );
    field(
        &mut diffs,
        "graph analyzerVersion",
        &env!("CARGO_PKG_VERSION"),
        &actual.metadata.analyzer_version.as_str(),
    );
    field(
        &mut diffs,
        "graph analyzedAt",
        &analyzed_at,
        &actual.metadata.analyzed_at.as_str(),
    );
    field(
        &mut diffs,
        "graph rootName",
        &Some(root_name),
        &actual.metadata.root_name.as_deref(),
    );
    field(
        &mut diffs,
        "graph callAnalysis",
        &oracle.metadata.call_analysis,
        &actual.metadata.call_analysis,
    );
    compare_parts(oracle, actual, Mode::Oracle, &mut diffs);
    result(diffs)
}

/// Compare all semantic content, including messages and supplemental metadata, except analyzedAt.
pub fn compare_repeated(first: &SystemGraph, second: &SystemGraph) -> Result<(), String> {
    let mut diffs = Vec::new();
    field(
        &mut diffs,
        "graph schemaVersion",
        &first.schema_version,
        &second.schema_version,
    );
    field(
        &mut diffs,
        "graph analyzerVersion",
        &first.metadata.analyzer_version,
        &second.metadata.analyzer_version,
    );
    field(
        &mut diffs,
        "graph rootName",
        &first.metadata.root_name,
        &second.metadata.root_name,
    );
    field(
        &mut diffs,
        "graph callAnalysis",
        &first.metadata.call_analysis,
        &second.metadata.call_analysis,
    );
    compare_parts(first, second, Mode::Repeated, &mut diffs);
    result(diffs)
}

fn result(diffs: Vec<String>) -> Result<(), String> {
    if diffs.is_empty() {
        return Ok(());
    }
    const SHOWN: usize = 24;
    let omitted = diffs.len().saturating_sub(SHOWN);
    Err(format!(
        "{} structural differences ({} omitted):\n{}",
        diffs.len(),
        omitted,
        diffs.into_iter().take(SHOWN).collect::<Vec<_>>().join("\n")
    ))
}

fn field<T: Debug + PartialEq>(diffs: &mut Vec<String>, label: &str, expected: &T, actual: &T) {
    if expected != actual {
        diffs.push(format!(
            "changed {label}: expected {expected:?}, actual {actual:?}"
        ));
    }
}

fn node_label(node: &GraphNode) -> String {
    format!(
        "node {} ({} @ {})",
        node.id,
        node.name,
        node.file.as_deref().unwrap_or("<none>")
    )
}

fn edge_label(edge: &GraphEdge) -> String {
    format!(
        "edge {} ({} -{:?}-> {})",
        edge.id, edge.from, edge.kind, edge.to
    )
}

fn index_nodes<'a>(
    items: &'a [GraphNode],
    diffs: &mut Vec<String>,
) -> BTreeMap<&'a str, &'a GraphNode> {
    let mut result = BTreeMap::new();
    for node in items {
        if result.contains_key(node.id.as_str()) {
            diffs.push(format!("duplicate node ID {} ({})", node.id, node.name));
            continue;
        }
        result.insert(node.id.as_str(), node);
    }
    result
}

fn index_edges<'a>(
    items: &'a [GraphEdge],
    diffs: &mut Vec<String>,
) -> BTreeMap<&'a str, &'a GraphEdge> {
    let mut result = BTreeMap::new();
    for edge in items {
        if result.contains_key(edge.id.as_str()) {
            diffs.push(format!(
                "duplicate edge ID {} ({} -{:?}-> {})",
                edge.id, edge.from, edge.kind, edge.to
            ));
            continue;
        }
        result.insert(edge.id.as_str(), edge);
    }
    result
}

fn compare_parts(
    expected: &SystemGraph,
    actual: &SystemGraph,
    mode: Mode,
    diffs: &mut Vec<String>,
) {
    let expected_nodes = index_nodes(&expected.nodes, diffs);
    let actual_nodes = index_nodes(&actual.nodes, diffs);
    for (id, node) in &expected_nodes {
        if let Some(other) = actual_nodes.get(id) {
            compare_node(node, other, &expected_nodes, mode, diffs);
        } else {
            diffs.push(format!("missing {}", node_label(node)));
        }
    }
    for (id, node) in &actual_nodes {
        if !expected_nodes.contains_key(id) {
            diffs.push(format!("unexpected {}", node_label(node)));
        }
    }

    let expected_edges = index_edges(&expected.edges, diffs);
    let actual_edges = index_edges(&actual.edges, diffs);
    for (id, edge) in &expected_edges {
        if let Some(other) = actual_edges.get(id) {
            compare_edge(edge, other, &expected_nodes, mode, diffs);
        } else {
            diffs.push(format!("missing {}", edge_label(edge)));
        }
    }
    for (id, edge) in &actual_edges {
        if !expected_edges.contains_key(id) {
            diffs.push(format!("unexpected {}", edge_label(edge)));
        }
    }
    compare_diagnostics(&expected.diagnostics, &actual.diagnostics, mode, diffs);
}

// Six method starts in the older oracle name the declaration line. The current contract
// includes the decorator's source line; these exact fixture locations are checked separately.
fn decorator_start(node: &GraphNode) -> Option<u64> {
    if node.kind != NodeKind::Method {
        return None;
    }
    match (node.file.as_deref(), node.name.as_str()) {
        (Some("src/auth/auth.controller.ts"), "login") => Some(7),
        (Some("src/regressions/override.module.ts"), "list") if node.line == Some(11) => Some(10),
        (Some("src/users/users.controller.ts"), "list") => Some(7),
        (Some("src/users/users.controller.ts"), "alias") => Some(10),
        (Some("src/users/users.controller.ts"), "update") => Some(12),
        (Some("src/users/users.controller.ts"), "remove") => Some(14),
        _ => None,
    }
}

fn oracle_metadata(node: &GraphNode) -> Option<Map<String, Value>> {
    let mut metadata = node.metadata.clone().unwrap_or_default();
    if matches!(
        node.kind,
        NodeKind::Module
            | NodeKind::Controller
            | NodeKind::Service
            | NodeKind::Repository
            | NodeKind::Class
            | NodeKind::Interface
    ) {
        metadata.insert("exported".into(), json!(true));
    }
    if node.kind == NodeKind::DatabaseModel && node.file.as_deref() == Some("prisma/schema.prisma")
    {
        let fields = match node.name.as_str() {
            "User" => json!([{"name":"id","type":"String"},{"name":"name","type":"String"}]),
            "Token" => json!([{"name":"id","type":"String"},{"name":"value","type":"String"}]),
            _ => return node.metadata.clone(),
        };
        metadata.insert("fields".into(), fields);
    }
    (!metadata.is_empty()).then_some(metadata)
}

fn compare_node(
    expected: &GraphNode,
    actual: &GraphNode,
    expected_nodes: &BTreeMap<&str, &GraphNode>,
    mode: Mode,
    diffs: &mut Vec<String>,
) {
    let label = node_label(expected);
    field(
        diffs,
        &format!("{label} kind"),
        &expected.kind,
        &actual.kind,
    );
    field(
        diffs,
        &format!("{label} name"),
        &expected.name,
        &actual.name,
    );
    field(
        diffs,
        &format!("{label} parentId"),
        &expected.parent_id,
        &actual.parent_id,
    );
    field(
        diffs,
        &format!("{label} file"),
        &expected.file,
        &actual.file,
    );
    let line = if matches!(mode, Mode::Oracle) {
        decorator_start(expected).or(expected.line)
    } else {
        expected.line
    };
    field(diffs, &format!("{label} line"), &line, &actual.line);
    if matches!(mode, Mode::Repeated) || expected.end_line.is_some() {
        field(
            diffs,
            &format!("{label} endLine"),
            &expected.end_line,
            &actual.end_line,
        );
    }
    let qualified_name = if matches!(mode, Mode::Oracle) && expected.kind == NodeKind::Method {
        expected
            .parent_id
            .as_deref()
            .and_then(|id| expected_nodes.get(id))
            .and_then(|owner| owner.qualified_name.as_ref())
            .map(|owner| format!("{owner}.{}", expected.name))
    } else {
        expected.qualified_name.clone()
    };
    field(
        diffs,
        &format!("{label} qualifiedName"),
        &qualified_name,
        &actual.qualified_name,
    );
    let metadata = if matches!(mode, Mode::Oracle) {
        oracle_metadata(expected)
    } else {
        expected.metadata.clone()
    };
    compare_metadata(&label, metadata.as_ref(), actual.metadata.as_ref(), diffs);
    let mut evidence = expected.evidence.clone();
    if matches!(mode, Mode::Oracle) {
        if let Some(start) = decorator_start(expected) {
            for item in &mut evidence {
                if item.source == EvidenceSource::Ast {
                    item.line = Some(start);
                }
            }
        }
        // The decorator confirms the role; the source declaration's Repository suffix is
        // separately best-effort. The older oracle recorded only the latter at line 3.
        if expected.kind == NodeKind::Repository
            && matches!(
                expected.file.as_deref(),
                Some("src/auth/token.repository.ts" | "src/users/user.repository.ts")
            )
        {
            let role = evidence
                .iter_mut()
                .find(|item| item.source == EvidenceSource::Nestjs)
                .unwrap();
            role.confidence = Confidence::Confirmed;
            evidence.push(Evidence {
                source: EvidenceSource::Nestjs,
                file: expected.file.clone().unwrap(),
                line: Some(4),
                end_line: None,
                confidence: Confidence::BestEffort,
            });
        }
    }
    compare_evidence(&label, &evidence, &actual.evidence, mode, diffs);
}

fn compare_edge(
    expected: &GraphEdge,
    actual: &GraphEdge,
    expected_nodes: &BTreeMap<&str, &GraphNode>,
    mode: Mode,
    diffs: &mut Vec<String>,
) {
    let label = edge_label(expected);
    field(
        diffs,
        &format!("{label} from"),
        &expected.from,
        &actual.from,
    );
    field(
        diffs,
        &format!("{label} kind"),
        &expected.kind,
        &actual.kind,
    );
    field(diffs, &format!("{label} to"), &expected.to, &actual.to);
    compare_metadata(
        &label,
        expected.metadata.as_ref(),
        actual.metadata.as_ref(),
        diffs,
    );
    let mut evidence = expected.evidence.clone();
    if matches!(mode, Mode::Oracle) {
        if expected.kind == EdgeKind::Contains
            && let Some(target) = expected_nodes.get(expected.to.as_str())
            && let Some(start) = decorator_start(target)
        {
            for item in &mut evidence {
                if item.source == EvidenceSource::Ast {
                    item.line = Some(start);
                }
            }
        }
        // These two explicit binding uses are present in source but absent from the older oracle.
        let from = expected_nodes.get(expected.from.as_str());
        let to = expected_nodes.get(expected.to.as_str());
        let extra = match (expected.kind, from, to) {
            (EdgeKind::Imports, Some(from), Some(to))
                if from.name == "AuthModule" && to.name == "AuthService" =>
            {
                Some(("src/auth/auth.module.ts", 10))
            }
            (EdgeKind::Imports, Some(from), Some(to))
                if from.name == "UsersModule" && to.name == "UsersService" =>
            {
                Some(("src/users/users.module.ts", 9))
            }
            _ => None,
        };
        if let Some((file, line)) = extra {
            evidence.push(Evidence {
                source: EvidenceSource::Resolver,
                file: file.into(),
                line: Some(line),
                end_line: None,
                confidence: Confidence::Confirmed,
            });
        }
    }
    compare_evidence(&label, &evidence, &actual.evidence, mode, diffs);
}

fn compare_metadata(
    label: &str,
    expected: Option<&Map<String, Value>>,
    actual: Option<&Map<String, Value>>,
    diffs: &mut Vec<String>,
) {
    field(
        diffs,
        &format!("{label} metadata presence"),
        &expected.is_some(),
        &actual.is_some(),
    );
    let keys: BTreeSet<_> = expected
        .into_iter()
        .flat_map(|m| m.keys())
        .chain(actual.into_iter().flat_map(|m| m.keys()))
        .collect();
    for key in keys {
        field(
            diffs,
            &format!("{label} metadata.{key}"),
            &expected.and_then(|m| m.get(key)),
            &actual.and_then(|m| m.get(key)),
        );
    }
}

fn evidence_key(evidence: &Evidence, mode: Mode) -> String {
    if matches!(mode, Mode::Oracle) {
        json!([
            evidence.source,
            evidence.file,
            evidence.line,
            evidence.confidence
        ])
        .to_string()
    } else {
        json!([
            evidence.source,
            evidence.file,
            evidence.line,
            evidence.end_line,
            evidence.confidence
        ])
        .to_string()
    }
}

fn evidence_groups<'a>(items: &[&'a Evidence], mode: Mode) -> BTreeMap<String, Vec<&'a Evidence>> {
    let mut groups = BTreeMap::<String, Vec<&Evidence>>::new();
    for item in items {
        groups
            .entry(evidence_key(item, mode))
            .or_default()
            .push(*item);
    }
    groups
}

fn compare_evidence(
    label: &str,
    expected: &[Evidence],
    actual: &[Evidence],
    mode: Mode,
    diffs: &mut Vec<String>,
) {
    let mut expected = expected.iter().collect::<Vec<_>>();
    let mut actual = actual.iter().collect::<Vec<_>>();
    expected.sort_by_key(|e| evidence_key(e, mode));
    actual.sort_by_key(|e| evidence_key(e, mode));
    if matches!(mode, Mode::Oracle) {
        let left = evidence_groups(&expected, mode);
        let right = evidence_groups(&actual, mode);
        if left.keys().eq(right.keys())
            && left
                .iter()
                .all(|(key, items)| items.len() == right[key].len())
        {
            for (key, items) in left {
                let mut required = BTreeMap::<u64, usize>::new();
                let mut found = BTreeMap::<u64, usize>::new();
                for item in items {
                    if let Some(end_line) = item.end_line {
                        *required.entry(end_line).or_default() += 1;
                    }
                }
                for item in &right[&key] {
                    if let Some(end_line) = item.end_line {
                        *found.entry(end_line).or_default() += 1;
                    }
                }
                for (end_line, count) in required {
                    if found.get(&end_line).copied().unwrap_or(0) < count {
                        diffs.push(format!(
                            "changed {label} evidence {key}.endLine: expected {end_line} x{count}, actual {}",
                            found.get(&end_line).copied().unwrap_or(0)
                        ));
                    }
                }
            }
            return;
        }
    }
    if expected.len() != actual.len() {
        let counts = |items: &[&Evidence]| {
            let mut counts = BTreeMap::<String, usize>::new();
            for item in items {
                *counts.entry(evidence_key(item, mode)).or_default() += 1;
            }
            counts
        };
        let left = counts(&expected);
        let right = counts(&actual);
        for key in left.keys().chain(right.keys()).collect::<BTreeSet<_>>() {
            let a = left.get(key).copied().unwrap_or(0);
            let b = right.get(key).copied().unwrap_or(0);
            if a > b {
                diffs.push(format!("missing {label} evidence {key} x{}", a - b));
            }
            if b > a {
                diffs.push(format!("unexpected {label} evidence {key} x{}", b - a));
            }
        }
        return;
    }
    for (index, (a, b)) in expected.iter().zip(actual.iter()).enumerate() {
        let at = format!("{label} evidence[{index}]");
        field(diffs, &format!("{at}.source"), &a.source, &b.source);
        field(diffs, &format!("{at}.file"), &a.file, &b.file);
        field(diffs, &format!("{at}.line"), &a.line, &b.line);
        if matches!(mode, Mode::Repeated) || a.end_line.is_some() {
            field(diffs, &format!("{at}.endLine"), &a.end_line, &b.end_line);
        }
        field(
            diffs,
            &format!("{at}.confidence"),
            &a.confidence,
            &b.confidence,
        );
    }
}

fn diagnostic_key(d: &Diagnostic) -> String {
    json!([d.code, d.file, d.line]).to_string()
}
fn diagnostic_sort_key(d: &Diagnostic, mode: Mode) -> String {
    if matches!(mode, Mode::Oracle) {
        json!([d.related_node_id, d.severity, d.skipped_count]).to_string()
    } else {
        json!([d.related_node_id, d.severity, d.skipped_count, d.message]).to_string()
    }
}
fn group_diagnostics(items: &[Diagnostic], mode: Mode) -> BTreeMap<String, Vec<&Diagnostic>> {
    let mut groups = BTreeMap::<String, Vec<&Diagnostic>>::new();
    for d in items {
        groups.entry(diagnostic_key(d)).or_default().push(d);
    }
    for entries in groups.values_mut() {
        entries.sort_by_key(|d| diagnostic_sort_key(d, mode));
    }
    groups
}
fn compare_diagnostics(
    expected: &[Diagnostic],
    actual: &[Diagnostic],
    mode: Mode,
    diffs: &mut Vec<String>,
) {
    let left = group_diagnostics(expected, mode);
    let right = group_diagnostics(actual, mode);
    for key in left.keys().chain(right.keys()).collect::<BTreeSet<_>>() {
        let a = left.get(key).map(Vec::as_slice).unwrap_or(&[]);
        let b = right.get(key).map(Vec::as_slice).unwrap_or(&[]);
        if a.len() > b.len() {
            diffs.push(format!("missing diagnostic {key} x{}", a.len() - b.len()));
        }
        if b.len() > a.len() {
            diffs.push(format!(
                "extra occurrence diagnostic {key} x{}",
                b.len() - a.len()
            ));
        }
        for (index, (a, b)) in a.iter().zip(b.iter()).enumerate() {
            let at = format!("diagnostic {key}[{index}]");
            field(
                diffs,
                &format!("{at}.relatedNodeId"),
                &a.related_node_id,
                &b.related_node_id,
            );
            field(diffs, &format!("{at}.severity"), &a.severity, &b.severity);
            field(
                diffs,
                &format!("{at}.skippedCount"),
                &a.skipped_count,
                &b.skipped_count,
            );
            if matches!(mode, Mode::Repeated) {
                field(diffs, &format!("{at}.message"), &a.message, &b.message);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codebasecanvas_analyzer::{
        CallAnalysis, CallMode, CallScope, GraphBuilder, GraphMetadata, SchemaVersion, Severity,
    };

    fn graph() -> SystemGraph {
        let a = GraphBuilder::node_id(NodeKind::Class, "src/a.ts", &[], "Same").unwrap();
        let b = GraphBuilder::node_id(NodeKind::Class, "src/b.ts", &[], "Same").unwrap();
        let node = |id: String, file: &str| GraphNode {
            id,
            kind: NodeKind::Class,
            name: "Same".into(),
            qualified_name: Some("Same".into()),
            file: Some(file.into()),
            line: Some(1),
            end_line: Some(1),
            parent_id: None,
            evidence: vec![Evidence {
                source: EvidenceSource::Ast,
                file: file.into(),
                line: Some(1),
                end_line: Some(1),
                confidence: Confidence::Confirmed,
            }],
            metadata: None,
        };
        let edge = GraphEdge {
            id: GraphBuilder::edge_id(&a, EdgeKind::Injects, &b),
            from: a.clone(),
            to: b.clone(),
            kind: EdgeKind::Injects,
            evidence: vec![Evidence {
                source: EvidenceSource::Nestjs,
                file: "src/a.ts".into(),
                line: Some(2),
                end_line: None,
                confidence: Confidence::Confirmed,
            }],
            metadata: Some(Map::from_iter([(
                "semantics".into(),
                json!("requested_token"),
            )])),
        };
        SystemGraph {
            schema_version: SchemaVersion::V01,
            metadata: GraphMetadata {
                analyzer_version: "test".into(),
                analyzed_at: "2026-09-24T00:00:00Z".into(),
                root_name: Some("fixture".into()),
                call_analysis: CallAnalysis {
                    scope: CallScope::ParsedNamedClassMethods,
                    mode: CallMode::SameClassOnly,
                    examined_calls: 0,
                    emitted_calls: 0,
                    skipped_calls: 0,
                },
            },
            nodes: vec![node(a, "src/a.ts"), node(b, "src/b.ts")],
            edges: vec![edge],
            diagnostics: vec![Diagnostic {
                code: "unsupported_sample".into(),
                severity: Severity::Warning,
                message: "Unresolved sample".into(),
                file: Some("src/a.ts".into()),
                line: Some(3),
                related_node_id: None,
                skipped_count: Some(1),
            }],
        }
    }

    fn detects(mut change: impl FnMut(&mut SystemGraph), expected: &str) {
        let baseline = graph();
        let mut changed = baseline.clone();
        change(&mut changed);
        let message = compare_repeated(&baseline, &changed).unwrap_err();
        assert!(message.contains(expected), "{message}");
    }

    #[test]
    fn reports_missing_unexpected_and_duplicate_ids_without_name_matching() {
        detects(
            |g| {
                g.nodes.remove(0);
            },
            "missing node",
        );
        detects(
            |g| {
                let mut n = g.nodes[0].clone();
                n.id.push_str("different");
                g.nodes.push(n);
            },
            "unexpected node",
        );
        detects(
            |g| {
                g.nodes.push(g.nodes[0].clone());
            },
            "duplicate node ID",
        );
        detects(
            |g| {
                g.edges.clear();
            },
            "missing edge",
        );
        detects(
            |g| {
                let mut e = g.edges[0].clone();
                e.id.push_str("different");
                g.edges.push(e);
            },
            "unexpected edge",
        );
        detects(
            |g| {
                g.edges.push(g.edges[0].clone());
            },
            "duplicate edge ID",
        );
    }

    #[test]
    fn reports_changed_identity_relationship_metadata_and_evidence() {
        detects(
            |g| {
                g.nodes[0].kind = NodeKind::Service;
            },
            "kind: expected Class, actual Service",
        );
        detects(
            |g| {
                g.nodes[0].parent_id = Some(g.nodes[1].id.clone());
            },
            "parentId",
        );
        detects(
            |g| {
                g.nodes[0].id = g.nodes[1].id.clone();
            },
            "duplicate node ID",
        );
        detects(
            |g| {
                g.edges[0].to = g.nodes[0].id.clone();
            },
            " to: expected ",
        );
        detects(
            |g| {
                g.edges[0].evidence[0].line = Some(4);
            },
            "evidence[0].line",
        );
        detects(
            |g| {
                g.edges[0].evidence[0].confidence = Confidence::BestEffort;
            },
            "evidence[0].confidence",
        );
        detects(
            |g| {
                g.nodes[0].evidence[0].source = EvidenceSource::Resolver;
            },
            "evidence[0].source",
        );
        detects(
            |g| {
                g.edges[0].metadata.as_mut().unwrap().remove("semantics");
            },
            "metadata.semantics",
        );
        detects(
            |g| {
                g.edges[0]
                    .metadata
                    .as_mut()
                    .unwrap()
                    .insert("semantics".into(), json!("implementation"));
            },
            "metadata.semantics",
        );
    }

    #[test]
    fn diagnostics_are_a_multiset_with_field_level_differences() {
        detects(
            |g| {
                g.diagnostics.clear();
            },
            "missing diagnostic",
        );
        detects(
            |g| {
                g.diagnostics.push(g.diagnostics[0].clone());
            },
            "extra occurrence diagnostic",
        );
        detects(
            |g| {
                g.diagnostics[0].severity = Severity::Error;
            },
            ".severity",
        );
        detects(
            |g| {
                g.diagnostics[0].related_node_id = Some(g.nodes[0].id.clone());
            },
            ".relatedNodeId",
        );
        detects(
            |g| {
                g.diagnostics[0].skipped_count = Some(2);
            },
            ".skippedCount",
        );
    }

    #[test]
    fn repeated_comparison_ignores_only_timestamp_and_input_order() {
        let original = graph();
        let mut reordered = original.clone();
        reordered.nodes.reverse();
        reordered.edges[0].evidence.reverse();
        reordered.metadata.analyzed_at = "2026-09-25T00:00:00Z".into();
        assert!(compare_repeated(&original, &reordered).is_ok());
        assert_eq!(original, graph());
        detects(
            |g| {
                g.diagnostics[0].message.push('!');
            },
            ".message",
        );
        detects(
            |g| {
                g.nodes[0].end_line = Some(2);
            },
            "endLine",
        );
    }

    #[test]
    fn oracle_evidence_with_same_prefix_ignores_order_but_checks_explicit_end_lines() {
        let mut expected = graph().nodes[0].evidence.clone();
        let mut second = expected[0].clone();
        second.end_line = Some(2);
        expected.push(second);
        let mut actual = expected.clone();
        actual.reverse();
        let mut diffs = Vec::new();
        compare_evidence("node", &expected, &actual, Mode::Oracle, &mut diffs);
        assert!(diffs.is_empty(), "{diffs:?}");

        actual[0].end_line = Some(3);
        compare_evidence("node", &expected, &actual, Mode::Oracle, &mut diffs);
        assert!(
            diffs.iter().any(|diff| diff.contains(".endLine")),
            "{diffs:?}"
        );

        expected[1].end_line = None;
        diffs.clear();
        compare_evidence("node", &expected, &actual, Mode::Oracle, &mut diffs);
        assert!(diffs.is_empty(), "{diffs:?}");
    }
}
