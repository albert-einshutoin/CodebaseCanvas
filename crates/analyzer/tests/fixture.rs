use codebasecanvas_analyzer::SystemGraph;

#[test]
fn hand_authored_nestjs_fixture_matches_wire_contract() {
    let graph = SystemGraph::from_json(include_str!(
        "../../../examples/nestjs-sample/expected-graph.json"
    ))
    .unwrap();
    assert_eq!(graph.metadata.root_name.as_deref(), Some("nestjs-sample"));
}
