use codebasecanvas_analyzer::{SystemGraph, graph_v02::SystemGraphV02};
use serde_json::Value;

#[test]
fn shared_experimental_repository_request_contract() {
    let cases: Value = serde_json::from_str(include_str!(
        "../../../contracts/v02-repository-request.json"
    ))
    .unwrap();
    for case in cases["valid"].as_array().unwrap() {
        let input = case["graph"].to_string();
        let mut graph = SystemGraphV02::from_json(&input)
            .unwrap_or_else(|error| panic!("{}: {error}", case["name"]));
        let original = serde_json::to_value(&graph).unwrap();
        let canonical = graph.to_json().unwrap();
        if case["name"] == "same-file default request" {
            // The Web fixture accepts these exact fields; Rust serialization changes no value.
            assert_eq!(
                serde_json::from_str::<Value>(&canonical).unwrap(),
                case["graph"]
            );
        }
        assert_eq!(
            serde_json::to_value(&graph).unwrap(),
            original,
            "{}",
            case["name"]
        );
        let reread = SystemGraphV02::from_json(&canonical).unwrap();
        assert_eq!(reread.to_json().unwrap(), canonical, "{}", case["name"]);
        assert_eq!(
            reread.framework_declarations.len(),
            case["graph"]["frameworkDeclarations"]
                .as_array()
                .unwrap()
                .len(),
            "{}",
            case["name"]
        );
        graph.framework_declarations.reverse();
        assert_eq!(graph.to_json().unwrap(), canonical, "{}", case["name"]);
        assert!(SystemGraph::from_json(&input).is_err(), "{}", case["name"]);
    }
    for case in cases["invalid"].as_array().unwrap() {
        assert!(
            SystemGraphV02::from_json(&case["graph"].to_string()).is_err(),
            "{}",
            case["name"]
        );
    }
    for case in cases["invalidJson"].as_array().unwrap() {
        assert!(
            SystemGraphV02::from_json(case["raw"].as_str().unwrap()).is_err(),
            "{}",
            case["name"]
        );
    }
    for case in cases["validJson"].as_array().unwrap() {
        let graph = SystemGraphV02::from_json(case["raw"].as_str().unwrap()).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&graph.to_json().unwrap()).unwrap()["nodes"][0]["metadata"]
                ["__proto__"],
            serde_json::from_str::<Value>(case["raw"].as_str().unwrap()).unwrap()["nodes"][0]["metadata"]
                ["__proto__"]
        );
    }
    let mut mixed = cases["valid"][1]["graph"].clone();
    mixed["schemaVersion"] = Value::String("0.1".into());
    assert!(SystemGraph::from_json(&mixed.to_string()).is_err());
}

#[test]
fn invalid_in_memory_graph_cannot_serialize() {
    let cases: Value = serde_json::from_str(include_str!(
        "../../../contracts/v02-repository-request.json"
    ))
    .unwrap();
    let mut graph = SystemGraphV02::from_json(&cases["valid"][1]["graph"].to_string()).unwrap();
    graph.framework_declarations[0].id = "bad".into();
    assert!(graph.to_json().is_err());
}
