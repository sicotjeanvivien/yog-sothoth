/// A value the store cannot hold or give back as its domain type; the persistence layer
/// drops the whole group and names the field, the caller logs and counts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldOutOfRange {
    pub field: &'static str,
}
