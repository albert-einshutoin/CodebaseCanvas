#[path = "support/fixture_repo.rs"]
mod fixture_repo;
#[path = "support/structural.rs"]
mod structural;

use codebasecanvas_analyzer::{SystemGraph, discovery::RepositoryRoot, pipeline};
use fixture_repo::Repo;

const FIXED_TIME: &str = "2026-09-24T00:00:00Z";

fn oracle() -> SystemGraph {
    SystemGraph::from_json(include_str!(
        "../../../examples/nestjs-sample/expected-graph.json"
    ))
    .unwrap()
}

#[test]
fn production_fixture_matches_independent_structural_oracle() {
    let repo = Repo::new();
    repo.copy_fixture();
    let actual = pipeline::analyze(&RepositoryRoot::open(&repo.0).unwrap(), FIXED_TIME).unwrap();
    let expected = oracle();
    expected.validate().unwrap();
    actual.graph.validate().unwrap();
    let root_name = repo.0.file_name().unwrap().to_str().unwrap();
    structural::compare_oracle(&expected, &actual.graph, root_name, FIXED_TIME).unwrap();
}

use codebasecanvas_analyzer::{Confidence, EdgeKind, GraphBuilder, GraphNode, NodeKind, Severity};
use std::collections::BTreeMap;

fn declaration(file: &str, scope: &[&str], name: &str) -> String {
    GraphBuilder::node_id(NodeKind::Class, file, scope, name).unwrap()
}

fn method(owner: &str, staticness: &str, name: &str) -> String {
    GraphBuilder::method_id(owner, staticness, name)
}

fn node<'a>(graph: &'a SystemGraph, id: &str) -> &'a GraphNode {
    graph.nodes.iter().find(|node| node.id == id).unwrap()
}

fn has_edge(graph: &SystemGraph, from: &str, kind: EdgeKind, to: &str) -> bool {
    graph
        .edges
        .iter()
        .any(|edge| edge.from == from && edge.kind == kind && edge.to == to)
}

fn source_line(repo: &Repo, file: &str, line: usize) -> String {
    std::fs::read_to_string(repo.0.join(file))
        .unwrap()
        .lines()
        .nth(line - 1)
        .unwrap()
        .trim()
        .into()
}

