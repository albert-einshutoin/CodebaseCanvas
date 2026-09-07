use crate::{Diagnostic, Evidence, GraphEdge, GraphMetadata, GraphNode, SystemGraph};
use std::collections::BTreeMap;

/// Accumulates recognizer findings into one validated graph.
#[derive(Debug)]
pub struct GraphBuilder {
    metadata: GraphMetadata,
    nodes: BTreeMap<String, GraphNode>,
    edges: BTreeMap<String, GraphEdge>,
    diagnostics: Vec<Diagnostic>,
    first_error: Option<String>,
}

impl GraphBuilder {
    pub fn new(metadata: GraphMetadata) -> Self {
        Self {
            metadata,
            nodes: BTreeMap::new(),
            edges: BTreeMap::new(),
            diagnostics: Vec::new(),
            first_error: None,
        }
    }

    pub fn node_id(kind: crate::NodeKind, file: &str, scope: &[&str], name: &str) -> String {
        let tag = match kind {
            crate::NodeKind::Interface => "interface",
            crate::NodeKind::DatabaseModel => "database_model",
            _ => "class",
        };
        let mut parts = vec![file];
        parts.extend_from_slice(scope);
        parts.push(name);
        crate::canonical_id(tag, &parts)
    }

    pub fn method_id(owner: &str, staticness: &str, name: &str) -> String {
        crate::canonical_id("method", &[owner, staticness, name])
    }

    pub fn endpoint_id(http_method: &str, path: &str, handler: &str) -> String {
        crate::canonical_id("endpoint", &[http_method, path, handler])
    }

    pub fn external_id(module: &str, exported_name: &str) -> String {
        crate::canonical_id("external", &[module, exported_name])
    }

    pub fn edge_id(from: &str, kind: crate::EdgeKind, to: &str) -> String {
        crate::canonical_id("edge", &[from, kind.wire(), to])
    }

    pub fn add_node(&mut self, mut node: GraphNode) -> Result<(), String> {
        if let Some(error) = &self.first_error {
            return Err(error.clone());
        }
        if node.id.is_empty() || node.name.is_empty() || !super::evidence(&node.evidence) {
            return self.fail("Invalid node finding");
        }
        if !super::valid_metadata(&node.metadata) {
            return self.fail("Invalid node metadata");
        }
        match self.nodes.get_mut(&node.id) {
            Some(existing) => {
                if same_node(existing, &node) {
                    merge_evidence(&mut existing.evidence, node.evidence);
                    Ok(())
                } else {
                    self.fail(&format!("Contradictory node finding: {}", node.id))
                }
            }
            None => {
                node.evidence.dedup();
                self.nodes.insert(node.id.clone(), node);
                Ok(())
            }
        }
    }

    pub fn add_edge(&mut self, mut edge: GraphEdge) -> Result<(), String> {
        if let Some(error) = &self.first_error {
            return Err(error.clone());
        }
        let expected = Self::edge_id(&edge.from, edge.kind, &edge.to);
        if edge.id != expected || edge.from.is_empty() || edge.to.is_empty() {
            return self.fail("Invalid edge identity");
        }
        if !super::evidence(&edge.evidence) {
            return self.fail("Invalid edge evidence");
        }
        if !super::valid_metadata(&edge.metadata) {
            return self.fail("Invalid edge metadata");
        }
        match self.edges.get_mut(&edge.id) {
            Some(existing) => {
                if existing.from != edge.from
                    || existing.to != edge.to
                    || existing.kind != edge.kind
                    || existing.metadata != edge.metadata
                {
                    return self.fail(&format!("Contradictory edge finding: {}", edge.id));
                }
                merge_evidence(&mut existing.evidence, edge.evidence);
                Ok(())
            }
            None => {
                edge.evidence.dedup();
                self.edges.insert(edge.id.clone(), edge);
                Ok(())
            }
        }
    }

    pub fn add_diagnostic(&mut self, diagnostic: Diagnostic) -> Result<(), String> {
        if let Some(error) = &self.first_error {
            return Err(error.clone());
        }
        self.diagnostics.push(diagnostic);
        Ok(())
    }

    pub fn finish(self) -> Result<SystemGraph, String> {
        if let Some(error) = self.first_error {
            return Err(error);
        }
        let GraphBuilder {
            metadata,
            nodes,
            edges,
            diagnostics,
            ..
        } = self;
        let graph = SystemGraph {
            schema_version: crate::SchemaVersion::V01,
            metadata,
            nodes: nodes.into_values().collect(),
            edges: edges.into_values().collect(),
            diagnostics,
        };
        graph.validate()?;
        let mut graph = graph;
        graph.canonicalize();
        Ok(graph)
    }

    fn fail<T>(&mut self, error: &str) -> Result<T, String> {
        let error = error.to_owned();
        self.first_error = Some(error.clone());
        Err(error)
    }
}

fn same_node(a: &GraphNode, b: &GraphNode) -> bool {
    a.id == b.id
        && a.kind == b.kind
        && a.name == b.name
        && a.qualified_name == b.qualified_name
        && a.file == b.file
        && a.line == b.line
        && a.end_line == b.end_line
        && a.parent_id == b.parent_id
        && a.metadata == b.metadata
}

fn merge_evidence(target: &mut Vec<Evidence>, incoming: Vec<Evidence>) {
    for item in incoming {
        if !target.contains(&item) {
            target.push(item);
        }
    }
}
