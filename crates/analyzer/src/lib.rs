//! SystemGraph v0.1 wire types. Use `from_json` / `to_json` at I/O boundaries.
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};

// An optional wire field may be absent, but explicit null is not a value.
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

fn integer<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    struct Integer;
    impl serde::de::Visitor<'_> for Integer {
        type Value = u64;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a nonnegative safe integer")
        }
        fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<u64, E> {
            if v <= MAX_INTEGER {
                Ok(v)
            } else {
                Err(E::custom("unsafe integer"))
            }
        }
        fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<u64, E> {
            u64::try_from(v)
                .map_err(E::custom)
                .and_then(|v| self.visit_u64(v))
        }
        fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<u64, E> {
            if v.is_finite() && v >= 0.0 && v <= MAX_INTEGER as f64 && v.fract() == 0.0 {
                Ok(v as u64)
            } else {
                Err(E::custom("expected safe integer"))
            }
        }
    }
    d.deserialize_any(Integer)
}
fn present_integer<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
    integer(d).map(Some)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    #[serde(rename = "module")]
    Module,
    #[serde(rename = "controller")]
    Controller,
    #[serde(rename = "service")]
    Service,
    #[serde(rename = "repository")]
    Repository,
    #[serde(rename = "class")]
    Class,
    #[serde(rename = "interface")]
    Interface,
    #[serde(rename = "method")]
    Method,
    #[serde(rename = "endpoint")]
    Endpoint,
    #[serde(rename = "database_model")]
    DatabaseModel,
    #[serde(rename = "external_dependency")]
    ExternalDependency,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeKind {
    #[serde(rename = "contains")]
    Contains,
    #[serde(rename = "imports")]
    Imports,
    #[serde(rename = "injects")]
    Injects,
    #[serde(rename = "calls")]
    Calls,
    #[serde(rename = "exposes")]
    Exposes,
    #[serde(rename = "reads")]
    Reads,
    #[serde(rename = "writes")]
    Writes,
    #[serde(rename = "depends_on")]
    DependsOn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceSource {
    #[serde(rename = "ast")]
    Ast,
    #[serde(rename = "nestjs")]
    Nestjs,
    #[serde(rename = "prisma")]
    Prisma,
    #[serde(rename = "resolver")]
    Resolver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Confidence {
    #[serde(rename = "confirmed")]
    Confirmed,
    #[serde(rename = "best_effort")]
    BestEffort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    #[serde(rename = "info")]
    Info,
    #[serde(rename = "warning")]
    Warning,
    #[serde(rename = "error")]
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchemaVersion {
    #[serde(rename = "0.1")]
    V01,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallScope {
    #[serde(rename = "parsed_named_class_methods")]
    ParsedNamedClassMethods,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallMode {
    #[serde(rename = "same_class_only")]
    SameClassOnly,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SystemGraph {
    pub schema_version: SchemaVersion,
    pub metadata: GraphMetadata,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphMetadata {
    pub analyzer_version: String,
    pub analyzed_at: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub root_name: Option<String>,
    pub call_analysis: CallAnalysis,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CallAnalysis {
    pub scope: CallScope,
    pub mode: CallMode,
    #[serde(deserialize_with = "integer")]
    pub examined_calls: u64,
    #[serde(deserialize_with = "integer")]
    pub emitted_calls: u64,
    #[serde(deserialize_with = "integer")]
    pub skipped_calls: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphNode {
    pub id: String,
    pub kind: NodeKind,
    pub name: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub qualified_name: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub file: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_integer"
    )]
    pub line: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_integer"
    )]
    pub end_line: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub parent_id: Option<String>,
    pub evidence: Vec<Evidence>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub metadata: Option<Map<String, Value>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
    pub evidence: Vec<Evidence>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub metadata: Option<Map<String, Value>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Evidence {
    pub source: EvidenceSource,
    pub file: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_integer"
    )]
    pub line: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_integer"
    )]
    pub end_line: Option<u64>,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub file: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_integer"
    )]
    pub line: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub related_node_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_integer"
    )]
    pub skipped_count: Option<u64>,
}

const MAX_INTEGER: u64 = 9_007_199_254_740_991;

pub fn is_repository_path(path: &str) -> bool {
    !path.is_empty()
        && !path
            .chars()
            .any(|c| c == '\\' || c == ':' || c.is_control())
        && path
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != "..")
}

