//! Snapshot-local import bindings. This is not the complete analysis pipeline.
use crate::{
    Confidence, Diagnostic, EdgeKind, Evidence, EvidenceSource, GraphBuilder, GraphEdge, GraphNode,
    NodeKind, Severity,
    discovery::{RepositoryRoot, discover},
    typescript::{LexicalScope, line_at},
};
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Class, ClassType, IdentifierReference, MethodDefinition, MethodDefinitionKind,
    TSExternalModuleDeclaration, TSGlobalDeclaration, TSInterfaceDeclaration,
    TSNamespaceDeclaration,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_resolver::{
    FileMetadata, FileSystem, ResolveError, ResolveOptions, ResolverGeneric, TsConfig,
};
use oxc_semantic::{Scoping, SemanticBuilder, SymbolId};
use oxc_span::{SourceType, Span};
use oxc_syntax::{
    module_record::{ExportExportName, ImportImportName},
    scope::{ScopeFlags, ScopeId},
};
use std::{
    cell::Cell,
    collections::{BTreeMap, HashMap},
    fs, io,
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSite {
    pub file: String,
    pub start: u32,
    pub end: u32,
    pub line: u64,
}
impl SourceSite {
    fn new(file: &str, source: &str, span: Span) -> Self {
        Self {
            file: file.into(),
            start: span.start,
            end: span.end,
            line: line_at(source, span.start),
        }
    }
    fn evidence(&self) -> Evidence {
        Evidence {
            source: EvidenceSource::Resolver,
            file: self.file.clone(),
            line: Some(self.line),
            end_line: None,
            confidence: Confidence::Confirmed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnresolvedReason {
    MissingModule,
    MissingExport,
    UnsupportedExport,
    UnsupportedImport,
    UnsupportedConfig,
    BoundaryViolation,
    ParseIncomplete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    LocalSymbol {
        id: String,
        file: String,
        declaration: SourceSite,
        kind: NodeKind,
        export_type_only: bool,
    },
    /// No runtime class claim: only the original import specifier and export name.
    ExternalSymbol {
        id: String,
        specifier: String,
        exported_name: String,
        package_root: String,
    },
    Unresolved {
        reason: UnresolvedReason,
    },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportFinding {
    pub site: SourceSite,
    pub specifier: String,
    pub exported_name: String,
    pub local_name: Option<String>,
    pub type_only: bool,
    pub resolution: Resolution,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceFinding {
    pub site: SourceSite,
    pub consumer_id: Option<String>,
    // Kept with the reference so file/offset lookups never re-resolve a spelling.
    import: ImportFinding,
}
// Source declarations remain observable even when their symbol cannot be resolved uniquely.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DeclarationSite {
    pub id: String,
    pub ambiguous: bool,
    pub class_value: bool,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ImportResolver {
    local_references: BTreeMap<(String, u32), (String, NodeKind)>,
    type_only_references: BTreeMap<(String, u32), usize>,
    declarations: BTreeMap<(String, u32), DeclarationSite>,
    sources: BTreeMap<String, String>,
    imports: Vec<ImportFinding>,
    references: Vec<ReferenceFinding>,
    diagnostics: Vec<Diagnostic>,
}
impl ImportResolver {
    /// Exact source snapshot used to establish semantic reference bindings.
    pub(crate) fn local_reference(&self, file: &str, start: u32) -> Option<&(String, NodeKind)> {
        self.local_references.get(&(file.to_owned(), start))
    }
    pub(crate) fn declaration_at(&self, file: &str, start: u32) -> Option<&DeclarationSite> {
        self.declarations.get(&(file.to_owned(), start))
    }
    /// Diagnostic-only lexical type binding at an unresolved value reference.
    /// This must never establish a runtime decorator or create an import edge.
    pub(crate) fn type_only_reference(&self, file: &str, start: u32) -> Option<&ImportFinding> {
        self.type_only_references
            .get(&(file.to_owned(), start))
            .map(|index| &self.imports[*index])
    }
    pub(crate) fn is_class_value(&self, id: &str) -> bool {
        self.declarations
            .values()
            .any(|d| d.id == id && d.class_value && !d.ambiguous)
    }
    pub fn sources(&self) -> impl Iterator<Item = (&str, &str)> {
        self.sources
            .iter()
            .map(|(file, source)| (file.as_str(), source.as_str()))
    }
    pub fn source(&self, file: &str) -> Option<&str> {
        self.sources.get(file).map(String::as_str)
    }

    pub fn imports(&self) -> &[ImportFinding] {
        &self.imports
    }
    pub fn references(&self) -> impl Iterator<Item = &ReferenceFinding> {
        self.references.iter()
    }
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
    pub fn import_for<'a>(&self, reference: &'a ReferenceFinding) -> &'a ImportFinding {
        &reference.import
    }
    /// Byte offset must come from this snapshot's source, not a name search.
    pub fn at_reference(&self, file: &str, start: u32) -> Option<&ReferenceFinding> {
        // Lower bound preserves the previous first-match behavior for equal keys.
        let index = self
            .references
            .partition_point(|r| (r.site.file.as_str(), r.site.start) < (file, start));
        self.references
            .get(index)
            .filter(|r| r.site.file == file && r.site.start == start)
    }

    pub fn analyze(root: &RepositoryRoot) -> Result<Self, String> {
        let discovered = discover(root)?;
        let mut result = Self {
            sources: BTreeMap::new(),
            local_references: BTreeMap::new(),
            type_only_references: BTreeMap::new(),
            declarations: BTreeMap::new(),
            imports: vec![],
            references: vec![],
            diagnostics: discovered.diagnostics,
        };
        let filesystem = RootFileSystem {
            root: root.clone(),
            denied: Arc::new(AtomicBool::new(false)),
        };
        let unsupported_config = check_config(
            root,
            &filesystem,
            discovered.tsconfig.as_deref(),
            &mut result.diagnostics,
        )?;
        let module_resolver = ResolverGeneric::new_with_file_system(
            filesystem.clone(),
            ResolveOptions {
                extensions: vec![".ts".into(), ".tsx".into()],
                modules: vec![],
                main_fields: vec![],
                exports_fields: vec![],
                imports_fields: vec![],
                node_path: false,
                ..ResolveOptions::default()
            },
        );
        let mut files = BTreeMap::new();
        for file in discovered.files {
            let path = root.resolve(&file)?;
            let source = fs::read_to_string(path)
                .map_err(|_| "resolver source cannot be read".to_owned())?;
            files.insert(file.clone(), parse_file(&file, &source));
            result.sources.insert(file, source);
        }
        for (file, facts) in &files {
            result.diagnostics.extend(facts.diagnostics.clone());
            for (start, declaration) in &facts.declaration_sites {
                result
                    .declarations
                    .insert((file.clone(), *start), declaration.clone());
            }
            for reference in &facts.references {
                if let Some(decl) = facts.declarations.get(&reference.symbol) {
                    result.local_references.insert(
                        (file.clone(), reference.site.start),
                        (decl.id.clone(), decl.kind),
                    );
                }
            }
            for (site, specifier) in &facts.reexports {
                let reason = if is_relative(specifier)
                    && checked_request(&filesystem, file, specifier).is_err()
                {
                    UnresolvedReason::BoundaryViolation
                } else {
                    UnresolvedReason::UnsupportedExport
                };
                result.diagnostics.push(diagnostic(site, &reason));
            }
            for raw in &facts.imports {
                let resolution = resolve_target(
                    root,
                    &filesystem,
                    &module_resolver,
                    &files,
                    file,
                    raw,
                    unsupported_config,
                );
                let type_only = raw.type_only
                    || matches!(
                        &resolution,
                        Resolution::LocalSymbol {
                            export_type_only: true,
                            ..
                        }
                    );
                let finding = ImportFinding {
                    site: raw.site.clone(),
                    specifier: raw.specifier.clone(),
                    exported_name: raw.exported.clone(),
                    local_name: raw.local.clone(),
                    type_only,
                    resolution,
                };
                if let Resolution::Unresolved { reason } = &finding.resolution {
                    result.diagnostics.push(diagnostic(&finding.site, reason));
                }
                for reference in facts
                    .references
                    .iter()
                    .filter(|r| Some(r.symbol) == raw.symbol)
                {
                    result.references.push(ReferenceFinding {
                        site: reference.site.clone(),
                        consumer_id: reference.consumer.clone(),
                        import: finding.clone(),
                    });
                }
                if finding.type_only {
                    for (site, symbol) in &facts.missing_value_references {
                        if Some(*symbol) == raw.symbol {
                            result
                                .type_only_references
                                .insert((file.clone(), site.start), result.imports.len());
                        }
                    }
                }
                result.imports.push(finding);
            }
        }
        result
            .references
            .sort_by_key(|r| (r.site.file.clone(), r.site.start));
        Ok(result)
    }

    /// Apply after generic declarations. Never creates local declarations or call metadata.
    pub fn apply_imports(&self, builder: &mut GraphBuilder) -> Result<(), String> {
        for d in &self.diagnostics {
            builder.add_diagnostic(d.clone())?;
        }
        for reference in &self.references {
            let Some(consumer) = &reference.consumer_id else {
                continue;
            };
            let import = &reference.import;
            let target = match &import.resolution {
                Resolution::LocalSymbol { id, .. } => id,
                Resolution::ExternalSymbol {
                    id, exported_name, ..
                } => {
                    builder.add_node(GraphNode {
                        id: id.clone(),
                        kind: NodeKind::ExternalDependency,
                        name: exported_name.clone(),
                        qualified_name: None,
                        file: None,
                        line: None,
                        end_line: None,
                        parent_id: None,
                        evidence: vec![import.site.evidence()],
                        metadata: None,
                    })?;
                    id
                }
                Resolution::Unresolved { .. } => continue,
            };
            builder.add_edge(GraphEdge {
                id: GraphBuilder::edge_id(consumer, EdgeKind::Imports, target),
                from: consumer.clone(),
                to: target.clone(),
                kind: EdgeKind::Imports,
                evidence: vec![reference.site.evidence()],
                metadata: None,
            })?;
        }
        Ok(())
    }
}
fn diagnostic(site: &SourceSite, reason: &UnresolvedReason) -> Diagnostic {
    Diagnostic {
        code: format!("TS_IMPORT_{reason:?}").to_uppercase(),
        severity: if matches!(
            reason,
            UnresolvedReason::BoundaryViolation | UnresolvedReason::ParseIncomplete
        ) {
            Severity::Error
        } else {
            Severity::Warning
        },
        message: match reason {
            UnresolvedReason::MissingModule => {
                "Import module was not resolved inside the repository"
            }
            UnresolvedReason::MissingExport => "Requested export was not found",
            UnresolvedReason::UnsupportedExport => {
                "Export does not identify a supported local class or interface"
            }
            UnresolvedReason::UnsupportedImport => "This import form is not supported",
            UnresolvedReason::UnsupportedConfig => {
                "Resolution-affecting tsconfig options are not supported"
            }
            UnresolvedReason::BoundaryViolation => {
                "Reference crosses the repository or excluded-source boundary"
            }
            UnresolvedReason::ParseIncomplete => "Source parsing or semantic analysis failed",
        }
        .into(),
        file: Some(site.file.clone()),
        line: Some(site.line),
        related_node_id: None,
        skipped_count: None,
    }
}
#[derive(Clone)]
struct RawImport {
    site: SourceSite,
    specifier: String,
    exported: String,
    local: Option<String>,
    type_only: bool,
    symbol: Option<SymbolId>,
    unsupported: bool,
}
#[derive(Clone)]
struct Declaration {
    id: String,
    site: SourceSite,
    kind: NodeKind,
}
struct RawReference {
    site: SourceSite,
    symbol: SymbolId,
    consumer: Option<String>,
}
#[derive(Default)]
struct FileFacts {
    declarations: HashMap<SymbolId, Declaration>,
    declaration_sites: BTreeMap<u32, DeclarationSite>,
    imports: Vec<RawImport>,
    references: Vec<RawReference>,
    missing_value_references: Vec<(SourceSite, SymbolId)>,
    exports: BTreeMap<String, Option<(Declaration, bool)>>,
    reexports: Vec<(SourceSite, String)>,
    diagnostics: Vec<Diagnostic>,
    failed: bool,
}
struct Collector<'s> {
    file: &'s str,
    source: &'s str,
    scoping: &'s Scoping,
    scope: LexicalScope,
    consumers: Vec<Option<String>>,
    classes: Vec<Option<String>>,
    ambient: bool,
    declarations: HashMap<SymbolId, Declaration>,
    declaration_sites: BTreeMap<u32, DeclarationSite>,
    references: Vec<RawReference>,
    missing_value_references: Vec<(SourceSite, SymbolId)>,
}
impl Collector<'_> {
    fn declaration(
        &mut self,
        symbol: Option<SymbolId>,
        name: &str,
        kind: NodeKind,
        span: Span,
    ) -> Option<String> {
        let symbol = symbol?;
        let scope: Vec<_> = self.scope.path.iter().map(String::as_str).collect();
        let id = GraphBuilder::node_id(kind, self.file, &scope, name).ok()?;
        let ambiguous = !self.scoping.symbol_redeclarations(symbol).is_empty();
        self.declaration_sites.insert(
            span.start,
            DeclarationSite {
                id: id.clone(),
                ambiguous,
                class_value: false,
            },
        );
        // Merged symbols have no unique reference target, regardless of declaration order.
        if !ambiguous {
            self.declarations.insert(
                symbol,
                Declaration {
                    id: id.clone(),
                    site: SourceSite::new(self.file, self.source, span),
                    kind,
                },
            );
        }
        Some(id)
    }
}
impl<'a> Visit<'a> for Collector<'_> {
    fn enter_scope(&mut self, flags: ScopeFlags, _: &Cell<Option<ScopeId>>) {
        self.scope.enter(flags);
    }
    fn leave_scope(&mut self) {
        self.scope.leave();
    }
    fn visit_ts_namespace_declaration(&mut self, n: &TSNamespaceDeclaration<'a>) {
        let ambient = self.ambient;
        self.ambient |= n.declare;
        self.scope.path.push(n.id.name.to_string());
        walk::walk_ts_namespace_declaration(self, n);
        self.scope.path.pop();
        self.ambient = ambient;
    }
    fn visit_ts_external_module_declaration(&mut self, n: &TSExternalModuleDeclaration<'a>) {
        let ambient = self.ambient;
        self.ambient = true;
        walk::walk_ts_external_module_declaration(self, n);
        self.ambient = ambient;
    }
    fn visit_ts_global_declaration(&mut self, n: &TSGlobalDeclaration<'a>) {
        let ambient = self.ambient;
        self.ambient = true;
        walk::walk_ts_global_declaration(self, n);
        self.ambient = ambient;
    }
    fn visit_class(&mut self, c: &Class<'a>) {
        let id = if c.r#type == ClassType::ClassDeclaration {
            c.id.as_ref().and_then(|id| {
                self.declaration(
                    id.symbol_id.get(),
                    id.name.as_str(),
                    NodeKind::Class,
                    c.span,
                )
            })
        } else {
            None
        };
        if let Some(declaration) = self.declaration_sites.get_mut(&c.span.start) {
            declaration.class_value = !c.declare && !self.ambient;
        }
        self.classes.push(id.clone());
        self.consumers.push(id);
        walk::walk_class(self, c);
        self.consumers.pop();
        self.classes.pop();
    }
    fn visit_ts_interface_declaration(&mut self, i: &TSInterfaceDeclaration<'a>) {
        let id = self.declaration(
            i.id.symbol_id.get(),
            i.id.name.as_str(),
            NodeKind::Interface,
            i.span,
        );
        self.consumers.push(id);
        walk::walk_ts_interface_declaration(self, i);
        self.consumers.pop();
    }
    fn visit_method_definition(&mut self, m: &MethodDefinition<'a>) {
        let owner = self.classes.last().cloned().flatten();
        let id = if m.kind == MethodDefinitionKind::Constructor {
            owner
        } else {
            owner.and_then(|owner| {
                m.key.static_name().map(|name| {
                    GraphBuilder::method_id(
                        &owner,
                        if m.r#static { "static" } else { "instance" },
                        &name,
                    )
                })
            })
        };
        self.consumers.push(id);
        walk::walk_method_definition(self, m);
        self.consumers.pop();
    }
    fn visit_identifier_reference(&mut self, i: &IdentifierReference<'a>) {
        let Some(reference_id) = i.reference_id.get() else {
            return;
        };
        let reference = self.scoping.get_reference(reference_id);
        if let Some(symbol) = reference.symbol_id() {
            self.references.push(RawReference {
                site: SourceSite::new(self.file, self.source, i.span),
                symbol,
                consumer: self.consumers.last().cloned().flatten(),
            });
        } else if let Some(symbol) = self.scoping.find_binding(reference.scope_id(), i.name) {
            // A type-only import can be lexically visible but invalid as a value.
            // Retain the exact scoped site for diagnostics, without resolving it.
            self.missing_value_references
                .push((SourceSite::new(self.file, self.source, i.span), symbol));
        }
    }
}
fn parse_file(file: &str, source: &str) -> FileFacts {
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
    let mut facts = FileFacts::default();
    if parsed.panicked || !parsed.diagnostics.is_empty() {
        facts.failed = true;
    }
    let semantic = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !semantic.diagnostics.is_empty() {
        facts.failed = true;
    }
    if facts.failed {
        facts.diagnostics.push(diagnostic(
            &SourceSite::new(file, source, Span::new(0, 0)),
            &UnresolvedReason::ParseIncomplete,
        ));
        return facts;
    }
    let scoping = semantic.semantic.scoping();
    let mut collector = Collector {
        file,
        source,
        scoping,
        scope: LexicalScope::default(),
        consumers: vec![],
        classes: vec![],
        ambient: file.ends_with(".d.ts"),
        declarations: HashMap::new(),
        declaration_sites: BTreeMap::new(),
        references: vec![],
        missing_value_references: vec![],
    };
    collector.visit_program(&parsed.program);
    facts.references = collector.references;
    facts.missing_value_references = collector.missing_value_references;
    for entry in &parsed.module_record.import_entries {
        let (exported, unsupported) = match &entry.import_name {
            ImportImportName::Name(n) => (n.name.to_string(), false),
            ImportImportName::Default(_) => ("default".into(), true),
            ImportImportName::NamespaceObject => ("*".into(), true),
        };
        facts.imports.push(RawImport {
            site: SourceSite::new(file, source, entry.local_name.span),
            specifier: entry.module_request.name.to_string(),
            exported,
            local: Some(entry.local_name.name.to_string()),
            type_only: entry.is_type,
            symbol: scoping.get_root_binding(entry.local_name.name.as_str().into()),
            unsupported,
        });
    }
    for entry in &parsed.module_record.local_export_entries {
        let ExportExportName::Name(exported) = &entry.export_name else {
            continue;
        };
        let symbol = entry
            .local_name
            .name()
            .and_then(|local| scoping.get_root_binding(local.as_str().into()));
        if symbol.is_none() {
            facts.failed = true;
            facts.diagnostics.push(diagnostic(
                &SourceSite::new(file, source, exported.span),
                &UnresolvedReason::ParseIncomplete,
            ));
        }
        if symbol.is_some_and(|symbol| {
            !scoping.symbol_redeclarations(symbol).is_empty()
                || facts
                    .imports
                    .iter()
                    .any(|import| import.symbol == Some(symbol))
        }) {
            facts.diagnostics.push(diagnostic(
                &SourceSite::new(file, source, exported.span),
                &UnresolvedReason::UnsupportedExport,
            ));
        }
        // A real export of a function/variable is distinct from a missing export.
        let declaration = symbol.and_then(|symbol| collector.declarations.get(&symbol));
        facts.exports.insert(
            exported.name.to_string(),
            declaration.map(|decl| (decl.clone(), entry.is_type)),
        );
    }
    for entry in parsed
        .module_record
        .indirect_export_entries
        .iter()
        .chain(parsed.module_record.star_export_entries.iter())
    {
        if let Some(request) = &entry.module_request {
            facts.reexports.push((
                SourceSite::new(file, source, entry.span),
                request.name.to_string(),
            ));
        }
    }
    // Side-effect imports have no binding or representable consumer, but remain visible.
    for (request, occurrences) in &parsed.module_record.requested_modules {
        for occurrence in occurrences {
            if occurrence.is_import
                && !parsed
                    .module_record
                    .import_entries
                    .iter()
                    .any(|entry| entry.module_request.span == occurrence.span)
            {
                facts.imports.push(RawImport {
                    site: SourceSite::new(file, source, occurrence.span),
                    specifier: request.to_string(),
                    exported: String::new(),
                    local: None,
                    type_only: occurrence.is_type,
                    symbol: None,
                    unsupported: true,
                });
            }
        }
    }
    facts.declarations = collector.declarations;
    facts.declaration_sites = collector.declaration_sites;
    facts.imports.sort_by_key(|i| i.site.start);
    facts
}

