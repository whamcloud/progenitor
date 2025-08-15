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

    ///Error enum for the `renamed_parameters` operation
    #[derive(Debug, Clone, :: serde :: Serialize, :: serde :: Deserialize)]
    pub enum RenamedParametersError {
        # [doc = concat ! ("Error response for status code " , "4")]
        Status4xx(Error),
        # [doc = concat ! ("Error response for status code " , "5")]
        Status5xx(Error),
        /// Error response for an unknown status code
        UnknownValue(serde_json::Value),
    }

    impl std::str::FromStr for RenamedParametersError {
        type Err = std::string::String;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let (status_code, value) = match s.split_once(':') {
                Some((status_code, value)) => (status_code, value),
                None => return Err("Unable to split status code and value".to_string()),
            };
            let status_code: u16 = match status_code.parse() {
                Ok(code) => code,
                Err(e) => return Err(format!("Unable to parse status code: {}", e)),
            };
            match status_code {
                _ => match serde_json::from_str(value) {
                    Ok(json_value) => Ok(Self::UnknownValue(json_value)),
                    Err(_) => Err("Unable to parse as JSON".to_string()),
                },
            }
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
#[allow(mismatched_lifetime_syntaxes)]
impl Client {
    ///Sends a 'GET' request to '/{ref}/{type}/{trait}'
    #[allow(unused_variables)]
    #[allow(irrefutable_let_patterns)]
    pub async fn renamed_parameters<'a>(
        &'a self,
        ref_: &'a str,
        type_: &'a str,
        trait_: &'a str,
        if_: &'a str,
        in_: &'a str,
        use_: &'a str,
    ) -> Result<ResponseValue<()>, Error<types::RenamedParametersError>> {
        let url = format!(
            "{}/{}/{}/{}",
            self.baseurl(),
            encode_path(&ref_.to_string()),
            encode_path(&type_.to_string()),
            encode_path(&trait_.to_string()),
        );
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
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .query(&progenitor_client::QueryParam::new("if", &if_))
            .query(&progenitor_client::QueryParam::new("in", &in_))
            .query(&progenitor_client::QueryParam::new("use", &use_))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "renamed_parameters",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            204u16 => Ok(ResponseValue::empty(response)),
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::<types::RenamedParametersError>::from_response::<
                    types::RenamedParametersError,
                >(response)
                .await?,
            )),
            500u16..=599u16 => Err(Error::ErrorResponse(
                ResponseValue::<types::RenamedParametersError>::from_response::<
                    types::RenamedParametersError,
                >(response)
                .await?,
            )),
            _ => Err(Error::UnexpectedResponse(Box::new(response))),
        }
    }
}

#[cfg(feature = "middleware")]
#[allow(clippy::all)]
#[allow(mismatched_lifetime_syntaxes)]
impl MiddlewareClient {
    ///Sends a 'GET' request to '/{ref}/{type}/{trait}'
    #[allow(unused_variables)]
    #[allow(irrefutable_let_patterns)]
    pub async fn renamed_parameters<'a>(
        &'a self,
        ref_: &'a str,
        type_: &'a str,
        trait_: &'a str,
        if_: &'a str,
        in_: &'a str,
        use_: &'a str,
    ) -> Result<ResponseValue<()>, Error<types::RenamedParametersError>> {
        let url = format!(
            "{}/{}/{}/{}",
            self.baseurl(),
            encode_path(&ref_.to_string()),
            encode_path(&type_.to_string()),
            encode_path(&trait_.to_string()),
        );
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
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .query(&progenitor_client::QueryParam::new("if", &if_))
            .query(&progenitor_client::QueryParam::new("in", &in_))
            .query(&progenitor_client::QueryParam::new("use", &use_))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "renamed_parameters",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            204u16 => Ok(ResponseValue::empty(response)),
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::<types::RenamedParametersError>::from_response::<
                    types::RenamedParametersError,
                >(response)
                .await?,
            )),
            500u16..=599u16 => Err(Error::ErrorResponse(
                ResponseValue::<types::RenamedParametersError>::from_response::<
                    types::RenamedParametersError,
                >(response)
                .await?,
            )),
            _ => Err(Error::UnexpectedResponse(Box::new(response))),
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