#[test]
fn fixture_identity_evidence_counts_and_forbidden_relationships() {
    let repo = Repo::new();
    repo.copy_fixture();
    let graph = repo.analyze().graph;
    let counts = |values: Vec<String>| {
        let mut result = BTreeMap::new();
        for value in values {
            *result.entry(value).or_insert(0usize) += 1;
        }
        result
    };
    assert_eq!(graph.nodes.len(), 57);
    assert_eq!(graph.edges.len(), 90);
    assert_eq!(graph.diagnostics.len(), 15);
    assert_eq!(
        counts(
            graph
                .nodes
                .iter()
                .map(|n| serde_json::to_value(n.kind)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_owned())
                .collect()
        ),
        BTreeMap::from([
            ("module".into(), 6),
            ("controller".into(), 3),
            ("service".into(), 3),
            ("repository".into(), 2),
            ("class".into(), 5),
            ("interface".into(), 1),
            ("method".into(), 17),
            ("endpoint".into(), 6),
            ("database_model".into(), 2),
            ("external_dependency".into(), 12),
        ])
    );
    assert_eq!(
        counts(
            graph
                .edges
                .iter()
                .map(|e| serde_json::to_value(e.kind)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_owned())
                .collect()
        ),
        BTreeMap::from([
            ("contains".into(), 26),
            ("depends_on".into(), 8),
            ("injects".into(), 5),
            ("exposes".into(), 6),
            ("calls".into(), 2),
            ("imports".into(), 43),
        ])
    );
    assert_eq!(
        counts(graph.diagnostics.iter().map(|d| d.code.clone()).collect()),
        BTreeMap::from([
            ("unsupported_call_injected_receiver".into(), 5),
            ("unsupported_call_computed_target".into(), 1),
            ("unsupported_call_nested_function".into(), 1),
            ("unsupported_di_interface".into(), 1),
            ("unsupported_di_type_only".into(), 1),
            ("unsupported_di_custom_token".into(), 1),
            ("unsupported_module_forward_ref".into(), 1),
            ("unsupported_module_dynamic".into(), 1),
            ("unsupported_module_spread".into(), 1),
            ("unsupported_module_provider".into(), 2),
        ])
    );
    assert!(
        graph
            .diagnostics
            .iter()
            .all(|d| d.severity == Severity::Warning
                && !d.message.is_empty()
                && !d.message.contains(repo.0.to_str().unwrap()))
    );
    assert_eq!(
        (
            graph.metadata.call_analysis.examined_calls,
            graph.metadata.call_analysis.emitted_calls,
            graph.metadata.call_analysis.skipped_calls
        ),
        (9, 2, 7)
    );

    let app = declaration("src/app.module.ts", &[], "AppModule");
    let auth_module = declaration("src/auth/auth.module.ts", &[], "AuthModule");
    let users_module = declaration("src/users/users.module.ts", &[], "UsersModule");
    let unsupported_module = declaration(
        "src/regressions/unsupported.module.ts",
        &[],
        "UnsupportedModule",
    );
    let unsupported_consumer = declaration(
        "src/regressions/unsupported.module.ts",
        &[],
        "UnsupportedConsumer",
    );
    let users_service = declaration("src/users/users.service.ts", &[], "UsersService");
    let auth_service = declaration("src/auth/auth.service.ts", &[], "AuthService");
    let auth_controller = declaration("src/auth/auth.controller.ts", &[], "AuthController");
    let users_controller = declaration("src/users/users.controller.ts", &[], "UsersController");
    let override_controller = declaration(
        "src/regressions/override.module.ts",
        &[],
        "OverrideController",
    );
    let mock = declaration(
        "src/regressions/override.module.ts",
        &[],
        "MockUsersService",
    );
    let port = GraphBuilder::node_id(NodeKind::Interface, "src/regressions/ports.ts", &[], "Port")
        .unwrap();
    let type_only = declaration("src/regressions/ports.ts", &[], "TypeOnlyToken");
    assert_ne!(auth_service, users_service);
    assert_eq!(node(&graph, &users_service).kind, NodeKind::Service);
    assert!(node(&graph, &users_service).parent_id.is_none());
    assert_eq!(
        graph.nodes.iter().filter(|n| n.id == users_service).count(),
        1
    );
    assert!(has_edge(
        &graph,
        &auth_module,
        EdgeKind::Contains,
        &users_service
    ));
    assert!(has_edge(
        &graph,
        &users_module,
        EdgeKind::Contains,
        &users_service
    ));
    assert!(has_edge(&graph, &app, EdgeKind::DependsOn, &auth_module));
    assert!(!has_edge(&graph, &app, EdgeKind::Contains, &auth_module));
    assert!(node(&graph, &auth_module).parent_id.is_none());
    assert!(has_edge(
        &graph,
        &unsupported_module,
        EdgeKind::Contains,
        &unsupported_consumer
    ));
    assert!(has_edge(
        &graph,
        &unsupported_module,
        EdgeKind::Imports,
        &users_module
    ));
    assert!(!has_edge(
        &graph,
        &unsupported_module,
        EdgeKind::DependsOn,
        &users_module
    ));
    assert!(!has_edge(
        &graph,
        &unsupported_module,
        EdgeKind::Contains,
        &users_service
    ));
    assert!(!has_edge(
        &graph,
        &unsupported_module,
        EdgeKind::Contains,
        &mock
    ));

    let alpha = declaration("src/regressions/identity.ts", &["Alpha"], "Same");
    let beta = declaration("src/regressions/identity.ts", &["Beta"], "Same");
    assert_ne!(alpha, beta);
    assert_eq!(node(&graph, &alpha).name, node(&graph, &beta).name);
    let static_find = method(&users_service, "static", "find");
    let instance_find = method(&users_service, "instance", "find");
    assert_ne!(static_find, instance_find);
    assert_eq!(
        node(&graph, &static_find).name,
        node(&graph, &instance_find).name
    );
    let root_map = GraphBuilder::external_id("rxjs", "map");
    let operators_map = GraphBuilder::external_id("rxjs/operators", "map");
    assert_ne!(root_map, operators_map);
    assert_eq!(
        node(&graph, &root_map).name,
        node(&graph, &operators_map).name
    );
    let external_bindings = declaration("src/regressions/identity.ts", &[], "ExternalBindings");
    let inspect = method(&external_bindings, "instance", "inspect");
    assert!(has_edge(&graph, &inspect, EdgeKind::Imports, &root_map));
    assert!(has_edge(
        &graph,
        &inspect,
        EdgeKind::Imports,
        &operators_map
    ));

    assert_eq!(node(&graph, &port).kind, NodeKind::Interface);
    assert_eq!(node(&graph, &type_only).kind, NodeKind::Class);
    assert_eq!(node(&graph, &unsupported_consumer).kind, NodeKind::Service);
    assert!(
        !graph
            .edges
            .iter()
            .any(|e| e.kind == EdgeKind::Injects && e.from == unsupported_consumer)
    );
    assert!(!graph.edges.iter().any(
        |e| e.kind == EdgeKind::Injects && (e.to == port || e.to == type_only || e.to == mock)
    ));
    assert!(has_edge(
        &graph,
        &override_controller,
        EdgeKind::Injects,
        &users_service
    ));
    let requested = graph
        .edges
        .iter()
        .find(|e| e.from == override_controller && e.kind == EdgeKind::Injects)
        .unwrap();
    assert_eq!(
        requested.metadata.as_ref().unwrap()["semantics"],
        "requested_token"
    );
    assert_eq!(
        requested.evidence[0].source,
        codebasecanvas_analyzer::EvidenceSource::Nestjs
    );
    assert_eq!(requested.evidence[0].confidence, Confidence::Confirmed);

    let auth_login = method(&auth_service, "instance", "login");
    let auth_normalize = method(&auth_service, "instance", "normalize");
    let choose = method(&users_service, "instance", "choose");
    assert!(has_edge(
        &graph,
        &auth_login,
        EdgeKind::Calls,
        &auth_normalize
    ));
    assert!(has_edge(&graph, &choose, EdgeKind::Calls, &instance_find));
    assert!(!has_edge(&graph, &choose, EdgeKind::Calls, &static_find));
    let injected_callers = [
        method(&auth_controller, "instance", "login"),
        auth_login.clone(),
        method(&users_service, "instance", "list"),
        method(&users_controller, "instance", "list"),
        method(&override_controller, "instance", "list"),
    ];
    for caller in &injected_callers {
        assert!(node(&graph, caller).kind == NodeKind::Method);
        assert!(
            graph
                .edges
                .iter()
                .filter(|e| e.kind == EdgeKind::Calls && &e.from == caller)
                .all(|e| caller == &auth_login && e.to == auth_normalize)
        );
        assert!(
            graph
                .diagnostics
                .iter()
                .any(|d| d.code == "unsupported_call_injected_receiver"
                    && d.related_node_id.as_deref() == Some(caller)
                    && d.skipped_count == Some(1))
        );
    }
    assert_eq!(
        graph
            .diagnostics
            .iter()
            .filter_map(|d| d.skipped_count)
            .sum::<u64>(),
        7
    );
    assert!(
        graph
            .diagnostics
            .iter()
            .any(|d| d.code == "unsupported_call_computed_target"
                && d.related_node_id.as_deref() == Some(&choose)
                && d.line == Some(12)
                && d.skipped_count == Some(1))
    );
    assert!(
        graph
            .diagnostics
            .iter()
            .any(|d| d.code == "unsupported_call_nested_function"
                && d.related_node_id.as_deref() == Some(&choose)
                && d.line == Some(13)
                && d.skipped_count == Some(1))
    );

    let get_users: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| {
            n.kind == NodeKind::Endpoint
                && n.metadata.as_ref().and_then(|m| m.get("httpMethod"))
                    == Some(&serde_json::json!("GET"))
                && n.metadata.as_ref().and_then(|m| m.get("path"))
                    == Some(&serde_json::json!("/users"))
        })
        .collect();
    assert_eq!(get_users.len(), 2);
    let handlers = [
        method(&users_controller, "instance", "list"),
        method(&users_controller, "instance", "alias"),
    ];
    for handler in &handlers {
        let endpoint = get_users
            .iter()
            .find(|n| n.metadata.as_ref().unwrap()["controllerMethodId"] == *handler)
            .unwrap();
        assert_eq!(
            endpoint.parent_id.as_deref(),
            Some(users_controller.as_str())
        );
        assert!(has_edge(
            &graph,
            &users_controller,
            EdgeKind::Exposes,
            &endpoint.id
        ));
        assert!(has_edge(&graph, &endpoint.id, EdgeKind::DependsOn, handler));
    }
    assert_ne!(get_users[0].id, get_users[1].id);
    assert!(
        !graph
            .edges
            .iter()
            .any(|e| matches!(e.kind, EdgeKind::Reads | EdgeKind::Writes))
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::DatabaseModel)
            .count(),
        2
    );

    for (name, start, end, fields) in [
        (
            "User",
            2,
            5,
            serde_json::json!([{"name":"id","type":"String"},{"name":"name","type":"String"}]),
        ),
        (
            "Token",
            7,
            10,
            serde_json::json!([{"name":"id","type":"String"},{"name":"value","type":"String"}]),
        ),
    ] {
        let model_id =
            GraphBuilder::node_id(NodeKind::DatabaseModel, "prisma/schema.prisma", &[], name)
                .unwrap();
        let model = node(&graph, &model_id);
        assert_eq!((model.line, model.end_line), (Some(start), Some(end)));
        assert_eq!(model.metadata.as_ref().unwrap()["fields"], fields);
        assert_eq!(
            (model.evidence[0].line, model.evidence[0].end_line),
            (Some(start), Some(end))
        );
        assert_eq!(
            model.evidence[0].source,
            codebasecanvas_analyzer::EvidenceSource::Prisma
        );
        assert_eq!(model.evidence[0].confidence, Confidence::Confirmed);
    }
    let choice = node(&graph, &choose);
    assert_eq!((choice.line, choice.end_line), (Some(10), Some(15)));
    assert_eq!(
        (choice.evidence[0].line, choice.evidence[0].end_line),
        (Some(10), Some(15))
    );
    assert_eq!(node(&graph, &users_service).end_line, Some(16));
    for (file, decorator, declaration) in [
        ("src/auth/auth.controller.ts", 7, "login()"),
        ("src/regressions/override.module.ts", 10, "list()"),
        ("src/users/users.controller.ts", 7, "list()"),
        ("src/users/users.controller.ts", 10, "alias()"),
        ("src/users/users.controller.ts", 12, "update()"),
        ("src/users/users.controller.ts", 14, "remove()"),
    ] {
        assert!(source_line(&repo, file, decorator).starts_with('@'));
        assert!(source_line(&repo, file, decorator + 1).starts_with(declaration));
    }
    for declaration in graph.nodes.iter().filter(|n| {
        matches!(
            n.kind,
            NodeKind::Module
                | NodeKind::Controller
                | NodeKind::Service
                | NodeKind::Repository
                | NodeKind::Class
                | NodeKind::Interface
        )
    }) {
        assert!(
            source_line(
                &repo,
                declaration.file.as_deref().unwrap(),
                declaration.line.unwrap() as usize
            )
            .contains("export "),
            "{}",
            declaration.id
        );
    }
    assert_eq!(
        source_line(&repo, "src/auth/token.repository.ts", 3),
        "@Injectable()"
    );
    assert!(source_line(&repo, "src/auth/token.repository.ts", 4).contains("TokenRepository"));
    assert_eq!(
        source_line(&repo, "src/users/user.repository.ts", 3),
        "@Injectable()"
    );
    assert!(source_line(&repo, "src/users/user.repository.ts", 4).contains("UserRepository"));
    assert_eq!(
        source_line(&repo, "src/auth/auth.module.ts", 10),
        "exports: [AuthService],"
    );
    assert_eq!(
        source_line(&repo, "src/users/users.module.ts", 9),
        "exports: [UsersService],"
    );
}

