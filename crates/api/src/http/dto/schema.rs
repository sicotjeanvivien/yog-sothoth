//! Schema-only types: each names a wire form of the API contract and never holds
//! a value. A DTO field points at one with `#[schema(value_type = …)]`.

use utoipa::openapi::{ObjectBuilder, RefOr, Schema, Type};
use utoipa::{PartialSchema, ToSchema};

/// A non-negative decimal written as a string — amounts, prices, rates.
pub(crate) struct UnsignedDecimal;

/// A decimal that carries a direction, written as a string (`"-0.2157"`).
pub(crate) struct SignedDecimal;

/// An integer written as a string of digits: it can exceed 2^53.
pub(crate) struct U128String;

/// A count or an id, as a JSON number.
///
/// ⚠️ Never `int64`: the web's generated zod reads an `int64` as a `BigInt`.
pub(crate) struct Count;

fn string_matching(pattern: &str) -> RefOr<Schema> {
    ObjectBuilder::new()
        .schema_type(Type::String)
        .pattern(Some(pattern))
        .into()
}

impl PartialSchema for UnsignedDecimal {
    fn schema() -> RefOr<Schema> {
        string_matching(r"^\d+(\.\d+)?$")
    }
}
impl ToSchema for UnsignedDecimal {}

impl PartialSchema for SignedDecimal {
    fn schema() -> RefOr<Schema> {
        string_matching(r"^-?\d+(\.\d+)?$")
    }
}
impl ToSchema for SignedDecimal {}

impl PartialSchema for U128String {
    fn schema() -> RefOr<Schema> {
        string_matching(r"^\d+$")
    }
}
impl ToSchema for U128String {}

impl PartialSchema for Count {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .schema_type(Type::Integer)
            .minimum(Some(0))
            .into()
    }
}
impl ToSchema for Count {}
