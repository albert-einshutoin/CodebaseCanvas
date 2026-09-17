//! Declared HTTP entrypoints only; not runtime URLs or handler execution.
use crate::{
    Confidence, Diagnostic, EdgeKind, Evidence, EvidenceSource, GraphBuilder, GraphEdge, GraphNode,
    NodeKind, Severity,
    resolver::{ImportResolver, Resolution, SourceSite},
    typescript::line_at,
};
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    CallExpression, Class, ClassElement, Decorator, Expression, MethodDefinitionKind,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, Span};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct RouteFinding {
    pub controller_id: String,
    pub handler_id: String,
    pub http_method: String,
    pub prefix: String,
    pub path: String,
    pub normalized_path: String,
    pub controller_site: SourceSite,
    pub handler_site: SourceSite,
    pub site: SourceSite,
    pub evidence: Vec<Evidence>,
}

/// Unsupported findings are represented by diagnostics tied to existing declarations.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RouteFindings {
    pub routes: Vec<RouteFinding>,
    pub diagnostics: Vec<Diagnostic>,
}
impl RouteFindings {
    pub fn apply(&self, builder: &mut GraphBuilder) -> Result<(), String> {
        for route in &self.routes {
            if builder
                .node(&route.controller_id)
                .is_none_or(|n| n.kind != NodeKind::Controller)
                || builder.node(&route.handler_id).is_none_or(|n| {
                    n.kind != NodeKind::Method
                        || n.parent_id.as_deref() != Some(&route.controller_id)
                })
            {
                return Err("Route requires an existing controller and its owned handler".into());
            }
            let id = GraphBuilder::endpoint_id(
                &route.http_method,
                &route.normalized_path,
                &route.handler_id,
            );
            builder.add_node(GraphNode {
                id: id.clone(),
                kind: NodeKind::Endpoint,
                name: format!("{} {}", route.http_method, route.normalized_path),
                qualified_name: None,
                file: Some(route.site.file.clone()),
                line: Some(route.site.line),
                end_line: None,
                parent_id: Some(route.controller_id.clone()),
                evidence: route.evidence.clone(),
                metadata: Some(
                    serde_json::from_value(serde_json::json!({
                        "httpMethod": route.http_method,
                        "path": route.normalized_path,
                        "controllerMethodId": route.handler_id,
                    }))
                    .expect("object"),
                ),
            })?;
            for (from, kind, to) in [
                (&route.controller_id, EdgeKind::Exposes, &id),
                (&id, EdgeKind::DependsOn, &route.handler_id),
            ] {
                builder.add_edge(GraphEdge {
                    id: GraphBuilder::edge_id(from, kind, to),
                    from: from.clone(),
                    to: to.clone(),
                    kind,
                    evidence: route.evidence.clone(),
                    metadata: None,
                })?;
            }
        }
        for diagnostic in &self.diagnostics {
            builder.add_diagnostic(diagnostic.clone())?;
        }
        Ok(())
    }
}

/// Collect after declaration roles and module composition, using the resolver's retained input.
pub fn analyze(resolver: &ImportResolver, builder: &GraphBuilder) -> Result<RouteFindings, String> {
    let mut result = RouteFindings::default();
    for (file, source) in resolver.sources() {
        let allocator = Allocator::default();
        let parsed = Parser::new(
            &allocator,
            source,
            if file.ends_with(".tsx") {
                SourceType::tsx()
            } else {
                SourceType::ts()
            }
            .with_module(true),
        )
        .parse();
        if parsed.panicked || !parsed.diagnostics.is_empty() {
            continue; // The existing extractor/resolver owns parse diagnostics.
        }
        Collector {
            file,
            source,
            resolver,
            builder,
            result: &mut result,
        }
        .visit_program(&parsed.program);
    }
    Ok(result)
}

