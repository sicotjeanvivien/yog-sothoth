use std::fs;
use std::path::Path;

use serde_json::json;

use super::{document, finish};
use crate::http::query::{MAX_HISTORY_DAYS, MAX_LIMIT, MAX_TOP_LIMIT};

const COMMITTED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/openapi.json");

/// Red when a route or a DTO changed without regenerating the committed
/// document. `YOG_WRITE_OPENAPI=1` rewrites it instead of comparing.
#[test]
fn the_committed_document_is_the_one_the_routes_declare() {
    let derived = serde_json::to_string_pretty(&document()).expect("serialises") + "\n";
    if std::env::var_os("YOG_WRITE_OPENAPI").is_some() {
        fs::write(COMMITTED, &derived).expect("crates/api/openapi.json is writable");
        return;
    }
    let committed = fs::read_to_string(COMMITTED).unwrap_or_default();
    assert!(
        committed == derived,
        "crates/api/openapi.json is stale: run `YOG_WRITE_OPENAPI=1 cargo test -p yog-api contract`, \
         then `npm run gen:api` in web/"
    );
}

/// Red when a route is mounted with a literal path — `Router::route`, or
/// `OpenApiRouter::route`, which serves it without a contract — outside the probes.
#[test]
fn every_api_route_is_mounted_with_its_contract() {
    let mut found = Vec::new();
    literal_routes(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut found,
    );
    found.retain(|(_, path)| path != "/healthz" && path != "/readyz");
    assert!(
        found.is_empty(),
        "mount these with `.routes(routes!(handler))` and a `#[utoipa::path]`: {found:?}"
    );
}

fn literal_routes(dir: &Path, found: &mut Vec<(String, String)>) {
    for entry in fs::read_dir(dir).expect("readable source directory") {
        let path = entry.expect("readable directory entry").path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if path.is_dir() {
            if name != "tests" {
                literal_routes(&path, found);
            }
            continue;
        }
        if !name.ends_with(".rs") || name.ends_with("_tests.rs") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("readable source file");
        for call in [".route(", ".route_service(", ".nest(", ".nest_service("] {
            for (at, _) in source.match_indices(call) {
                let rest = source[at + call.len()..].trim_start();
                if let Some(literal) = rest.strip_prefix('"') {
                    let end = literal.find('"').expect("a closed string literal");
                    found.push((name.to_string(), literal[..end].to_string()));
                }
            }
        }
    }
}

#[test]
fn every_api_route_is_in_the_document() {
    let doc = document();
    let paths = doc["paths"].as_object().expect("paths");
    assert_eq!(paths.len(), 14, "{:?}", paths.keys().collect::<Vec<_>>());
}

/// A document of one schema, as `finish` receives it.
fn with_schema(schema: serde_json::Value) -> serde_json::Value {
    json!({ "paths": {}, "components": { "schemas": { "P": {}, "M": {}, "X": schema } } })
}

/// Red if `finish` stops running the walk, or the walk requires a key that may be absent.
#[test]
fn a_nullable_property_is_required_and_an_absent_one_is_not() {
    let mut doc = with_schema(json!({
        "type": "object",
        "required": ["id"],
        "properties": {
            "id": { "type": "string" },
            "feeBps": { "type": ["string", "null"] },
            "price": { "oneOf": [{ "$ref": "#/components/schemas/P" }, { "type": "null" }] },
            "meteoraDammV2": { "$ref": "#/components/schemas/M" }
        }
    }));
    finish(&mut doc);
    assert_eq!(
        doc["components"]["schemas"]["X"]["required"],
        json!(["id", "feeBps", "price"])
    );
}

/// Red if `finish` stops refusing an `int64`.
#[test]
#[should_panic(expected = "#/components/schemas/X/properties/swapBuckets24h is an int64")]
fn an_int64_is_refused() {
    let mut doc = with_schema(json!({
        "properties": { "swapBuckets24h": { "type": "integer", "format": "int64" } }
    }));
    finish(&mut doc);
}

/// Red if `finish` stops refusing a `$ref` to a schema it does not declare.
#[test]
#[should_panic(expected = "undeclared schemas: [\"PoolSortParam\"]")]
fn a_ref_to_an_undeclared_schema_is_refused() {
    let mut doc = with_schema(json!({
        "properties": { "sort": { "$ref": "#/components/schemas/PoolSortParam" } }
    }));
    finish(&mut doc);
}

/// Red if a path item's `parameters` or `summary` is treated as an operation.
#[test]
fn only_the_operations_of_a_path_item_answer_problems() {
    let mut doc = json!({
        "paths": { "/api/x": {
            "summary": "x",
            "parameters": [{ "name": "address", "in": "path" }],
            "get": { "responses": {} }
        } },
        "components": { "schemas": { "ProblemDetails": {} } }
    });
    finish(&mut doc);
    let item = &doc["paths"]["/api/x"];
    assert!(item["get"]["responses"]["default"].is_object());
    assert_eq!(item["summary"], "x");
}

/// Red when a bound written in a `#[param]` drifts from the one the server validates.
#[test]
fn the_documented_bounds_are_the_validated_ones() {
    let doc = document();
    let mut seen = 0;
    for (path, item) in doc["paths"].as_object().expect("paths") {
        for param in item["get"]["parameters"].as_array().into_iter().flatten() {
            let max = match (path.as_str(), param["name"].as_str()) {
                ("/api/pools/top", Some("limit")) => MAX_TOP_LIMIT,
                (_, Some("limit")) => MAX_LIMIT,
                (_, Some("days")) => MAX_HISTORY_DAYS,
                _ => continue,
            };
            assert_eq!(param["schema"]["minimum"], 1, "{path}");
            assert_eq!(param["schema"]["maximum"], max, "{path}");
            seen += 1;
        }
    }
    assert_eq!(seen, 6, "limit on five routes, days on one");
}

/// Red when an `Option` the API omits (`skip_serializing_if`) is not declared
/// `nullable = false`: the walk would make it a required key.
#[test]
fn every_omitted_option_is_declared_absent_not_null() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/http/dto/response");
    let mut undeclared = Vec::new();
    for entry in fs::read_dir(&dir).expect("readable response directory") {
        let path = entry.expect("readable entry").path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.ends_with(".rs") {
            continue;
        }
        let lines: Vec<String> = fs::read_to_string(&path)
            .expect("readable source file")
            .lines()
            .map(str::to_string)
            .collect();
        for (i, line) in lines.iter().enumerate() {
            if !line.contains("skip_serializing_if") {
                continue;
            }
            let attrs = lines[..i]
                .iter()
                .rev()
                .take_while(|l| l.trim_start().starts_with("#["))
                .chain(
                    lines[i..]
                        .iter()
                        .take_while(|l| l.trim_start().starts_with("#[")),
                );
            if !attrs.into_iter().any(|l| l.contains("nullable = false")) {
                undeclared.push(format!("{name}:{}", i + 1));
            }
        }
    }
    assert!(
        undeclared.is_empty(),
        "add `#[schema(nullable = false)]`: {undeclared:?}"
    );
}

#[test]
fn every_operation_answers_its_failures_as_problem_details() {
    let doc = document();
    for (path, item) in doc["paths"].as_object().expect("paths") {
        assert_eq!(
            item["get"]["responses"]["default"]["content"]["application/problem+json"]["schema"]["$ref"],
            "#/components/schemas/ProblemDetails",
            "{path}"
        );
    }
}
