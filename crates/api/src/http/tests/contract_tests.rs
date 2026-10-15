use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;

use super::{document, finish};
use crate::http::query::{MAX_HISTORY_DAYS, MAX_LIMIT, MAX_SEARCH_LEN, MAX_TOP_LIMIT};

const COMMITTED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/openapi.json");

/// Red when a route or a DTO changed without regenerating the committed
/// document. `YOG_WRITE_OPENAPI=1` rewrites it instead of comparing.
#[test]
fn the_committed_document_is_the_one_the_routes_declare() {
    let derived = document();
    if std::env::var("YOG_WRITE_OPENAPI").as_deref() == Ok("1") {
        let text = serde_json::to_string_pretty(&derived).expect("serialises") + "\n";
        fs::write(COMMITTED, text).expect("crates/api/openapi.json is writable");
        return;
    }
    let committed: serde_json::Value = fs::read_to_string(COMMITTED)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
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
    for path in source_files(dir) {
        let source = fs::read_to_string(&path).expect("readable source file");
        for call in [".route(", ".route_service(", ".nest(", ".nest_service("] {
            for (at, _) in source.match_indices(call) {
                let rest = source[at + call.len()..].trim_start();
                if let Some(literal) = rest.strip_prefix('"') {
                    let end = literal.find('"').expect("a closed string literal");
                    found.push((shown(&path), literal[..end].to_string()));
                }
            }
        }
    }
}

/// Every non-test `.rs` file under `dir`, subdirectories included.
fn source_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).expect("readable source directory") {
        let path = entry.expect("readable directory entry").path();
        let name = file_name(&path);
        if path.is_dir() {
            if name != "tests" {
                files.extend(source_files(&path));
            }
        } else if name.ends_with(".rs") && !name.ends_with("_tests.rs") {
            files.push(path);
        }
    }
    files
}