fn http_method(export: &str) -> Option<&'static str> {
    match export {
        "Get" => Some("GET"),
        "Post" => Some("POST"),
        "Put" => Some("PUT"),
        "Patch" => Some("PATCH"),
        "Delete" => Some("DELETE"),
        _ => None,
    }
}
fn candidate(export: &str, controller: bool) -> bool {
    if controller {
        matches!(export, "Controller" | "Version")
    } else {
        http_method(export).is_some()
            || matches!(
                export,
                "All" | "Head" | "Options" | "RequestMapping" | "Version"
            )
    }
}
fn literal_path(call: &CallExpression<'_>) -> Option<String> {
    if call.arguments.is_empty() {
        return Some(String::new());
    }
    if call.arguments.len() == 1
        && let Some(Expression::StringLiteral(value)) = call.arguments[0].as_expression()
    {
        return Some(value.value.to_string());
    }
    None
}
fn normalize(prefix: &str, path: &str) -> Option<String> {
    let joined = format!("{prefix}/{path}");
    let normalized = format!(
        "/{}",
        joined
            .split('/')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("/")
    );
    crate::route(&normalized).then_some(normalized)
}
struct Collector<'s> {
    file: &'s str,
    source: &'s str,
    resolver: &'s ImportResolver,
    builder: &'s GraphBuilder,
    result: &'s mut RouteFindings,
}
impl Collector<'_> {
    fn site(&self, span: Span) -> SourceSite {
        SourceSite {
            file: self.file.into(),
            start: span.start,
            end: span.end,
            line: line_at(self.source, span.start),
        }
    }
    fn diagnostic(&mut self, span: Span, related: &str, code: &str) {
        self.result.diagnostics.push(Diagnostic {
            code: format!("unsupported_route_{code}"),
            severity: Severity::Warning,
            message: match code {
                "origin" => "Route decorator has no confirmed supported value import origin.",
                "configuration" => "Route configuration is outside static path support.",
                "ambiguous" => "Merged controller has no unique route declaration origin.",
                "multiple" => "Multiple route decorators are not combined in this PoC.",
                "handler" => {
                    "Route requires a uniquely identified direct instance method implementation."
                }
                "path" => {
                    "Route path requires a literal value satisfying the normalized route contract."
                }
                _ => "Route declaration is unsupported.",
            }
            .into(),
            file: Some(self.file.into()),
            line: Some(line_at(self.source, span.start)),
            related_node_id: Some(related.into()),
            skipped_count: None,
        });
    }
    // Missing value references are diagnostic-only scoped type bindings; never
    // promote them to runtime provenance or search declarations by spelling.
    fn decorators<'a>(
        &mut self,
        decorators: &'a [Decorator<'a>],
        controller: bool,
        related: &str,
    ) -> (Vec<(&'a CallExpression<'a>, String)>, bool) {
        let mut calls = Vec::new();
        let mut unknown = false;
        for decorator in decorators {
            let (callee, call) = match &decorator.expression {
                Expression::CallExpression(call) => (&call.callee, Some(call.as_ref())),
                expression => (expression, None),
            };
            let Expression::Identifier(id) = callee else {
                // Namespace/composite/custom effects are not expanded. Their
                // presence alone must not suppress a separately confirmed direct route.
                continue;
            };
            let Some(reference) = self.resolver.at_reference(self.file, id.span.start) else {
                if self
                    .resolver
                    .type_only_reference(self.file, id.span.start)
                    .is_some_and(|binding| {
                        binding.specifier == "@nestjs/common"
                            && candidate(&binding.exported_name, controller)
                    })
                {
                    self.diagnostic(decorator.span, related, "origin");
                    unknown = true;
                }
                continue;
            };
            let binding = self.resolver.import_for(reference);
            if !candidate(&binding.exported_name, controller) {
                continue;
            }
            if binding.type_only && binding.specifier != "@nestjs/common" {
                continue;
            }
            if matches!(binding.resolution, Resolution::Unresolved { .. }) || binding.type_only {
                self.diagnostic(decorator.span, related, "origin");
                unknown = true;
                continue;
            }
            if binding.specifier != "@nestjs/common" {
                continue;
            }
            if !matches!(&binding.resolution, Resolution::ExternalSymbol {specifier, exported_name, ..}
                if specifier == "@nestjs/common" && exported_name == &binding.exported_name)
            {
                continue;
            }
            if binding.exported_name == "Version"
                || (!controller && http_method(&binding.exported_name).is_none())
            {
                self.diagnostic(decorator.span, related, "configuration");
                unknown = true;
            } else if let Some(call) = call {
                calls.push((call, binding.exported_name.clone()));
            } else {
                self.diagnostic(decorator.span, related, "origin");
                unknown = true;
            }
        }
        (calls, unknown)
    }
    fn class(&mut self, class: &Class<'_>) {
        let Some(declaration) = self.resolver.declaration_at(self.file, class.span.start) else {
            return;
        };
        let controller_id = declaration.id.clone();
        let ambiguous = declaration.ambiguous;
        let Some(node) = self.builder.node(&controller_id) else {
            return;
        };
        let is_controller = node.kind == NodeKind::Controller;
        let (calls, unknown) = self.decorators(&class.decorators, true, &controller_id);
        if unknown {
            return;
        }
        if calls.len() > 1 {
            self.diagnostic(class.span, &controller_id, "multiple");
            return;
        }
        let Some((controller_call, _)) = calls.first() else {
            return;
        };
        if !is_controller {
            return;
        }
        if ambiguous {
            self.diagnostic(class.span, &controller_id, "ambiguous");
            return;
        }
        let Some(prefix) = literal_path(controller_call) else {
            self.diagnostic(controller_call.span, &controller_id, "path");
            return;
        };
        if normalize(&prefix, "").is_none() {
            self.diagnostic(controller_call.span, &controller_id, "path");
            return;
        }
        let mut implementations = BTreeMap::new();
        for element in &class.body.body {
            if let ClassElement::MethodDefinition(method) = element
                && !method.r#static
                && method.kind == MethodDefinitionKind::Method
                && method.value.body.is_some()
                && let Some(name) = method.key.static_name()
            {
                *implementations.entry(name.into_owned()).or_insert(0usize) += 1;
            }
        }
        for element in &class.body.body {
            let (decorators, method) = match element {
                ClassElement::MethodDefinition(method) => {
                    (&method.decorators, Some(method.as_ref()))
                }
                ClassElement::PropertyDefinition(property) => (&property.decorators, None),
                _ => continue,
            };
            // Overload signatures do not establish routes; only the implementation does.
            if method.is_some_and(|m| m.value.body.is_none()) {
                continue;
            }
            let handler_id = method.and_then(|m| {
                m.key.static_name().map(|name| {
                    GraphBuilder::method_id(
                        &controller_id,
                        if m.r#static { "static" } else { "instance" },
                        &name,
                    )
                })
            });
            let handler = handler_id
                .as_deref()
                .and_then(|id| self.builder.node(id))
                .filter(|n| {
                    n.kind == NodeKind::Method && n.parent_id.as_deref() == Some(&controller_id)
                });
            let related = handler.map_or(controller_id.as_str(), |n| n.id.as_str());
            let (routes, unknown) = self.decorators(decorators, false, related);
            if unknown || routes.is_empty() {
                continue;
            }
            if routes.len() > 1 {
                self.diagnostic(element.span(), related, "multiple");
                continue;
            }
            let valid_method = method.is_some_and(|m| {
                !m.r#static
                    && m.kind == MethodDefinitionKind::Method
                    && m.key.static_name().is_some()
                    && m.key
                        .static_name()
                        .is_some_and(|name| implementations.get(name.as_ref()) == Some(&1))
            });
            if !valid_method || handler.is_none() {
                self.diagnostic(element.span(), related, "handler");
                continue;
            }
            let (call, export) = &routes[0];
            let Some(path) = literal_path(call) else {
                self.diagnostic(call.span, related, "path");
                continue;
            };
            let Some(normalized_path) = normalize(&prefix, &path) else {
                self.diagnostic(call.span, related, "path");
                continue;
            };
            let site = self.site(call.span);
            self.result.routes.push(RouteFinding {
                controller_id: controller_id.clone(),
                handler_id: handler.expect("validated handler").id.clone(),
                http_method: http_method(export).expect("supported decorator").into(),
                prefix: prefix.clone(),
                path,
                normalized_path,
                controller_site: self.site(controller_call.span),
                handler_site: self.site(element.span()),
                evidence: vec![Evidence {
                    source: EvidenceSource::Nestjs,
                    file: self.file.into(),
                    line: Some(site.line),
                    end_line: None,
                    confidence: Confidence::Confirmed,
                }],
                site,
            });
        }
    }
}
impl<'a> Visit<'a> for Collector<'_> {
    fn visit_class(&mut self, class: &Class<'a>) {
        self.class(class);
        // Process each class body independently; nested methods never belong to an outer class.
        walk::walk_class(self, class);
    }
}