#[test]
fn repeated_pipeline_analysis_is_order_independent_without_normalizing_content() {
    let repo = Repo::new();
    repo.copy_fixture();
    let first = repo.analyze_at(FIXED_TIME).graph;
    let second = repo.analyze_at("2026-09-25T00:00:00Z").graph;
    let first_before = first.clone();
    let second_before = second.clone();
    assert_ne!(first.metadata.analyzed_at, second.metadata.analyzed_at);
    structural::compare_repeated(&first, &second).unwrap();
    let mut reordered = second.clone();
    reordered.nodes.reverse();
    reordered.edges.reverse();
    reordered.diagnostics.reverse();
    for node in &mut reordered.nodes {
        node.evidence.reverse();
    }
    for edge in &mut reordered.edges {
        edge.evidence.reverse();
    }
    structural::compare_repeated(&first, &reordered).unwrap();
    assert_eq!(first, first_before);
    assert_eq!(second, second_before);
    assert_eq!(
        first.to_json().unwrap(),
        repo.analyze_at(FIXED_TIME).graph.to_json().unwrap()
    );
}

#[test]
fn provider_override_forms_do_not_create_membership_or_implementation_edges() {
    let repo = Repo::new();
    repo.write("main.ts", "import { Injectable, Module } from '@nestjs/common';
class Token { find() {} }
class Mock { find() {} }
@Injectable() class Consumer { constructor(private readonly dep: Token) {} run() { this.dep.find(); } }
class Safe {}
@Module({providers: [
  Consumer, Safe,
  {provide: Token, useClass: Mock},
  {provide: 'VALUE', useValue: 1},
  {provide: 'ALIAS', useExisting: Token},
  {provide: 'FACTORY', useFactory: () => 1},
]}) class M {}
");
    let graph = repo.analyze().graph;
    graph.validate().unwrap();
    let module = declaration("main.ts", &[], "M");
    let consumer = declaration("main.ts", &[], "Consumer");
    let safe = declaration("main.ts", &[], "Safe");
    let token = declaration("main.ts", &[], "Token");
    let mock = declaration("main.ts", &[], "Mock");
    assert!(has_edge(&graph, &module, EdgeKind::Contains, &consumer));
    assert!(has_edge(&graph, &module, EdgeKind::Contains, &safe));
    assert!(has_edge(&graph, &consumer, EdgeKind::Injects, &token));
    assert!(!has_edge(&graph, &consumer, EdgeKind::Injects, &mock));
    assert!(!has_edge(&graph, &module, EdgeKind::Contains, &token));
    assert!(!has_edge(&graph, &module, EdgeKind::Contains, &mock));
    let run = method(&consumer, "instance", "run");
    assert_eq!(node(&graph, &run).kind, NodeKind::Method);
    assert!(
        !graph
            .edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::Calls && edge.from == run)
    );
    assert!(
        graph
            .diagnostics
            .iter()
            .any(|d| d.code == "unsupported_call_injected_receiver"
                && d.related_node_id.as_deref() == Some(&run)
                && d.skipped_count == Some(1))
    );
    let mut provider_lines: Vec<_> = graph
        .diagnostics
        .iter()
        .filter(|d| {
            d.code == "unsupported_module_provider" && d.related_node_id.as_deref() == Some(&module)
        })
        .map(|d| d.line)
        .collect();
    provider_lines.sort();
    assert_eq!(provider_lines, [Some(8), Some(9), Some(10), Some(11)]);
}