fn shown(path: &Path) -> String {
    path.strip_prefix(env!("CARGO_MANIFEST_DIR"))
        .unwrap_or(path)
        .display()
        .to_string()
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string()
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

/// Red if `finish` stops bounding an unsigned integer by its width, or widens a declared bound.
#[test]
fn an_unsigned_integer_carries_its_maximum() {
    let mut doc = with_schema(json!({ "properties": {
        "decimals": { "type": "integer", "format": "u-int8", "minimum": 0 },
        "eventIndex": { "type": ["integer", "null"], "format": "u-int16", "minimum": 0 },
        "rpcLatencyMs": { "type": "integer", "format": "u-int32", "minimum": 0 },
        "protocolFeePercent": { "type": "integer", "format": "u-int8", "maximum": 100 }
    } }));
    finish(&mut doc);
    let props = &doc["components"]["schemas"]["X"]["properties"];
    assert_eq!(props["decimals"]["maximum"], 255);
    assert_eq!(props["eventIndex"]["maximum"], 65_535);
    assert_eq!(props["rpcLatencyMs"]["maximum"], 4_294_967_295_u64);
    assert_eq!(props["protocolFeePercent"]["maximum"], 100);
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

/// Red if plain text is promised to an operation with nothing to parse, or denied to one with parameters.
#[test]
fn only_an_operation_with_parameters_may_answer_plain_text() {
    let mut doc = json!({
        "paths": {
            "/api/stats": { "get": { "responses": {} } },
            "/api/pools": { "get": { "parameters": [{ "name": "limit", "in": "query" }], "responses": {} } }
        },
        "components": { "schemas": { "ProblemDetailsResponse": {} } }
    });
    finish(&mut doc);
    let content = |path: &str| doc["paths"][path]["get"]["responses"]["default"]["content"].clone();
    assert!(content("/api/stats")["text/plain"].is_null());
    assert!(content("/api/stats")["application/problem+json"].is_object());
    assert!(content("/api/pools")["text/plain"].is_object());
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
        "components": { "schemas": { "ProblemDetailsResponse": {} } }
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
                (_, Some("q")) => {
                    assert_eq!(param["schema"]["maxLength"], MAX_SEARCH_LEN, "{path}");
                    seen += 1;
                    continue;
                }
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
    assert_eq!(seen, 7, "limit on five routes, days and q on one each");
}

/// Red when `skip_serializing_if` and `nullable = false` part: without the
/// second the walk requires the key, without the first the zod refuses `null`.
#[test]
fn an_option_is_omitted_exactly_when_declared_absent() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/http/dto/response");
    let mut unpaired = Vec::new();
    for path in source_files(&dir) {
        let name = shown(&path);
        let lines: Vec<String> = fs::read_to_string(&path)
            .expect("readable source file")
            .lines()
            .map(str::to_string)
            .collect();
        for (i, line) in lines.iter().enumerate() {
            let other = if line.contains("skip_serializing_if") {
                "nullable = false"
            } else if line.contains("nullable = false") {
                "skip_serializing_if"
            } else {
                continue;
            };
            let is_attr = |l: &&String| l.trim_start().starts_with("#[");
            let mut attrs = lines[..i]
                .iter()
                .rev()
                .take_while(is_attr)
                .chain(lines[i..].iter().take_while(is_attr));
            if !attrs.any(|l| l.contains(other)) {
                unpaired.push(format!("{name}:{} lacks `{other}`", i + 1));
            }
        }
    }
    assert!(unpaired.is_empty(), "{unpaired:?}");
}

/// Red when a schema is named without `Response` and is not one of the wire
/// forms `dto/schema.rs` declares.
#[test]
fn every_schema_is_a_response_or_a_wire_form() {
    let schema_rs = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/http/dto/schema.rs");
    let source = fs::read_to_string(schema_rs).expect("readable schema.rs");
    let forms: Vec<&str> = source
        .lines()
        .filter_map(|l| l.strip_prefix("pub(crate) struct ")?.strip_suffix(';'))
        .collect();
    let doc = document();
    let misnamed: Vec<&String> = doc["components"]["schemas"]
        .as_object()
        .expect("schemas")
        .keys()
        .filter(|name| !name.ends_with("Response") && !forms.contains(&name.as_str()))
        .collect();
    assert!(
        misnamed.is_empty(),
        "name these `*Response`, or declare them as wire forms in dto/schema.rs: {misnamed:?}"
    );
}

#[test]
fn every_operation_answers_its_failures_as_problem_details() {
    let doc = document();
    for (path, item) in doc["paths"].as_object().expect("paths") {
        assert_eq!(
            item["get"]["responses"]["default"]["content"]["application/problem+json"]["schema"]["$ref"],
            "#/components/schemas/ProblemDetailsResponse",
            "{path}"
        );
    }
}

/// Red when a response has no `description`, which OpenAPI 3.1 requires.
#[test]
fn every_response_has_a_description() {
    let doc = document();
    let mut missing = Vec::new();
    for (path, item) in doc["paths"].as_object().expect("paths") {
        for method in super::HTTP_METHODS {
            let Some(responses) = item[method]["responses"].as_object() else {
                continue;
            };
            for (status, response) in responses {
                if response["description"].as_str().is_none_or(str::is_empty) {
                    missing.push(format!("{method} {path} {status}"));
                }
            }
        }
    }
    assert!(missing.is_empty(), "{missing:?}");
}

/// Red when a handler's `#[utoipa::path]` declares another body or query type
/// than its signature returns and extracts: nothing else ties the two.
#[test]
fn every_handler_declares_the_body_and_query_it_has() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/http/handlers");
    let mut drift = Vec::new();
    let mut checked = 0;
    for path in source_files(&dir) {
        let name = shown(&path);
        let source = fs::read_to_string(&path).expect("readable source file");
        for (at, _) in source.match_indices("#[utoipa::path(") {
            let rest = &source[at..];
            let attr: String = rest[..rest.find("\n)]").expect("closed attribute")]
                .split_whitespace()
                .collect();
            let fn_at = rest.find("async fn ").expect("the annotated handler");
            let fn_end = fn_at + rest[fn_at..].find('{').expect("a body");
            let sig: String = rest[fn_at..fn_end].split_whitespace().collect();
            let body = between(&attr, "body=", ')').unwrap_or_default();
            let query = between(&attr, "params(", ')').unwrap_or_default();
            let returned = generic_of(&sig, "Json<").unwrap_or("String");
            let extracted = generic_of(&sig, "Query<").unwrap_or_default();
            if body != returned || query != extracted {
                drift.push(format!(
                    "{name}: body {body} for {returned}, params {query} for {extracted}"
                ));
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 14, "one #[utoipa::path] per /api route");
    assert!(drift.is_empty(), "{drift:?}");
}

fn between<'a>(text: &'a str, start: &str, end: char) -> Option<&'a str> {
    let from = text.find(start)? + start.len();
    Some(&text[from..from + text[from..].find(end)?])
}

/// The type argument of the first `wrapper` (`Json<`, `Query<`) in `text`.
fn generic_of<'a>(text: &'a str, wrapper: &str) -> Option<&'a str> {
    let from = text.find(wrapper)? + wrapper.len();
    let mut depth = 1;
    for (i, c) in text[from..].char_indices() {
        match c {
            '<' => depth += 1,
            '>' if depth == 1 => return Some(&text[from..from + i]),
            '>' => depth -= 1,
            _ => {}
        }
    }
    None
}
