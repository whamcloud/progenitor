// Copyright 2025 Oxide Computer Company

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

use derive_more::Display;

use heck::ToPascalCase;
use openapiv3::{Components, Parameter, ReferenceOr, Response, StatusCode};
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use typify::{TypeId, TypeSpace};

use crate::{
    template::PathTemplate,
    util::{items, parameter_map, sanitize, unique_ident_from, Case},
    Error, Generator, Result, TagStyle,
};
use crate::{to_schema::ToSchema, util::ReferenceOrExt};

/// The intermediate representation of an operation that will become a method.
pub(crate) struct OperationMethod {
    pub operation_id: String,
    pub tags: Vec<String>,
    pub method: HttpMethod,
    pub path: PathTemplate,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub params: Vec<OperationParameter>,
    pub responses: Vec<OperationResponse>,
    pub dropshot_paginated: Option<DropshotPagination>,
    dropshot_websocket: bool,
}

pub enum HttpMethod {
    Get,
    Put,
    Post,
    Delete,
    Options,
    Head,
    Patch,
    Trace,
}

impl std::str::FromStr for HttpMethod {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "get" => Ok(Self::Get),
            "put" => Ok(Self::Put),
            "post" => Ok(Self::Post),
            "delete" => Ok(Self::Delete),
            "options" => Ok(Self::Options),
            "head" => Ok(Self::Head),
            "patch" => Ok(Self::Patch),
            "trace" => Ok(Self::Trace),
            _ => Err(Error::InternalError(format!("bad method: {s}"))),
        }
    }
}
impl HttpMethod {
    fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::Get => "get",
            HttpMethod::Put => "put",
            HttpMethod::Post => "post",
            HttpMethod::Delete => "delete",
            HttpMethod::Options => "options",
            HttpMethod::Head => "head",
            HttpMethod::Patch => "patch",
            HttpMethod::Trace => "trace",
        }
    }
}

struct MethodSigBody {
    success: TokenStream,
    error: TokenStream,
    body: TokenStream,
}

struct BuilderImpl {
    doc: String,
    sig: TokenStream,
    body: TokenStream,
}

pub struct DropshotPagination {
    pub item: TypeId,
    pub first_page_params: Vec<String>,
}

pub struct OperationParameter {
    /// Sanitized parameter name.
    pub name: String,
    /// Original parameter name provided by the API.
    pub api_name: String,
    pub description: Option<String>,
    pub typ: OperationParameterType,
    pub kind: OperationParameterKind,
}

#[derive(Eq, PartialEq)]
pub enum OperationParameterType {
    Type(TypeId),
    RawBody,
}

#[derive(Debug, PartialEq, Eq)]
pub enum OperationParameterKind {
    Path,
    Query(bool),
    Header(bool),
    // TODO bodies may be optional
    Body(BodyContentType),
}

impl OperationParameterKind {
    fn is_required(&self) -> bool {
        match self {
            OperationParameterKind::Path => true,
            OperationParameterKind::Query(required) => *required,
            OperationParameterKind::Header(required) => *required,
            // TODO may be optional
            OperationParameterKind::Body(_) => true,
        }
    }
    fn is_optional(&self) -> bool {
        !self.is_required()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum BodyContentType {
    OctetStream,
    Json,
    FormUrlencoded,
    Text(String),
}

impl FromStr for BodyContentType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let offset = s.find(';').unwrap_or(s.len());
        match &s[..offset] {
            "application/octet-stream" => Ok(Self::OctetStream),
            "application/json" => Ok(Self::Json),
            "application/x-www-form-urlencoded" => Ok(Self::FormUrlencoded),
            s if s.starts_with("text/") => {
                // Handle any text/* content type
                Ok(BodyContentType::Text(s.to_string()))
            }
            // Be more lenient with other content types - treat them as octet-stream
            _ => {
                eprintln!(
                    "Warning: Treating unknown content type '{s}' as application/octet-stream"
                );
                Ok(BodyContentType::OctetStream)
            }
        }
    }
}

impl std::fmt::Display for BodyContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::OctetStream => "application/octet-stream",
            Self::Json => "application/json",
            Self::FormUrlencoded => "application/x-www-form-urlencoded",
            Self::Text(typ) => typ,
        })
    }
}

#[derive(Debug)]
pub(crate) struct OperationResponse {
    pub status_code: OperationResponseStatus,
    pub typ: OperationResponseKind,
    // TODO this isn't currently used because dropshot doesn't give us a
    // particularly useful message here.
    #[allow(dead_code)]
    description: Option<String>,
}

impl Eq for OperationResponse {}
impl PartialEq for OperationResponse {
    fn eq(&self, other: &Self) -> bool {
        self.status_code == other.status_code
    }
}
impl Ord for OperationResponse {
    fn cmp(&self, other: &Self) -> Ordering {
        self.status_code.cmp(&other.status_code)
    }
}
impl PartialOrd for OperationResponse {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Display, Hash)]
pub(crate) enum OperationResponseStatus {
    Code(u16),
    Range(u16),
    Default,
}

impl OperationResponseStatus {
    fn to_value(&self) -> u16 {
        match self {
            OperationResponseStatus::Code(code) => {
                assert!(*code < 1000);
                *code
            }
            OperationResponseStatus::Range(range) => {
                assert!(*range < 10);
                *range * 100
            }
            OperationResponseStatus::Default => 1000,
        }
    }

    pub fn is_success_or_default(&self) -> bool {
        matches!(
            self,
            OperationResponseStatus::Default
                | OperationResponseStatus::Code(101)
                | OperationResponseStatus::Code(200..=299)
                | OperationResponseStatus::Range(2)
        )
    }

    pub fn is_error_or_default(&self) -> bool {
        matches!(
            self,
            OperationResponseStatus::Default
                | OperationResponseStatus::Code(400..=599)
                | OperationResponseStatus::Range(4..=5)
        )
    }

    #[allow(dead_code)]
    pub fn is_default(&self) -> bool {
        matches!(self, OperationResponseStatus::Default)
    }
}

impl Ord for OperationResponseStatus {
    fn cmp(&self, other: &Self) -> Ordering {
        self.to_value().cmp(&other.to_value())
    }
}