#[test]
fn dynamic_route_does_not_erase_supported_sibling_or_invent_endpoint() {
    let repo = Repo::new();
    repo.write(
        "main.ts",
        "import { Controller, Get } from '@nestjs/common';
const PATH = 'hidden';
@Controller('items') class ItemsController {
  @Get('ok') ok() {}
  @Get(PATH) dynamic() {}
}
",
    );
    let graph = repo.analyze().graph;
    graph.validate().unwrap();
    let controller = declaration("main.ts", &[], "ItemsController");
    let ok = method(&controller, "instance", "ok");
    let dynamic = method(&controller, "instance", "dynamic");
    assert_eq!(node(&graph, &controller).kind, NodeKind::Controller);
    assert_eq!(node(&graph, &dynamic).kind, NodeKind::Method);
    let endpoints: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Endpoint)
        .collect();
    assert_eq!(endpoints.len(), 1);
    let endpoint = endpoints[0];
    assert_eq!(endpoint.metadata.as_ref().unwrap()["path"], "/items/ok");
    assert!(has_edge(
        &graph,
        &controller,
        EdgeKind::Exposes,
        &endpoint.id
    ));
    assert!(has_edge(&graph, &endpoint.id, EdgeKind::DependsOn, &ok));
    assert!(
        !graph
            .edges
            .iter()
            .any(|e| e.kind == EdgeKind::DependsOn && e.to == dynamic)
    );
    assert!(
        graph
            .diagnostics
            .iter()
            .any(|d| d.code == "unsupported_route_path"
                && d.file.as_deref() == Some("main.ts")
                && d.line == Some(5))
    );
}

