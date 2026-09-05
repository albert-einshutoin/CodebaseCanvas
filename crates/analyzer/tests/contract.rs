use codebasecanvas_analyzer::{SystemGraph, canonical_id};
use serde_json::Value;

#[test]
fn shared_wire_contract() {
    let cases: Value = serde_json::from_str(include_str!("../../../contracts/cases.json")).unwrap();
    let graph = &cases["graph"];
    let parsed = SystemGraph::from_json(&graph.to_string()).unwrap();
    assert_eq!(serde_json::to_value(&parsed).unwrap(), *graph);
    for case in cases["valid"].as_array().unwrap() {
        SystemGraph::from_json(&case["graph"].to_string()).unwrap();
    }
    for case in cases["invalid"].as_array().unwrap() {
        let mut invalid = graph.clone();
        let mut target = &mut invalid;
        let path = case["path"].as_array().unwrap();
        if path.is_empty() {
            assert!(
                SystemGraph::from_json(&case["value"].to_string()).is_err(),
                "{}",
                case["name"]
            );
            continue;
        }
        for part in &path[..path.len() - 1] {
            target = if let Some(index) = part.as_u64() {
                &mut target[index as usize]
            } else {
                &mut target[part.as_str().unwrap()]
            };
        }
        let last = path.last().unwrap();
        if let Some(index) = last.as_u64() {
            target[index as usize] = case["value"].clone();
        } else {
            target[last.as_str().unwrap()] = case["value"].clone();
        }
        assert!(
            SystemGraph::from_json(&invalid.to_string()).is_err(),
            "{}",
            case["name"]
        );
    }
    for raw in cases["validJson"].as_array().unwrap() {
        let raw = raw.as_str().unwrap();
        let graph = SystemGraph::from_json(raw).unwrap();
        assert_eq!(
            serde_json::to_value(graph).unwrap(),
            serde_json::from_str::<Value>(raw).unwrap()
        );
    }
    for raw in cases["invalidJson"].as_array().unwrap() {
        assert!(SystemGraph::from_json(raw.as_str().unwrap()).is_err());
    }
    for vector in cases["ids"].as_array().unwrap() {
        let parts: Vec<_> = vector["parts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_str().unwrap())
            .collect();
        assert_eq!(
            canonical_id(vector["tag"].as_str().unwrap(), &parts),
            vector["expected"].as_str().unwrap()
        );
    }
}

#[test]
fn canonical_output_and_numeric_spellings() {
    let cases: Value = serde_json::from_str(include_str!("../../../contracts/cases.json")).unwrap();
    let mut graph = SystemGraph::from_json(&cases["graph"].to_string()).unwrap();
    let canonical = graph.to_json().unwrap();
    graph.nodes.reverse();
    graph.edges.reverse();
    let duplicate = graph.nodes[0].evidence[0].clone();
    graph.nodes[0].evidence.push(duplicate);
    assert_eq!(graph.to_json().unwrap(), canonical);
    SystemGraph::from_json(&canonical).unwrap();
    SystemGraph::from_json(&canonical.replace("\"line\": 1", "\"line\": 1.0")).unwrap();
    graph.nodes[0].id = "bad".into();
    assert!(graph.to_json().is_err());
}
