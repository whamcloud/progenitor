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

    ///`BodyWithDefaults`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "s"
    ///  ],
    ///  "properties": {
    ///    "forty-two": {
    ///      "default": 42,
    ///      "type": "integer",
    ///      "format": "uint32",
    ///      "minimum": 0.0
    ///    },
    ///    "s": {
    ///      "type": "string"
    ///    },
    ///    "something": {
    ///      "default": true,
    ///      "type": [
    ///        "boolean",
    ///        "null"
    ///      ]
    ///    },
    ///    "yes": {
    ///      "default": false,
    ///      "type": "boolean"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct BodyWithDefaults {
        #[serde(rename = "forty-two", default = "defaults::default_u64::<u32, 42>")]
        pub forty_two: u32,
        pub s: ::std::string::String,
        #[serde(default = "defaults::body_with_defaults_something")]
        pub something: ::std::option::Option<bool>,
        #[serde(default)]
        pub yes: bool,
    }

    impl ::std::convert::From<&BodyWithDefaults> for BodyWithDefaults {
        fn from(value: &BodyWithDefaults) -> Self {
            value.clone()
        }
    }

    ///Error information from a response.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Error information from a response.",
    ///  "type": "object",
    ///  "required": [
    ///    "message",
    ///    "request_id"
    ///  ],
    ///  "properties": {
    ///    "error_code": {
    ///      "type": "string"
    ///    },
    ///    "message": {
    ///      "type": "string"
    ///    },
    ///    "request_id": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct Error {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub error_code: ::std::option::Option<::std::string::String>,
        pub message: ::std::string::String,
        pub request_id: ::std::string::String,
    }

    impl ::std::convert::From<&Error> for Error {
        fn from(value: &Error) -> Self {
            value.clone()
        }
    }

    /// Generation of default values for serde.
    pub mod defaults {
        pub(super) fn default_u64<T, const V: u64>() -> T
        where
            T: ::std::convert::TryFrom<u64>,
            <T as ::std::convert::TryFrom<u64>>::Error: ::std::fmt::Debug,
        {
            T::try_from(V).unwrap()
        }

        pub(super) fn body_with_defaults_something() -> ::std::option::Option<bool> {
            ::std::option::Option::Some(true)
        }
    }
}

#[derive(Clone, Debug)]
///Client for pagination-demo
///
///Version: 9000.0.0
pub struct Client {
    pub(crate) baseurl: String,
    pub(crate) client: reqwest::Client,
}

/// Client with middleware support for enhanced request/response processing.
///
/// This client type is only available when the "middleware" feature is enabled.
#[cfg(feature = "middleware")]
#[derive(Clone, Debug)]
///Client for pagination-demo
///
///Version: 9000.0.0
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
        "9000.0.0"
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
        "9000.0.0"
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
#[allow(elided_named_lifetimes)]
impl Client {
    ///Sends a 'POST' request to '/'
    #[allow(unused_variables)]
    #[allow(irrefutable_let_patterns)]
    pub async fn default_params<'a>(
        &'a self,
        body: &'a types::BodyWithDefaults,
    ) -> Result<ResponseValue<ByteStream>, Error<()>> {
        let url = format!("{}/", self.baseurl(),);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        #[allow(unused_variables)]
        let mut request = self
            .client()
            .post(url)
            .header(
                ::reqwest::header::CONTENT_TYPE,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .body(serde_json::to_string(&body).map_err(|e| Error::InvalidRequest(e.to_string()))?)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "default_params",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200..=299 => Ok(ResponseValue::stream(response)),
            _ => Err(Error::ErrorResponse(ResponseValue::empty(response))),
        }
    }
}

#[cfg(feature = "middleware")]
#[allow(clippy::all)]
#[allow(elided_named_lifetimes)]
impl MiddlewareClient {
    ///Sends a 'POST' request to '/'
    #[allow(unused_variables)]
    #[allow(irrefutable_let_patterns)]
    pub async fn default_params<'a>(
        &'a self,
        body: &'a types::BodyWithDefaults,
    ) -> Result<ResponseValue<ByteStream>, Error<()>> {
        let url = format!("{}/", self.baseurl(),);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        #[allow(unused_variables)]
        let mut request = self
            .client()
            .post(url)
            .header(
                ::reqwest::header::CONTENT_TYPE,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .body(serde_json::to_string(&body).map_err(|e| Error::InvalidRequest(e.to_string()))?)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "default_params",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200..=299 => Ok(ResponseValue::stream(response)),
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
