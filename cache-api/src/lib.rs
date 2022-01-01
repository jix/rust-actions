//! Rust client for the GitHub Actions Cache API.
//!
//! Note that this API is only documented via the official client's [source code]. As GitHub
//! explicitly supports [pinning specific versions] of their official actions, though, I do not
//! expect frequent changes to this API that break backwards compatibility.
//!
//! [source code]:https://github.com/actions/toolkit/tree/main/packages/cache
//! [pinning specific versions]:https://docs.github.com/en/actions/learn-github-actions/finding-and-customizing-actions#using-shas
use bytes::Bytes;
use reqwest::{Client, RequestBuilder};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that may occur within this crate.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error("did not find a runtime token in the ACTIONS_RUNTIME_TOKEN environment variable")]
    NoRuntimeToken,
    #[error("did not find the endpoint URL in the ACTIONS_CACHE_URL environment variable")]
    NoEndpointUrl,
}

/// Result type used for fallible operations in this crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Metadata for a cache hit.
#[derive(Deserialize, Debug)]
pub struct CacheHit {
    /// The full key under which the found entry was stored.
    #[serde(rename = "cacheKey")]
    pub key: String,
    /// The scope (i.e. the branch which stored the entry).
    pub scope: String,
}

/// Client for the cache API.
///
/// Reusing a single client for multiple requests is potentially more efficient due to connection
/// reuse.
pub struct Cache {
    client: Client,
    token: String,
    endpoint: String,
}

impl Cache {
    /// Creates a new client instance.
    pub fn new() -> Result<Self> {
        let token = std::env::var("ACTIONS_RUNTIME_TOKEN").map_err(|_| Error::NoRuntimeToken)?;

        let endpoint = format!(
            "{}/_apis/artifactcache",
            std::env::var("ACTIONS_CACHE_URL")
                .map_err(|_| Error::NoEndpointUrl)?
                .trim_end_matches('/')
        );

        let client = Client::builder().build()?;

        Ok(Self {
            client,
            token,
            endpoint,
        })
    }

    /// Adds authorization and accept headers needed for an API request.
    fn api_request(&self, builder: RequestBuilder) -> RequestBuilder {
        builder.bearer_auth(&self.token).header(
            reqwest::header::ACCEPT,
            "application/json;api-version=6.0-preview.1",
        )
    }

    /// Performs a cache lookup and returns the URL for a matching entry.
    ///
    /// * `key_space` - parameter is an identifier, usually a hex string, which must match exactly
    /// * `key_prefixes` - list of key prefixes to look up in order of preference
    ///
    /// See the [official documentation] for the precedence in case of multiple matching entries.
    /// Note that `key_space` is not exposed by the official client and thus not mentioned there.
    ///
    /// [official documentation]: https://docs.github.com/en/actions/advanced-guides/caching-dependencies-to-speed-up-workflows#matching-a-cache-key
    pub async fn get_url(
        &self,
        key_space: &str,
        key_prefixes: &[&str],
    ) -> Result<Option<(CacheHit, String)>> {
        #[derive(Deserialize)]
        pub struct GetResponse {
            #[serde(flatten)]
            hit: CacheHit,
            #[serde(rename = "archiveLocation")]
            location: String,
        }

        let response = self
            .api_request(self.client.get(format!("{}/cache", self.endpoint)))
            .query(&[("keys", &*key_prefixes.join(",")), ("version", key_space)])
            .send()
            .await?;

        tracing::debug!(response_headers = ?response.headers());

        if response.status() == reqwest::StatusCode::NO_CONTENT {
            Ok(None)
        } else {
            let response: GetResponse = response.error_for_status()?.json().await?;
            Ok(Some((response.hit, response.location)))
        }
    }

    /// Performs a cache lookup and returns the content of a matching entry.
    ///
    /// See [`get_url`][Self::get_url] for details about the lookup.
    pub async fn get_bytes(
        &self,
        key_space: &str,
        keys: &[&str],
    ) -> Result<Option<(CacheHit, Bytes)>> {
        if let Some((hit, location)) = self.get_url(key_space, keys).await? {
            let response = self.client.get(location).send().await?;

            tracing::debug!(response_headers = ?response.headers());

            Ok(Some((hit, response.bytes().await?)))
        } else {
            Ok(None)
        }
    }

    /// Stores an entry in the cache.
    pub async fn put_bytes(&self, key_space: &str, key: &str, data: Bytes) -> Result<()> {
        #[derive(Serialize)]
        struct ReserveRequest<'a> {
            key: &'a str,
            version: &'a str,
        }
        #[derive(Deserialize)]
        struct ReserveResponse {
            #[serde(rename = "cacheId")]
            cache_id: i64,
        }

        let response = self
            .api_request(self.client.post(format!("{}/caches", self.endpoint)))
            .json(&ReserveRequest {
                key,
                version: key_space,
            })
            .send()
            .await?;

        tracing::debug!(response_headers = ?response.headers());

        let ReserveResponse { cache_id } = response.error_for_status()?.json().await?;

        let response = self
            .api_request(
                self.client
                    .patch(format!("{}/caches/{}", self.endpoint, cache_id)),
            )
            .header(
                reqwest::header::CONTENT_RANGE,
                format!("bytes {}-{}/*", 0, data.len()),
            )
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .body(data.clone())
            .send()
            .await?;

        tracing::debug!(response_headers = ?response.headers());

        response.error_for_status()?;

        #[derive(Serialize)]
        struct RequestBody<'a> {
            key: &'a str,
            version: &'a str,
        }

        #[derive(Serialize)]
        struct FinalizeRequest {
            size: usize,
        }

        let response = self
            .api_request(
                self.client
                    .post(format!("{}/caches/{}", self.endpoint, cache_id)),
            )
            .json(&FinalizeRequest { size: data.len() })
            .send()
            .await?;

        tracing::debug!(response_headers = ?response.headers());

        response.error_for_status()?;
        Ok(())
    }
}