/// Lowercase UTF-8 hex components keep scoped names and nested IDs unambiguous.
pub fn canonical_id(tag: &str, parts: &[&str]) -> String {
    let mut id = tag.to_owned();
    const HEX: &[u8] = b"0123456789abcdef";
    for part in parts {
        id.push(':');
        for b in part.bytes() {
            id.push(HEX[(b >> 4) as usize] as char);
            id.push(HEX[(b & 15) as usize] as char);
        }
    }
    id
}
fn id_parts(id: &str) -> Option<(&str, Vec<String>)> {
    let mut split = id.split(':');
    let tag = split.next()?;
    let mut parts = Vec::new();
    for hex in split {
        if hex.is_empty()
            || hex.len() % 2 != 0
            || !hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return None;
        }
        let bytes: Option<Vec<u8>> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
            .collect();
        parts.push(String::from_utf8(bytes?).ok()?);
    }
    if tag.is_empty() || parts.is_empty() {
        None
    } else {
        Some((tag, parts))
    }
}
impl NodeKind {
    fn class_like(self) -> bool {
        matches!(
            self,
            Self::Module | Self::Controller | Self::Service | Self::Repository | Self::Class
        )
    }
    fn declaration(self) -> bool {
        self.class_like() || matches!(self, Self::Interface | Self::Method)
    }
}
impl EdgeKind {
    fn wire(self) -> &'static str {
        match self {
            Self::Contains => "contains",
            Self::Imports => "imports",
            Self::Injects => "injects",
            Self::Calls => "calls",
            Self::Exposes => "exposes",
            Self::Reads => "reads",
            Self::Writes => "writes",
            Self::DependsOn => "depends_on",
        }
    }
}
fn position(file: Option<&str>, line: Option<u64>, end: Option<u64>) -> bool {
    file.is_none_or(is_repository_path)
        && line.is_none_or(|n| file.is_some() && (1..=MAX_INTEGER).contains(&n))
        && end.is_none_or(|n| n <= MAX_INTEGER && line.is_some_and(|start| n >= start))
}
fn evidence(items: &[Evidence]) -> bool {
    !items.is_empty()
        && items
            .iter()
            .all(|e| position(Some(&e.file), e.line, e.end_line))
}
fn timestamp(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b[19] != b'Z'
        || !b
            .iter()
            .enumerate()
            .all(|(i, b)| [4, 7, 10, 13, 16, 19].contains(&i) || b.is_ascii_digit())
    {
        return false;
    }
    let year: u32 = s[0..4].parse().unwrap();
    let month: usize = s[5..7].parse().unwrap();
    let day: u32 = s[8..10].parse().unwrap();
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    (1..=12).contains(&month)
        && day > 0
        && day <= days[month - 1]
        && s[11..13].parse::<u32>().unwrap() < 24
        && s[14..16].parse::<u32>().unwrap() < 60
        && s[17..19].parse::<u32>().unwrap() < 60
}
fn route(path: &str) -> bool {
    path.starts_with('/')
        && (path == "/"
            || (!path.ends_with('/')
                && path[1..]
                    .split('/')
                    .all(|p| !p.is_empty() && p != "." && p != "..")))
        && !path.chars().any(|c| {
            matches!(c, '\\' | '?' | '#')
                || (c.is_whitespace() || c == '\u{feff}')
                || c <= '\u{1f}'
                || c == '\u{7f}'
        })
}
fn meta<'a>(metadata: &'a Option<Map<String, Value>>, key: &str) -> Option<&'a str> {
    metadata.as_ref()?.get(key)?.as_str()
}

fn valid_metadata(metadata: &Option<Map<String, Value>>) -> bool {
    let Some(metadata) = metadata else {
        return true;
    };
    let mut pending: Vec<_> = metadata.values().map(|v| (v, 0)).collect();
    while let Some((value, depth)) = pending.pop() {
        if depth > 32 {
            return false;
        }
        match value {
            Value::Number(number) => {
                let Some(number) = number.as_f64() else {
                    return false;
                };
                if !number.is_finite()
                    || (number.fract() == 0.0 && number.abs() > MAX_INTEGER as f64)
                {
                    return false;
                }
            }
            Value::Array(items) => pending.extend(items.iter().map(|v| (v, depth + 1))),
            Value::Object(items) => pending.extend(items.values().map(|v| (v, depth + 1))),
            _ => (),
        }
    }
    true
}

