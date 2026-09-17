//! Static module composition findings over the resolver's in-process source input.
use crate::{
    Confidence, Diagnostic, EdgeKind, Evidence, EvidenceSource, GraphBuilder, GraphEdge, NodeKind,
    Severity,
    resolver::{ImportResolver, Resolution, SourceSite},
    typescript::line_at,
};
use oxc_allocator::Allocator;
use oxc_ast::ast::{ArrayExpressionElement, Class, Expression, ObjectPropertyKind, PropertyKind};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, Span};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ModuleField {
    Imports,
    Controllers,
    Providers,
    Exports,
}
impl ModuleField {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "imports" => Some(Self::Imports),
            "controllers" => Some(Self::Controllers),
            "providers" => Some(Self::Providers),
            "exports" => Some(Self::Exports),
            _ => None,
        }
    }
    fn accepts(self, kind: NodeKind) -> bool {
        match self {
            Self::Imports => kind == NodeKind::Module,
            Self::Controllers => kind == NodeKind::Controller,
            Self::Providers => matches!(
                kind,
                NodeKind::Class | NodeKind::Service | NodeKind::Repository
            ),
            Self::Exports => matches!(
                kind,
                NodeKind::Module
                    | NodeKind::Controller
                    | NodeKind::Class
                    | NodeKind::Service
                    | NodeKind::Repository
            ),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleEntry {
    pub field: ModuleField,
    pub site: SourceSite,
    /// A canonical existing declaration ID, or the scoped unsupported reason.
    pub target: Result<String, String>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleFinding {
    pub module_id: String,
    pub site: SourceSite,
    pub entries: Vec<ModuleEntry>,
    pub diagnostics: Vec<Diagnostic>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleFindings {
    pub modules: Vec<ModuleFinding>,
}
impl ModuleFindings {
    pub fn apply(&self, builder: &mut GraphBuilder) -> Result<(), String> {
        let mut edges = Vec::new();
        for module in &self.modules {
            if builder
                .node(&module.module_id)
                .is_none_or(|n| n.kind != NodeKind::Module)
            {
                return Err("Module finding requires an existing module declaration".into());
            }
            for entry in &module.entries {
                let Ok(target) = &entry.target else { continue };
                if builder
                    .node(target)
                    .is_none_or(|n| !entry.field.accepts(n.kind))
                {
                    return Err(
                        "Module entry requires an existing declaration of the appropriate kind"
                            .into(),
                    );
                }
                let kind = match entry.field {
                    ModuleField::Imports => EdgeKind::DependsOn,
                    ModuleField::Controllers | ModuleField::Providers => EdgeKind::Contains,
                    ModuleField::Exports => continue,
                };
                edges.push(GraphEdge {
                    id: GraphBuilder::edge_id(&module.module_id, kind, target),
                    from: module.module_id.clone(),
                    to: target.clone(),
                    kind,
                    evidence: vec![Evidence {
                        source: EvidenceSource::Nestjs,
                        file: entry.site.file.clone(),
                        line: Some(entry.site.line),
                        end_line: None,
                        confidence: Confidence::Confirmed,
                    }],
                    metadata: None,
                });
            }
        }
        builder.apply_module_composition(edges)?;
        for module in &self.modules {
            for diagnostic in &module.diagnostics {
                builder.add_diagnostic(diagnostic.clone())?;
            }
        }
        Ok(())
    }
}

/// Call after all declarations and roles are inserted. No graph mutations during collection.
pub fn analyze(
    resolver: &ImportResolver,
    builder: &GraphBuilder,
) -> Result<ModuleFindings, String> {
    let mut result = ModuleFindings {
        modules: Vec::new(),
    };
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
        } // Existing extraction/resolver diagnostics remain authoritative.
        let mut visitor = Collector {
            file,
            source,
            resolver,
            builder,
            modules: &mut result.modules,
        };
        visitor.visit_program(&parsed.program);
    }
    Ok(result)
}
struct Collector<'a> {
    file: &'a str,
    source: &'a str,
    resolver: &'a ImportResolver,
    builder: &'a GraphBuilder,
    modules: &'a mut Vec<ModuleFinding>,
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
    fn diagnostic(&self, finding: &mut ModuleFinding, span: Span, code: &str) {
        finding.diagnostics.push(Diagnostic {
            code: code.into(),
            severity: Severity::Warning,
            message: match code {
                "unsupported_module_forward_ref" => "forwardRef module reference is not expanded.",
                "unsupported_module_dynamic" => "Dynamic module invocation is not expanded.",
                "unsupported_module_spread" => "Array spread is not expanded.",
                "unsupported_module_provider" => "Provider object is not expanded.",
                "unsupported_module_reference" => {
                    "Entry is not a resolved value reference to a supported declaration."
                }
                "unsupported_module_kind" => {
                    "Declaration kind does not match this module metadata field."
                }
                _ => "Module metadata value cannot be established statically.",
            }
            .into(),
            file: Some(self.file.into()),
            line: Some(line_at(self.source, span.start)),
            related_node_id: Some(finding.module_id.clone()),
            skipped_count: None,
        });
    }
    fn origin(&self, expr: &Expression<'_>, exported: &str) -> bool {
        let Expression::Identifier(id) = expr else {
            return false;
        };
        let Some(reference) = self.resolver.at_reference(self.file, id.span.start) else {
            return false;
        };
        let binding = self.resolver.import_for(reference);
        !binding.type_only
            && binding.specifier == "@nestjs/common"
            && binding.exported_name == exported
            && matches!(&binding.resolution, Resolution::ExternalSymbol {specifier,exported_name,..} if specifier=="@nestjs/common" && exported_name==exported)
    }
    fn target(&self, expr: &Expression<'_>, field: ModuleField) -> Result<String, String> {
        let Expression::Identifier(id) = expr else {
            return Err("unsupported_module_reference".into());
        };
        let target =
            if let Some(reference) = self.resolver.at_reference(self.file, id.span.start) {
                let binding = self.resolver.import_for(reference);
                match &binding.resolution {
                    Resolution::LocalSymbol {
                        id,
                        kind: NodeKind::Class,
                        ..
                    } if !binding.type_only => Some(id.as_str()),
                    _ => None,
                }
            } else {
                self.resolver
                    .local_reference(self.file, id.span.start)
                    .filter(|(_, kind)| *kind == NodeKind::Class)
                    .map(|(id, _)| id.as_str())
            }
            .ok_or("unsupported_module_reference")?;
        let node = self
            .builder
            .node(target)
            .ok_or("unsupported_module_reference")?;
        if !field.accepts(node.kind) {
            return Err("unsupported_module_kind".into());
        }
        Ok(target.into())
    }
    fn entry(
        &self,
        finding: &mut ModuleFinding,
        field: ModuleField,
        expr: Option<&Expression<'_>>,
        span: Span,
        spread: bool,
    ) {
        let target = if spread {
            Err("unsupported_module_spread".into())
        } else {
            match expr {
                Some(Expression::Identifier(_)) => self.target(expr.unwrap(), field),
                Some(Expression::ObjectExpression(_)) if field == ModuleField::Providers => {
                    Err("unsupported_module_provider".into())
                }
                Some(Expression::CallExpression(call)) => {
                    Err(if self.origin(&call.callee, "forwardRef") {
                        "unsupported_module_forward_ref"
                    } else {
                        "unsupported_module_dynamic"
                    }
                    .into())
                }
                _ => Err("unsupported_module_reference".into()),
            }
        };
        if let Err(code) = &target {
            self.diagnostic(finding, span, code);
        }
        finding.entries.push(ModuleEntry {
            field,
            site: self.site(span),
            target,
        });
    }
}
impl<'a> Visit<'a> for Collector<'_> {
    fn visit_class(&mut self, class: &Class<'a>) {
        if let Some(id) = self.resolver.declaration_at(self.file, class.span.start)
            && self
                .builder
                .node(id)
                .is_some_and(|n| n.kind == NodeKind::Module)
        {
            let calls: Vec<_> = class
                .decorators
                .iter()
                .filter_map(|d| match &d.expression {
                    Expression::CallExpression(call) if self.origin(&call.callee, "Module") => {
                        Some(call)
                    }
                    _ => None,
                })
                .collect();
            if !calls.is_empty() {
                let mut finding = ModuleFinding {
                    module_id: id.into(),
                    site: self.site(calls[0].span),
                    entries: vec![],
                    diagnostics: vec![],
                };
                if calls.len() != 1 {
                    self.diagnostic(&mut finding, class.span, "unsupported_module_metadata");
                } else {
                    let call = calls[0];
                    let object = if call.arguments.len() == 1 {
                        call.arguments[0].as_expression().and_then(|e| match e {
                            Expression::ObjectExpression(o) => Some(o),
                            _ => None,
                        })
                    } else {
                        None
                    };
                    if let Some(object) = object {
                        // Any unknown object key can overwrite any field. Do not
                        // confuse this with an array spread, whose sibling entries survive.
                        let unsafe_object = object.properties.iter().any(|p| match p {
                            ObjectPropertyKind::SpreadProperty(_) => true,
                            ObjectPropertyKind::ObjectProperty(p) => p.computed,
                        });
                        if unsafe_object {
                            self.diagnostic(
                                &mut finding,
                                object.span,
                                "unsupported_module_metadata",
                            );
                        } else {
                            let mut fields: BTreeMap<ModuleField, Vec<_>> = BTreeMap::new();
                            for property in &object.properties {
                                if let ObjectPropertyKind::ObjectProperty(p) = property
                                    && let Some(field) =
                                        p.key.static_name().as_deref().and_then(ModuleField::parse)
                                {
                                    fields.entry(field).or_default().push(p);
                                }
                            }
                            for (field, properties) in fields {
                                let p = properties[0];
                                if properties.len() != 1 || p.kind != PropertyKind::Init || p.method
                                {
                                    self.diagnostic(
                                        &mut finding,
                                        p.span,
                                        "unsupported_module_metadata",
                                    );
                                    continue;
                                }
                                if let Expression::ArrayExpression(array) = &p.value {
                                    for entry in &array.elements {
                                        self.entry(
                                            &mut finding,
                                            field,
                                            entry.as_expression(),
                                            entry.span(),
                                            matches!(
                                                entry,
                                                ArrayExpressionElement::SpreadElement(_)
                                            ),
                                        );
                                    }
                                } else {
                                    self.diagnostic(
                                        &mut finding,
                                        p.value.span(),
                                        "unsupported_module_metadata",
                                    );
                                }
                            }
                        }
                    } else {
                        self.diagnostic(&mut finding, call.span, "unsupported_module_metadata");
                    }
                }
                self.modules.push(finding);
            }
        }
        walk::walk_class(self, class);
    }
}