#[test]
fn repeated_call_sites_keep_site_and_grouped_unknown_counts() {
    let repo = Repo::new();
    repo.write(
        "main.ts",
        "class Calls {
  find() {}
  run() {
    this.find();
    this.find();
    unknown();
    unknown();
  }
}
",
    );
    let graph = repo.analyze().graph;
    graph.validate().unwrap();
    let owner = declaration("main.ts", &[], "Calls");
    let run = method(&owner, "instance", "run");
    let find = method(&owner, "instance", "find");
    assert_eq!(
        (
            graph.metadata.call_analysis.examined_calls,
            graph.metadata.call_analysis.emitted_calls,
            graph.metadata.call_analysis.skipped_calls
        ),
        (4, 2, 2)
    );
    let calls: Vec<_> = graph
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Calls)
        .collect();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        (calls[0].from.as_str(), calls[0].to.as_str()),
        (run.as_str(), find.as_str())
    );
    assert_eq!(
        calls[0].evidence.iter().map(|e| e.line).collect::<Vec<_>>(),
        [Some(4), Some(5)]
    );
    assert!(
        calls[0]
            .evidence
            .iter()
            .all(|e| e.source == codebasecanvas_analyzer::EvidenceSource::Ast
                && e.confidence == Confidence::Confirmed)
    );
    let unknown: Vec<_> = graph
        .diagnostics
        .iter()
        .filter(|d| d.code == "unsupported_call_unknown_receiver")
        .collect();
    assert_eq!(unknown.len(), 1);
    assert_eq!(
        (
            unknown[0].related_node_id.as_deref(),
            unknown[0].line,
            unknown[0].skipped_count
        ),
        (Some(run.as_str()), Some(6), Some(2))
    );
}

