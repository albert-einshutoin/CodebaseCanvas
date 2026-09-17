//! Static NestJS class roles; no decorator argument, membership or runtime inference.
use crate::{
    Confidence, Diagnostic, Evidence, EvidenceSource, GraphBuilder, GraphNode, NodeKind, Severity,
    resolver::{ImportResolver, Resolution},
    typescript::{ExtractionSummary, line_at},
};
use oxc_ast::ast::{Class, Expression};

/// Extract declarations with roles before their first insertion into the builder.
/// The resolver owns the source, so reference offsets cannot use a different snapshot.
/// Apply `resolver.apply_imports(builder)` after extracting all snapshot files.
pub fn extract_file(
    file: &str,
    resolver: &ImportResolver,
    builder: &mut GraphBuilder,
) -> Result<ExtractionSummary, String> {
    let source = resolver
        .source(file)
        .ok_or("File is absent from resolver snapshot")?;
    crate::typescript::extract(file, source, Some(resolver), builder)
}

pub(crate) fn classify(
    class: &Class<'_>,
    file: &str,
    source: &str,
    resolver: &ImportResolver,
    node: &mut GraphNode,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut candidates = Vec::new();
    for decorator in &class.decorators {
        let line = line_at(source, decorator.span.start);
        let report = |code: &str, message: &str| Diagnostic {
            code: format!("NESTJS_ROLE_{code}"),
            severity: Severity::Warning,
            message: message.into(),
            file: Some(file.into()),
            line: Some(line),
            related_node_id: Some(node.id.clone()),
            skipped_count: None,
        };
        let Expression::CallExpression(call) = &decorator.expression else {
            diagnostics.push(report(
                "UNSUPPORTED",
                "Role classification requires a direct decorator call",
            ));
            continue;
        };
        let Expression::Identifier(callee) = &call.callee else {
            diagnostics.push(report(
                "UNSUPPORTED",
                "Decorator callee is not a supported named import reference",
            ));
            continue;
        };
        let Some(reference) = resolver.at_reference(file, callee.span.start) else {
            // Local and shadowed decorators have no proven framework origin.
            // Their spelling must never create a role candidate.
            let mut diagnostic = report(
                "ORIGIN_UNKNOWN",
                "Decorator has no supported value import reference; local, shadowed or type-only origins are not classified",
            );
            diagnostic.severity = Severity::Info;
            diagnostics.push(diagnostic);
            continue;
        };
        let binding = resolver.import_for(reference);
        let role = match binding.exported_name.as_str() {
            "Module" => NodeKind::Module,
            "Controller" => NodeKind::Controller,
            "Injectable" => NodeKind::Service,
            _ => continue,
        };
        if let Resolution::Unresolved { .. } = &binding.resolution {
            // Keep the resolver's import-site reason; add only class-scoped impact.
            diagnostics.push(report(
                "UNRESOLVED",
                "Decorator role is unknown; see the import resolution diagnostic",
            ));
            continue;
        }
        if binding.specifier != "@nestjs/common" {
            continue;
        }
        if binding.type_only {
            diagnostics.push(report(
                "TYPE_ONLY",
                "Type-only import cannot establish a runtime decorator role",
            ));
            continue;
        }
        if !matches!(&binding.resolution, Resolution::ExternalSymbol { specifier, exported_name, .. }
            if specifier == "@nestjs/common" && exported_name == &binding.exported_name)
        {
            continue;
        }
        candidates.push(role);
        node.evidence.push(Evidence {
            source: EvidenceSource::Nestjs,
            file: file.into(),
            line: Some(line),
            end_line: Some(line_at(source, decorator.span.end.saturating_sub(1))),
            confidence: Confidence::Confirmed,
        });
    }
    let Some(&role) = candidates.first() else {
        return;
    };
    if candidates.iter().any(|candidate| *candidate != role) {
        diagnostics.push(Diagnostic {
            code: "NESTJS_ROLE_CONFLICT".into(),
            severity: Severity::Warning,
            message: "Conflicting static role candidates are not classified in v0.1".into(),
            file: Some(file.into()),
            line: node.line,
            related_node_id: Some(node.id.clone()),
            skipped_count: None,
        });
        return;
    }
    node.kind = role;
    if role == NodeKind::Service && node.name.ends_with("Repository") {
        node.kind = NodeKind::Repository;
        // Declaration evidence stays confirmed; naming is a separate heuristic.
        node.evidence.push(Evidence {
            source: EvidenceSource::Nestjs,
            file: file.into(),
            line: Some(line_at(
                source,
                class.id.as_ref().expect("named declaration").span.start,
            )),
            end_line: None,
            confidence: Confidence::BestEffort,
        });
    }
}
