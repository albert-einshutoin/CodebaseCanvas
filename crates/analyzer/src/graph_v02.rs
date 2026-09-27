//! Experimental repository-request wire contract. Production I/O remains SystemGraph v0.1.
use crate::{
    Confidence, Diagnostic, EvidenceSource, GraphEdge, GraphMetadata, GraphNode, GraphParts,
    NodeKind, canonical_id, canonicalize_graph_parts, integer, is_repository_path, present,
    present_integer,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

const MAX_INTEGER: u64 = 9_007_199_254_740_991;
const PROFILE: &str = "@nestjs/typeorm@7.0.0+typeorm@0.2.24/ordinary_class";
const RULE: &str = "@nestjs/typeorm@7.0.0/getRepositoryToken:ordinary_class";
const SOURCE_SHA256: &str = "8395dc1f59d3471724312c4053cbd7af0dff38fd354d0720580b72452af633f9";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchemaVersionV02 {
    #[serde(rename = "0.2")]
    V02,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SystemGraphV02 {
    pub schema_version: SchemaVersionV02,
    pub metadata: GraphMetadata,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub diagnostics: Vec<Diagnostic>,
    pub framework_declarations: Vec<RepositoryRequest>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryRequest {
    pub id: String,
    pub kind: RepositoryRequestKind,
    pub owner_id: String,
    pub site: RequestSite,
    pub origin: RequestOrigin,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub entity_ref: Option<EntityRef>,
    pub connection: Connection,
    pub token: Token,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub derivation: Option<Derivation>,
    pub evidence: Vec<ClaimEvidence>,
    pub runtime: Runtime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepositoryRequestKind {
    #[serde(rename = "typeorm_repository_request")]
    TypeormRepositoryRequest,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequestSite {
    pub file: String,
    #[serde(deserialize_with = "integer")]
    pub start_byte: u64,
    #[serde(deserialize_with = "integer")]
    pub end_byte: u64,
    #[serde(deserialize_with = "integer")]
    pub line: u64,
    #[serde(deserialize_with = "integer")]
    pub parameter_index: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequestOrigin {
    pub specifier: String,
    pub exported_name: String,
    pub local_name: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub locked_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntityRef {
    pub id: String,
    pub kind: NodeKind,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "syntax", deny_unknown_fields)]
pub enum Connection {
    #[serde(rename = "omitted")]
    Omitted {
        #[serde(rename = "normalizedName")]
        normalized_name: String,
    },
    #[serde(rename = "literal")]
    Literal {
        value: String,
        #[serde(rename = "normalizedName")]
        normalized_name: String,
    },
    #[serde(rename = "unknown_expression")]
    UnknownExpression,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", deny_unknown_fields)]
pub enum Token {
    #[serde(rename = "derived_under_profile")]
    DerivedUnderProfile {
        kind: String,
        value: String,
        #[serde(rename = "comparisonKey")]
        comparison_key: String,
    },
    #[serde(rename = "unknown")]
    Unknown { reason: UnknownReason },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnknownReason {
    ConnectionExpressionUnknown,
    EntityUnresolved,
    ProfileUnknown,
    ClassShapeUnknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Derivation {
    pub profile: String,
    pub rule: String,
    pub source_sha256: String,
    pub dependency_profile: String,
    pub entity_shape: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Claim {
    DecoratorOrigin,
    ArgumentReference,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaimEvidence {
    pub claim: Claim,
    pub source: EvidenceSource,
    pub file: String,
    #[serde(deserialize_with = "integer")]
    pub line: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_integer"
    )]
    pub end_line: Option<u64>,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unverified {
    #[serde(rename = "unverified")]
    Unverified,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Runtime {
    pub provider_existence: Unverified,
    pub visibility: Unverified,
    pub injection: Unverified,
}

impl SystemGraphV02 {
    pub fn from_json(json: &str) -> Result<Self, String> {
        let graph: Self = serde_json::from_str(json).map_err(|error| error.to_string())?;
        graph.validate()?;
        Ok(graph)
    }

    pub fn to_json(&self) -> Result<String, String> {
        self.validate()?;
        let mut graph = self.clone();
        canonicalize_graph_parts(&mut graph.nodes, &mut graph.edges, &mut graph.diagnostics);
        graph.framework_declarations.sort_by(|a, b| a.id.cmp(&b.id));
        for declaration in &mut graph.framework_declarations {
            declaration.evidence.sort_by_cached_key(|item| {
                serde_json::to_string(item).expect("finite evidence fields")
            });
        }
        serde_json::to_string_pretty(&graph).map_err(|error| error.to_string())
    }

    pub fn validate(&self) -> Result<(), String> {
        GraphParts {
            metadata: &self.metadata,
            nodes: &self.nodes,
            edges: &self.edges,
            diagnostics: &self.diagnostics,
        }
        .validate()?;
        let nodes: HashMap<&str, &GraphNode> = self
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect();
        let mut ids = HashSet::new();
        for declaration in &self.framework_declarations {
            if !ids.insert(&declaration.id) {
                return Err("Duplicate framework declaration ID".into());
            }
            declaration.validate(&nodes)?;
        }
        Ok(())
    }
}

impl RepositoryRequest {
    fn validate(&self, nodes: &HashMap<&str, &GraphNode>) -> Result<(), String> {
        let fail = || Err(format!("Invalid repository request: {}", self.id));
        let site = &self.site;
        if !is_repository_path(&site.file)
            || site.start_byte > MAX_INTEGER
            || site.end_byte > MAX_INTEGER
            || site.end_byte <= site.start_byte
            || !(1..=MAX_INTEGER).contains(&site.line)
            || site.parameter_index > MAX_INTEGER
            || self.id
                != canonical_id(
                    "typeorm_decl",
                    &[
                        &site.file,
                        "typeorm_repository_request",
                        &site.start_byte.to_string(),
                        &format!("parameter:{}", site.parameter_index),
                    ],
                )
        {
            return fail();
        }
        let Some(owner) = nodes.get(self.owner_id.as_str()) else {
            return fail();
        };
        if !owner.kind.class_like() || owner.file.as_deref() != Some(&site.file) {
            return fail();
        }
        if self.origin.specifier != "@nestjs/typeorm"
            || self.origin.exported_name != "InjectRepository"
            || self.origin.local_name.is_empty()
            || self
                .origin
                .locked_version
                .as_ref()
                .is_some_and(String::is_empty)
        {
            return fail();
        }
        if let Some(entity) = &self.entity_ref {
            let Some(target) = nodes.get(entity.id.as_str()) else {
                return fail();
            };
            if !target.kind.class_like()
                || target.file.is_none()
                || entity.kind != target.kind
                || entity.name != target.name
            {
                return fail();
            }
        }
        let connection_name = match &self.connection {
            Connection::Omitted { normalized_name } if normalized_name == "default" => {
                Some("default")
            }
            Connection::Literal {
                value,
                normalized_name,
            } if !value.is_empty() && value == normalized_name => Some(normalized_name.as_str()),
            Connection::UnknownExpression => None,
            _ => return fail(),
        };
        let expected_reason = if connection_name.is_none() {
            Some(UnknownReason::ConnectionExpressionUnknown)
        } else if self.entity_ref.is_none() {
            Some(UnknownReason::EntityUnresolved)
        } else if self.origin.locked_version.as_deref() != Some("7.0.0") {
            Some(UnknownReason::ProfileUnknown)
        } else {
            None
        };
        match &self.token {
            Token::Unknown { reason } => {
                if self.derivation.is_some()
                    || expected_reason.is_some_and(|expected| *reason != expected)
                    || (expected_reason.is_none() && *reason != UnknownReason::ClassShapeUnknown)
                {
                    return fail();
                }
            }
            Token::DerivedUnderProfile {
                kind,
                value,
                comparison_key,
            } => {
                let (Some(entity), Some(connection_name), Some(derivation)) =
                    (&self.entity_ref, connection_name, &self.derivation)
                else {
                    return fail();
                };
                if expected_reason.is_some()
                    || entity.kind != NodeKind::Class
                    || kind != "string"
                    || derivation.profile != PROFILE
                    || derivation.rule != RULE
                    || derivation.source_sha256 != SOURCE_SHA256
                    || derivation.dependency_profile != "producer_verified"
                    || derivation.entity_shape != "resolved_local_non_inherited"
                {
                    return fail();
                }
                let expected_value = if connection_name == "default" {
                    format!("{}Repository", entity.name)
                } else {
                    format!("{connection_name}_{}Repository", entity.name)
                };
                if *value != expected_value
                    || *comparison_key != canonical_id("nest_string", &[value])
                {
                    return fail();
                }
            }
        }
        let mut claims = HashSet::new();
        for item in &self.evidence {
            if !claims.insert(item.claim)
                || item.source != EvidenceSource::Resolver
                || item.confidence != Confidence::Confirmed
                || item.file != site.file
                || item.line != site.line
                || item
                    .end_line
                    .is_some_and(|end| end < item.line || end > MAX_INTEGER)
            {
                return fail();
            }
        }
        if !claims.contains(&Claim::DecoratorOrigin)
            || claims.contains(&Claim::ArgumentReference) != self.entity_ref.is_some()
        {
            return fail();
        }
        Ok(())
    }
}
