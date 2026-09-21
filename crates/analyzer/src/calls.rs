//! Same-class syntax findings. Confirmed correspondence is not runtime dispatch proof.
use crate::{
    CallAnalysis, CallMode, CallScope, Confidence, Diagnostic, EdgeKind, Evidence, EvidenceSource,
    GraphBuilder, GraphEdge, NodeKind, Severity,
    resolver::{ImportResolver, SourceSite},
    typescript::{LexicalScope, line_at},
};
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_span::SourceType;
use oxc_syntax::{
    operator::UnaryOperator,
    scope::{ScopeFlags, ScopeId},
};
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
};

#[derive(Debug, Clone, PartialEq)]
pub struct CallSite {
    pub caller_id: String,
    pub site: SourceSite,
    /// Canonical target, or the scoped unsupported reason code.
    pub target: Result<String, String>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct CallFindings {
    sites: Vec<CallSite>,
    summary: CallAnalysis,
    diagnostics: BTreeMap<(String, String), Diagnostic>,
    incomplete_files: BTreeSet<String>,
}
impl CallFindings {
    pub fn sites(&self) -> &[CallSite] {
        &self.sites
    }
    pub fn summary(&self) -> &CallAnalysis {
        &self.summary
    }
    pub fn diagnostics(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.values()
    }
    pub fn incomplete_files(&self) -> &BTreeSet<String> {
        &self.incomplete_files
    }
    pub fn apply(&self, builder: &mut GraphBuilder) -> Result<(), String> {
        let edges = self
            .sites
            .iter()
            .filter_map(|s| {
                let Ok(to) = &s.target else { return None };
                Some(GraphEdge {
                    id: GraphBuilder::edge_id(&s.caller_id, EdgeKind::Calls, to),
                    from: s.caller_id.clone(),
                    to: to.clone(),
                    kind: EdgeKind::Calls,
                    evidence: vec![Evidence {
                        source: EvidenceSource::Ast,
                        file: s.site.file.clone(),
                        line: Some(s.site.line),
                        end_line: None,
                        confidence: Confidence::Confirmed,
                    }],
                    metadata: None,
                })
            })
            .collect();
        builder.apply_call_analysis(
            self.summary.clone(),
            edges,
            self.diagnostics.values().cloned().collect(),
            self.incomplete_files.clone(),
        )
    }
    fn record(&mut self, caller_id: &str, site: SourceSite, target: Result<String, &'static str>) {
        self.summary.examined_calls += 1;
        match &target {
            Ok(_) => self.summary.emitted_calls += 1,
            Err(reason) => {
                self.summary.skipped_calls += 1;
                let code = format!("unsupported_call_{reason}");
                let d = self
                    .diagnostics
                    .entry((caller_id.into(), code.clone()))
                    .or_insert_with(|| Diagnostic {
                        code,
                        severity: Severity::Warning,
                        message: message(reason).into(),
                        file: Some(site.file.clone()),
                        line: Some(site.line),
                        related_node_id: Some(caller_id.into()),
                        skipped_count: Some(0),
                    });
                *d.skipped_count.as_mut().expect("call count") += 1;
                d.line = Some(d.line.expect("call line").min(site.line));
            }
        }
        self.sites.push(CallSite {
            caller_id: caller_id.into(),
            site,
            target: target.map_err(|r| format!("unsupported_call_{r}")),
        });
    }
}
fn message(reason: &str) -> &'static str {
    match reason {
        "nested_function" => "Call inside a nested function is outside same-class call support.",
        "computed_target" => "Computed call target is ambiguous.",
        "injected_receiver" => "Requested token does not prove the executed implementation.",
        "optional" => "Optional call or receiver is outside direct call support.",
        "wrapped_target" => "Wrapped call target or receiver is outside direct call support.",
        "higher_order" => "A call result is not a supported method receiver or target.",
        "inheritance" => "Inherited or super method dispatch is not resolved.",
        "ambiguous_target" => {
            "Declaration collisions or local writes prevent a unique method target."
        }
        "missing_method" => "No supported method with matching owner and staticness exists.",
        _ => "Receiver is not the containing class this.",
    }
}
/// Analyze the retained snapshot after declarations. No file is read here.
pub fn analyze(resolver: &ImportResolver, builder: &GraphBuilder) -> Result<CallFindings, String> {
    let mut result = CallFindings {
        sites: vec![],
        summary: CallAnalysis {
            scope: CallScope::ParsedNamedClassMethods,
            mode: CallMode::SameClassOnly,
            examined_calls: 0,
            emitted_calls: 0,
            skipped_calls: 0,
        },
        diagnostics: BTreeMap::new(),
        incomplete_files: BTreeSet::new(),
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
            result.incomplete_files.insert(file.into());
            continue; // Declaration extraction / resolver own the existing file diagnostics.
        }
        if resolver
            .diagnostics()
            .iter()
            .any(|d| d.code == "TS_IMPORT_PARSEINCOMPLETE" && d.file.as_deref() == Some(file))
        {
            result.incomplete_files.insert(file.into());
        }
        let mut collector = Collector {
            file,
            source,
            resolver,
            builder,
            result: &mut result,
            scope: LexicalScope::default(),
            error: None,
        };
        collector.visit_program(&parsed.program);
        if let Some(error) = collector.error {
            return Err(error);
        }
    }
    result
        .sites
        .sort_by_key(|s| (s.site.file.clone(), s.site.start));
    Ok(result)
}
struct Collector<'s> {
    file: &'s str,
    source: &'s str,
    resolver: &'s ImportResolver,
    builder: &'s GraphBuilder,
    result: &'s mut CallFindings,
    scope: LexicalScope,
    error: Option<String>,
}
impl<'a> Visit<'a> for Collector<'_> {
    fn enter_scope(&mut self, flags: ScopeFlags, _: &Cell<Option<ScopeId>>) {
        self.scope.enter(flags);
    }
    fn leave_scope(&mut self) {
        self.scope.leave();
    }
    fn visit_ts_namespace_declaration(&mut self, n: &TSNamespaceDeclaration<'a>) {
        self.scope.path.push(n.id.name.to_string());
        walk::walk_ts_namespace_declaration(self, n);
        self.scope.path.pop();
    }
    fn visit_class(&mut self, c: &Class<'a>) {
        if c.r#type == ClassType::ClassDeclaration
            && let Some(name) = &c.id
        {
            // The same lexical helper as declaration extraction also identifies parsed
            // methods in files where semantic binding failed; those targets stay unknown.
            let scope: Vec<_> = self.scope.path.iter().map(String::as_str).collect();
            let owner =
                GraphBuilder::node_id(NodeKind::Class, self.file, &scope, name.name.as_str())
                    .expect("class ID");
            let declaration = self.resolver.declaration_at(self.file, c.span.start);
            let ambiguous =
                declaration.is_none_or(|d| d.id != owner || d.ambiguous || !d.class_value);
            let facts = ClassFacts::new(c);
            for element in &c.body.body {
                let ClassElement::MethodDefinition(m) = element else {
                    continue;
                };
                if m.kind == MethodDefinitionKind::Constructor {
                    continue;
                }
                let (Some(name), Some(body)) = (m.key.static_name(), &m.value.body) else {
                    continue;
                };
                let caller = GraphBuilder::method_id(&owner, staticness(m.r#static), &name);
                if self.builder.node(&caller).is_none_or(|n| {
                    n.kind != NodeKind::Method
                        || n.parent_id.as_deref() != Some(&owner)
                        || n.file.as_deref() != Some(self.file)
                }) {
                    self.error =
                        Some("Call analysis requires declarations from the same snapshot".into());
                    continue;
                }
                BodyCollector {
                    file: self.file,
                    source: self.source,
                    caller: &caller,
                    owner: &owner,
                    static_: m.r#static,
                    ambiguous,
                    facts: &facts,
                    result: self.result,
                    nested: 0,
                }
                .visit_function_body(body);
            }
        }
        // BodyCollector only visits a nested class's outer-context expressions.
        // This walk separately discovers its method declarations with canonical scope.
        walk::walk_class(self, c);
    }
}
fn staticness(value: bool) -> &'static str {
    if value { "static" } else { "instance" }
}
#[derive(Default)]
struct ClassFacts {
    methods: BTreeMap<(bool, String), usize>,
    blocked: BTreeSet<(bool, String)>,
    all_blocked: BTreeSet<bool>,
    parameters: BTreeSet<String>,
    inherited: bool,
}
impl ClassFacts {
    fn key(&mut self, key: &PropertyKey<'_>, static_: bool) {
        if let Some(name) = key.static_name() {
            self.blocked.insert((static_, name.into_owned()));
        } else if !matches!(key, PropertyKey::PrivateIdentifier(_)) {
            self.all_blocked.insert(static_);
        }
    }
    fn new(c: &Class<'_>) -> Self {
        let mut facts = Self {
            inherited: c.heritage.is_some(),
            ..Self::default()
        };
        for element in &c.body.body {
            let static_ = match element {
                ClassElement::MethodDefinition(m) => {
                    if m.kind == MethodDefinitionKind::Constructor {
                        for p in &m.value.params.items {
                            if p.accessibility.is_some() || p.readonly || p.r#override {
                                if let BindingPattern::BindingIdentifier(id) = &p.pattern {
                                    facts.parameters.insert(id.name.to_string());
                                    facts.blocked.insert((false, id.name.to_string()));
                                } else {
                                    facts.all_blocked.insert(false);
                                }
                            }
                        }
                    } else if let Some(name) = m.key.static_name() {
                        *facts
                            .methods
                            .entry((m.r#static, name.to_string()))
                            .or_default() += 1;
                        if m.computed
                            || m.kind != MethodDefinitionKind::Method
                            || m.value.body.is_none()
                        {
                            facts.blocked.insert((m.r#static, name.into_owned()));
                        }
                    } else if !matches!(m.key, PropertyKey::PrivateIdentifier(_)) {
                        facts.all_blocked.insert(m.r#static);
                    }
                    m.r#static
                }
                ClassElement::PropertyDefinition(p) => {
                    facts.key(&p.key, p.r#static);
                    p.r#static
                }
                ClassElement::AccessorProperty(p) => {
                    facts.key(&p.key, p.r#static);
                    p.r#static
                }
                ClassElement::StaticBlock(_) => true,
                _ => continue,
            };
            let mut writes = Writes {
                facts: &mut facts,
                static_,
            };
            // Keys belong to the context defining this class, not to the member's this.
            match element {
                ClassElement::MethodDefinition(m) => {
                    writes.visit_function(&m.value, ScopeFlags::Function);
                }
                ClassElement::PropertyDefinition(p) => {
                    if let Some(value) = &p.value {
                        writes.visit_expression(value);
                    }
                }
                ClassElement::AccessorProperty(p) => {
                    if let Some(value) = &p.value {
                        writes.visit_expression(value);
                    }
                }
                ClassElement::StaticBlock(b) => writes.visit_static_block(b),
                _ => {}
            }
        }
        facts
    }
}
/// Visit definition-time expressions without entering the nested class's this/body.
/// Decorators remain outside the existing call-analysis scope.
fn visit_class_definition_expressions<'a>(visitor: &mut impl Visit<'a>, class: &Class<'a>) {
    if let Some(heritage) = &class.heritage {
        visitor.visit_expression(&heritage.expression);
    }
    for element in &class.body.body {
        if element.computed()
            && let Some(key) = element.property_key()
        {
            visitor.visit_property_key(key);
        }
    }
}

/// Writes are conservative across nested functions, but stop at inner member bodies.
struct Writes<'s> {
    facts: &'s mut ClassFacts,
    static_: bool,
}
impl Writes<'_> {
    fn member(&mut self, m: &MemberExpression<'_>) {
        if matches!(
            m.object().get_inner_expression(),
            Expression::ThisExpression(_)
        ) {
            if let Some(name) = m.static_property_name() {
                self.facts.blocked.insert((self.static_, name.into()));
            } else {
                self.facts.all_blocked.insert(self.static_);
            }
        }
    }
}
impl<'a> Visit<'a> for Writes<'_> {
    fn visit_class(&mut self, class: &Class<'a>) {
        visit_class_definition_expressions(self, class);
    }
    fn visit_simple_assignment_target(&mut self, t: &SimpleAssignmentTarget<'a>) {
        if let Some(m) = t.as_member_expression() {
            self.member(m);
        }
        // Assertions on assignment targets are still writes, not receiver evaluation.
        match t {
            SimpleAssignmentTarget::TSAsExpression(e) => {
                if let Some(m) = e.expression.get_inner_expression().as_member_expression() {
                    self.member(m);
                }
            }
            SimpleAssignmentTarget::TSSatisfiesExpression(e) => {
                if let Some(m) = e.expression.get_inner_expression().as_member_expression() {
                    self.member(m);
                }
            }
            SimpleAssignmentTarget::TSNonNullExpression(e) => {
                if let Some(m) = e.expression.get_inner_expression().as_member_expression() {
                    self.member(m);
                }
            }
            SimpleAssignmentTarget::TSTypeAssertion(e) => {
                if let Some(m) = e.expression.get_inner_expression().as_member_expression() {
                    self.member(m);
                }
            }
            _ => {}
        }
        walk::walk_simple_assignment_target(self, t);
    }
    fn visit_unary_expression(&mut self, e: &UnaryExpression<'a>) {
        if e.operator == UnaryOperator::Delete
            && let Some(m) = e.argument.get_inner_expression().as_member_expression()
        {
            self.member(m);
        }
        walk::walk_unary_expression(self, e);
    }
}
struct BodyCollector<'s> {
    file: &'s str,
    source: &'s str,
    caller: &'s str,
    owner: &'s str,
    static_: bool,
    ambiguous: bool,
    facts: &'s ClassFacts,
    result: &'s mut CallFindings,
    nested: usize,
}
impl BodyCollector<'_> {
    fn target(&self, c: &CallExpression<'_>) -> Result<String, &'static str> {
        if self.nested > 0 {
            return Err("nested_function");
        }
        if c.optional {
            return Err("optional");
        }
        if matches!(c.callee, Expression::ComputedMemberExpression(_)) {
            return Err("computed_target");
        }
        let Expression::StaticMemberExpression(m) = &c.callee else {
            return Err(match &c.callee {
                Expression::CallExpression(_) => "higher_order",
                Expression::Super(_) => "inheritance",
                e if wrapped(e) => "wrapped_target",
                _ => "unknown_receiver",
            });
        };
        if m.optional {
            return Err("optional");
        }
        match &m.object {
            Expression::ThisExpression(_) => {}
            Expression::Super(_) => return Err("inheritance"),
            Expression::CallExpression(_) => return Err("higher_order"),
            Expression::StaticMemberExpression(receiver)
                if !self.static_
                    && matches!(receiver.object, Expression::ThisExpression(_))
                    && self
                        .facts
                        .parameters
                        .contains(receiver.property.name.as_str()) =>
            {
                return Err("injected_receiver");
            }
            e if wrapped(e) => return Err("wrapped_target"),
            _ => return Err("unknown_receiver"),
        }
        let key = (self.static_, m.property.name.to_string());
        if self.ambiguous
            || self.facts.all_blocked.contains(&self.static_)
            || self.facts.blocked.contains(&key)
            || self.facts.methods.get(&key).is_some_and(|n| *n != 1)
        {
            return Err("ambiguous_target");
        }
        if !self.facts.methods.contains_key(&key) {
            return Err(if self.facts.inherited {
                "inheritance"
            } else {
                "missing_method"
            });
        }
        Ok(GraphBuilder::method_id(
            self.owner,
            staticness(self.static_),
            m.property.name.as_str(),
        ))
    }
}
fn wrapped(e: &Expression<'_>) -> bool {
    matches!(
        e,
        Expression::ParenthesizedExpression(_)
            | Expression::TSAsExpression(_)
            | Expression::TSTypeAssertion(_)
            | Expression::TSNonNullExpression(_)
            | Expression::TSSatisfiesExpression(_)
            | Expression::TSInstantiationExpression(_)
            | Expression::ChainExpression(_)
    )
}
impl<'a> Visit<'a> for BodyCollector<'_> {
    fn visit_class(&mut self, class: &Class<'a>) {
        visit_class_definition_expressions(self, class);
    }
    fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
        self.nested += 1;
        walk::walk_function(self, f, flags);
        self.nested -= 1;
    }
    fn visit_arrow_function_expression(&mut self, f: &ArrowFunctionExpression<'a>) {
        self.nested += 1;
        walk::walk_arrow_function_expression(self, f);
        self.nested -= 1;
    }
    fn visit_call_expression(&mut self, c: &CallExpression<'a>) {
        self.result.record(
            self.caller,
            SourceSite {
                file: self.file.into(),
                start: c.span.start,
                end: c.span.end,
                line: line_at(self.source, c.span.start),
            },
            self.target(c),
        );
        walk::walk_call_expression(self, c);
    }
}
