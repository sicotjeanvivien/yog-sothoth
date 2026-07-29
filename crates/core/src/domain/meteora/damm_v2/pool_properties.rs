// NOTE: this module has no repository trait of its own any more.
//
// It held `MeteoraDammV2PoolPropertiesRepository`, the indexer's write contract
// for the satellite (`set_fee_config`, `set_has_dynamic_fee`). Both went with
// the indexer's property writes: the satellite is now written only by
// yog-context, through the generic `PoolAccountResolver`, and read only through
// the generic `PoolPropertiesLookup`. Neither names this protocol, so neither
// belongs here.

mod model;

pub use model::{MeteoraDammV2PoolAccountProperties, MeteoraDammV2PoolProperties};
