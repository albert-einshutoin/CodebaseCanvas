//! Generic TypeScript declaration extraction.
//!
//! This module only records syntax that Oxc can establish directly: named class
//! and interface declarations, named class methods, and their source evidence.
//! NestJS roles, imports, calls, and type inference belong to later recognizers.

use crate::{
    Confidence, Diagnostic, EdgeKind, Evidence, EvidenceSource, GraphBuilder, GraphEdge, GraphNode,
    NodeKind, Severity, is_repository_path,
};
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    BlockStatement, Class, ClassType, Declaration, ExportDefaultDeclarationKind, FunctionBody,
    MethodDefinition, MethodDefinitionKind, ModuleExportName, StaticBlock, SwitchStatement,
    TSInterfaceDeclaration, TSNamespaceDeclaration,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_span::{SourceType, Span};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtractionSummary {
    pub nodes: usize,
    pub edges: usize,
    pub diagnostics: usize,
}

#[derive(Clone, Debug)]
struct ClassContext {
    id: String,
    qualified_name: String,
}

#[derive(Default)]
struct ExportCollector {
    scope: Vec<String>,
    exported: BTreeSet<(Vec<String>, String)>,
}

impl ExportCollector {
    fn add_name(&mut self, name: impl Into<String>) {
        self.exported.insert((self.scope.clone(), name.into()));
    }

    fn add_declaration(&mut self, declaration: &Declaration<'_>) {
        match declaration {
            Declaration::ClassDeclaration(class) => {
                if let Some(id) = &class.id {
                    self.add_name(id.name.to_string());
                }
            }
            Declaration::TSInterfaceDeclaration(interface) => {
                self.add_name(interface.id.name.to_string());
            }
            _ => {}
        }
    }

    fn add_default(&mut self, declaration: &ExportDefaultDeclarationKind<'_>) {
        match declaration {
            ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                if let Some(id) = &class.id {
                    self.add_name(id.name.to_string());
                }
            }
            ExportDefaultDeclarationKind::TSInterfaceDeclaration(interface) => {
                self.add_name(interface.id.name.to_string());
            }
            _ => {}
        }
    }
}

impl<'a> Visit<'a> for ExportCollector {
    fn visit_ts_namespace_declaration(&mut self, namespace: &TSNamespaceDeclaration<'a>) {
        self.scope.push(namespace.id.name.to_string());
        walk::walk_ts_namespace_declaration(self, namespace);
        self.scope.pop();
    }

    fn visit_export_declaration(&mut self, export: &oxc_ast::ast::ExportDeclaration<'a>) {
        self.add_declaration(&export.declaration);
        walk::walk_export_declaration(self, export);
    }

    fn visit_export_named_declaration(
        &mut self,
        export: &oxc_ast::ast::ExportNamedDeclaration<'a>,
    ) {
        for specifier in &export.specifiers {
            if let Some(name) = export_name(&specifier.local) {
                self.add_name(name);
            }
        }
        walk::walk_export_named_declaration(self, export);
    }

    fn visit_export_from_declaration(&mut self, export: &oxc_ast::ast::ExportFromDeclaration<'a>) {
        // Oxc represents `export { name } from "..."` separately, so its
        // specifiers never describe a local declaration in this file.
        walk::walk_export_from_declaration(self, export);
    }

    fn visit_export_default_declaration(
        &mut self,
        export: &oxc_ast::ast::ExportDefaultDeclaration<'a>,
    ) {
        self.add_default(&export.declaration);
        walk::walk_export_default_declaration(self, export);
    }
}