impl SystemGraph {
    pub fn from_json(json: &str) -> Result<Self, String> {
        let graph: Self = serde_json::from_str(json).map_err(|e| e.to_string())?;
        graph.validate()?;
        Ok(graph)
    }

    /// The builder (#16) must validate before emitting. Array order is canonicalized here.
    pub fn to_json(&self) -> Result<String, String> {
        self.validate()?;
        let mut graph = self.clone();
        graph.nodes.sort_by(|a, b| a.id.cmp(&b.id));
        graph.edges.sort_by(|a, b| a.id.cmp(&b.id));
        fn sort_evidence(items: &mut Vec<Evidence>) {
            items.sort_by_cached_key(|e| serde_json::to_string(e).expect("finite evidence fields"));
            items.dedup();
        }
        for n in &mut graph.nodes {
            sort_evidence(&mut n.evidence);
        }
        for e in &mut graph.edges {
            sort_evidence(&mut e.evidence);
        }
        graph
            .diagnostics
            .sort_by_cached_key(|d| serde_json::to_string(d).expect("finite diagnostic fields"));
        serde_json::to_string_pretty(&graph).map_err(|e| e.to_string())
    }

    pub fn validate(&self) -> Result<(), String> {
        let fail = |message: &str| Err(message.to_owned());
        if self.metadata.analyzer_version.is_empty()
            || self
                .metadata
                .root_name
                .as_ref()
                .is_some_and(|s| s.is_empty())
            || !timestamp(&self.metadata.analyzed_at)
        {
            return fail("Invalid graph metadata");
        }
        let nodes: HashMap<&str, &GraphNode> =
            self.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
        if nodes.len() != self.nodes.len() {
            return fail("Duplicate node ID");
        }
        let mut edge_ids = HashSet::new();
        let mut owners: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut members: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut exposing: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut handlers: HashMap<&str, Vec<&str>> = HashMap::new();
        for e in &self.edges {
            let (Some(a), Some(b)) = (nodes.get(e.from.as_str()), nodes.get(e.to.as_str())) else {
                return fail("Dangling edge");
            };
            if !edge_ids.insert(&e.id)
                || e.id != canonical_id("edge", &[&e.from, e.kind.wire(), &e.to])
                || !evidence(&e.evidence)
                || !valid_metadata(&e.metadata)
            {
                return fail("Invalid or duplicate edge ID/evidence");
            }
            use EdgeKind::*;
            use NodeKind::*;
            let valid = match e.kind {
                Contains if a.kind == Module && b.kind.class_like() && b.kind != Module => {
                    members.entry(&b.id).or_default().push(&a.id);
                    true
                }
                Contains if a.kind.class_like() && b.kind == Method => {
                    owners.entry(&b.id).or_default().push(&a.id);
                    true
                }
                Imports => {
                    a.kind.declaration() && (b.kind.declaration() || b.kind == ExternalDependency)
                }
                Injects => {
                    a.kind.class_like()
                        && (b.kind.class_like() || b.kind == ExternalDependency)
                        && meta(&e.metadata, "semantics") == Some("requested_token")
                }
                Calls => {
                    a.kind == Method
                        && b.kind == Method
                        && a.parent_id.is_some()
                        && a.parent_id == b.parent_id
                }
                Exposes if a.kind == Controller && b.kind == Endpoint => {
                    exposing.entry(&b.id).or_default().push(&a.id);
                    true
                }
                DependsOn if a.kind == Module && b.kind == Module => true,
                DependsOn if a.kind == Endpoint && b.kind == Method => {
                    handlers.entry(&a.id).or_default().push(&b.id);
                    true
                }
                _ => false,
            };
            if !valid {
                return fail("Unsupported edge semantics");
            }
        }
        for n in &self.nodes {
            if n.name.is_empty()
                || n.qualified_name.as_ref().is_some_and(|s| s.is_empty())
                || !position(n.file.as_deref(), n.line, n.end_line)
                || !evidence(&n.evidence)
                || !valid_metadata(&n.metadata)
            {
                return fail("Invalid node source/evidence/name");
            }
            let Some((tag, p)) = id_parts(&n.id) else {
                return fail("Invalid node ID");
            };
            use NodeKind::*;
            if n.kind.class_like() || matches!(n.kind, Interface | DatabaseModel) {
                let expected = if n.kind.class_like() {
                    "class"
                } else if n.kind == Interface {
                    "interface"
                } else {
                    "database_model"
                };
                if tag != expected
                    || p.len() < 2
                    || Some(p[0].as_str()) != n.file.as_deref()
                    || p.last() != Some(&n.name)
                    || (n.kind == DatabaseModel && p.len() != 2)
                {
                    return fail("Declaration identity mismatch");
                }
            } else if n.kind == Method {
                if tag != "method"
                    || p.len() != 3
                    || Some(p[0].as_str()) != n.parent_id.as_deref()
                    || !["instance", "static"].contains(&p[1].as_str())
                    || p[2] != n.name
                {
                    return fail("Method identity mismatch");
                }
            } else if n.kind == ExternalDependency {
                if tag != "external" || p.len() != 2 || p[1] != n.name {
                    return fail("External identity mismatch");
                }
            } else {
                let (Some(http), Some(path), Some(method)) = (
                    meta(&n.metadata, "httpMethod"),
                    meta(&n.metadata, "path"),
                    meta(&n.metadata, "controllerMethodId"),
                ) else {
                    return fail("Endpoint metadata missing");
                };
                if !["GET", "POST", "PUT", "PATCH", "DELETE"].contains(&http)
                    || !route(path)
                    || n.id != canonical_id("endpoint", &[http, path, method])
                {
                    return fail("Endpoint metadata/identity mismatch");
                }
                let controllers = exposing
                    .get(n.id.as_str())
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                let methods = handlers
                    .get(n.id.as_str())
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                if controllers.len() != 1
                    || methods.len() != 1
                    || methods[0] != method
                    || nodes[methods[0]].parent_id.as_deref() != Some(controllers[0])
                    || n.parent_id.as_deref() != Some(controllers[0])
                {
                    return fail("Endpoint handler mismatch");
                }
            }
            let parents = if n.kind == Method {
                owners.get(n.id.as_str())
            } else {
                members.get(n.id.as_str())
            }
            .map(Vec::as_slice)
            .unwrap_or_default();
            let expected = if parents.len() == 1 {
                Some(parents[0])
            } else {
                None
            };
            if (n.kind == Method && (parents.len() != 1 || n.parent_id.as_deref() != expected))
                || (n.kind.class_like() && n.parent_id.as_deref() != expected)
            {
                return fail("Invalid membership/lexical parent");
            }
            if !n.kind.class_like() && !matches!(n.kind, Method | Endpoint) && n.parent_id.is_some()
            {
                return fail("Unexpected parent");
            }
            let mut seen = HashSet::from([n.id.as_str()]);
            let mut parent = n.parent_id.as_deref();
            while let Some(id) = parent {
                if !seen.insert(id) {
                    return fail("Cyclic parent");
                }
                let Some(p) = nodes.get(id) else {
                    return fail("Dangling parent");
                };
                parent = p.parent_id.as_deref();
            }
        }
        let mut skipped = 0u64;
        let mut groups = HashSet::new();
        for d in &self.diagnostics {
            if d.code.is_empty()
                || d.message.is_empty()
                || !position(d.file.as_deref(), d.line, None)
                || d.related_node_id
                    .as_ref()
                    .is_some_and(|id| !nodes.contains_key(id.as_str()))
            {
                return fail("Invalid diagnostic");
            }
            if let Some(count) = d.skipped_count {
                if !(1..=MAX_INTEGER).contains(&count)
                    || d.file.is_none()
                    || d.line.is_none()
                    || !d.related_node_id.as_ref().is_some_and(|id| {
                        nodes
                            .get(id.as_str())
                            .is_some_and(|n| n.kind == NodeKind::Method)
                    })
                    || !groups.insert((&d.related_node_id, &d.code))
                {
                    return fail("Invalid skipped-call diagnostic");
                }
                skipped = skipped
                    .checked_add(count)
                    .filter(|n| *n <= MAX_INTEGER)
                    .ok_or("Skipped count overflow")?;
            }
        }
        let c = &self.metadata.call_analysis;
        let calls = self
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Calls)
            .count() as u64;
        if [c.examined_calls, c.emitted_calls, c.skipped_calls]
            .iter()
            .any(|n| *n > MAX_INTEGER)
            || c.emitted_calls.checked_add(c.skipped_calls) != Some(c.examined_calls)
            || skipped != c.skipped_calls
            || c.emitted_calls < calls
            || (c.emitted_calls > 0 && calls == 0)
        {
            return fail("Call coverage mismatch");
        }
        Ok(())
    }
}
