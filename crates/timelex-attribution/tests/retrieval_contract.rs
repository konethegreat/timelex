// Written by Kone Tshivhinda & Claude
//! The Python worker and this workspace share one retrieval contract. These tests keep the frozen
//! examples honest: every request and response validates against its schema, the pair agrees on the
//! tenant, and a document cannot smuggle a namespace of its own.

use serde_json::{json, Value};

const REQUEST_SCHEMA: &str = include_str!("../../../contracts/attribution/retrieval-request.schema.json");
const RESPONSE_SCHEMA: &str = include_str!("../../../contracts/attribution/retrieval-response.schema.json");
const INDEX_REQUEST: &str = include_str!("../../../contracts/examples/retrieval-index-request.v1.0.json");
const INDEX_RESPONSE: &str = include_str!("../../../contracts/examples/retrieval-index-response.v1.0.json");
const SEARCH_REQUEST: &str = include_str!("../../../contracts/examples/retrieval-search-request.v1.0.json");
const SEARCH_RESPONSE: &str = include_str!("../../../contracts/examples/retrieval-search-response.v1.0.json");

fn validator(schema: &str) -> jsonschema::Validator {
    let schema: Value = serde_json::from_str(schema).expect("schema is valid JSON");
    jsonschema::validator_for(&schema).expect("schema compiles")
}

fn value(example: &str) -> Value {
    serde_json::from_str(example).expect("example is valid JSON")
}

#[test]
fn both_retrieval_schemas_compile() {
    validator(REQUEST_SCHEMA);
    validator(RESPONSE_SCHEMA);
}

#[test]
fn frozen_examples_satisfy_the_contract() {
    let request = validator(REQUEST_SCHEMA);
    let response = validator(RESPONSE_SCHEMA);
    for example in [INDEX_REQUEST, SEARCH_REQUEST] {
        let instance = value(example);
        assert!(request.is_valid(&instance), "{instance}");
    }
    for example in [INDEX_RESPONSE, SEARCH_RESPONSE] {
        let instance = value(example);
        assert!(response.is_valid(&instance), "{instance}");
    }
}

#[test]
fn each_response_answers_its_request_in_the_same_tenant() {
    for (request, response) in [(INDEX_REQUEST, INDEX_RESPONSE), (SEARCH_REQUEST, SEARCH_RESPONSE)] {
        let request = value(request);
        let response = value(response);
        assert_eq!(response["tenantId"], request["tenantId"]);
        assert_eq!(response["operation"], request["operation"]);
        assert_eq!(response["schemaVersion"], request["schemaVersion"]);
    }
}

#[test]
fn a_document_cannot_name_its_own_tenant() {
    let mut request = value(INDEX_REQUEST);
    request["documents"][0]["tenantId"] = json!("tn_otherfirm");
    assert!(!validator(REQUEST_SCHEMA).is_valid(&request));
}

#[test]
fn an_unlisted_field_is_rejected() {
    let mut request = value(SEARCH_REQUEST);
    request["office"] = json!("johannesburg");
    assert!(!validator(REQUEST_SCHEMA).is_valid(&request));

    let mut response = value(SEARCH_RESPONSE);
    response["matches"][0]["text"] = json!("lease termination notice");
    assert!(!validator(RESPONSE_SCHEMA).is_valid(&response));
}

#[test]
fn an_index_request_cannot_carry_a_query_and_the_reverse() {
    let mut index = value(INDEX_REQUEST);
    index["query"] = json!("lease");
    assert!(!validator(REQUEST_SCHEMA).is_valid(&index));

    let mut search = value(SEARCH_REQUEST);
    search["documents"] = value(INDEX_REQUEST)["documents"].clone();
    assert!(!validator(REQUEST_SCHEMA).is_valid(&search));
}

#[test]
fn the_frozen_search_score_is_rounded_to_six_decimals() {
    let response = value(SEARCH_RESPONSE);
    let score = response["matches"][0]["score"].as_f64().expect("numeric score");
    assert_eq!(format!("{score:.6}").parse::<f64>().unwrap(), score);
    assert!((0.0..=1.0).contains(&score));
}