struct Collector<'a> {
    file: &'a str,
    source: &'a str,
    exported: &'a BTreeSet<(Vec<String>, String)>,
    scope: Vec<String>,
    classes: Vec<Option<ClassContext>>,
    nodes: BTreeMap<String, GraphNode>,
    edges: BTreeMap<String, GraphEdge>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Collector<'a> {
    fn new(file: &'a str, source: &'a str, exported: &'a BTreeSet<(Vec<String>, String)>) -> Self {
        Self {
            file,
            source,
            exported,
            scope: Vec::new(),
            classes: Vec::new(),
            nodes: BTreeMap::new(),
            edges: BTreeMap::new(),
            diagnostics: Vec::new(),
        }
    }

    fn node_id(&self, kind: NodeKind, name: &str) -> Option<String> {
        let scope: Vec<&str> = self.scope.iter().map(String::as_str).collect();
        GraphBuilder::node_id(kind, self.file, &scope, name).ok()
    }

    fn qualified_name(&self, name: &str) -> String {
        let mut parts: Vec<_> = self
            .scope
            .iter()
            .filter(|part| !part.starts_with('@'))
            .cloned()
            .collect();
        parts.push(name.to_owned());
        parts.join(".")
    }

    fn is_exported(&self, name: &str) -> bool {
        self.exported
            .contains(&(self.scope.clone(), name.to_owned()))
    }

    fn evidence(&self, span: Span) -> Evidence {
        Evidence {
            source: EvidenceSource::Ast,
            file: self.file.to_owned(),
            line: Some(line_at(self.source, span.start)),
            end_line: Some(end_line(self.source, span)),
            confidence: Confidence::Confirmed,
        }
    }

    fn declaration_metadata(&self, exported: bool) -> Option<Map<String, Value>> {
        exported.then(|| Map::from_iter([("exported".to_owned(), Value::Bool(true))]))
    }

    fn add_node(&mut self, node: GraphNode) {
        if let Some(existing) = self.nodes.get_mut(&node.id) {
            if existing.kind == node.kind
                && existing.name == node.name
                && existing.qualified_name == node.qualified_name
                && existing.file == node.file
                && existing.parent_id == node.parent_id
                && existing.metadata == node.metadata
            {
                for evidence in node.evidence {
                    if !existing.evidence.contains(&evidence) {
                        existing.evidence.push(evidence);
                    }
                }
                return;
            }
        } else {
            self.nodes.insert(node.id.clone(), node);
            return;
        }
        self.diagnostics.push(Diagnostic {
            code: "TS_CONTRADICTORY_DECLARATION".to_owned(),
            severity: Severity::Error,
            message: "Declarations with one canonical ID disagree".to_owned(),
            file: Some(self.file.to_owned()),
            line: None,
            related_node_id: None,
            skipped_count: None,
        });
    }

    fn add_edge(&mut self, edge: GraphEdge) {
        if let Some(existing) = self.edges.get_mut(&edge.id) {
            for evidence in edge.evidence {
                if !existing.evidence.contains(&evidence) {
                    existing.evidence.push(evidence);
                }
            }
        } else {
            self.edges.insert(edge.id.clone(), edge);
        }
    }

    fn add_class(&mut self, class: &Class<'a>) -> Option<ClassContext> {
        if class.r#type != ClassType::ClassDeclaration {
            if class.id.is_none() {
                self.diagnostics.push(Diagnostic {
                    code: "TS_ANONYMOUS_CLASS".to_owned(),
                    severity: Severity::Warning,
                    message: "Anonymous class expression was not added to the graph".to_owned(),
                    file: Some(self.file.to_owned()),
                    line: Some(line_at(self.source, class.span.start)),
                    related_node_id: None,
                    skipped_count: None,
                });
            }
            return None;
        }
        let Some(id) = &class.id else {
            self.diagnostics.push(Diagnostic {
                code: "TS_ANONYMOUS_CLASS".to_owned(),
                severity: Severity::Warning,
                message: "Anonymous class declaration was not added to the graph".to_owned(),
                file: Some(self.file.to_owned()),
                line: Some(line_at(self.source, class.span.start)),
                related_node_id: None,
                skipped_count: None,
            });
            return None;
        };
        let name = id.name.to_string();
        let node_id = self.node_id(NodeKind::Class, &name)?;
        let context = ClassContext {
            id: node_id.clone(),
            qualified_name: self.qualified_name(&name),
        };
        self.add_node(GraphNode {
            id: node_id,
            kind: NodeKind::Class,
            name,
            qualified_name: Some(context.qualified_name.clone()),
            file: Some(self.file.to_owned()),
            line: Some(line_at(self.source, class.span.start)),
            end_line: Some(end_line(self.source, class.span)),
            parent_id: None,
            evidence: vec![self.evidence(class.span)],
            metadata: self.declaration_metadata(self.is_exported(id.name.as_str())),
        });
        Some(context)
    }

    fn add_interface(&mut self, interface: &TSInterfaceDeclaration<'a>) {
        let name = interface.id.name.to_string();
        let Some(node_id) = self.node_id(NodeKind::Interface, &name) else {
            return;
        };
        self.add_node(GraphNode {
            id: node_id,
            kind: NodeKind::Interface,
            name: name.clone(),
            qualified_name: Some(self.qualified_name(&name)),
            file: Some(self.file.to_owned()),
            line: Some(line_at(self.source, interface.span.start)),
            end_line: Some(end_line(self.source, interface.span)),
            parent_id: None,
            evidence: vec![self.evidence(interface.span)],
            metadata: self.declaration_metadata(self.is_exported(&name)),
        });
    }

    fn add_method(&mut self, method: &MethodDefinition<'a>) {
        let Some(owner) = self.classes.last().and_then(Option::as_ref).cloned() else {
            return;
        };
        if method.kind == MethodDefinitionKind::Constructor {
            return;
        }
        let Some(name) = method.key.static_name() else {
            self.diagnostics.push(Diagnostic {
                code: "TS_UNSUPPORTED_METHOD_NAME".to_owned(),
                severity: Severity::Warning,
                message: "Computed class method name was not added to the graph".to_owned(),
                file: Some(self.file.to_owned()),
                line: Some(line_at(self.source, method.span.start)),
                related_node_id: Some(owner.id.clone()),
                skipped_count: None,
            });
            return;
        };
        let name = name.into_owned();
        let staticness = if method.r#static {
            "static"
        } else {
            "instance"
        };
        let id = GraphBuilder::method_id(&owner.id, staticness, &name);
        let evidence = self.evidence(method.span);
        self.add_node(GraphNode {
            id: id.clone(),
            kind: NodeKind::Method,
            name: name.clone(),
            qualified_name: Some(format!("{}.{}", owner.qualified_name, name)),
            file: Some(self.file.to_owned()),
            line: Some(line_at(self.source, method.span.start)),
            end_line: Some(end_line(self.source, method.span)),
            parent_id: Some(owner.id.clone()),
            evidence: vec![evidence.clone()],
            metadata: None,
        });
        self.add_edge(GraphEdge {
            id: GraphBuilder::edge_id(&owner.id, EdgeKind::Contains, &id),
            from: owner.id.clone(),
            to: id,
            kind: EdgeKind::Contains,
            evidence: vec![evidence],
            metadata: None,
        });
    }
}

