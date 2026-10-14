//! The API contract: the OpenAPI document read from [`super::api_routes`], as
//! committed in `crates/api/openapi.json`, from which the web generates its zod.
//! Only tests build it — the API does not serve it.

use std::time::Duration;

use serde_json::{Value, json};
use utoipa::openapi::{ComponentsBuilder, InfoBuilder, OpenApiBuilder};
use utoipa_axum::router::OpenApiRouter;

use crate::application::WorkSlots;
use crate::http::dto::response::{PROBLEM_CONTENT_TYPE, ProblemDetails};

/// The document as committed: the routes' declarations, then [`finish`].
pub(crate) fn document() -> Value {
    let base = OpenApiBuilder::new()
        .info(
            InfoBuilder::new()
                .title("Yog-Scope API")
                .version(env!("CARGO_PKG_VERSION"))
                .build(),
        )
        .components(Some(
            ComponentsBuilder::new()
                .schema_from::<ProblemDetails>()
                .build(),
        ))
        .build();
    let routes = super::api_routes(WorkSlots::new(1, Duration::ZERO));
    let declared = OpenApiRouter::with_openapi(base)
        .merge(routes)
        .into_openapi();
    let mut doc = serde_json::to_value(declared).expect("the OpenAPI document serialises");
    finish(&mut doc);
    doc
}

/// The rules the declarations leave implicit, applied once to the whole
/// document rather than repeated on every field and route.
fn finish(doc: &mut Value) {
    answer_problems(doc);
    require_nullable(doc);
    refuse_int64(doc, "#");
    refuse_dangling_refs(doc);
}

/// The keys of a path item that are operations; it also holds `parameters`, `summary`…
const HTTP_METHODS: [&str; 8] = [
    "get", "put", "post", "delete", "options", "head", "patch", "trace",
];

/// Every operation answers its failures as RFC 9457 Problem Details — except
/// a query string or a path axum cannot parse, which it refuses in plain text.
fn answer_problems(doc: &mut Value) {
    let problem = json!({
        "description": "A failure: RFC 9457 Problem Details, or plain text when the \
                        query string or the path cannot be parsed.",
        "content": {
            PROBLEM_CONTENT_TYPE: { "schema": { "$ref": "#/components/schemas/ProblemDetails" } },
            "text/plain": { "schema": { "type": "string" } }
        }
    });
    let Some(paths) = doc["paths"].as_object_mut() else {
        return;
    };
    for item in paths.values_mut() {
        let Some(item) = item.as_object_mut() else {
            continue;
        };
        for (method, operation) in item.iter_mut() {
            if HTTP_METHODS.contains(&method.as_str()) {
                operation["responses"]["default"] = problem.clone();
            }
        }
    }
}

/// A nullable property is a required one: the API writes `null`, it never
/// omits the key. A key that may be absent is declared `nullable = false`.
fn require_nullable(node: &mut Value) {
    match node {
        Value::Object(map) => {
            let nullable: Vec<Value> = map
                .get("properties")
                .and_then(Value::as_object)
                .map(|props| {
                    props
                        .iter()
                        .filter(|(_, schema)| is_nullable(schema))
                        .map(|(name, _)| json!(name))
                        .collect()
                })
                .unwrap_or_default();
            if !nullable.is_empty() {
                let required = map
                    .entry("required")
                    .or_insert_with(|| json!([]))
                    .as_array_mut()
                    .expect("`required` of an object schema is an array");
                for name in nullable {
                    if !required.contains(&name) {
                        required.push(name);
                    }
                }
            }
            map.values_mut().for_each(require_nullable);
        }
        Value::Array(items) => items.iter_mut().for_each(require_nullable),
        _ => {}
    }
}

fn is_nullable(schema: &Value) -> bool {
    let null = json!("null");
    schema["type"]
        .as_array()
        .is_some_and(|types| types.contains(&null))
        || schema["oneOf"]
            .as_array()
            .is_some_and(|variants| variants.iter().any(|v| v["type"] == null))
}

/// Panics on an `int64` anywhere in the document: the web's generated zod reads
/// one as a `BigInt`. The field points at `Count` instead.
fn refuse_int64(node: &Value, at: &str) {
    match node {
        Value::Object(map) => {
            if let Some(format @ ("int64" | "uint64" | "u-int64")) =
                map.get("format").and_then(Value::as_str)
            {
                panic!("{at} is an {format}: point it at `Count` (`value_type = Count`)");
            }
            for (key, value) in map {
                refuse_int64(value, &format!("{at}/{key}"));
            }
        }
        Value::Array(items) => {
            for (i, value) in items.iter().enumerate() {
                refuse_int64(value, &format!("{at}/{i}"));
            }
        }
        _ => {}
    }
}

/// Panics on a `$ref` to a schema the document does not declare: the generator
/// skips it, and the zod of that field silently disappears.
fn refuse_dangling_refs(doc: &Value) {
    let declared = doc["components"]["schemas"]
        .as_object()
        .expect("the document declares schemas");
    let mut refs = Vec::new();
    collect_refs(doc, &mut refs);
    let mut dangling: Vec<&str> = refs
        .iter()
        .filter_map(|r| r.strip_prefix("#/components/schemas/"))
        .filter(|name| !declared.contains_key(*name))
        .collect();
    dangling.sort_unstable();
    dangling.dedup();
    assert!(dangling.is_empty(), "undeclared schemas: {dangling:?}");
}

fn collect_refs<'a>(node: &'a Value, refs: &mut Vec<&'a str>) {
    match node {
        Value::Object(map) => {
            if let Some(target) = map.get("$ref").and_then(Value::as_str) {
                refs.push(target);
            }
            map.values().for_each(|v| collect_refs(v, refs));
        }
        Value::Array(items) => items.iter().for_each(|v| collect_refs(v, refs)),
        _ => {}
    }
}

#[cfg(test)]
#[path = "contract_tests.rs"]
mod tests;
