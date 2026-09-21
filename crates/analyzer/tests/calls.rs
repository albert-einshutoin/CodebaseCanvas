use codebasecanvas_analyzer::{
    CallAnalysis, CallMode, CallScope, EdgeKind, GraphBuilder, GraphMetadata, SystemGraph, calls,
    discovery::RepositoryRoot, nestjs_roles, resolver::ImportResolver,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Repo(PathBuf);
impl Repo {
    fn new(source: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "canvas-calls-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let repo = Self(path);
        repo.write("main.ts", source);
        repo
    }
    fn write(&self, file: &str, source: &str) {
        fs::write(self.0.join(file), source).unwrap();
    }
    fn resolver(&self) -> ImportResolver {
        ImportResolver::analyze(&RepositoryRoot::open(&self.0).unwrap()).unwrap()
    }
}
impl Drop for Repo {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn builder(r: &ImportResolver) -> GraphBuilder {
    let mut b = GraphBuilder::new(GraphMetadata {
        analyzer_version: "calls-test".into(),
        analyzed_at: "2026-09-21T00:00:00Z".into(),
        root_name: None,
        call_analysis: CallAnalysis {
            scope: CallScope::ParsedNamedClassMethods,
            mode: CallMode::SameClassOnly,
            examined_calls: 0,
            emitted_calls: 0,
            skipped_calls: 0,
        },
    });
    for (file, _) in r.sources() {
        nestjs_roles::extract_file(file, r, &mut b).unwrap();
    }
    b
}
fn graph(r: &ImportResolver) -> SystemGraph {
    let mut b = builder(r);
    calls::analyze(r, &b).unwrap().apply(&mut b).unwrap();
    r.apply_imports(&mut b).unwrap();
    b.finish().unwrap()
}
#[test]
fn direct_calls_count_sites_before_edge_deduplication() {
    let repo = Repo::new(
        "class A { run() {\n this.find();\n this.find();\n } find() {} static find() {} static run() { this.find(); } }",
    );
    let g = graph(&repo.resolver());
    assert_eq!(g.metadata.call_analysis.examined_calls, 3);
    assert_eq!(g.metadata.call_analysis.emitted_calls, 3);
    assert_eq!(g.metadata.call_analysis.skipped_calls, 0);
    let edges: Vec<_> = g
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Calls)
        .collect();
    assert_eq!(edges.len(), 2);
    assert_eq!(edges.iter().map(|e| e.evidence.len()).sum::<usize>(), 3);
    assert_ne!(edges[0].to, edges[1].to);
}

fn call_edges(g: &SystemGraph) -> Vec<codebasecanvas_analyzer::GraphEdge> {
    g.edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Calls)
        .cloned()
        .collect()
}
fn counts(g: &SystemGraph) -> (u64, u64, u64) {
    let c = &g.metadata.call_analysis;
    (c.examined_calls, c.emitted_calls, c.skipped_calls)
}
fn method(file: &str, scope: &[&str], class: &str, staticness: &str, name: &str) -> String {
    GraphBuilder::method_id(
        &GraphBuilder::node_id(codebasecanvas_analyzer::NodeKind::Class, file, scope, class)
            .unwrap(),
        staticness,
        name,
    )
}
fn reasons(g: &SystemGraph) -> std::collections::BTreeMap<String, u64> {
    let mut counts = std::collections::BTreeMap::new();
    for d in &g.diagnostics {
        if let Some(n) = d.skipped_count {
            *counts.entry(d.code.clone()).or_default() += n;
        }
    }
    counts
}
fn pipeline(r: &ImportResolver, with_calls: bool, reverse: bool) -> SystemGraph {
    use codebasecanvas_analyzer::{nestjs_di, nestjs_modules, nestjs_routes};
    let mut b = builder(r);
    nestjs_modules::analyze(r, &b)
        .unwrap()
        .apply(&mut b)
        .unwrap();
    let routes = nestjs_routes::analyze(r, &b).unwrap();
    let di = nestjs_di::analyze(r, &b).unwrap();
    let calls = calls::analyze(r, &b).unwrap();
    if reverse {
        r.apply_imports(&mut b).unwrap();
        if with_calls {
            calls.apply(&mut b).unwrap();
        }
        di.apply(&mut b).unwrap();
        routes.apply(&mut b).unwrap();
    } else {
        routes.apply(&mut b).unwrap();
        di.apply(&mut b).unwrap();
        if with_calls {
            calls.apply(&mut b).unwrap();
        }
        r.apply_imports(&mut b).unwrap();
    }
    b.finish().unwrap()
}
#[test]
fn fixture_calls_and_existing_components_match_independent_oracle() {
    let root = RepositoryRoot::open(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/nestjs-sample"),
    )
    .unwrap();
    let r = ImportResolver::analyze(&root).unwrap();
    let actual = pipeline(&r, true, false);
    let expected = SystemGraph::from_json(include_str!(
        "../../../examples/nestjs-sample/expected-graph.json"
    ))
    .unwrap();
    assert_eq!(counts(&actual), (9, 2, 7));
    assert_eq!(
        actual.metadata.call_analysis,
        expected.metadata.call_analysis
    );
    assert_eq!(call_edges(&actual).len(), 2);
    assert_eq!(call_edges(&actual), call_edges(&expected));
    let diags = |g: &SystemGraph| {
        g.diagnostics
            .iter()
            .filter(|d| d.skipped_count.is_some())
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(diags(&actual), diags(&expected));
    let findings = calls::analyze(&r, &builder(&r)).unwrap();
    assert_eq!(findings.sites().len(), 9);
    for s in findings.sites() {
        assert!(
            r.source(&s.site.file).unwrap()[s.site.start as usize..s.site.end as usize]
                .contains('(')
        );
    }
    let baseline = pipeline(&r, false, false);
    assert_eq!(actual.nodes, baseline.nodes);
    assert_eq!(
        actual
            .edges
            .iter()
            .filter(|e| e.kind != EdgeKind::Calls)
            .cloned()
            .collect::<Vec<_>>(),
        baseline.edges
    );
    assert_eq!(
        actual
            .diagnostics
            .iter()
            .filter(|d| d.skipped_count.is_none())
            .cloned()
            .collect::<Vec<_>>(),
        baseline.diagnostics
    );
    let tuples = |g: &SystemGraph| {
        g.edges
            .iter()
            .map(|e| (e.from.clone(), format!("{:?}", e.kind), e.to.clone()))
            .collect::<std::collections::BTreeSet<_>>()
    };
    assert_eq!(tuples(&actual), tuples(&expected));
    for n in &actual.nodes {
        let oracle = expected.nodes.iter().find(|e| e.id == n.id).unwrap();
        assert_eq!((n.kind, &n.parent_id), (oracle.kind, &oracle.parent_id));
    }
    assert_eq!(actual, pipeline(&r, true, true));
}
#[test]
fn names_in_other_classes_files_and_scopes_never_resolve() {
    let repo = Repo::new(
        "class A { run(){ this.find(); } } class B { find(){} } namespace N { class A { find(){} run(){ this.find(); } } }",
    );
    repo.write("other.ts", "export class A { find(){} }");
    let g = graph(&repo.resolver());
    assert_eq!(counts(&g), (2, 1, 1));
    assert_eq!(
        call_edges(&g)[0].to,
        method("main.ts", &["N"], "A", "instance", "find")
    );
    assert_eq!(reasons(&g)["unsupported_call_missing_method"], 1);
}
#[test]
fn injected_tokens_and_provider_overrides_are_not_dispatch_targets() {
    let repo = Repo::new(
        "import {Injectable, Module} from '@nestjs/common'; class Token { find(){} } class Mock { find(){} } @Injectable() class A { constructor(private service: Token) {} run(){ this.service.find(); } } @Module({providers:[{provide:Token,useClass:Mock}]}) class M {}",
    );
    let g = pipeline(&repo.resolver(), true, false);
    assert_eq!(counts(&g), (1, 0, 1));
    assert_eq!(reasons(&g)["unsupported_call_injected_receiver"], 1);
    assert!(call_edges(&g).is_empty());
}
#[test]
fn unsupported_shapes_have_distinct_reasons_and_nested_precedence() {
    let repo = Repo::new(
        "class A { find(){} run(){\n this['find'](); unknown.find(); this.missing();\n (()=>this['find']())();\n function f(x = this.find()) { this.find(); }\n const arrow = () => this.find?.();\n this.find?.(); this?.find(); (this.find)(); (this as A).find();\n this.find()(); this.find().other(); super.find();\n } }",
    );
    let g = graph(&repo.resolver());
    // Each wrapper and nested call remains observable as a separate site.
    assert_eq!(g.metadata.call_analysis.examined_calls, 17);
    let reasons = reasons(&g);
    assert_eq!(reasons["unsupported_call_nested_function"], 4);
    assert_eq!(reasons["unsupported_call_computed_target"], 1);
    assert_eq!(reasons["unsupported_call_optional"], 2);
    assert_eq!(reasons["unsupported_call_wrapped_target"], 3);
    assert_eq!(reasons["unsupported_call_higher_order"], 2);
    assert_eq!(reasons["unsupported_call_inheritance"], 1);
}
#[test]
fn nested_arguments_and_callees_are_distinct_sites() {
    let repo = Repo::new(
        "class A { one(x?: unknown){} two(){} run(){ this.one(this.two()); this.two()(); factory(this.one()).x(this.two()); } }",
    );
    let r = repo.resolver();
    let b = builder(&r);
    let f = calls::analyze(&r, &b).unwrap();
    let positions: std::collections::BTreeSet<_> = f
        .sites()
        .iter()
        .map(|s| (s.site.start, s.site.end))
        .collect();
    assert_eq!(positions.len(), 8);
    let g = graph(&r);
    assert_eq!(counts(&g), (8, 5, 3));
    assert_eq!(call_edges(&g).len(), 2);
}
#[test]
fn unknowns_aggregate_per_method_and_reason_at_first_position() {
    let repo = Repo::new(
        "class A { run(){\n x();\n y(); this['a']();\n this['b']();\n } other(){ x(); } }",
    );
    let g = graph(&repo.resolver());
    assert_eq!(counts(&g), (5, 0, 5));
    let ds: Vec<_> = g
        .diagnostics
        .iter()
        .filter(|d| d.skipped_count.is_some())
        .collect();
    assert_eq!(ds.len(), 3);
    let run = method("main.ts", &[], "A", "instance", "run");
    let d = ds
        .iter()
        .find(|d| {
            d.related_node_id.as_ref() == Some(&run)
                && d.code == "unsupported_call_unknown_receiver"
        })
        .unwrap();
    assert_eq!((d.line, d.skipped_count), (Some(2), Some(2)));
}
#[test]
fn nested_class_methods_own_calls_and_body_exclusions_hold() {
    let repo = Repo::new(
        "function deco(...x: unknown[]){} @deco(outside()) class A { field = outside(); constructor(){ outside(); } static { outside(); } find(){} run(x = outside()){ class Inner { field = outside(); constructor(){ outside(); } @deco(outside()) inner(){ this.target(); (()=>this.target())(); } target(){} } this.find(); } }",
    );
    let r = repo.resolver();
    let f = calls::analyze(&r, &builder(&r)).unwrap();
    assert_eq!(f.sites().len(), 4);
    let g = graph(&r);
    assert_eq!(counts(&g), (4, 2, 2));
    let outer = method("main.ts", &[], "A", "instance", "run");
    assert_eq!(f.sites().iter().filter(|s| s.caller_id == outer).count(), 1);
    assert_eq!(f.sites().iter().filter(|s| s.caller_id != outer).count(), 3);
}
#[test]
fn overwrite_patterns_block_only_the_affected_staticness() {
    for write in [
        "this.find = replacement;",
        "this.find ||= replacement;",
        "this['find'] = replacement;",
        "delete this.find;",
        "this.find++;",
        "({x: this.find} = source);",
        "[this.find] = source;",
        "for (this.find of source) {}",
        "(this.find as any) = replacement;",
        "const a = () => { this.find = replacement; };",
    ] {
        let repo = Repo::new(&format!(
            "class A {{ find(){{}} static find(){{}} run(){{ this.find(); }} static run(){{ this.find(); }} constructor(){{ {write} }} }}"
        ));
        let g = graph(&repo.resolver());
        assert_eq!(counts(&g), (2, 1, 1), "{write}");
        assert_eq!(
            reasons(&g)["unsupported_call_ambiguous_target"],
            1,
            "{write}"
        );
        assert_eq!(
            call_edges(&g)[0].to,
            method("main.ts", &[], "A", "static", "find")
        );
    }
}
#[test]
fn fields_parameters_overloads_and_dynamic_writes_prevent_false_edges() {
    for members in [
        "find = replacement; find(){}",
        "constructor(public find: any) {} find(){}",
        "find(): void; find(){}",
        "get find(){ return replacement; }",
        "find(){} constructor(){ this[key] = replacement; }",
        "find(){} [key] = replacement;",
        "find(){} [key]() {}",
        "find(){} constructor(){ delete this[key]; }",
    ] {
        let repo = Repo::new(&format!("class A {{ {members} run(){{ this.find(); }} }}"));
        let g = graph(&repo.resolver());
        assert_eq!(counts(&g), (1, 0, 1), "{members}");
        assert_eq!(
            reasons(&g)["unsupported_call_ambiguous_target"],
            1,
            "{members}"
        );
    }
}
#[test]
fn inheritance_is_not_resolved_but_own_method_is() {
    let repo = Repo::new(
        "class Base { find(){} } class A extends Base { own(){} run(){ this.find(); super.find(); this.own(); } }",
    );
    let g = graph(&repo.resolver());
    assert_eq!(counts(&g), (3, 1, 2));
    assert_eq!(reasons(&g)["unsupported_call_inheritance"], 2);
}
#[test]
fn zero_parse_failure_and_mixed_files_are_distinct() {
    let repo = Repo::new("class Empty { run(){} }");
    let r = repo.resolver();
    let f = calls::analyze(&r, &builder(&r)).unwrap();
    assert!(f.incomplete_files().is_empty());
    assert_eq!(counts(&graph(&r)), (0, 0, 0));
    repo.write("broken.ts", "class Broken { run( {");
    let r = repo.resolver();
    let mut b = builder(&r);
    let before: Vec<_> = r.diagnostics().to_vec();
    let f = calls::analyze(&r, &b).unwrap();
    assert_eq!(
        f.incomplete_files().iter().collect::<Vec<_>>(),
        vec!["broken.ts"]
    );
    assert_eq!(f.diagnostics().count(), 0);
    f.apply(&mut b).unwrap();
    r.apply_imports(&mut b).unwrap();
    let g = b.finish().unwrap();
    assert_eq!(counts(&g), (0, 0, 0));
    assert!(g.diagnostics.iter().any(|d| d.code == "TS_PARSE_ERROR"));
    assert_eq!(
        g.diagnostics
            .iter()
            .filter(|d| d.code == "TS_IMPORT_PARSEINCOMPLETE")
            .count(),
        1
    );
    assert_eq!(before, r.diagnostics());
    repo.write("valid.ts", "class A { run(){ this.find(); } find(){} }");
    assert_eq!(counts(&graph(&repo.resolver())), (1, 1, 0));
}
#[test]
fn snapshot_is_retained_and_no_source_is_reread() {
    let repo = Repo::new("class A { find(){} run(){ this.find(); } }");
    let r = repo.resolver();
    repo.write("main.ts", "class B { bad( {");
    assert_eq!(counts(&graph(&r)), (1, 1, 0));
}
#[cfg(unix)]
#[test]
fn replacing_source_with_outside_symlink_does_not_read_it() {
    let repo = Repo::new("class A { find(){} run(){ this.find(); } }");
    let outside = Repo::new("class Outside { bad() { outside(); } }");
    let r = repo.resolver();
    fs::remove_file(repo.0.join("main.ts")).unwrap();
    std::os::unix::fs::symlink(outside.0.join("main.ts"), repo.0.join("main.ts")).unwrap();
    let g = graph(&r);
    assert_eq!(counts(&g), (1, 1, 0));
    assert!(!g.nodes.iter().any(|n| n.name == "Outside"));
}
#[test]
fn batch_reapplication_is_rejected_even_for_zero_calls() {
    for source in [
        "class A { run(){} }",
        "class A { find(){} run(){ this.find(); x(); } }",
    ] {
        let repo = Repo::new(source);
        let r = repo.resolver();
        let mut b = builder(&r);
        let f = calls::analyze(&r, &b).unwrap();
        f.apply(&mut b).unwrap();
        assert!(f.apply(&mut b).is_err());
        assert!(b.finish().is_err());
    }
}
#[test]
fn inconsistent_batch_and_missing_parse_diagnostic_are_rejected() {
    let repo = Repo::new("class A { run(){} }");
    let r = repo.resolver();
    let mut b = builder(&r);
    let mut summary = calls::analyze(&r, &b).unwrap().summary().clone();
    summary.examined_calls = 1;
    assert!(
        b.apply_call_analysis(summary, vec![], vec![], Default::default())
            .is_err()
    );
    assert!(b.finish().is_err());
    let mut b = builder(&r);
    let summary = calls::analyze(&r, &b).unwrap().summary().clone();
    b.apply_call_analysis(summary, vec![], vec![], ["broken.ts".into()].into())
        .unwrap();
    assert!(b.finish().is_err());
}

#[test]
fn same_line_sites_keep_counts_even_when_wire_evidence_deduplicates() {
    let repo = Repo::new("class A { find(){} run(){ this.find(); this.find(); } }");
    let g = graph(&repo.resolver());
    assert_eq!(counts(&g), (2, 2, 0));
    assert_eq!(call_edges(&g).len(), 1);
    assert_eq!(call_edges(&g)[0].evidence.len(), 1);
}
#[test]
fn nested_functions_keep_unknown_this_even_in_object_methods() {
    let repo = Repo::new(
        "class A { find(){} run(){ const o = { find(){ this.find(); } }; function f(){ this.find(); } const a = () => this.find(); this.find(); } }",
    );
    let g = graph(&repo.resolver());
    assert_eq!(counts(&g), (4, 1, 3));
    assert_eq!(reasons(&g)["unsupported_call_nested_function"], 3);
}
#[test]
fn static_writes_and_other_classes_do_not_taint_instance_methods() {
    let repo = Repo::new(
        "class A { find(){} static find(){} static { this.find = other; } run(){ this.find(); } static run(){ this.find(); } } class B { find(){} constructor(){ this.find = other; } }",
    );
    let g = graph(&repo.resolver());
    assert_eq!(counts(&g), (2, 1, 1));
    assert_eq!(
        call_edges(&g)[0].to,
        method("main.ts", &[], "A", "instance", "find")
    );
}
#[test]
fn private_methods_and_staticness_mismatch_do_not_connect_by_name() {
    let repo =
        Repo::new("class A { #find(){} static find(){} run(){ this.find(); this.#find(); } }");
    let g = graph(&repo.resolver());
    assert_eq!(counts(&g), (2, 0, 2));
    assert_eq!(reasons(&g)["unsupported_call_missing_method"], 1);
    assert_eq!(reasons(&g)["unsupported_call_unknown_receiver"], 1);
}
#[test]
fn only_parse_failure_is_not_a_completed_empty_scope() {
    let repo = Repo::new("class A { run( {");
    let r = repo.resolver();
    let b = builder(&r);
    let f = calls::analyze(&r, &b).unwrap();
    assert_eq!(f.incomplete_files().len(), 1);
    let g = graph(&r);
    assert_eq!(counts(&g), (0, 0, 0));
    assert!(g.diagnostics.iter().any(|d| d.code == "TS_PARSE_ERROR"));
}
#[test]
fn batch_validation_rejects_counter_diagnostic_and_target_inconsistencies() {
    let repo = Repo::new(
        "class A { find(){} run(){\n this.find();\n this.find();\n x();\n y(); } } class B { find(){} }",
    );
    let r = repo.resolver();
    let g = graph(&r);
    let edges: Vec<_> = call_edges(&g)
        .into_iter()
        .flat_map(|e| {
            e.evidence.clone().into_iter().map(move |v| {
                let mut e = e.clone();
                e.evidence = vec![v];
                e
            })
        })
        .collect();
    let ds: Vec<_> = g
        .diagnostics
        .iter()
        .filter(|d| d.skipped_count.is_some())
        .cloned()
        .collect();
    for case in 0..6 {
        let mut b = builder(&r);
        let mut edges = edges.clone();
        let mut ds = ds.clone();
        let mut c = g.metadata.call_analysis.clone();
        match case {
            0 => {
                c.emitted_calls = 1;
                c.examined_calls = 3;
            }
            1 => {
                ds[0].skipped_count = Some(1);
            }
            2 => {
                ds.push(ds[0].clone());
            }
            3 => {
                edges[0].to = method("main.ts", &[], "B", "instance", "find");
                edges[0].id = GraphBuilder::edge_id(&edges[0].from, EdgeKind::Calls, &edges[0].to);
            }
            4 => {
                ds[0].skipped_count = Some(0);
            }
            5 => {
                edges[0].evidence[0].file = "other.ts".into();
            }
            _ => unreachable!(),
        }
        assert!(
            b.apply_call_analysis(c, edges, ds, Default::default())
                .is_err(),
            "case {case}"
        );
        assert!(b.finish().is_err());
    }
    let mut b = builder(&r);
    let mut reverse_edges = edges.clone();
    reverse_edges.reverse();
    let mut reverse_ds = ds.clone();
    reverse_ds.reverse();
    b.apply_call_analysis(
        g.metadata.call_analysis.clone(),
        reverse_edges,
        reverse_ds,
        Default::default(),
    )
    .unwrap();
    r.apply_imports(&mut b).unwrap();
    assert_eq!(g, b.finish().unwrap());
    for mutate_edge in [true, false] {
        let mut b = builder(&r);
        calls::analyze(&r, &b).unwrap().apply(&mut b).unwrap();
        if mutate_edge {
            assert!(b.add_edge(edges[0].clone()).is_err());
        } else {
            assert!(b.add_diagnostic(ds[0].clone()).is_err());
        }
        assert!(b.finish().is_err());
    }
}
#[test]
fn semantic_failure_retains_named_body_sites_as_unknown() {
    let repo = Repo::new("class A { find(){} run(){ let x; let x; super.find(); this.find(); } }");
    let g = graph(&repo.resolver());
    assert_eq!(counts(&g), (2, 0, 2));
    assert!(
        g.diagnostics
            .iter()
            .any(|d| d.code == "TS_IMPORT_PARSEINCOMPLETE")
    );
}
