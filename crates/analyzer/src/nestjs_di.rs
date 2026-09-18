//! Source-level constructor token requests, never provider implementation selection.
use crate::{
    Confidence, Diagnostic, EdgeKind, Evidence, EvidenceSource, GraphBuilder, GraphEdge, NodeKind,
    Severity,
    resolver::{ImportResolver, Resolution, SourceSite},
    typescript::line_at,
};
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    BindingPattern, Class, ClassElement, Decorator, Expression, FormalParameter,
    MethodDefinitionKind, TSType, TSTypeName,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_span::{SourceType, Span};

#[derive(Debug, Clone, PartialEq)]
pub struct DiFinding {
    pub consumer_id: String,
    pub parameter_index: usize,
    pub site: SourceSite,
    pub target: Result<String, String>,
    pub evidence: Vec<Evidence>,
}
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DiFindings {
    pub parameters: Vec<DiFinding>,
    pub diagnostics: Vec<Diagnostic>,
}
fn consumer(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Module | NodeKind::Controller | NodeKind::Service | NodeKind::Repository
    )
}
impl DiFindings {
    pub fn apply(&self, builder: &mut GraphBuilder) -> Result<(), String> {
        for p in &self.parameters {
            if builder
                .node(&p.consumer_id)
                .is_none_or(|n| !consumer(n.kind) || n.file.as_deref() != Some(&p.site.file))
            {
                return Err("DI requires an existing NestJS consumer in the finding source".into());
            }
            let Ok(target) = &p.target else { continue };
            if builder.node(target).is_none_or(|n| !n.kind.class_like()) {
                return Err("DI requires an existing class token".into());
            }
            builder.add_edge(GraphEdge {
                id: GraphBuilder::edge_id(&p.consumer_id, EdgeKind::Injects, target),
                from: p.consumer_id.clone(),
                to: target.clone(),
                kind: EdgeKind::Injects,
                evidence: p.evidence.clone(),
                metadata: Some(
                    serde_json::from_value(serde_json::json!({"semantics":"requested_token"}))
                        .expect("object"),
                ),
            })?;
        }
        for d in &self.diagnostics {
            builder.add_diagnostic(d.clone())?;
        }
        Ok(())
    }
}
/// Collect after all declarations/roles; Module and route findings may be applied before or after DI.
pub fn analyze(resolver: &ImportResolver, builder: &GraphBuilder) -> Result<DiFindings, String> {
    let mut result = DiFindings::default();
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
            continue;
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
struct Collector<'a> {
    file: &'a str,
    source: &'a str,
    resolver: &'a ImportResolver,
    builder: &'a GraphBuilder,
    result: &'a mut DiFindings,
}
impl Collector<'_> {
    fn origin<'b>(&'b self, d: &Decorator<'_>) -> Option<&'b str> {
        let expr = match &d.expression {
            Expression::CallExpression(c) => &c.callee,
            expr => expr,
        };
        let Expression::Identifier(id) = expr else {
            return None;
        };
        let reference = self.resolver.at_reference(self.file, id.span.start)?;
        let b = self.resolver.import_for(reference);
        (!b.type_only
            && b.specifier == "@nestjs/common"
            && matches!(&b.resolution, Resolution::ExternalSymbol {specifier, exported_name, ..}
                if specifier == "@nestjs/common" && exported_name == &b.exported_name))
        .then_some(b.exported_name.as_str())
    }
    fn decorators(&self, decorators: &[Decorator<'_>]) -> Option<&'static str> {
        if decorators.iter().any(|d| self.origin(d) == Some("Inject")) {
            Some("unsupported_di_custom_token")
        } else if decorators.iter().any(|d| self.origin(d).is_none()) {
            Some("unsupported_di_decorator_origin")
        } else if !decorators.is_empty() {
            Some("unsupported_di_decorator")
        } else {
            None
        }
    }
    fn token(&self, p: &FormalParameter<'_>) -> Result<String, String> {
        if let Some(code) = self.decorators(&p.decorators) {
            return Err(code.into());
        }
        if !matches!(&p.pattern, BindingPattern::BindingIdentifier(_)) || p.initializer.is_some() {
            return Err("unsupported_di_parameter".into());
        }
        let Some(annotation) = &p.type_annotation else {
            return Err("unsupported_di_type".into());
        };
        let TSType::TSTypeReference(reference) = &annotation.type_annotation else {
            return Err("unsupported_di_type".into());
        };
        let TSTypeName::IdentifierReference(id) = &reference.type_name else {
            return Err("unsupported_di_type".into());
        };
        if reference.type_arguments.is_some() {
            return Err("unsupported_di_type".into());
        }
        let (target, kind) = if let Some(reference) =
            self.resolver.at_reference(self.file, id.span.start)
        {
            let binding = self.resolver.import_for(reference);
            // An established interface is intrinsically type-only; keep the oracle's
            // more specific reason ahead of the general type-only export guard.
            if matches!(
                &binding.resolution,
                Resolution::LocalSymbol {
                    kind: NodeKind::Interface,
                    ..
                }
            ) {
                return Err("unsupported_di_interface".into());
            }
            if binding.type_only {
                return Err("unsupported_di_type_only".into());
            }
            match &binding.resolution {
                Resolution::LocalSymbol {
                    export_type_only: true,
                    ..
                } => return Err("unsupported_di_type_only".into()),
                Resolution::LocalSymbol { id, kind, .. } => (id, *kind),
                Resolution::ExternalSymbol { .. } => return Err("unsupported_di_external".into()),
                Resolution::Unresolved { .. } => return Err("unsupported_di_reference".into()),
            }
        } else {
            let Some((id, kind)) = self.resolver.local_reference(self.file, id.span.start) else {
                return Err("unsupported_di_reference".into());
            };
            (id, *kind)
        };
        if kind == NodeKind::Interface {
            return Err("unsupported_di_interface".into());
        }
        if kind != NodeKind::Class || !self.resolver.is_class_value(target) {
            return Err("unsupported_di_class_value".into());
        }
        if self
            .builder
            .node(target)
            .is_none_or(|n| !n.kind.class_like())
        {
            return Err("unsupported_di_reference".into());
        }
        Ok(target.clone())
    }
    fn diagnostic(&mut self, consumer: &str, span: Span, code: &str) {
        let reason = match code {
            "unsupported_di_custom_token" => {
                "Explicit NestJS Inject token forms are outside this PoC; type annotation is not a fallback."
            }
            "unsupported_di_decorator" => {
                "NestJS parameter decorator is outside this PoC; type annotation is not a fallback."
            }
            "unsupported_di_decorator_origin" => {
                "Parameter decorator origin is unverified; type annotation is not a fallback."
            }
            "unsupported_di_dependencies" => {
                "Class-level Dependencies token specification is not expanded."
            }
            "unsupported_di_interface" => "Interface is not a runtime class value.",
            "unsupported_di_type_only" => {
                "Explicit type-only binding or export cannot establish a class token."
            }
            "unsupported_di_external" => {
                "External symbol identity does not establish class value status."
            }
            "unsupported_di_class_value" => {
                "Declaration does not establish a non-ambient class value."
            }
            "unsupported_di_ambiguous" => "Consumer or constructor implementation is ambiguous.",
            "unsupported_di_parameter" => {
                "Default, rest or destructured parameter is outside this PoC."
            }
            "unsupported_di_type" => "Only a simple identifier type annotation is supported.",
            _ => "Parameter reference has no unique supported class declaration binding.",
        };
        self.result.diagnostics.push(Diagnostic {
            code: code.into(),
            severity: Severity::Warning,
            message: reason.into(),
            file: Some(self.file.into()),
            line: Some(line_at(self.source, span.start)),
            related_node_id: Some(consumer.into()),
            skipped_count: None,
        });
    }
    fn parameter(
        &mut self,
        consumer: &str,
        index: usize,
        span: Span,
        target: Result<String, String>,
    ) {
        if let Err(code) = &target {
            self.diagnostic(consumer, span, code);
        }
        let line = line_at(self.source, span.start);
        self.result.parameters.push(DiFinding {
            consumer_id: consumer.into(),
            parameter_index: index,
            site: SourceSite {
                file: self.file.into(),
                start: span.start,
                end: span.end,
                line,
            },
            target,
            evidence: vec![Evidence {
                source: EvidenceSource::Nestjs,
                file: self.file.into(),
                line: Some(line),
                end_line: None,
                confidence: Confidence::Confirmed,
            }],
        });
    }
}
impl<'a> Visit<'a> for Collector<'_> {
    fn visit_class(&mut self, class: &Class<'a>) {
        if let Some(declaration) = self.resolver.declaration_at(self.file, class.span.start)
            && self
                .builder
                .node(&declaration.id)
                .is_some_and(|n| consumer(n.kind))
        {
            let id = declaration.id.clone();
            let constructors: Vec<_> = class
                .body
                .body
                .iter()
                .filter_map(|element| match element {
                    ClassElement::MethodDefinition(m)
                        if m.kind == MethodDefinitionKind::Constructor
                            && m.value.body.is_some() =>
                    {
                        Some(m)
                    }
                    _ => None,
                })
                .collect();
            let blocked = if declaration.ambiguous || constructors.len() > 1 {
                Some("unsupported_di_ambiguous")
            } else if class
                .decorators
                .iter()
                .any(|d| self.origin(d) == Some("Dependencies"))
            {
                Some("unsupported_di_dependencies")
            } else if !declaration.class_value {
                Some("unsupported_di_class_value")
            } else {
                None
            };
            if let Some(code) = blocked {
                self.diagnostic(&id, class.span, code);
            } else if let Some(constructor) = constructors.first() {
                for (index, p) in constructor.value.params.items.iter().enumerate() {
                    self.parameter(&id, index, p.span, self.token(p));
                }
                if let Some(rest) = &constructor.value.params.rest {
                    let code = self
                        .decorators(&rest.decorators)
                        .unwrap_or("unsupported_di_parameter");
                    self.parameter(
                        &id,
                        constructor.value.params.items.len(),
                        rest.span,
                        Err(code.into()),
                    );
                }
            }
        }
        walk::walk_class(self, class);
    }
}
