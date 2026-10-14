use std::fs;
use std::path::Path;

use serde_json::json;

use super::{document, refuse_int64, require_nullable};

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

/// Red if the walk no longer runs, or also requires a key that may be absent.
#[test]
fn a_nullable_property_is_required_and_an_absent_one_is_not() {
    let mut schema = json!({
        "type": "object",
        "required": ["id"],
        "properties": {
            "id": { "type": "string" },
            "feeBps": { "type": ["string", "null"] },
            "price": { "oneOf": [{ "$ref": "#/components/schemas/P" }, { "type": "null" }] },
            "meteoraDammV2": { "$ref": "#/components/schemas/M" }
        }
    });
    require_nullable(&mut schema);
    assert_eq!(schema["required"], json!(["id", "feeBps", "price"]));
}

#[test]
#[should_panic(expected = "#/properties/swapBuckets24h is an int64")]
fn an_int64_is_refused() {
    let schema = json!({
        "properties": { "swapBuckets24h": { "type": "integer", "format": "int64" } }
    });
    refuse_int64(&schema, "#");
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