#[test]
fn prisma_unknowns_do_not_change_call_coverage() {
    let repo = Repo::new();
    repo.write(
        "schema.prisma",
        "model User {\n id Int\n broken What(\"secret\")\n good String\n}\n",
    );
    let graph = repo.analyze().graph;
    graph.validate().unwrap();
    let model_id =
        GraphBuilder::node_id(NodeKind::DatabaseModel, "schema.prisma", &[], "User").unwrap();
    assert_eq!(
        node(&graph, &model_id).metadata.as_ref().unwrap()["fields"],
        serde_json::json!([{"name":"id","type":"Int"},{"name":"good","type":"String"}])
    );
    let unknowns: Vec<_> = graph
        .diagnostics
        .iter()
        .filter(|d| d.code == "PRISMA_UNSUPPORTED_FIELD")
        .collect();
    assert_eq!(unknowns.len(), 1);
    assert_eq!(
        (
            unknowns[0].file.as_deref(),
            unknowns[0].line,
            unknowns[0].related_node_id.as_deref(),
            unknowns[0].skipped_count
        ),
        (Some("schema.prisma"), Some(3), None, None)
    );
    assert!(!unknowns[0].message.contains("secret"));
    assert_eq!(
        (
            graph.metadata.call_analysis.examined_calls,
            graph.metadata.call_analysis.emitted_calls,
            graph.metadata.call_analysis.skipped_calls
        ),
        (0, 0, 0)
    );
}
