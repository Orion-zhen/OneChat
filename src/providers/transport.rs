use std::{future::Future, pin::Pin};

use rig_core::http_client::{DynHttpClient, HeaderMap, HttpMiddleware, Method, Uri};

use crate::domain::{GenerationError, Provider};

use super::{sdk_headers, sdk_http_client};

pub(crate) fn sdk_transport(provider: &Provider) -> Result<DynHttpClient, GenerationError> {
    let headers = CustomHeaders(sdk_headers(provider)?);
    let client = rig_reqwest::ReqwestClient::from(sdk_http_client(provider)?);
    Ok(DynHttpClient::new(client).with_middleware(headers))
}

struct CustomHeaders(HeaderMap);

impl HttpMiddleware for CustomHeaders {
    fn before_request_headers<'a>(
        &'a self,
        _method: &'a Method,
        _uri: &'a Uri,
        headers: &'a mut HeaderMap,
    ) -> Pin<Box<dyn Future<Output = rig_core::http_client::Result<()>> + Send + 'a>> {
        Box::pin(async move {
            headers.extend(self.0.clone());
            Ok(())
        })
    }
}