impl PartialOrd for OperationResponseStatus {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub(crate) enum OperationResponseKind {
    Type(TypeId),
    None,
    Raw,
    Upgrade,
    // New variant for multiple response types
    Multiple {
        // Map of status code to type ID
        variants: BTreeMap<OperationResponseStatus, TypeId>,
        // Name for the enum that will contain all variants
        enum_name: String,
    },
}

impl OperationResponseKind {
    pub fn into_tokens(self, type_space: &TypeSpace) -> TokenStream {
        match self {
            OperationResponseKind::Type(type_id) => {
                let type_name = type_space.get_type(&type_id).unwrap().ident();
                quote! { #type_name }
            }
            OperationResponseKind::None => {
                quote! { () }
            }
            OperationResponseKind::Raw => {
                quote! { ByteStream }
            }
            OperationResponseKind::Upgrade => {
                quote! { reqwest::Upgraded }
            }
            OperationResponseKind::Multiple { enum_name, .. } => {
                // For multiple response types, we'll generate an enum
                let enum_ident = format_ident!("{}", enum_name);
                quote! { #enum_ident }
            }
        }
    }
}

impl Generator {
    pub(crate) fn process_operation(
        &mut self,
        operation: &openapiv3::Operation,
        components: &Option<Components>,
        path: &str,
        method: &str,
        path_parameters: &[ReferenceOr<Parameter>],
    ) -> Result<OperationMethod> {
        let operation_id = operation.operation_id.as_ref().unwrap();

        let mut combined_path_parameters = parameter_map(path_parameters, components)?;
        for operation_param in items(&operation.parameters, components) {
            let parameter = operation_param?;
            combined_path_parameters.insert(&parameter.parameter_data_ref().name, parameter);
        }

        // Filter out any path parameters that have been overridden by an
        // operation parameter
        let mut params = combined_path_parameters
            .values()
            .map(|parameter| {
                match parameter {
                    openapiv3::Parameter::Path {
                        parameter_data,
                        style: openapiv3::PathStyle::Simple,
                    } => {
                        // Path parameters MUST be required.
                        assert!(parameter_data.required);

                        let schema = parameter_data.schema()?.to_schema();

                        let name = sanitize(
                            &format!("{}-{}", operation_id, &parameter_data.name),
                            Case::Pascal,
                        );
                        let typ = self.type_space.add_type_with_name(&schema, Some(name))?;

                        Ok(OperationParameter {
                            name: sanitize(&parameter_data.name, Case::Snake),
                            api_name: parameter_data.name.clone(),
                            description: parameter_data.description.clone(),
                            typ: OperationParameterType::Type(typ),
                            kind: OperationParameterKind::Path,
                        })
                    }
                    openapiv3::Parameter::Query {
                        parameter_data,
                        allow_reserved: _, // We always encode reserved chars
                        style: openapiv3::QueryStyle::Form,
                        allow_empty_value: _, // Irrelevant for this client
                    } => {
                        let schema = parameter_data.schema()?.to_schema();
                        let name = sanitize(
                            &format!(
                                "{}-{}",
                                operation.operation_id.as_ref().unwrap(),
                                &parameter_data.name,
                            ),
                            Case::Pascal,
                        );

                        let type_id = self.type_space.add_type_with_name(&schema, Some(name))?;

                        let ty = self.type_space.get_type(&type_id).unwrap();

                        // If the type is itself optional, then we'll treat it
                        // as optional (irrespective of the `required` field on
                        // the parameter) and use the "inner" type.
                        let details = ty.details();
                        let (type_id, required) =
                            if let typify::TypeDetails::Option(inner_type_id) = details {
                                (inner_type_id, false)
                            } else {
                                (type_id, parameter_data.required)
                            };

                        Ok(OperationParameter {
                            name: sanitize(&parameter_data.name, Case::Snake),
                            api_name: parameter_data.name.clone(),
                            description: parameter_data.description.clone(),
                            typ: OperationParameterType::Type(type_id),
                            kind: OperationParameterKind::Query(required),
                        })
                    }
                    openapiv3::Parameter::Header {
                        parameter_data,
                        style: openapiv3::HeaderStyle::Simple,
                    } => {
                        let schema = parameter_data.schema()?.to_schema();
                        let name = sanitize(
                            &format!(
                                "{}-{}",
                                operation.operation_id.as_ref().unwrap(),
                                &parameter_data.name,
                            ),
                            Case::Pascal,
                        );

                        let typ = self.type_space.add_type_with_name(&schema, Some(name))?;

                        Ok(OperationParameter {
                            name: sanitize(&parameter_data.name, Case::Snake),
                            api_name: parameter_data.name.clone(),
                            description: parameter_data.description.clone(),
                            typ: OperationParameterType::Type(typ),
                            kind: OperationParameterKind::Header(parameter_data.required),
                        })
                    }
                    openapiv3::Parameter::Path { style, .. } => Err(Error::UnexpectedFormat(
                        format!("unsupported style of path parameter {style:#?}",),
                    )),
                    openapiv3::Parameter::Query { style, .. } => Err(Error::UnexpectedFormat(
                        format!("unsupported style of query parameter {style:#?}",),
                    )),
                    cookie @ openapiv3::Parameter::Cookie { .. } => Err(Error::UnexpectedFormat(
                        format!("cookie parameters are not supported {cookie:#?}",),
                    )),
                }
            })
            .collect::<Result<Vec<_>>>()?;

        let dropshot_websocket = operation.extensions.get("x-dropshot-websocket").is_some();
        if dropshot_websocket {
            self.uses_websockets = true;
        }

        if let Some(body_param) = self.get_body_param(operation, components, method)? {
            params.push(body_param);
        }

        let tmp = crate::template::parse(path)?;
        let names = tmp.names();

        sort_params(&mut params, &names);

        let mut success = false;

        let mut responses =
            operation
                .responses
                .default
                .iter()
                .map(|response_or_ref| {
                    Ok((
                        OperationResponseStatus::Default,
                        response_or_ref.item(components)?,
                    ))
                })
                .chain(operation.responses.responses.iter().map(
                    |(status_code, response_or_ref)| {
                        Ok((
                            match status_code {
                                StatusCode::Code(code) => OperationResponseStatus::Code(*code),
                                StatusCode::Range(range) => OperationResponseStatus::Range(*range),
                            },
                            response_or_ref.item(components)?,
                        ))
                    },
                ))
                .map(|v: Result<(OperationResponseStatus, &Response)>| {
                    let (status_code, response) = v?;

                    // We categorize responses as "typed" based on the
                    // "application/json" content type, "upgrade" if it's a
                    // websocket channel without a meaningful content-type,
                    // "raw" if there's any other response content type (we don't
                    // investigate further), or "none" if there is no content.
                    // TODO if there are multiple response content types we could
                    // treat those like different response types and create an
                    // enum; the generated client method would check for the
                    // content type of the response just as it currently examines
                    // the status code.
                    let typ = if let Some(mt) = response.content.iter().find_map(|(x, v)| {
                        (x == "application/json" || x.starts_with("application/json;")).then_some(v)
                    }) {
                        assert!(mt.encoding.is_empty());

                        let typ = if let Some(schema) = &mt.schema {
                            let schema = schema.to_schema();
                            let name = sanitize(
                                &format!("{}-response", operation.operation_id.as_ref().unwrap(),),
                                Case::Pascal,
                            );
                            self.type_space.add_type_with_name(&schema, Some(name))?
                        } else {
                            todo!("media type encoding, no schema: {:#?}", mt);
                        };

                        OperationResponseKind::Type(typ)
                    } else if dropshot_websocket {
                        OperationResponseKind::Upgrade
                    } else if response.content.first().is_some() {
                        OperationResponseKind::Raw
                    } else {
                        OperationResponseKind::None
                    };

                    // See if there's a status code that covers success cases.
                    if matches!(
                        status_code,
                        OperationResponseStatus::Default
                            | OperationResponseStatus::Code(200..=299)
                            | OperationResponseStatus::Range(2)
                    ) {
                        success = true;
                    }

                    let description = if response.description.is_empty() {
                        None
                    } else {
                        Some(response.description.clone())
                    };

                    Ok(OperationResponse {
                        status_code,
                        typ,
                        description,
                    })
                })
                .collect::<Result<Vec<_>>>()?;

        // If the API has declined to specify the characteristics of a
        // successful response, we cons up a generic one. Note that this is
        // technically permissible within OpenAPI, but advised against by the
        // spec.
        if !success {
            responses.push(OperationResponse {
                status_code: OperationResponseStatus::Range(2),
                typ: OperationResponseKind::Raw,
                description: None,
            });
        }

        // Must accept HTTP 101 Switching Protocols
        if dropshot_websocket {
            responses.push(OperationResponse {
                status_code: OperationResponseStatus::Code(101),
                typ: OperationResponseKind::Upgrade,
                description: None,
            })
        }

        let dropshot_paginated = self.dropshot_pagination_data(operation, &params, &responses);

        if dropshot_websocket && dropshot_paginated.is_some() {
            return Err(Error::InvalidExtension(format!(
                "conflicting extensions in {operation_id:?}"
            )));
        }

        Ok(OperationMethod {
            operation_id: sanitize(operation_id, Case::Snake),
            tags: operation.tags.clone(),
            method: HttpMethod::from_str(method)?,
            path: tmp,
            summary: operation.summary.clone().filter(|s| !s.is_empty()),
            description: operation.description.clone().filter(|s| !s.is_empty()),
            params,
            responses,
            dropshot_paginated,
            dropshot_websocket,
        })
    }

    pub(crate) fn positional_method(
        &mut self,
        method: &OperationMethod,
        has_inner: bool,
    ) -> Result<TokenStream> {
        let operation_id = format_ident!("{}", method.operation_id);

        // Render each parameter as it will appear in the method signature.
        let params = method
            .params
            .iter()
            .map(|param| {
                let name = format_ident!("{}", param.name);
                let typ = match (&param.typ, param.kind.is_optional()) {
                    (OperationParameterType::Type(type_id), false) => self
                        .type_space
                        .get_type(type_id)
                        .unwrap()
                        .parameter_ident_with_lifetime("a"),
                    (OperationParameterType::Type(type_id), true) => {
                        let t = self
                            .type_space
                            .get_type(type_id)
                            .unwrap()
                            .parameter_ident_with_lifetime("a");
                        quote! { Option<#t> }
                    }
                    (OperationParameterType::RawBody, false) => match &param.kind {
                        OperationParameterKind::Body(BodyContentType::OctetStream) => {
                            quote! { B }
                        }
                        OperationParameterKind::Body(BodyContentType::Text(_)) => {
                            quote! { String }
                        }
                        _ => unreachable!(),
                    },
                    (OperationParameterType::RawBody, true) => unreachable!(),
                };
                quote! {
                    #name: #typ
                }
            })
            .collect::<Vec<_>>();

        let raw_body_param = method.params.iter().any(|param| {
            param.typ == OperationParameterType::RawBody
                && param.kind == OperationParameterKind::Body(BodyContentType::OctetStream)
        });

        let bounds = if raw_body_param {
            quote! { <'a, B: Into<reqwest::Body> > }
        } else {
            quote! { <'a> }
        };

        let doc_comment = make_doc_comment(method);

        let MethodSigBody {
            success: success_type,
            error: error_type,
            body,
        } = self.method_sig_body(method, quote! { Self }, quote! { self }, has_inner)?;

        let method_impl = quote! {
            #[doc = #doc_comment]
            #[allow(unused_variables)]
            #[allow(irrefutable_let_patterns)]
            pub async fn #operation_id #bounds (
                &'a self,
                #(#params),*
            ) -> Result<
                ResponseValue<#success_type>,
                Error<#error_type>,
            > {
                #body
            }
        };

        let stream_impl = method.dropshot_paginated.as_ref().map(|page_data| {
            // We're now using futures.
            self.uses_futures = true;

            let stream_id = format_ident!("{}_stream", method.operation_id);

            // The parameters are the same as those to the paged method, but
            // without "page_token"
            let stream_params = method
                .params
                .iter()
                .zip(params)
                .filter_map(|(param, stream)| {
                    if param.name.as_str() == "page_token" {
                        None
                    } else {
                        Some(stream)
                    }
                });

            // The values passed to get the first page are the inputs to the
            // stream method with "None" for the page_token.
            let first_params = method.params.iter().map(|param| {
                if param.api_name.as_str() == "page_token" {
                    // The page_token is None when getting the first page.
                    quote! { None }
                } else {
                    // All other parameters are passed through directly.
                    format_ident!("{}", param.name).to_token_stream()
                }
            });

            // The values passed to get subsequent pages are...
            // - the state variable for the page_token
            // - None for all other query parameters
            // - The initial inputs for non-query parameters
            let step_params = method.params.iter().map(|param| {
                if param.api_name.as_str() == "page_token" {
                    quote! { state.as_deref() }
                } else if param.api_name.as_str() != "limit"
                    && matches!(param.kind, OperationParameterKind::Query(_))
                {
                    // Query parameters (other than "page_token" and "limit")
                    // are None; having page_token as Some(_) is mutually
                    // exclusive with other query parameters.
                    quote! { None }
                } else {
                    // Non-query parameters are passed in; this is necessary
                    // e.g. to specify the right path. (We don't really expect
                    // to see a body parameter here, but we pass it through
                    // regardless.)
                    format_ident!("{}", param.name).to_token_stream()
                }
            });

            // The item type that we've saved (by picking apart the original
            // function's return type) will be the Item type parameter for the
            // Stream type we return.
            let item = self.type_space.get_type(&page_data.item).unwrap();
            let item_type = item.ident();

            let doc_comment = make_stream_doc_comment(method);

            quote! {
                #[doc = #doc_comment]
                #[allow(unused_variables)]
                #[allow(irrefutable_let_patterns)]
                pub fn #stream_id #bounds (
                    &'a self,
                    #(#stream_params),*
                ) -> impl futures::Stream<Item = Result<
                    #item_type,
                    Error<#error_type>,
                >> + Unpin + '_ {
                    use futures::StreamExt;
                    use futures::TryFutureExt;
                    use futures::TryStreamExt;

                    // Execute the operation with the basic parameters
                    // (omitting page_token) to get the first page.
                    self.#operation_id( #(#first_params,)* )
                        .map_ok(move |page| {
                            let page = page.into_inner();

                            // Create a stream from the items of the first page.
                            let first =
                                futures::stream::iter(page.items).map(Ok);

                            // We unfold subsequent pages using page.next_page
                            // as the seed value. Each iteration returns its
                            // items and the next page token.
                            let rest = futures::stream::try_unfold(
                                page.next_page,
                                move |state| async move {
                                    if state.is_none() {
                                        // The page_token was None so we've
                                        // reached the end.
                                        Ok(None)
                                    } else {
                                        // Get the next page; here we set all
                                        // query parameters to None (except for
                                        // the page_token), and all other
                                        // parameters as specified at the start
                                        // of this method.
                                        self.#operation_id(
                                            #(#step_params,)*
                                        )
                                        .map_ok(|page| {
                                            let page = page.into_inner();
                                            Some((
                                                futures::stream::iter(
                                                    page.items
                                                ).map(Ok),
                                                page.next_page,
                                            ))
                                        })
                                        .await
                                    }
                                },
                            )
                            .try_flatten();

                            first.chain(rest)
                        })
                        .try_flatten_stream()
                        .boxed()
                }
            }
        });

        let all = quote! {
            #method_impl
            #stream_impl
        };

        Ok(all)
    }

    /// Common code generation between positional and builder interface-styles.
    /// Returns a struct with the success and error types and the core body
    /// implementation that marshals arguments and executes the request.
    fn method_sig_body(
        &self,
        method: &OperationMethod,
        client_type: TokenStream,
        client_value: TokenStream,
        has_inner: bool,
    ) -> Result<MethodSigBody> {
        let param_names = method
            .params
            .iter()
            .map(|param| format_ident!("{}", param.name))
            .collect::<Vec<_>>();

        // Generate a unique Ident for internal variables
        let url_ident = unique_ident_from("url", &param_names);
        let request_ident = unique_ident_from("request", &param_names);
        let response_ident = unique_ident_from("response", &param_names);
        let result_ident = unique_ident_from("result", &param_names);

        // Generate code for query parameters.
        let query_params = method
            .params
            .iter()
            .filter_map(|param| match &param.kind {
                OperationParameterKind::Query(_) => {
                    let qn = &param.api_name;
                    let qn_ident = format_ident!("{}", &param.name);
                    Some(quote! {
                        &progenitor_client::QueryParam::new(#qn, &#qn_ident)
                    })
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        // Headers
        let headers = method
            .params
            .iter()
            .filter_map(|param| match &param.kind {
                OperationParameterKind::Header(required) => {
                    let hn = &param.api_name;
                    let hn_ident = format_ident!("{}", &param.name);
                    let res = if *required {
                        quote! {
                            header_map.append(
                                #hn,
                                #hn_ident.to_string().try_into()?
                            );
                        }
                    } else {
                        quote! {
                            if let Some(value) = #hn_ident {
                                header_map.append(
                                    #hn,
                                    value.to_string().try_into()?
                                );
                            }
                        }
                    };
                    Some(res)
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        let headers_size = headers.len() + 1;
        let headers_build = quote! {
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(#headers_size);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(#client_type::api_version()),
            );

            #(#headers)*
        };

        let headers_use = quote! {
            .headers(header_map)
        };

        let websock_hdrs = if method.dropshot_websocket {
            quote! {
                .header(::reqwest::header::CONNECTION, "Upgrade")
                .header(::reqwest::header::UPGRADE, "websocket")
                .header(::reqwest::header::SEC_WEBSOCKET_VERSION, "13")
                .header(
                    ::reqwest::header::SEC_WEBSOCKET_KEY,
                    ::base64::Engine::encode(
                        &::base64::engine::general_purpose::STANDARD,
                        ::rand::random::<[u8; 16]>(),
                    )
                )
            }
        } else {
            quote! {}
        };

        // Generate the path rename map; then use it to generate code for
        // assigning the path parameters to the `url` variable.
        let url_renames = method
            .params
            .iter()
            .filter_map(|param| match &param.kind {
                OperationParameterKind::Path => Some((&param.api_name, &param.name)),
                _ => None,
            })
            .collect();

        let url_path = method.path.compile(url_renames, client_value.clone());
        let url_path = quote! {
            let #url_ident = #url_path;
        };

        // Generate code to handle the body param.
        let body_func = method
            .params
            .iter()
            .filter_map(|param| match (&param.kind, &param.typ) {
                (
                    OperationParameterKind::Body(BodyContentType::OctetStream),
                    OperationParameterType::RawBody,
                ) => Some(quote! {
                    // Set the content type (this is handled by helper
                    // functions for other MIME types).
                    .header(
                        ::reqwest::header::CONTENT_TYPE,
                        ::reqwest::header::HeaderValue::from_static("application/octet-stream"),
                    )
                    .body(body)
                }),
                (
                    OperationParameterKind::Body(BodyContentType::Text(mime_type)),
                    OperationParameterType::RawBody,
                ) => Some(quote! {
                    // Set the content type (this is handled by helper
                    // functions for other MIME types).
                    .header(
                        ::reqwest::header::CONTENT_TYPE,
                        ::reqwest::header::HeaderValue::from_static(#mime_type),
                    )
                    .body(body)
                }),
                (
                    OperationParameterKind::Body(BodyContentType::Json),
                    OperationParameterType::Type(_),
                ) => Some(quote! {
                    // Manual JSON serialization for reqwest-middleware compatibility
                    .header(
                        ::reqwest::header::CONTENT_TYPE,
                        ::reqwest::header::HeaderValue::from_static("application/json"),
                    )
                    .body(serde_json::to_string(&body).map_err(|e| Error::InvalidRequest(e.to_string()))?)
                }),
                (
                    OperationParameterKind::Body(BodyContentType::FormUrlencoded),
                    OperationParameterType::Type(_),
                ) => Some(quote! {
                    // This uses progenitor_client::RequestBuilderExt which
                    // returns an error in the case of a serialization failure.
                    .form_urlencoded(&body)?
                }),
                (OperationParameterKind::Body(_), _) => {
                    unreachable!("invalid body kind/type combination")
                }
                _ => None,
            });
        // ... and there can be at most one body.
        assert!(body_func.clone().count() <= 1);

        // Success response handling
        let (success_response_items, response_type_kind) =
            self.extract_responses(method, OperationResponseStatus::is_success_or_default);

        let (success_type, success_response_matches) = match &response_type_kind {
            OperationResponseKind::Multiple { enum_name, .. } => {
                let enum_ident = format_ident!("{}", enum_name);
                let matches = success_response_items.iter().map(|response| {
                    let pat = match &response.status_code {
                        OperationResponseStatus::Code(code) => quote! { #code },
                        OperationResponseStatus::Range(_) | OperationResponseStatus::Default => {
                            quote! { 200 ..= 299 }
                        }
                    };
                    let variant_name = match &response.status_code {
                        OperationResponseStatus::Code(code) => format_ident!("Status{}", code),
                        OperationResponseStatus::Range(range) => format_ident!("Status{}xx", range),
                        OperationResponseStatus::Default => format_ident!("Default"),
                    };
                    quote! {
                        #pat => {
                            ResponseValue::from_response(#response_ident).await
                                .map(|v| types::#enum_ident::#variant_name(v))
                        }
                    }
                });
                (quote! { types::#enum_ident }, matches.collect::<Vec<_>>())
            }
            _ => {
                let matches = success_response_items.iter().map(|response| {
                    let pat = match &response.status_code {
                        OperationResponseStatus::Code(code) => quote! { #code },
                        OperationResponseStatus::Range(_) | OperationResponseStatus::Default => {
                            quote! { 200 ..= 299 }
                        }
                    };
                    match &response.typ {
                        OperationResponseKind::Type(_) => quote! {
                            #pat => {
                                ResponseValue::from_response(#response_ident).await
                            }
                        },
                        OperationResponseKind::None => quote! {
                            #pat => {
                                Ok(ResponseValue::empty(#response_ident))
                            }
                        },
                        OperationResponseKind::Raw => quote! {
                            #pat => {
                                Ok(ResponseValue::stream(#response_ident))
                            }
                        },
                        OperationResponseKind::Upgrade => quote! {
                            #pat => {
                                ResponseValue::upgrade(#response_ident).await
                            }
                        },
                        OperationResponseKind::Multiple { .. } => unreachable!(),
                    }
                });
                (
                    response_type_kind.clone().into_tokens(&self.type_space),
                    matches.collect::<Vec<_>>(),
                )
            }
        };

        // Error response handling
        let (error_response_items, error_type_kind) =
            self.extract_responses(method, OperationResponseStatus::is_error_or_default);

        // Check if we actually have error responses that would generate an enum
        let has_actual_error_responses = error_response_items.iter().any(|response| {
            matches!(
                response.status_code,
                OperationResponseStatus::Code(400..=599) | OperationResponseStatus::Range(4..=5)
            )
        });

        let (error_type, error_response_matches, default_response) = if !has_actual_error_responses
        {
            (
                quote! { () },
                Vec::<TokenStream>::new(),
                quote! {
                    _ => Err(Error::ErrorResponse(ResponseValue::empty(#response_ident))),
                },
            )
        } else {
            let error_enum_name = format!("{}Error", method.operation_id.to_pascal_case());
            let error_enum_ident = format_ident!("{}", error_enum_name);

            let matches = error_response_items.iter().map(|response| {
            let pat = match &response.status_code {
                OperationResponseStatus::Code(code) => quote! { #code },
                OperationResponseStatus::Range(r) => {
                    let min = r * 100;
                    let max = min + 99;
                    quote! { #min ..= #max }
                }
                OperationResponseStatus::Default => quote! { _ },
            };
            let variant_name = match &response.status_code {
                OperationResponseStatus::Code(code) => format_ident!("Status{}", code),
                OperationResponseStatus::Range(range) => format_ident!("Status{}xx", range),
                OperationResponseStatus::Default => format_ident!("Default"),
            };
            match &response.typ {
                OperationResponseKind::Type(_) => {
                    quote! {
                        #pat => {
                            Err(Error::ErrorResponse(
                                ResponseValue::<types::#error_enum_ident>::from_response::<types::#error_enum_ident>(#response_ident)
                                    .await?
                            ))
                        }
                    }
                }
                OperationResponseKind::None => quote! {
                    #pat => {
                        Err(Error::ErrorResponse(
                                ResponseValue::<types::#error_enum_ident>::from_response::<types::#error_enum_ident>(#response_ident)
                                    .await?
                        ))
                    }
                },
                OperationResponseKind::Raw => quote! {
                    #pat => {
                        Err(Error::ErrorResponse(
                            ResponseValue::stream(#response_ident)
                                .map(|s| types::#error_enum_ident::#variant_name(s))
                        ))
                    }
                },
                OperationResponseKind::Upgrade => quote! {
                    #pat => {
                        Err(Error::ErrorResponse(
                            ResponseValue::upgrade(#response_ident).await
                                .map(|u| types::#error_enum_ident::#variant_name(u))
                        ))
                    }
                },
                OperationResponseKind::Multiple { enum_name, .. } => {
                    let response_enum_ident = format_ident!("{}", enum_name);
                    quote! {
                        #pat => {
                            Err(Error::ErrorResponse(
                                ResponseValue::from_response::<types::#response_enum_ident>(#response_ident)
                                    .await?
                                    .map(|v| types::#error_enum_ident::#variant_name(v))
                            ))
                        }
                    }
                }
            }
        }).collect::<Vec<_>>();

            let default_response = match error_response_items
                .iter()
                .any(|r| matches!(r.status_code, OperationResponseStatus::Default))
            {
                true => quote! {},
                false => quote! {
                    _ => {
                        Err(Error::UnexpectedResponse(Box::new(#response_ident)))

                    },
                },
            };

            (
                quote! { types::#error_enum_ident },
                matches,
                default_response,
            )
        };

        let accept_header = matches!(
            (&response_type_kind, &error_type_kind),
            (OperationResponseKind::Type(_), _)
                | (OperationResponseKind::None, OperationResponseKind::Type(_))
        )
        .then(|| {
            quote! {
                    .header(
                        ::reqwest::header::ACCEPT,
                        ::reqwest::header::HeaderValue::from_static(
                            "application/json",
                        ),
                    )
            }
        });

        let inner = match has_inner {
            true => quote! { &#client_value.inner, },
            false => quote! {},
        };
        let pre_hook = self.settings.pre_hook.as_ref().map(|hook| {
            quote! {
                (#hook)(#inner &#request_ident);
            }
        });
        let pre_hook_async = self.settings.pre_hook_async.as_ref().map(|hook| {
            quote! {
                match (#hook)(#inner &mut #request_ident).await {
                    Ok(_) => (),
                    Err(e) => return Err(Error::Custom(e.to_string())),
                }
            }
        });
        let _post_hook = self.settings.post_hook.as_ref().map(|hook| {
            quote! {
                (#hook)(#inner &#result_ident);
            }
        });
        let _post_hook_async = self.settings.post_hook_async.as_ref().map(|hook| {
            quote! {
                match (#hook)(#inner &#result_ident).await {
                    Ok(_) => (),
                    Err(e) => return Err(Error::Custom(e.to_string())),
                }
            }
        });

        let operation_id = &method.operation_id;
        let method_func = format_ident!("{}", method.method.as_str());

        let body_impl = quote! {
            #url_path

            #headers_build

            #[allow(unused_mut)]
            #[allow(unused_variables)]
            let mut #request_ident = #client_value.client()
                . #method_func (#url_ident)
                #accept_header
                #(#body_func)*
                #( .query(#query_params) )*
                #headers_use
                #websock_hdrs
                .build()?;

            let info = OperationInfo {
                operation_id: #operation_id,
            };

            #pre_hook
            #pre_hook_async
            #client_value
                .pre(&mut #request_ident, &info)
                .await?;

            let #result_ident = #client_value
                .exec(#request_ident, &info)
                .await;

            #client_value
                .post(&#result_ident, &info)
                .await?;

            let #response_ident = #result_ident?;

            match #response_ident.status().as_u16() {
                // These will be of the form...
                // 201 => ResponseValue::from_response(response).await,
                // 200..299 => ResponseValue::empty(response),
                // TODO this kind of enumerated response isn't implemented
                // ... or in the case of an operation with multiple
                // successful response types...
                // 200 => {
                //     ResponseValue::from_response()
                //         .await?
                //         .map(OperationXResponse::ResponseTypeA)
                // }
                // 201 => {
                //     ResponseValue::from_response()
                //         .await?
                //         .map(OperationXResponse::ResponseTypeB)
                // }
                #(#success_response_matches)*

                // This is almost identical to the success types except
                // they are wrapped in Error::ErrorResponse...
                // 400 => {
                //     Err(Error::ErrorResponse(
                //         ResponseValue::from_response(response.await?)
                //     ))
                // }
                #(#error_response_matches)*

                // The default response is either an Error with a known
                // type if the operation defines a default (as above) or
                // an Error::UnexpectedResponse...
                // _ => Err(Error::UnexpectedResponse(response)),
                #default_response
            }
        };

        Ok(MethodSigBody {
            success: success_type,
            error: error_type,
            body: body_impl,
        })
    }

    /// Extract responses that match criteria specified by the `filter`. The
    /// result is a `Vec<OperationResponse>` that enumerates the cases matching
    /// the filter, and a `TokenStream` that represents the generated type for
    /// those cases.
    pub(crate) fn extract_responses<'a>(
        &self,
        method: &'a OperationMethod,
        filter: fn(&OperationResponseStatus) -> bool,
    ) -> (Vec<&'a OperationResponse>, OperationResponseKind) {
        let mut response_items = method
            .responses
            .iter()
            .filter(|response| filter(&response.status_code))
            .collect::<Vec<_>>();
        response_items.sort();

        // If we have a success range and a default, we can pop off the default
        // since it will never be hit. Note that this is a no-op for error
        // responses.
        let len = response_items.len();
        if len >= 2 {
            if let (
                OperationResponse {
                    status_code: OperationResponseStatus::Range(2),
                    ..
                },
                OperationResponse {
                    status_code: OperationResponseStatus::Default,
                    ..
                },
            ) = (&response_items[0], &response_items[len - 1])
            {
                response_items.pop();
            }
        }

        // Collect all unique response types
        let response_types = response_items
            .iter()
            .map(|response| (response.status_code.clone(), response.typ.clone()))
            .collect::<Vec<_>>();

        // Check if we have multiple different response types
        let unique_types = response_types
            .iter()
            .map(|(_, typ)| typ)
            .collect::<BTreeSet<_>>();

        // If we have multiple different types, create a Multiple variant
        if unique_types.len() > 1 {
            // Only handle Type responses for now
            let variants = response_types
                .clone()
                .into_iter()
                .filter_map(|(status, typ)| {
                    if let OperationResponseKind::Type(type_id) = typ {
                        Some((status, type_id))
                    } else {
                        None
                    }
                })
                .collect::<BTreeMap<_, _>>();

            // Only proceed if we have at least one Type response
            if !variants.is_empty() {
                let enum_name = format!("{}Response", method.operation_id.to_pascal_case());

                return (
                    response_items,
                    OperationResponseKind::Multiple {
                        variants,
                        enum_name,
                    },
                );
            }
        }

        // Fall back to the original behavior if we don't have multiple Type responses
        let response_type = unique_types
            .into_iter()
            .next()
            .unwrap_or(&OperationResponseKind::None);

        (response_items, response_type.clone())
    }

    // Validates all the necessary conditions for Dropshot pagination. Returns
    // the paginated item type data if all conditions are met.
    fn dropshot_pagination_data(
        &self,
        operation: &openapiv3::Operation,
        parameters: &[OperationParameter],
        responses: &[OperationResponse],
    ) -> Option<DropshotPagination> {
        let value = operation.extensions.get("x-dropshot-pagination")?;

        // We expect to see at least "page_token" and "limit" parameters.
        if parameters
            .iter()
            .filter(|param| {
                matches!(
                    (param.api_name.as_str(), &param.kind),
                    ("page_token", OperationParameterKind::Query(false))
                        | ("limit", OperationParameterKind::Query(false))
                )
            })
            .count()
            != 2
        {
            return None;
        }

        // All query parameters must be optional since page_token may not be
        // specified in conjunction with other query parameters.
        if !parameters.iter().all(|param| match &param.kind {
            OperationParameterKind::Query(required) => !required,
            _ => true,
        }) {
            return None;
        }

        // A raw body parameter can only be passed to a single call as it may
        // be a streaming type. We can't use a streaming type for a paginated
        // interface because we can only stream it once rather than for the
        // multiple calls required to collect all pages.
        if parameters
            .iter()
            .any(|param| param.typ == OperationParameterType::RawBody)
        {
            return None;
        }

        // There must be exactly one successful response type.
        let mut success_response_items =
            responses
                .iter()
                .filter_map(|response| match (&response.status_code, &response.typ) {
                    (
                        OperationResponseStatus::Code(200..=299)
                        | OperationResponseStatus::Range(2),
                        OperationResponseKind::Type(type_id),
                    ) => Some(type_id),
                    _ => None,
                });

        let success_response = match (success_response_items.next(), success_response_items.next())
        {
            (None, _) | (_, Some(_)) => return None,
            (Some(success), None) => success,
        };

        let typ = self.type_space.get_type(success_response).ok()?;
        let details = match typ.details() {
            typify::TypeDetails::Struct(details) => details,
            _ => return None,
        };

        let properties = details.properties().collect::<BTreeMap<_, _>>();

        // There should be exactly two properties: items and next_page
        if properties.len() != 2 {
            return None;
        }

        // We need a next_page property that's an Option<String>.
        if let typify::TypeDetails::Option(ref opt_id) = self
            .type_space
            .get_type(properties.get("next_page")?)
            .ok()?
            .details()
        {
            if !matches!(
                self.type_space.get_type(opt_id).ok()?.details(),
                typify::TypeDetails::String
            ) {
                return None;
            }
        } else {
            return None;
        }

        match self
            .type_space
            .get_type(properties.get("items")?)
            .ok()?
            .details()
        {
            typify::TypeDetails::Vec(item) => {
                #[derive(serde::Deserialize, Default)]
                struct DropshotPaginationFormat {
                    required: Vec<String>,
                }
                let first_page_params =
                    serde_json::from_value::<DropshotPaginationFormat>(value.clone())
                        .unwrap_or_default()
                        .required;
                Some(DropshotPagination {
                    item,
                    first_page_params,
                })
            }
            _ => None,
        }
    }

    /// Create the builder structs along with their impl bodies.
    ///
    /// Builder structs are generally of this form for a mandatory `param_1`
    /// and an optional `param_2`:
    /// ```ignore
    /// struct OperationId<'a> {
    ///     client: &'a super::Client,
    ///     param_1: Result<SomeType, String>,
    ///     param_2: Result<Option<String>, String>,
    /// }
    /// ```
    ///
    /// All parameters are present and all their types are `Result<T, String>`
    /// or `Result<Option<T>, String>` for optional parameters. Each parameter
    /// also has a corresponding method:
    /// ```ignore
    /// impl<'a> OperationId<'a> {
    ///     pub fn param_1<V>(self, value: V)
    ///         where V: std::convert::TryInto<SomeType>
    ///     {
    ///         self.param_1 = value.try_into()
    ///             .map_err(|_| #err_msg.to_string());
    ///         self
    ///     }
    ///     pub fn param_2<V>(self, value: V)
    ///         where V: std::convert::TryInto<SomeType>
    ///     {
    ///         self.param_2 = value.try_into()
    ///             .map(Some)
    ///             .map_err(|_| #err_msg.to_string());
    ///         self
    ///     }
    /// }
    /// ```
    ///
    /// The Client's operation_id method simply invokes the builder's new
    /// method, which assigns an error value to mandatory field and a
    /// `Ok(None)` value to optional ones:
    /// ```ignore
    /// impl<'a> OperationId<'a> {
    ///     pub fn new(client: &'a super::Client) -> Self {
    ///         Self {
    ///             client,
    ///             param_1: Err("param_1 was not initialized".to_string()),
    ///             param_2: Ok(None),
    ///         }
    ///     }
    /// }
    /// ```
    ///
    /// Finally, builders have methods to execute the operation. This simply
    /// resolves each parameter with the ? (`Try` operator).
    /// ```ignore
    /// impl<'a> OperationId<'a> {
    ///     pub fn send(self) -> Result<
    ///         ResponseValue<SuccessType>,
    ///         Error<ErrorType>,
    ///     > {
    ///         let Self {
    ///             client,
    ///             param_1,
    ///             param_2,
    ///         } = self;
    ///
    ///         let param_1 = param_1.map_err(Error::InvalidRequest)?;
    ///         let param_2 = param_1.map_err(Error::InvalidRequest)?;
    ///
    ///         // ... execute the body (see `method_sig_body`) ...
    ///     }
    /// }
    /// ```
    ///
    /// Finally, paginated interfaces have a `stream()` method which uses the
    /// `send()` method above to fetch each page of results to assemble the
    /// items into a single `impl Stream`.
    pub(crate) fn builder_struct(
        &mut self,
        method: &OperationMethod,
        tag_style: TagStyle,
        has_inner: bool,
    ) -> Result<TokenStream> {
        let struct_name = sanitize(&method.operation_id, Case::Pascal);
        let struct_ident = format_ident!("{}", struct_name);

        // Generate an ident for each parameter.
        let param_names = method
            .params
            .iter()
            .map(|param| format_ident!("{}", param.name))
            .collect::<Vec<_>>();

        let client_ident = unique_ident_from("client", &param_names);

        let mut cloneable = true;

        // Generate the type for each parameter.
        let param_types = method
            .params
            .iter()
            .map(|param| match &param.typ {
                OperationParameterType::Type(type_id) => {
                    let ty = self.type_space.get_type(type_id)?;

                    // For body parameters only, if there's a builder we'll
                    // nest that within this builder.
                    if let (OperationParameterKind::Body(_), Some(builder_name)) =
                        (&param.kind, ty.builder())
                    {
                        Ok(quote! { Result<#builder_name, String> })
                    } else if param.kind.is_required() {
                        let t = ty.ident();
                        Ok(quote! { Result<#t, String> })
                    } else {
                        let t = ty.ident();
                        Ok(quote! { Result<Option<#t>, String> })
                    }
                }

                OperationParameterType::RawBody => {
                    cloneable = false;
                    Ok(quote! { Result<reqwest::Body, String> })
                }
            })
            .collect::<Result<Vec<_>>>()?;

        // Generate the default value value for each parameter. For optional
        // parameters it's just `Ok(None)`. For builders it's
        // `Ok(Default::default())`. For required, non-builders it's an Err(_)
        // that indicates which field isn't initialized.
        let param_values = method
            .params
            .iter()
            .map(|param| match &param.typ {
                OperationParameterType::Type(type_id) => {
                    let ty = self.type_space.get_type(type_id)?;

                    // Fill in the appropriate initial value for the
                    // param_types generated above.
                    if let (OperationParameterKind::Body(_), Some(_)) = (&param.kind, ty.builder())
                    {
                        Ok(quote! { Ok(::std::default::Default::default()) })
                    } else if param.kind.is_required() {
                        let err_msg = format!("{} was not initialized", param.name);
                        Ok(quote! { Err(#err_msg.to_string()) })
                    } else {
                        Ok(quote! { Ok(None) })
                    }
                }

                OperationParameterType::RawBody => {
                    let err_msg = format!("{} was not initialized", param.name);
                    Ok(quote! { Err(#err_msg.to_string()) })
                }
            })
            .collect::<Result<Vec<_>>>()?;

        // For builders we map `Ok` values to perform a `try_from` to attempt
        // to convert the builder into the desired type. No "finalization" is
        // required for non-builders (required or optional).
        let param_finalize = method
            .params
            .iter()
            .map(|param| match &param.typ {
                OperationParameterType::Type(type_id) => {
                    let ty = self.type_space.get_type(type_id)?;
                    if ty.builder().is_some() {
                        let type_name = ty.ident();
                        Ok(quote! {
                            .and_then(|v| #type_name::try_from(v)
                                .map_err(|e| e.to_string()))
                        })
                    } else {
                        Ok(quote! {})
                    }
                }
                OperationParameterType::RawBody => Ok(quote! {}),
            })
            .collect::<Result<Vec<_>>>()?;

        // For each parameter, we need an impl for the builder to let consumers
        // provide a value.
        let param_impls = method
            .params
            .iter()
            .map(|param| {
                let param_name = format_ident!("{}", param.name);
                match &param.typ {
                    OperationParameterType::Type(type_id) => {
                        let ty = self.type_space.get_type(type_id)?;
                        match (ty.builder(), param.kind.is_optional()) {
                            // TODO right now optional body parameters are not
                            // addressed
                            (Some(_), true) => {
                                unreachable!()
                            }
                            (None, true) => {
                                let typ = ty.ident();
                                let err_msg = format!(
                                    "conversion to `{}` for {} failed",
                                    ty.name(),
                                    param.name,
                                );
                                Ok(quote! {
                                    pub fn #param_name<V>(
                                        mut self,
                                        value: V,
                                    ) -> Self
                                        where V: std::convert::TryInto<#typ>,
                                    {
                                        self.#param_name = value.try_into()
                                            .map(Some)
                                            .map_err(|_| #err_msg.to_string());
                                        self
                                    }
                                })
                            }
                            (None, false) => {
                                let typ = ty.ident();
                                let err_msg = format!(
                                    "conversion to `{}` for {} failed",
                                    ty.name(),
                                    param.name,
                                );
                                Ok(quote! {
                                    pub fn #param_name<V>(
                                        mut self,
                                        value: V,
                                    ) -> Self
                                        where V: std::convert::TryInto<#typ>,
                                    {
                                        self.#param_name = value.try_into()
                                            .map_err(|_| #err_msg.to_string());
                                        self
                                    }
                                })
                            }

                            // For builder-capable bodies we offer a `body()`
                            // method that sets the full body (by constructing
                            // a builder **from** the body type). We also offer
                            // a `body_map()` method that operates on the
                            // builder itself.
                            (Some(builder_name), false) => {
                                assert_eq!(param.name, "body");
                                let typ = ty.ident();
                                let err_msg = format!(
                                    "conversion to `{}` for {} failed: {{}}",
                                    ty.name(),
                                    param.name,
                                );
                                Ok(quote! {
                                    pub fn body<V>(mut self, value: V) -> Self
                                    where
                                        V: std::convert::TryInto<#typ>,
                                        <V as std::convert::TryInto<#typ>>::Error:
                                            std::fmt::Display,
                                    {
                                        self.body = value.try_into()
                                            .map(From::from)
                                            .map_err(|s| format!(#err_msg, s));
                                        self
                                    }

                                    pub fn body_map<F>(mut self, f: F) -> Self
                                    where
                                        F: std::ops::FnOnce(#builder_name)
                                            -> #builder_name,
                                    {
                                        self.body = self.body.map(f);
                                        self
                                    }
                                })
                            }
                        }
                    }

                    OperationParameterType::RawBody => match param.kind {
                        OperationParameterKind::Body(BodyContentType::OctetStream) => {
                            let err_msg =
                                format!("conversion to `reqwest::Body` for {} failed", param.name,);

                            Ok(quote! {
                                pub fn #param_name<B>(mut self, value: B) -> Self
                                    where B: std::convert::TryInto<reqwest::Body>
                                {
                                    self.#param_name = value.try_into()
                                        .map_err(|_| #err_msg.to_string());
                                    self
                                }
                            })
                        }
                        OperationParameterKind::Body(BodyContentType::Text(_)) => {
                            let err_msg =
                                format!("conversion to `String` for {} failed", param.name,);

                            Ok(quote! {
                                pub fn #param_name<V>(mut self, value: V) -> Self
                                    where V: std::convert::TryInto<String>
                                {
                                    self.#param_name = value
                                        .try_into()
                                        .map_err(|_| #err_msg.to_string())
                                        .map(|v| v.into());
                                    self
                                }
                            })
                        }
                        _ => unreachable!(),
                    },
                }
            })
            .collect::<Result<Vec<_>>>()?;

        let MethodSigBody {
            success,
            error,
            body,
        } = self.method_sig_body(
            method,
            quote! { super::Client },
            quote! { #client_ident },
            has_inner,
        )?;

        let send_doc = format!(
            "Sends a '{}' request to '{}'",
            method.method.as_str().to_ascii_uppercase(),
            method.path,
        );

        let send_impl = quote! {
            #[doc = #send_doc]
            #[allow(irrefutable_let_patterns)]
            pub async fn send(self) -> Result<
                ResponseValue<#success>,
                Error<#error>,
            > {
                // Destructure the builder for convenience.
                #[allow(unused_variables)]
                let Self {
                    #client_ident,
                    #( #param_names, )*
                } = self;

                // Extract parameters into variables, returning an error if
                // a value has not been provided or there was a conversion
                // error.
                //
                // TODO we could do something a bit nicer by collecting all
                // errors rather than just reporting the first one.
                #(
                #[allow(unused_variables)]
                let #param_names =
                    #param_names
                        #param_finalize
                        .map_err(Error::InvalidRequest)?;
                )*

                // Do the work.
                #body
            }
        };

        let stream_impl = method.dropshot_paginated.as_ref().map(|page_data| {
            // We're now using futures.
            self.uses_futures = true;

            let step_params = method.params.iter().filter_map(|param| {
                if param.api_name.as_str() != "limit"
                    && matches!(param.kind, OperationParameterKind::Query(_))
                {
                    // Query parameters (other than "limit") are None; having
                    // page_token as Some(_), as we will during the loop below,
                    // is mutually exclusive with other query parameters.
                    let name = format_ident!("{}", param.name);
                    Some(quote! {
                        #name: Ok(None)
                    })
                } else {
                    None
                }
            });

            // The item type that we've saved (by picking apart the original
            // function's return type) will be the Item type parameter for the
            // Stream impl we return.
            let item = self.type_space.get_type(&page_data.item).unwrap();
            let item_type = item.ident();

            let stream_doc = format!(
                "Streams '{}' requests to '{}'",
                method.method.as_str().to_ascii_uppercase(),
                method.path,
            );

            quote! {
                #[doc = #stream_doc]
                #[allow(unused_variables)]
                #[allow(irrefutable_let_patterns)]
                pub fn stream(self) -> impl futures::Stream<Item = Result<
                    #item_type,
                    Error<#error>,
                >> + Unpin + 'a {
                    use ::futures::StreamExt;
                    use ::futures::TryFutureExt;
                    use ::futures::TryStreamExt;

                    // This is the builder template we'll use for iterative
                    // steps past the first; it has all query params set to
                    // None (the step will fill in page_token).
                    let next = Self {
                        #( #step_params, )*
                        ..self.clone()
                    };

                    self.send()
                        .map_ok(move |page| {
                            let page = page.into_inner();

                            // Create a stream from the first page of items.
                            let first =
                                futures::stream::iter(page.items).map(Ok);

                            // We unfold subsequent pages using page.next_page
                            // as the seed value. Each iteration returns its
                            // items and the new state which is a tuple of the
                            // next page token and the Self template.
                            let rest = futures::stream::try_unfold(
                                (page.next_page, next),
                                |(next_page, next)| async {
                                    if next_page.is_none() {
                                        // The page_token was None so we've
                                        // reached the end.
                                        Ok(None)
                                    } else {
                                        // Get the next page using the next
                                        // template (with query parameters set
                                        // to None), overriding page_token.
                                        Self {
                                            page_token: Ok(next_page),
                                            ..next.clone()
                                        }
                                        .send()
                                        .map_ok(|page| {
                                            let page = page.into_inner();
                                            Some((
                                                futures::stream::iter(
                                                    page.items
                                                ).map(Ok),
                                                (page.next_page, next),
                                            ))
                                        })
                                        .await
                                    }
                                },
                            )
                            .try_flatten();

                            first.chain(rest)
                        })
                        .try_flatten_stream()
                        .boxed()
                }
            }
        });

        let mut derives = vec![quote! { Debug }];
        if cloneable {
            derives.push(quote! { Clone });
        }

        let derive = quote! {
            #[derive( #( #derives ),* )]
        };

        // Build a reasonable doc comment depending on whether this struct is
        // the output from
        // 1. A Client method
        // 2. An extension trait method
        // 3. Several extension trait methods
        let struct_doc = match (tag_style, method.tags.len(), method.tags.first()) {
            (TagStyle::Merged, _, _) | (TagStyle::Separate, 0, _) => {
                let ty = format!("Client::{}", method.operation_id);
                format!("Builder for [`{ty}`]\n\n[`{ty}`]: super::{ty}",)
            }
            (TagStyle::Separate, 1, Some(tag)) => {
                let ty = format!(
                    "Client{}Ext::{}",
                    sanitize(tag, Case::Pascal),
                    method.operation_id
                );
                format!("Builder for [`{ty}`]\n\n[`{ty}`]: super::{ty}",)
            }
            (TagStyle::Separate, _, _) => {
                format!(
                    "Builder for `{}` operation\n\nSee {}\n\n{}",
                    method.operation_id,
                    method
                        .tags
                        .iter()
                        .map(|tag| {
                            format!(
                                "[`Client{}Ext::{}`]",
                                sanitize(tag, Case::Pascal),
                                method.operation_id,
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(", "),
                    method
                        .tags
                        .iter()
                        .map(|tag| {
                            let ty = format!(
                                "Client{}Ext::{}",
                                sanitize(tag, Case::Pascal),
                                method.operation_id,
                            );
                            format!("[`{ty}`]: super::{ty}")
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
            }
        };

        Ok(quote! {
            #[doc = #struct_doc]
            #derive
            pub struct #struct_ident<'a> {
                #client_ident: &'a super::Client,
                #( #param_names: #param_types, )*
            }

            impl<'a> #struct_ident<'a> {
                pub fn new(client: &'a super::Client) -> Self {
                    Self {
                        #client_ident: client,
                        #( #param_names: #param_values, )*
                    }
                }

                #( #param_impls )*
                #send_impl
                #stream_impl
            }
        })
    }

    fn builder_helper(&self, method: &OperationMethod) -> BuilderImpl {
        let operation_id = format_ident!("{}", method.operation_id);
        let struct_name = sanitize(&method.operation_id, Case::Pascal);
        let struct_ident = format_ident!("{}", struct_name);

        let params = method
            .params
            .iter()
            .map(|param| format!("\n    .{}({})", param.name, param.name))
            .collect::<Vec<_>>()
            .join("");

        let eg = format!(
            "\
            let response = client.{}(){}
    .send()
    .await;",
            method.operation_id, params,
        );

        // Note that it would be nice to have a non-ignored example that could
        // be validated by doc tests, but in order to use the Client we need
        // to import it, and in order to import it we need to know the name of
        // the containing crate... which we can't from this context.
        let doc = format!("{}```ignore\n{}\n```", make_doc_comment(method), eg);

        let sig = quote! {
            fn #operation_id(&self) -> builder:: #struct_ident
        };

        let body = quote! {
            builder:: #struct_ident ::new(self)
        };
        BuilderImpl { doc, sig, body }
    }

    /// Generates a pair of TokenStreams.
    ///
    /// The first includes all the operation code; impl Client for operations
    /// with no tags and code of this form for each tag:
    ///
    /// ```ignore
    /// pub trait ClientTagExt {
    ///     ...
    /// }
    ///
    /// impl ClientTagExt for Client {
    ///     ...
    /// }
    /// ```
    ///
    /// The second is the code for the prelude for each tag extension trait:
    ///
    /// ```ignore
    /// pub use super::ClientTagExt;
    /// ```
    pub(crate) fn builder_tags(
        &self,
        methods: &[OperationMethod],
        tag_info: &BTreeMap<&String, &openapiv3::Tag>,
    ) -> (TokenStream, TokenStream) {
        let mut base = Vec::new();
        let mut ext = BTreeMap::new();

        methods.iter().for_each(|method| {
            let BuilderImpl { doc, sig, body } = self.builder_helper(method);

            if method.tags.is_empty() {
                let impl_body = quote! {
                    #[doc = #doc]
                    pub #sig {
                        #body
                    }
                };
                base.push(impl_body);
            } else {
                let trait_sig = quote! {
                    #[doc = #doc]
                    #sig;
                };

                let impl_body = quote! {
                    #sig {
                        #body
                    }
                };
                method.tags.iter().for_each(|tag| {
                    ext.entry(tag.clone())
                        .or_insert_with(Vec::new)
                        .push((trait_sig.clone(), impl_body.clone()));
                });
            }
        });

        let base_impl = (!base.is_empty()).then(|| {
            quote! {
                impl Client {
                    #(#base)*
                }
            }
        });

        let (ext_impl, ext_use): (Vec<_>, Vec<_>) = ext
            .into_iter()
            .map(|(tag, trait_methods)| {
                let desc = tag_info
                    .get(&tag)
                    .and_then(|tag| tag.description.as_ref())
                    .map(|d| quote! { #[doc = #d] });
                let tr = format_ident!("Client{}Ext", sanitize(&tag, Case::Pascal));
                let (trait_methods, trait_impls): (Vec<TokenStream>, Vec<TokenStream>) =
                    trait_methods.into_iter().unzip();
                (
                    quote! {
                        #desc
                        pub trait #tr {
                            #(#trait_methods)*
                        }

                        impl #tr for Client {
                            #(#trait_impls)*
                        }
                    },
                    tr,
                )
            })
            .unzip();

        (
            quote! {
                #base_impl

                #(#ext_impl)*
            },
            quote! {
                #(pub use super::#ext_use;)*
            },
        )
    }

    pub(crate) fn builder_impl(&self, method: &OperationMethod) -> TokenStream {
        let BuilderImpl { doc, sig, body } = self.builder_helper(method);

        let impl_body = quote! {
            #[doc = #doc]
            pub #sig {
                #body
            }
        };

        impl_body
    }

    fn get_body_param(
        &mut self,
        operation: &openapiv3::Operation,
        components: &Option<Components>,
        method: &str,
    ) -> Result<Option<OperationParameter>> {
        // GET, HEAD, and OPTIONS requests should never have request bodies
        // This fixes an issue where Swagger 2.0 specs with global "consumes"
        // incorrectly generate body parameters for these methods
        if matches!(method.to_uppercase().as_str(), "GET" | "HEAD" | "OPTIONS") {
            return Ok(None);
        }

        let body = match &operation.request_body {
            Some(body) => body.item(components)?,
            None => return Ok(None),
        };

        // Additional check for DELETE requests: if the request body is a generic
        // object schema with no properties (likely from global "consumes"), skip it
        if method.to_uppercase() == "DELETE" {
            if let Some((content_str, media_type)) = body.content.first() {
                if content_str == "application/json" {
                    if let Some(schema) = &media_type.schema {
                        if let Ok(openapiv3::Schema {
                            schema_kind:
                                openapiv3::SchemaKind::Type(openapiv3::Type::Object(
                                    openapiv3::ObjectType { properties, .. },
                                )),
                            ..
                        }) = schema.item(components)
                        {
                            if properties.is_empty() {
                                // This is likely a fake request body from global consumes
                                return Ok(None);
                            }
                        }
                    }
                }
            }
        }

        let (content_str, media_type) = match (body.content.first(), body.content.len()) {
            (None, _) => return Ok(None),
            (Some(first), 1) => first,
            (_, n) => todo!(
                "more media types than expected for {}: {}",
                operation.operation_id.as_ref().unwrap(),
                n,
            ),
        };

        let schema = media_type.schema.as_ref().ok_or_else(|| {
            Error::UnexpectedFormat("No schema specified for request body".to_string())
        })?;

        let content_type = BodyContentType::from_str(content_str)?;

        let typ = match content_type {
            BodyContentType::OctetStream => {
                // For an octet stream, we expect a simple, specific schema:
                // "schema": {
                //     "type": "string",
                //     "format": "binary"
                // }
                match schema.item(components)? {
                    openapiv3::Schema {
                        schema_data:
                            openapiv3::SchemaData {
                                nullable: false,
                                discriminator: None,
                                default: None,
                                // Other fields that describe or document the
                                // schema are fine.
                                ..
                            },
                        schema_kind:
                            openapiv3::SchemaKind::Type(openapiv3::Type::String(
                                openapiv3::StringType {
                                    format:
                                        openapiv3::VariantOrUnknownOrEmpty::Item(
                                            openapiv3::StringFormat::Binary,
                                        ),
                                    pattern: None,
                                    enumeration,
                                    min_length: None,
                                    max_length: None,
                                },
                            )),
                    } if enumeration.is_empty() => Ok::<(), crate::Error>(()),
                    _ => {
                        // Instead of failing, be more lenient with octet-stream schemas
                        // This is the key change - accept any schema for octet-stream
                        eprintln!("Warning: Non-standard schema for application/octet-stream, but proceeding anyway");
                        Ok(())
                    }
                }?;
                OperationParameterType::RawBody
            }
            BodyContentType::Text(_) => {
                // For a plain text body, we expect a simple, specific schema:
                // "schema": {
                //     "type": "string",
                //     "format": "binary"
                // }
                match schema.item(components)? {
                    openapiv3::Schema {
                        schema_data:
                            openapiv3::SchemaData {
                                nullable: false,
                                discriminator: None,
                                default: None,
                                // Other fields that describe or document the
                                // schema are fine.
                                ..
                            },
                        schema_kind:
                            openapiv3::SchemaKind::Type(openapiv3::Type::String(
                                openapiv3::StringType {
                                    format:
                                        openapiv3::VariantOrUnknownOrEmpty::Item(
                                            openapiv3::StringFormat::Binary,
                                        ),
                                    pattern: None,
                                    enumeration,
                                    min_length: None,
                                    max_length: None,
                                },
                            )),
                    } if enumeration.is_empty() => Ok::<(), crate::Error>(()),
                    _ => {
                        // Be more lenient with text/* schemas as well
                        eprintln!("Warning: Non-standard schema for text/* content type, but proceeding anyway");
                        Ok(())
                    }
                }?;
                OperationParameterType::RawBody
            }

            BodyContentType::Json | BodyContentType::FormUrlencoded => {
                // TODO it would be legal to have the encoding field set for
                // application/x-www-form-urlencoded content, but I'm not sure
                // how to interpret the values.
                if !media_type.encoding.is_empty() {
                    todo!("media type encoding not empty: {:#?}", media_type);
                }
                let name = sanitize(
                    &format!("{}-body", operation.operation_id.as_ref().unwrap(),),
                    Case::Pascal,
                );
                let typ = self
                    .type_space
                    .add_type_with_name(&schema.to_schema(), Some(name))?;
                OperationParameterType::Type(typ)
            }
        };

        Ok(Some(OperationParameter {
            name: "body".to_string(),
            api_name: "body".to_string(),
            description: body.description.clone(),
            typ,
            kind: OperationParameterKind::Body(content_type),
        }))
    }

    /// Generate a response enum for an operation *only if there are multiple success response types*
    ///
    /// If there is only one success response type, do not generate a response enum.
    pub(crate) fn generate_operation_success_enum(
        &mut self,
        method: &OperationMethod,
    ) -> Result<Option<TokenStream>> {
        // Extract all *success* responses (2xx)
        let (success_responses, _) = self.extract_responses(method, |status| {
            matches!(
                status,
                OperationResponseStatus::Code(200..=299) | OperationResponseStatus::Range(2)
            )
        });

        // Only generate an enum if there are multiple unique success response types
        let mut unique_types = std::collections::BTreeSet::new();
        for response in &success_responses {
            unique_types.insert(&response.typ);
        }
        if unique_types.len() <= 1 {
            // Only one success type: do not generate a response enum
            return Ok(None);
        }

        // Otherwise, generate the enum as before
        let enum_name = format!("{}Success", method.operation_id.to_pascal_case());
        let enum_ident = format_ident!("{}", enum_name);

        let mut variants_tokens = Vec::new();
        let mut processed_status_codes = std::collections::BTreeSet::new();

        for response in &success_responses {
            let variant_name = match &response.status_code {
                OperationResponseStatus::Code(code) => {
                    if !processed_status_codes.insert(*code) {
                        continue;
                    }
                    format_ident!("Status{}", code)
                }
                OperationResponseStatus::Range(range) => {
                    if !processed_status_codes.insert(*range + 1000) {
                        continue;
                    }
                    format_ident!("Status{}xx", range)
                }
                OperationResponseStatus::Default => {
                    if !processed_status_codes.insert(0) {
                        continue;
                    }
                    format_ident!("Default")
                }
            };

            let status_str = response.status_code.to_string();
            let type_tokens = match &response.typ {
                OperationResponseKind::Type(type_id) => {
                    let type_name = self.type_space.get_type(type_id).unwrap();
                    let type_ident = type_name.ident();
                    quote! { #type_ident }
                }
                OperationResponseKind::None => quote! { () },
                OperationResponseKind::Raw => quote! { ByteStream },
                OperationResponseKind::Upgrade => quote! { reqwest::Upgraded },
                OperationResponseKind::Multiple { enum_name, .. } => {
                    let enum_ident = format_ident!("{}", enum_name);
                    quote! { #enum_ident }
                }
            };

            variants_tokens.push(quote! {
                #[doc = concat!("Response for status code ", #status_str)]
                #variant_name(#type_tokens)
            });
        }

        let enum_doc = format!("Response enum for the `{}` operation", method.operation_id);

        // Generate FromStr implementation
        let from_str_match_arms = success_responses
            .iter()
            .filter_map(|response| {
                let status_code = match &response.status_code {
                    OperationResponseStatus::Code(code) => Some(*code),
                    _ => None,
                };

                if let Some(code) = status_code {
                    let variant_name = format_ident!("Status{}", code);

                    match &response.typ {
                        OperationResponseKind::Type(_) => Some(quote! {
                            #code => Ok(Self::#variant_name(value.to_string())),
                        }),
                        OperationResponseKind::None => Some(quote! {
                            #code => Ok(Self::#variant_name(())),
                        }),
                        _ => None,
                    }
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let enum_def = quote! {
            #[doc = #enum_doc]
            #[derive(Debug, Clone, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
            pub enum #enum_ident {
                #(#variants_tokens),*
            }

            impl std::str::FromStr for #enum_ident {
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
                        #(#from_str_match_arms)*
                        _ => Err("Unknown status code".to_string()),
                    }
                }
            }
        };

        Ok(Some(enum_def))
    }

    /// Generate an error enum for an operation.
    ///
    /// If there are no error codes specified, return None (use `()` as the error type).
    pub(crate) fn generate_operation_error_enum(
        &mut self,
        method: &OperationMethod,
    ) -> Result<Option<TokenStream>> {
        // Extract all error responses (4xx and 5xx)
        let (error_responses, _) = self.extract_responses(method, |status| {
            matches!(
                status,
                OperationResponseStatus::Code(400..=599) | OperationResponseStatus::Range(4..=5)
            )
        });

        if error_responses.is_empty() {
            // No error codes: use unit type for error
            return Ok(None);
        }

        let enum_name = format!("{}Error", method.operation_id.to_pascal_case());
        let enum_ident = format_ident!("{}", enum_name);

        let mut variants_tokens = Vec::new();
        let mut processed_status_codes = std::collections::BTreeSet::new();

        for response in &error_responses {
            let variant_name = match &response.status_code {
                OperationResponseStatus::Code(code) => {
                    if !processed_status_codes.insert(*code) {
                        continue;
                    }
                    format_ident!("Status{}", code)
                }
                OperationResponseStatus::Range(range) => {
                    if !processed_status_codes.insert(*range + 1000) {
                        continue;
                    }
                    format_ident!("Status{}xx", range)
                }
                OperationResponseStatus::Default => {
                    if !processed_status_codes.insert(0) {
                        continue;
                    }
                    format_ident!("Default")
                }
            };

            let status_str = response.status_code.to_string();
            let type_tokens = match &response.typ {
                OperationResponseKind::Type(type_id) => {
                    let type_name = self.type_space.get_type(type_id).unwrap();
                    let type_ident = type_name.ident();
                    // Since error enums are generated inside the types module,
                    // we should use just the type name without the types:: prefix
                    let type_ident_str = type_ident.to_string();
                    if type_ident_str.starts_with("types ::")
                        || type_ident_str.starts_with("types::")
                    {
                        let simple_name = if type_ident_str.starts_with("types ::") {
                            type_ident_str.strip_prefix("types ::").unwrap().trim()
                        } else {
                            type_ident_str.strip_prefix("types::").unwrap()
                        };
                        let simple_ident = format_ident!("{}", simple_name);
                        quote! { #simple_ident }
                    } else {
                        quote! { #type_ident }
                    }
                }
                OperationResponseKind::None => {
                    quote! { () }
                }
                OperationResponseKind::Raw => {
                    quote! { ByteStream }
                }
                OperationResponseKind::Upgrade => {
                    quote! { reqwest::Upgraded }
                }
                OperationResponseKind::Multiple { enum_name, .. } => {
                    let enum_ident = format_ident!("{}", enum_name);
                    quote! { #enum_ident }
                }
            };

            variants_tokens.push(quote! {
                #[doc = concat!("Error response for status code ", #status_str)]
                #variant_name(#type_tokens)
            });
        }

        // Add UnknownValue variant
        variants_tokens.push(quote! {
            /// Error response for an unknown status code
            UnknownValue(serde_json::Value)
        });

        let enum_doc = format!("Error enum for the `{}` operation", method.operation_id);

        // Generate FromStr implementation
        let from_str_match_arms = error_responses
            .iter()
            .filter_map(|response| {
                let status_code = match &response.status_code {
                    OperationResponseStatus::Code(code) => Some(*code),
                    _ => None,
                };

                if let Some(code) = status_code {
                    let variant_name = format_ident!("Status{}", code);

                    match &response.typ {
                        OperationResponseKind::Type(_) => Some(quote! {
                            #code => Ok(Self::#variant_name(value.to_string())),
                        }),
                        OperationResponseKind::None => Some(quote! {
                            #code => Ok(Self::#variant_name(())),
                        }),
                        _ => None,
                    }
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let enum_def = quote! {
            #[doc = #enum_doc]
            #[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize)]
            pub enum #enum_ident {
                #(#variants_tokens),*
            }

            impl std::str::FromStr for #enum_ident {
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
                        #(#from_str_match_arms)*
                        _ => {
                            // Try to parse as JSON for unknown status codes
                            match serde_json::from_str(value) {
                                Ok(json_value) => Ok(Self::UnknownValue(json_value)),
                                Err(_) => Err("Unable to parse as JSON".to_string())
                            }
                        }
                    }
                }
            }
        };

        Ok(Some(enum_def))
    }
}

fn make_doc_comment(method: &OperationMethod) -> String {
    let mut buf = String::new();

    if let Some(summary) = &method.summary {
        buf.push_str(summary.trim_end_matches(['.', ',']));
        buf.push_str("\n\n");
    }
    if let Some(description) = &method.description {
        buf.push_str(description);
        buf.push_str("\n\n");
    }

    buf.push_str(&format!(
        "Sends a '{}' request to '{}'\n\n",
        method.method.as_str().to_ascii_uppercase(),
        method.path,
    ));

    if method
        .params
        .iter()
        .filter(|param| param.description.is_some())
        .count()
        > 0
    {
        buf.push_str("Arguments:\n");
        for param in &method.params {
            buf.push_str(&format!("- `{}`", param.name));
            if let Some(description) = &param.description {
                buf.push_str(": ");
                buf.push_str(description);
            }
            buf.push('\n');
        }
    }

    buf
}

fn make_stream_doc_comment(method: &OperationMethod) -> String {
    let mut buf = String::new();

    if let Some(summary) = &method.summary {
        buf.push_str(summary.trim_end_matches(['.', ',']));
        buf.push_str(" as a Stream\n\n");
    }
    if let Some(description) = &method.description {
        buf.push_str(description);
        buf.push_str("\n\n");
    }

    buf.push_str(&format!(
        "Sends repeated `{}` requests to `{}` until there are no more results.\n\n",
        method.method.as_str().to_ascii_uppercase(),
        method.path,
    ));

    if method
        .params
        .iter()
        .filter(|param| param.api_name.as_str() != "page_token")
        .filter(|param| param.description.is_some())
        .count()
        > 0
    {
        buf.push_str("Arguments:\n");
        for param in &method.params {
            if param.api_name.as_str() == "page_token" {
                continue;
            }

            buf.push_str(&format!("- `{}`", param.name));
            if let Some(description) = &param.description {
                buf.push_str(": ");
                buf.push_str(description);
            }
            buf.push('\n');
        }
    }

    buf
}

fn sort_params(raw_params: &mut [OperationParameter], names: &[String]) {
    raw_params.sort_by(
        |OperationParameter {
             kind: a_kind,
             api_name: a_name,
             ..
         },
         OperationParameter {
             kind: b_kind,
             api_name: b_name,
             ..
         }| {
            match (a_kind, b_kind) {
                // Path params are first and are in positional order.
                (OperationParameterKind::Path, OperationParameterKind::Path) => {
                    let a_index = names
                        .iter()
                        .position(|x| x == a_name)
                        .unwrap_or_else(|| panic!("{a_name} missing from path"));
                    let b_index = names
                        .iter()
                        .position(|x| x == b_name)
                        .unwrap_or_else(|| panic!("{b_name} missing from path"));
                    a_index.cmp(&b_index)
                }
                (OperationParameterKind::Path, OperationParameterKind::Query(_)) => Ordering::Less,
                (OperationParameterKind::Path, OperationParameterKind::Body(_)) => Ordering::Less,
                (OperationParameterKind::Path, OperationParameterKind::Header(_)) => Ordering::Less,

                // Query params are in lexicographic order.
                (OperationParameterKind::Query(_), OperationParameterKind::Body(_)) => {
                    Ordering::Less
                }
                (OperationParameterKind::Query(_), OperationParameterKind::Query(_)) => {
                    a_name.cmp(b_name)
                }
                (OperationParameterKind::Query(_), OperationParameterKind::Path) => {
                    Ordering::Greater
                }
                (OperationParameterKind::Query(_), OperationParameterKind::Header(_)) => {
                    Ordering::Less
                }

                // Body params are last and should be singular.
                (OperationParameterKind::Body(_), OperationParameterKind::Path) => {
                    Ordering::Greater
                }
                (OperationParameterKind::Body(_), OperationParameterKind::Query(_)) => {
                    Ordering::Greater
                }
                (OperationParameterKind::Body(_), OperationParameterKind::Header(_)) => {
                    Ordering::Greater
                }
                (OperationParameterKind::Body(_), OperationParameterKind::Body(_)) => {
                    panic!("should only be one body")
                }

                // Header params are in lexicographic order.
                (OperationParameterKind::Header(_), OperationParameterKind::Header(_)) => {
                    a_name.cmp(b_name)
                }
                (OperationParameterKind::Header(_), _) => Ordering::Greater,
            }
        },
    );
}

trait ParameterDataExt {
    fn schema(&self) -> Result<&openapiv3::ReferenceOr<openapiv3::Schema>>;
}

impl ParameterDataExt for openapiv3::ParameterData {
    fn schema(&self) -> Result<&openapiv3::ReferenceOr<openapiv3::Schema>> {
        match &self.format {
            openapiv3::ParameterSchemaOrContent::Schema(s) => Ok(s),
            openapiv3::ParameterSchemaOrContent::Content(c) => Err(Error::UnexpectedFormat(
                format!("unexpected content {c:#?}"),
            )),
        }
    }
}
