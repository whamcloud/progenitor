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

    ///A single page of results
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A single page of results",
    ///  "type": "object",
    ///  "required": [
    ///    "items"
    ///  ],
    ///  "properties": {
    ///    "items": {
    ///      "description": "list of items on this page of results",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "integer",
    ///        "format": "uint32",
    ///        "minimum": 0.0
    ///      }
    ///    },
    ///    "next_page": {
    ///      "description": "token used to fetch the next page of results (if
    /// any)",
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
    pub struct Uint32ResultsPage {
        ///list of items on this page of results
        pub items: ::std::vec::Vec<u32>,
        ///token used to fetch the next page of results (if any)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub next_page: ::std::option::Option<::std::string::String>,
    }

    impl ::std::convert::From<&Uint32ResultsPage> for Uint32ResultsPage {
        fn from(value: &Uint32ResultsPage) -> Self {
            value.clone()
        }
    }

    ///Error enum for the `paginated_u32s` operation
    #[derive(Debug, Clone, :: serde :: Serialize, :: serde :: Deserialize)]
    pub enum PaginatedU32sError {
        # [doc = concat ! ("Error response for status code " , "4")]
        Status4xx(Error),
        # [doc = concat ! ("Error response for status code " , "5")]
        Status5xx(Error),
        /// Error response for an unknown status code
        UnknownValue(serde_json::Value),
    }

    impl std::str::FromStr for PaginatedU32sError {
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
///Client for test_stream_pagination
///
///Version: 1.0.0
pub struct Client {
    pub(crate) baseurl: String,
    pub(crate) client: reqwest::Client,
}

/// Client with middleware support for enhanced request/response processing.
///
/// This client type is only available when the "middleware" feature is enabled.
#[cfg(feature = "middleware")]
#[derive(Clone, Debug)]
///Client for test_stream_pagination
///
///Version: 1.0.0
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
        "1.0.0"
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
        "1.0.0"
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
    ///Sends a 'GET' request to '/'
    ///
    ///Arguments:
    /// - `limit`: Maximum number of items returned by a single call
    /// - `page_token`: Token returned by previous call to retrieve the
    ///   subsequent page
    #[allow(unused_variables)]
    #[allow(irrefutable_let_patterns)]
    pub async fn paginated_u32s<'a>(
        &'a self,
        limit: Option<::std::num::NonZeroU32>,
        page_token: Option<&'a str>,
    ) -> Result<ResponseValue<types::Uint32ResultsPage>, Error<types::PaginatedU32sError>> {
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
            .get(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .query(&progenitor_client::QueryParam::new("limit", &limit))
            .query(&progenitor_client::QueryParam::new(
                "page_token",
                &page_token,
            ))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "paginated_u32s",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::<types::PaginatedU32sError>::from_response::<
                    types::PaginatedU32sError,
                >(response)
                .await?,
            )),
            500u16..=599u16 => Err(Error::ErrorResponse(
                ResponseValue::<types::PaginatedU32sError>::from_response::<
                    types::PaginatedU32sError,
                >(response)
                .await?,
            )),
            _ => Err(Error::UnexpectedResponse(Box::new(response))),
        }
    }

    ///Sends repeated `GET` requests to `/` until there are no more results.
    ///
    ///Arguments:
    /// - `limit`: Maximum number of items returned by a single call
    #[allow(unused_variables)]
    #[allow(irrefutable_let_patterns)]
    pub fn paginated_u32s_stream<'a>(
        &'a self,
        limit: Option<::std::num::NonZeroU32>,
    ) -> impl futures::Stream<Item = Result<u32, Error<types::PaginatedU32sError>>> + Unpin + '_
    {
        use futures::StreamExt;
        use futures::TryFutureExt;
        use futures::TryStreamExt;
        self.paginated_u32s(limit, None)
            .map_ok(move |page| {
                let page = page.into_inner();
                let first = futures::stream::iter(page.items).map(Ok);
                let rest = futures::stream::try_unfold(page.next_page, move |state| async move {
                    if state.is_none() {
                        Ok(None)
                    } else {
                        self.paginated_u32s(limit, state.as_deref())
                            .map_ok(|page| {
                                let page = page.into_inner();
                                Some((futures::stream::iter(page.items).map(Ok), page.next_page))
                            })
                            .await
                    }
                })
                .try_flatten();
                first.chain(rest)
            })
            .try_flatten_stream()
            .boxed()
    }
}

#[cfg(feature = "middleware")]
#[allow(clippy::all)]
#[allow(elided_named_lifetimes)]
impl MiddlewareClient {
    ///Sends a 'GET' request to '/'
    ///
    ///Arguments:
    /// - `limit`: Maximum number of items returned by a single call
    /// - `page_token`: Token returned by previous call to retrieve the
    ///   subsequent page
    #[allow(unused_variables)]
    #[allow(irrefutable_let_patterns)]
    pub async fn paginated_u32s<'a>(
        &'a self,
        limit: Option<::std::num::NonZeroU32>,
        page_token: Option<&'a str>,
    ) -> Result<ResponseValue<types::Uint32ResultsPage>, Error<types::PaginatedU32sError>> {
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
            .get(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .query(&progenitor_client::QueryParam::new("limit", &limit))
            .query(&progenitor_client::QueryParam::new(
                "page_token",
                &page_token,
            ))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "paginated_u32s",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            400u16..=499u16 => Err(Error::ErrorResponse(
                ResponseValue::<types::PaginatedU32sError>::from_response::<
                    types::PaginatedU32sError,
                >(response)
                .await?,
            )),
            500u16..=599u16 => Err(Error::ErrorResponse(
                ResponseValue::<types::PaginatedU32sError>::from_response::<
                    types::PaginatedU32sError,
                >(response)
                .await?,
            )),
            _ => Err(Error::UnexpectedResponse(Box::new(response))),
        }
    }

    ///Sends repeated `GET` requests to `/` until there are no more results.
    ///
    ///Arguments:
    /// - `limit`: Maximum number of items returned by a single call
    #[allow(unused_variables)]
    #[allow(irrefutable_let_patterns)]
    pub fn paginated_u32s_stream<'a>(
        &'a self,
        limit: Option<::std::num::NonZeroU32>,
    ) -> impl futures::Stream<Item = Result<u32, Error<types::PaginatedU32sError>>> + Unpin + '_
    {
        use futures::StreamExt;
        use futures::TryFutureExt;
        use futures::TryStreamExt;
        self.paginated_u32s(limit, None)
            .map_ok(move |page| {
                let page = page.into_inner();
                let first = futures::stream::iter(page.items).map(Ok);
                let rest = futures::stream::try_unfold(page.next_page, move |state| async move {
                    if state.is_none() {
                        Ok(None)
                    } else {
                        self.paginated_u32s(limit, state.as_deref())
                            .map_ok(|page| {
                                let page = page.into_inner();
                                Some((futures::stream::iter(page.items).map(Ok), page.next_page))
                            })
                            .await
                    }
                })
                .try_flatten();
                first.chain(rest)
            })
            .try_flatten_stream()
            .boxed()
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