impl<'a> Visit<'a> for Collector<'a> {
    fn visit_class(&mut self, class: &Class<'a>) {
        let context = self.add_class(class);
        self.classes.push(context);
        walk::walk_class(self, class);
        self.classes.pop();
    }

    fn visit_method_definition(&mut self, method: &MethodDefinition<'a>) {
        self.add_method(method);
        walk::walk_method_definition(self, method);
    }

    fn visit_ts_interface_declaration(&mut self, interface: &TSInterfaceDeclaration<'a>) {
        self.add_interface(interface);
        walk::walk_ts_interface_declaration(self, interface);
    }

    fn visit_ts_namespace_declaration(&mut self, namespace: &TSNamespaceDeclaration<'a>) {
        self.scope.push(namespace.id.name.to_string());
        walk::walk_ts_namespace_declaration(self, namespace);
        self.scope.pop();
    }

    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.scope.push(format!("@block:{}", block.span.start));
        walk::walk_block_statement(self, block);
        self.scope.pop();
    }

    fn visit_function_body(&mut self, body: &FunctionBody<'a>) {
        self.scope.push(format!("@function:{}", body.span.start));
        walk::walk_function_body(self, body);
        self.scope.pop();
    }

    fn visit_static_block(&mut self, block: &StaticBlock<'a>) {
        self.scope.push(format!("@static:{}", block.span.start));
        walk::walk_static_block(self, block);
        self.scope.pop();
    }

    fn visit_switch_statement(&mut self, switch: &SwitchStatement<'a>) {
        self.scope.push(format!("@switch:{}", switch.span.start));
        walk::walk_switch_statement(self, switch);
        self.scope.pop();
    }
}

pub fn extract_file(
    file: &str,
    source: &str,
    builder: &mut GraphBuilder,
) -> Result<ExtractionSummary, String> {
    if !is_repository_path(file) {
        return Err("source file must be repository-relative".to_owned());
    }
    let source_type = if file.ends_with(".tsx") {
        SourceType::tsx().with_module(true)
    } else if file.ends_with(".ts") {
        SourceType::ts().with_module(true)
    } else {
        return Err("source file must be TypeScript".to_owned());
    };
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if parsed.panicked || !parsed.diagnostics.is_empty() {
        builder.add_diagnostic(Diagnostic {
            code: "TS_PARSE_ERROR".to_owned(),
            severity: Severity::Error,
            message: "TypeScript source could not be parsed".to_owned(),
            file: Some(file.to_owned()),
            line: None,
            related_node_id: None,
            skipped_count: None,
        })?;
        return Ok(ExtractionSummary {
            nodes: 0,
            edges: 0,
            diagnostics: 1,
        });
    }

    let mut exports = ExportCollector::default();
    exports.visit_program(&parsed.program);
    let mut collector = Collector::new(file, source, &exports.exported);
    collector.visit_program(&parsed.program);
    let summary = ExtractionSummary {
        nodes: collector.nodes.len(),
        edges: collector.edges.len(),
        diagnostics: collector.diagnostics.len(),
    };
    for node in collector.nodes.into_values() {
        builder.add_node(node)?;
    }
    for edge in collector.edges.into_values() {
        builder.add_edge(edge)?;
    }
    for diagnostic in collector.diagnostics {
        builder.add_diagnostic(diagnostic)?;
    }
    Ok(summary)
}

fn export_name(name: &ModuleExportName<'_>) -> Option<String> {
    match name {
        ModuleExportName::IdentifierName(name) => Some(name.name.to_string()),
        ModuleExportName::IdentifierReference(name) => Some(name.name.to_string()),
        ModuleExportName::StringLiteral(_) => None,
    }
}

fn line_at(source: &str, offset: u32) -> u64 {
    let bytes = source.as_bytes();
    let end = (offset as usize).min(bytes.len());
    let mut line = 1;
    let mut index = 0;
    while index < end {
        match bytes[index] {
            b'\n' => {
                line += 1;
                index += 1;
            }
            b'\r' => {
                line += 1;
                index += 1;
                if index < end && bytes[index] == b'\n' {
                    index += 1;
                }
            }
            0xE2 if index + 2 < end && bytes[index + 1] == 0x80 => {
                if matches!(bytes[index + 2], 0xA8 | 0xA9) {
                    line += 1;
                    index += 3;
                } else {
                    index += 1;
                }
            }
            _ => index += 1,
        }
    }
    line
}

fn end_line(source: &str, span: Span) -> u64 {
    line_at(source, span.end.saturating_sub(1))
}
