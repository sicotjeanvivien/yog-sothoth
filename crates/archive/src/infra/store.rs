//! The bucket the dumps go to: the run sees an [`ObjectStore`], and this file is the only place
//! that knows it is S3-compatible (Scaleway in production, MinIO locally).

use std::sync::Arc;

use object_store::{ObjectStore, aws::AmazonS3Builder};
use yog_bootstrap::SecretKey;

/// An S3-compatible bucket and the credentials that may write to it.
#[derive(Debug)]
pub(crate) struct StoreSettings {
    /// The endpoint, e.g. `https://s3.fr-par.scw.cloud`. Plain `http://` is
    /// accepted for a local MinIO.
    pub(crate) url: String,
    pub(crate) bucket: String,
    pub(crate) region: String,
    /// Access key id and secret. The key is meant to be **write-only**: a
    /// compromised server must not be able to delete the backups.
    pub(crate) access_key: SecretKey,
    pub(crate) secret_key: SecretKey,
}

/// Open the S3-compatible store. Path-style requests, which both Scaleway and
/// a local MinIO accept; plain HTTP only when the endpoint says `http://`.
pub(crate) fn open_store(settings: &StoreSettings) -> object_store::Result<Arc<dyn ObjectStore>> {
    let store = AmazonS3Builder::new()
        .with_endpoint(&settings.url)
        .with_allow_http(settings.url.starts_with("http://"))
        .with_virtual_hosted_style_request(false)
        .with_bucket_name(&settings.bucket)
        .with_region(&settings.region)
        .with_access_key_id(settings.access_key.expose())
        .with_secret_access_key(settings.secret_key.expose())
        .build()?;
    Ok(Arc::new(store))
}