fn is_relative(specifier: &str) -> bool {
    specifier.starts_with("./") || specifier.starts_with("../")
}
// External identities are not repository paths and never trigger filesystem reads.
fn is_external_specifier(specifier: &str) -> bool {
    let name = if let Some(name) = specifier.strip_prefix("node:") {
        if !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '/'))
        {
            return false;
        }
        name
    } else {
        specifier
    };
    !name.is_empty()
        && !name.starts_with('#')
        && !name
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || matches!(c, ':' | '\\' | '?' | '%'))
        && name.split('/').all(|part| !matches!(part, "" | "." | ".."))
}
fn resolve_target(
    root: &RepositoryRoot,
    filesystem: &RootFileSystem,
    resolver: &ResolverGeneric<RootFileSystem>,
    files: &BTreeMap<String, FileFacts>,
    file: &str,
    raw: &RawImport,
    config: ConfigImpact,
) -> Resolution {
    let unresolved = |reason| Resolution::Unresolved { reason };
    if files[file].failed || (raw.local.is_some() && raw.symbol.is_none()) {
        return unresolved(UnresolvedReason::ParseIncomplete);
    }
    if is_relative(&raw.specifier) {
        if checked_request(filesystem, file, &raw.specifier).is_err() {
            return unresolved(UnresolvedReason::BoundaryViolation);
        }
        if config.relative {
            return unresolved(UnresolvedReason::UnsupportedConfig);
        }
        // Keep denied filesystem probes observable even for repeated unresolved requests.
        resolver.clear_cache();
        filesystem.denied.store(false, Ordering::Relaxed);
        let resolved = resolver.resolve_file(root.path().join(file), &raw.specifier);
        if resolved.is_err() && filesystem.denied.load(Ordering::Relaxed) {
            return unresolved(UnresolvedReason::BoundaryViolation);
        }
        let Ok(resolved) = resolved else {
            return unresolved(UnresolvedReason::MissingModule);
        };
        let Ok(target) = root.relative_path(resolved.path()) else {
            return unresolved(UnresolvedReason::BoundaryViolation);
        };
        let Some(target_facts) = files.get(&target) else {
            return unresolved(UnresolvedReason::MissingModule);
        };
        if target_facts.failed {
            return unresolved(UnresolvedReason::ParseIncomplete);
        }
        if raw.unsupported {
            return unresolved(UnresolvedReason::UnsupportedImport);
        }
        return match target_facts.exports.get(&raw.exported) {
            Some(Some((declaration, type_only))) => Resolution::LocalSymbol {
                id: declaration.id.clone(),
                file: target,
                declaration: declaration.site.clone(),
                kind: declaration.kind,
                export_type_only: *type_only,
            },
            Some(None) => unresolved(UnresolvedReason::UnsupportedExport),
            None => unresolved(if target_facts.reexports.is_empty() {
                UnresolvedReason::MissingExport
            } else {
                UnresolvedReason::UnsupportedExport
            }),
        };
    }
    if !is_external_specifier(&raw.specifier) {
        return unresolved(UnresolvedReason::BoundaryViolation);
    }
    // Validated explicit node: identities cannot be redirected by repository aliases.
    if config.bare && !raw.specifier.starts_with("node:") {
        return unresolved(UnresolvedReason::UnsupportedConfig);
    }
    if raw.unsupported {
        return unresolved(UnresolvedReason::UnsupportedImport);
    }
    let parts: Vec<_> = raw.specifier.split('/').collect();
    let package_root = if raw.specifier.starts_with('@') {
        if parts.len() < 2 || parts[0].len() == 1 {
            return unresolved(UnresolvedReason::UnsupportedImport);
        }
        parts[..2].join("/")
    } else {
        parts[0].into()
    };
    Resolution::ExternalSymbol {
        id: GraphBuilder::external_id(&raw.specifier, &raw.exported),
        specifier: raw.specifier.clone(),
        exported_name: raw.exported.clone(),
        package_root,
    }
}

