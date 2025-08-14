#![allow(elided_named_lifetimes)]
#[allow(unused_imports)]
use progenitor_client::{encode_path, ClientHooks, OperationInfo, RequestBuilderExt};
#[allow(unused_imports)]
pub use progenitor_client::{ByteStream, ClientInfo, Error, ResponseValue};
#[cfg(feature = "middleware")]
#[allow(unused_imports)]
pub use reqwest_middleware;
/// Types used as operation parameters and responses.
#[allow(clippy::all)]
pub mod types {
    /// Error types.
    pub mod error {
        /// Error from a `TryFrom` or `FromStr` implementation.
        pub struct ConversionError(::std::borrow::Cow<'static, str>);
        impl ::std::error::Error for ConversionError {}
        impl ::std::fmt::Display for ConversionError {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }

        impl ::std::fmt::Debug for ConversionError {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Debug::fmt(&self.0, f)
            }
        }

        impl From<&'static str> for ConversionError {
            fn from(value: &'static str) -> Self {
                Self(value.into())
            }
        }

        impl From<String> for ConversionError {
            fn from(value: String) -> Self {
                Self(value.into())
            }
        }
    }
}

#[derive(Clone, Debug)]
///Client for Parameter override test
///
///Minimal API for testing parameter overrides
///
///Version: v1
pub struct Client {
    pub(crate) baseurl: String,
    pub(crate) client: reqwest::Client,
}

/// Client with middleware support for enhanced request/response processing.
///
/// This client type is only available when the "middleware" feature is enabled.
#[cfg(feature = "middleware")]
#[derive(Clone, Debug)]
///Client for Parameter override test
///
///Minimal API for testing parameter overrides
///
///Version: v1
pub struct MiddlewareClient {
    pub(crate) baseurl: String,
    pub(crate) client: reqwest_middleware::ClientWithMiddleware,
}

impl Client {
    /// Create a new client.
    ///
    /// `baseurl` is the base URL provided to the internal
    /// `reqwest::Client`, and should include a scheme and hostname,
    /// as well as port and a path stem if applicable.
    pub fn new(baseurl: &str) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let client = {
            let dur = std::time::Duration::from_secs(15);
            reqwest::ClientBuilder::new()
                .connect_timeout(dur)
                .timeout(dur)
        };
        #[cfg(target_arch = "wasm32")]
        let client = reqwest::ClientBuilder::new();
        Self::new_with_client(
            baseurl,
            client.build().expect("Failed to build HTTP client"),
        )
    }

    /// Construct a new client with an existing `reqwest::Client`,
    /// allowing more control over its configuration.
    ///
    /// `baseurl` is the base URL provided to the internal
    /// `reqwest::Client`, and should include a scheme and hostname,
    /// as well as port and a path stem if applicable.
    pub fn new_with_client(baseurl: &str, client: reqwest::Client) -> Self {
        Self {
            baseurl: baseurl.to_string(),
            client,
        }
    }

    /// Construct a new client with an existing
    /// `reqwest_middleware::ClientWithMiddleware`,
    /// allowing the use of middleware for requests.
    ///
    /// `baseurl` is the base URL provided to the internal client, and should
    /// include
    /// a scheme and hostname, as well as port and a path stem if applicable.
    ///
    /// This method is only available when the "middleware" feature is enabled.
    #[cfg(feature = "middleware")]
    pub fn new_with_client_middleware(
        baseurl: &str,
        client: reqwest_middleware::ClientWithMiddleware,
    ) -> MiddlewareClient {
        MiddlewareClient {
            baseurl: baseurl.to_string(),
            client,
        }
    }
}

impl ClientInfo<(), reqwest::Client> for Client {
    fn api_version() -> &'static str {
        "v1"
    }

    fn baseurl(&self) -> &str {
        self.baseurl.as_str()
    }

    fn client(&self) -> &reqwest::Client {
        &self.client
    }

    fn inner(&self) -> &() {
        &()
    }
}

impl ClientHooks<(), reqwest::Client> for &Client {}
#[cfg(feature = "middleware")]
impl ClientHooks<(), reqwest_middleware::ClientWithMiddleware> for &MiddlewareClient {}
#[cfg(feature = "middleware")]
impl ClientInfo<(), reqwest_middleware::ClientWithMiddleware> for MiddlewareClient {
    fn api_version() -> &'static str {
        "v1"
    }

    fn baseurl(&self) -> &str {
        self.baseurl.as_str()
    }

    fn client(&self) -> &reqwest_middleware::ClientWithMiddleware {
        &self.client
    }

    fn inner(&self) -> &() {
        &()
    }
}

#[allow(clippy::all)]
#[allow(mismatched_lifetime_syntaxes)]
impl Client {
    ///Gets a key
    ///
    ///Sends a 'GET' request to '/key'
    ///
    ///Arguments:
    /// - `key`: The same key parameter that overlaps with the path level
    ///   parameter
    /// - `unique_key`: A key parameter that will not be overridden by the path
    ///   spec
    #[allow(unused_variables)]
    #[allow(irrefutable_let_patterns)]
    pub async fn key_get<'a>(
        &'a self,
        key: Option<bool>,
        unique_key: Option<&'a str>,
    ) -> Result<ResponseValue<()>, Error<()>> {
        let url = format!("{}/key", self.baseurl(),);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        #[allow(unused_variables)]
        let mut request = self
            .client()
            .get(url)
            .query(&progenitor_client::QueryParam::new("key", &key))
            .query(&progenitor_client::QueryParam::new(
                "uniqueKey",
                &unique_key,
            ))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "key_get",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => Ok(ResponseValue::empty(response)),
            _ => Err(Error::ErrorResponse(ResponseValue::empty(response))),
        }
    }
}

#[cfg(feature = "middleware")]
#[allow(clippy::all)]
#[allow(elided_named_lifetimes)]
impl MiddlewareClient {
    ///Gets a key
    ///
    ///Sends a 'GET' request to '/key'
    ///
    ///Arguments:
    /// - `key`: The same key parameter that overlaps with the path level
    ///   parameter
    /// - `unique_key`: A key parameter that will not be overridden by the path
    ///   spec
    #[allow(unused_variables)]
    #[allow(irrefutable_let_patterns)]
    pub async fn key_get<'a>(
        &'a self,
        key: Option<bool>,
        unique_key: Option<&'a str>,
    ) -> Result<ResponseValue<()>, Error<()>> {
        let url = format!("{}/key", self.baseurl(),);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        #[allow(unused_variables)]
        let mut request = self
            .client()
            .get(url)
            .query(&progenitor_client::QueryParam::new("key", &key))
            .query(&progenitor_client::QueryParam::new(
                "uniqueKey",
                &unique_key,
            ))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "key_get",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => Ok(ResponseValue::empty(response)),
            _ => Err(Error::ErrorResponse(ResponseValue::empty(response))),
        }
    }
}

/// Items consumers will typically use such as the Client.
pub mod prelude {
    #[allow(unused_imports)]
    pub use super::Client;
    #[cfg(feature = "middleware")]
    #[allow(unused_imports)]
    pub use super::MiddlewareClient;
}
