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
    call_analysis_applied: bool,
    incomplete_call_files: std::collections::BTreeSet<String>,
}

impl GraphBuilder {
    pub fn new(metadata: GraphMetadata) -> Self {
        Self {
            metadata,
            nodes: BTreeMap::new(),
            edges: BTreeMap::new(),
            diagnostics: Vec::new(),
            first_error: None,
            call_analysis_applied: false,
            incomplete_call_files: Default::default(),
        }
    }

    pub fn node_id(
        kind: crate::NodeKind,
        file: &str,
        scope: &[&str],
        name: &str,
    ) -> Result<String, String> {
        let tag = match kind {
            crate::NodeKind::Module
            | crate::NodeKind::Controller
            | crate::NodeKind::Service
            | crate::NodeKind::Repository
            | crate::NodeKind::Class => "class",
            crate::NodeKind::Interface => "interface",
            crate::NodeKind::DatabaseModel => "database_model",
            _ => return Err("node_id requires a declaration kind".into()),
        };
        let mut parts = vec![file];
        parts.extend_from_slice(scope);
        parts.push(name);
        Ok(crate::canonical_id(tag, &parts))
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

    pub(crate) fn node(&self, id: &str) -> Option<&GraphNode> {
        self.nodes.get(id)
    }

    /// One batch, one edge finding per emitted site (before tuple deduplication).
    /// Other metadata is never replaced. Rejection poisons finish like insertion errors.
    pub fn apply_call_analysis(
        &mut self,
        summary: crate::CallAnalysis,
        edges: Vec<GraphEdge>,
        diagnostics: Vec<Diagnostic>,
        incomplete_files: std::collections::BTreeSet<String>,
    ) -> Result<(), String> {
        use crate::{EdgeKind, NodeKind};
        if let Some(error) = &self.first_error {
            return Err(error.clone());
        }
        let prior = &self.metadata.call_analysis;
        if self.call_analysis_applied
            || prior.examined_calls != 0
            || prior.emitted_calls != 0
            || prior.skipped_calls != 0
            || self.edges.values().any(|e| e.kind == EdgeKind::Calls)
            || self.diagnostics.iter().any(|d| d.skipped_count.is_some())
        {
            return self.fail("Call analysis requires an unapplied empty call batch");
        }
        let mut keys = std::collections::BTreeSet::new();
        let skipped = diagnostics.iter().try_fold(0u64, |sum, d| {
            let node = self.nodes.get(d.related_node_id.as_ref()?)?;
            if node.kind != NodeKind::Method
                || node.file != d.file
                || d.line.is_none()
                || !d.code.starts_with("unsupported_call_")
                || d.skipped_count == Some(0)
                || !keys.insert((d.related_node_id.clone(), d.code.clone()))
            {
                return None;
            }
            sum.checked_add(d.skipped_count?)
        });
        let valid_edges = edges.iter().all(|e| {
            let (Some(from), Some(to)) = (self.nodes.get(&e.from), self.nodes.get(&e.to)) else {
                return false;
            };
            let same_staticness = crate::id_parts(&e.from)
                .zip(crate::id_parts(&e.to))
                .is_some_and(|((_, a), (_, b))| a.get(1) == b.get(1));
            e.kind == EdgeKind::Calls
                && from.kind == NodeKind::Method
                && to.kind == NodeKind::Method
                && from.parent_id.is_some()
                && from.parent_id == to.parent_id
                && same_staticness
                && e.evidence.len() == 1
                && e.evidence.iter().all(|v| {
                    from.file.as_deref() == Some(&v.file)
                        && v.line.is_some()
                        && v.source == crate::EvidenceSource::Ast
                        && v.confidence == crate::Confidence::Confirmed
                })
        });
        if summary.examined_calls > crate::MAX_INTEGER
            || summary.emitted_calls.checked_add(summary.skipped_calls)
                != Some(summary.examined_calls)
            || summary.emitted_calls != edges.len() as u64
            || skipped != Some(summary.skipped_calls)
            || !valid_edges
            || incomplete_files
                .iter()
                .any(|f| !crate::is_repository_path(f))
        {
            return self.fail("Inconsistent call analysis batch");
        }
        for edge in edges {
            self.add_edge(edge)?;
        }
        for diagnostic in diagnostics {
            self.add_diagnostic(diagnostic)?;
        }
        self.metadata.call_analysis = summary;
        self.incomplete_call_files = incomplete_files;
        self.call_analysis_applied = true;
        Ok(())
    }

    /// Apply confirmed module composition only, then derive display parents from
    /// distinct membership edges. This does not change normal insertion or finish.
    pub fn apply_module_composition(&mut self, edges: Vec<GraphEdge>) -> Result<(), String> {
        use crate::{EdgeKind, NodeKind};
        if let Some(error) = &self.first_error {
            return Err(error.clone());
        }
        for edge in &edges {
            let valid = self
                .nodes
                .get(&edge.from)
                .is_some_and(|n| n.kind == NodeKind::Module)
                && self.nodes.get(&edge.to).is_some_and(|n| match edge.kind {
                    EdgeKind::Contains => matches!(
                        n.kind,
                        NodeKind::Controller
                            | NodeKind::Service
                            | NodeKind::Repository
                            | NodeKind::Class
                    ),
                    EdgeKind::DependsOn => n.kind == NodeKind::Module,
                    _ => false,
                });
            if !valid {
                return self.fail("Invalid module composition target or relationship");
            }
        }
        for edge in edges {
            self.add_edge(edge)?;
        }
        let mut memberships: BTreeMap<String, std::collections::BTreeSet<String>> = BTreeMap::new();
        for edge in self.edges.values() {
            if edge.kind == EdgeKind::Contains
                && self
                    .nodes
                    .get(&edge.from)
                    .is_some_and(|n| n.kind == NodeKind::Module)
                && self.nodes.get(&edge.to).is_some_and(|n| {
                    matches!(
                        n.kind,
                        NodeKind::Controller
                            | NodeKind::Service
                            | NodeKind::Repository
                            | NodeKind::Class
                    )
                })
            {
                memberships
                    .entry(edge.to.clone())
                    .or_default()
                    .insert(edge.from.clone());
            }
        }
        for node in self.nodes.values_mut() {
            if matches!(
                node.kind,
                NodeKind::Controller | NodeKind::Service | NodeKind::Repository | NodeKind::Class
            ) {
                node.parent_id = memberships
                    .get(&node.id)
                    .filter(|owners| owners.len() == 1)
                    .and_then(|owners| owners.first().cloned());
            }
        }
        Ok(())
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
        if self.call_analysis_applied && edge.kind == crate::EdgeKind::Calls {
            return self.fail("Calls must be applied in one batch");
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
        if self.call_analysis_applied && diagnostic.skipped_count.is_some() {
            return self.fail("Call diagnostics must be applied in one batch");
        }
        self.diagnostics.push(diagnostic);
        Ok(())
    }

    pub fn finish(self) -> Result<SystemGraph, String> {
        if let Some(error) = self.first_error {
            return Err(error);
        }
        if self.incomplete_call_files.iter().any(|file| {
            !self.diagnostics.iter().any(|d| {
                d.file.as_ref() == Some(file)
                    && d.severity == crate::Severity::Error
                    && matches!(
                        d.code.as_str(),
                        "TS_PARSE_ERROR" | "TS_IMPORT_PARSEINCOMPLETE"
                    )
            })
        }) {
            return Err("Incomplete call scope requires its existing file diagnostic".into());
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
