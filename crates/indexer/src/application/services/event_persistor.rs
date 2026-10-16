//! Top-level event persistor: routes each `DomainEvent` to its protocol's sub-persistor.

use std::sync::Arc;
use yog_core::domain::DomainEvent;

use crate::application::services::MeteoraDammV2EventPersistor;

pub(crate) struct EventPersistor {
    meteora_damm_v2: Arc<MeteoraDammV2EventPersistor>,
}

impl EventPersistor {
    pub(crate) fn new(meteora_damm_v2: Arc<MeteoraDammV2EventPersistor>) -> Self {
        Self { meteora_damm_v2 }
    }

    /// Route a domain event to the protocol-specific persistor.
    pub(crate) async fn persist(&self, event: &DomainEvent) {
        match event {
            DomainEvent::MeteoraDammV2(e) => self.meteora_damm_v2.persist(e).await,
        }
    }
}