fn normalized(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                result.pop();
            }
            Component::CurDir => {}
            other => result.push(other.as_os_str()),
        }
    }
    result
}
#[derive(Clone)]
struct RootFileSystem {
    root: RepositoryRoot,
    denied: Arc<AtomicBool>,
}
impl RootFileSystem {
    fn candidate(&self, path: &Path) -> io::Result<PathBuf> {
        let path = normalized(path);
        let denied = || io::Error::new(io::ErrorKind::PermissionDenied, "repository boundary");
        let relative = path.strip_prefix(self.root.path()).map_err(|_| denied())?;
        if crate::discovery::logical_excluded(relative) {
            return Err(denied());
        }
        // Check the nearest existing ancestor too: missing leaves can hide an
        // outside-root directory symlink. No source/config bytes are read here.
        let mut ancestor = path.as_path();
        loop {
            match fs::symlink_metadata(ancestor) {
                Ok(_) => break,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    ancestor = ancestor.parent().ok_or_else(denied)?;
                }
                Err(e) => return Err(e),
            }
        }
        let canonical = fs::canonicalize(ancestor)?;
        if canonical != self.root.path() {
            let relative = self.root.relative_path(&canonical).map_err(|_| denied())?;
            if crate::discovery::logical_excluded(Path::new(&relative)) {
                return Err(denied());
            }
        }
        Ok(path)
    }
    fn checked(&self, path: &Path) -> io::Result<PathBuf> {
        // Module resolution needs no package source/metadata in the supported
        // relative-only mode. Stop package ancestry probing at this boundary.
        if path.file_name().is_some_and(|n| n == "package.json") {
            return Err(io::ErrorKind::NotFound.into());
        }
        self.candidate(path).inspect_err(|e| {
            if e.kind() == io::ErrorKind::PermissionDenied {
                self.denied.store(true, Ordering::Relaxed);
            }
        })
    }
}
impl FileSystem for RootFileSystem {
    fn new() -> Self {
        panic!("RootFileSystem requires an explicit RepositoryRoot")
    }
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        fs::read(self.checked(path)?)
    }
    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        fs::read_to_string(self.checked(path)?)
    }
    fn metadata(&self, path: &Path) -> io::Result<FileMetadata> {
        Ok(fs::metadata(self.checked(path)?)?.into())
    }
    fn symlink_metadata(&self, path: &Path) -> io::Result<FileMetadata> {
        Ok(fs::symlink_metadata(self.checked(path)?)?.into())
    }
    fn read_link(&self, path: &Path) -> Result<PathBuf, ResolveError> {
        Ok(fs::read_link(self.checked(path)?)?)
    }
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        fs::canonicalize(self.checked(path)?)
    }
}
fn checked_request(
    filesystem: &RootFileSystem,
    file: &str,
    specifier: &str,
) -> io::Result<PathBuf> {
    if specifier.contains('\\') || specifier.chars().any(char::is_control) {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let parent = filesystem.root.path().join(file);
    filesystem.candidate(
        &parent
            .parent()
            .unwrap_or(filesystem.root.path())
            .join(specifier),
    )
}

#[derive(Clone, Copy, Default)]
struct ConfigImpact {
    bare: bool,
    relative: bool,
}

fn check_config(
    root: &RepositoryRoot,
    filesystem: &RootFileSystem,
    file: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<ConfigImpact, String> {
    let Some(file) = file else {
        return Ok(ConfigImpact::default());
    };
    let path = root.resolve(file)?;
    let source =
        fs::read_to_string(&path).map_err(|_| "resolver config cannot be read".to_owned())?;
    let site = SourceSite::new(file, &source, Span::new(0, 0));
    // Oxc 11.24.3 drops moduleSuffixes during typed deserialization. Inspect
    // the same JSONC structurally before that information is lost.
    let mut json = source.trim_start_matches('\u{feff}').as_bytes().to_vec();
    json_strip_comments::strip_slice(&mut json)
        .map_err(|_| "resolver config cannot be parsed".to_owned())?;
    let raw: serde_json::Value = if json.iter().all(u8::is_ascii_whitespace) {
        serde_json::json!({})
    } else {
        serde_json::from_slice(&json).map_err(|_| "resolver config cannot be parsed".to_owned())?
    };
    let module_suffixes = raw
        .get("compilerOptions")
        .and_then(serde_json::Value::as_object)
        .is_some_and(|options| options.contains_key("moduleSuffixes"));
    let config = TsConfig::parse(true, &path, &path, source)
        .map_err(|_| "resolver config cannot be parsed".to_owned())?;
    // Unevaluated inherited/project settings can change relative lookup too.
    // paths/baseUrl alone affect bare specifiers, not the supported relative lookup.
    let relative = config.extends.is_some()
        || !config.references.is_empty()
        || config.compiler_options.root_dirs.is_some()
        || module_suffixes;
    let impact = ConfigImpact {
        relative,
        bare: relative
            || config.compiler_options.base_url.is_some()
            || config.compiler_options.paths.is_some(),
    };
    if !impact.bare {
        return Ok(ConfigImpact::default());
    }
    // Do not follow unsupported config chains. Validate their direct path
    // boundaries without reading referenced configs or sources.
    let mut candidates: Vec<PathBuf> = config
        .references
        .iter()
        .map(|r| root.path().join(&r.path))
        .collect();
    if let Some(base) = &config.compiler_options.base_url {
        candidates.push(root.path().join(base));
    }
    if let Some(paths) = &config.compiler_options.paths {
        let base = root.path().join(
            config
                .compiler_options
                .base_url
                .as_deref()
                .unwrap_or(Path::new(".")),
        );
        for paths in paths.values() {
            for path in paths {
                candidates.push(base.join(path.to_string_lossy().split('*').next().unwrap_or("")));
            }
        }
    }
    if let Some(extends) = &config.extends {
        use oxc_resolver::ExtendsField;
        let values: Vec<&str> = match extends {
            ExtendsField::Single(s) => vec![s],
            ExtendsField::Multiple(v) => v.iter().map(String::as_str).collect(),
        };
        for value in values {
            if is_relative(value) || Path::new(value).is_absolute() {
                candidates.push(root.path().join(value));
            }
        }
    }
    let violation = candidates.iter().any(|p| filesystem.candidate(p).is_err());
    diagnostics.push(diagnostic(
        &site,
        &if violation {
            UnresolvedReason::BoundaryViolation
        } else {
            UnresolvedReason::UnsupportedConfig
        },
    ));
    Ok(impact)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn filesystem_rejects_outside_and_excluded_bytes_before_reading() {
        let base = std::env::temp_dir().join(format!("resolver-fs-{}", std::process::id()));
        fs::create_dir_all(base.join("root/node_modules")).unwrap();
        fs::write(base.join("outside.ts"), [0xff]).unwrap();
        fs::write(base.join("root/node_modules/hidden.ts"), [0xff]).unwrap();
        std::os::unix::fs::symlink(base.join("outside.ts"), base.join("root/link.ts")).unwrap();
        let root = RepositoryRoot::open(&base.join("root")).unwrap();
        let filesystem = RootFileSystem {
            root: root.clone(),
            denied: Arc::new(AtomicBool::new(false)),
        };
        for path in [
            base.join("outside.ts"),
            root.path().join("link.ts"),
            root.path().join("node_modules/hidden.ts"),
        ] {
            assert_eq!(
                filesystem.read(&path).unwrap_err().kind(),
                io::ErrorKind::PermissionDenied
            );
            assert_eq!(
                filesystem.read_to_string(&path).unwrap_err().kind(),
                io::ErrorKind::PermissionDenied
            );
        }
        fs::remove_dir_all(base).unwrap();
    }
}

#[cfg(test)]
mod reference_lookup_tests {
    use super::*;

    #[test]
    fn duplicate_reference_keys_keep_first_match() {
        let path =
            std::env::temp_dir().join(format!("reference-duplicates-{}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        fs::write(
            path.join("main.ts"),
            "import {Token} from 'pkg'; class C { value = Token; }",
        )
        .unwrap();
        let mut resolver = ImportResolver::analyze(&RepositoryRoot::open(&path).unwrap()).unwrap();
        fs::remove_dir_all(&path).unwrap();
        let first = resolver.references[0].clone();
        let mut duplicate = first.clone();
        duplicate.consumer_id = None;
        resolver.references.insert(1, duplicate);
        assert_eq!(
            resolver.at_reference(&first.site.file, first.site.start),
            Some(&first)
        );
        assert_eq!(resolver.references().count(), 2);
    }

    #[test]
    fn merged_declaration_sites_are_retained_but_not_reference_targets() {
        for (index, declarations) in [
            "@Module({providers:[Service]}) class FeatureModule {} interface FeatureModule {}",
            "interface FeatureModule {} @Module({providers:[Service]}) class FeatureModule {}",
        ]
        .iter()
        .enumerate()
        {
            let path =
                std::env::temp_dir().join(format!("module-sites-{}-{index}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            let source = format!(
                "import {{Module}} from '@nestjs/common'; class Service {{}} {declarations}"
            );
            fs::write(path.join("main.ts"), &source).unwrap();
            let resolver = ImportResolver::analyze(&RepositoryRoot::open(&path).unwrap()).unwrap();
            let start = source.find("@Module").unwrap() as u32;
            let site = resolver.declaration_at("main.ts", start).unwrap();
            assert!(site.ambiguous);
            assert_eq!(
                site.id,
                GraphBuilder::node_id(NodeKind::Class, "main.ts", &[], "FeatureModule").unwrap()
            );
            let facts = parse_file("main.ts", &source);
            assert!(!facts.declarations.values().any(|d| d.id == site.id));
            fs::remove_dir_all(&path).unwrap();
        }
    }
}

#[cfg(test)]
mod class_value_tests {
    use super::*;

    #[test]
    fn class_value_requires_nonambient_source_and_restores_namespace_context() {
        for (file, source, values) in [
            (
                "main.ts",
                "abstract class Abstract {} declare class Ambient {} class Concrete {}",
                vec![true, false, true],
            ),
            ("main.d.ts", "class Ambient {}", vec![false]),
            (
                "main.ts",
                "declare namespace N { namespace Inner { class Ambient {} } } namespace M { class Concrete {} }",
                vec![false, true],
            ),
            (
                "main.ts",
                "declare module 'pkg' { class Ambient {} } class Concrete {}",
                vec![false, true],
            ),
            (
                "main.ts",
                "declare global { class Ambient {} } class Concrete {}",
                vec![false, true],
            ),
        ] {
            let facts = parse_file(file, source);
            assert_eq!(
                facts
                    .declaration_sites
                    .values()
                    .map(|d| d.class_value)
                    .collect::<Vec<_>>(),
                values,
                "{source}"
            );
        }
    }
}
