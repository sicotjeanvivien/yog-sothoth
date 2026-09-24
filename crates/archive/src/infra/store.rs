//! The bucket the dumps go to.
//!
//! The run sees an [`ObjectStore`], the generic trait of `object_store`, and
//! nothing of the provider behind it. This file is the only place that knows
//! the bucket is S3-compatible — Scaleway Object Storage in production, MinIO
//! in a local test — so changing provider means changing this file and
//! [`StoreConfig`], and nothing the run does.

use std::sync::Arc;

use object_store::{ObjectStore, aws::AmazonS3Builder};

use crate::bootstrap::config::StoreConfig;

/// Open the S3-compatible store. Path-style requests, which both Scaleway and
/// a local MinIO accept; plain HTTP only when the endpoint says `http://`.
pub(crate) fn open_store(config: &StoreConfig) -> object_store::Result<Arc<dyn ObjectStore>> {
    let store = AmazonS3Builder::new()
        .with_endpoint(&config.url)
        .with_allow_http(config.url.starts_with("http://"))
        .with_virtual_hosted_style_request(false)
        .with_bucket_name(&config.bucket)
        .with_region(&config.region)
        .with_access_key_id(config.access_key.expose())
        .with_secret_access_key(config.secret_key.expose())
        .build()?;
    Ok(Arc::new(store))
}
