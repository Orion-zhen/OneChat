mod catalog;
mod common;
mod error;

pub mod anthropic;
pub mod gemini;
pub mod openai;

use async_channel::Sender;
use tokio_util::sync::CancellationToken;

use crate::domain::{
    GenerationError, GenerationErrorKind, GenerationEvent, GenerationRequest, Message, Provider,
    ProviderKind, message_tool_calls,
};

pub use catalog::{AvailableModel, list_models};
pub(crate) use common::{
    consume_stream, insert_optional, remove_keys, sdk_base_url, sdk_headers, sdk_http_client,
    sdk_request,
};
pub(crate) use error::{classify_provider_error, sdk_completion_error, sdk_verify_error};

pub async fn test_connection(provider: &Provider) -> Result<(), GenerationError> {
    match provider.kind {
        ProviderKind::OpenAi | ProviderKind::OpenAiCompatible => {
            openai::test_connection(provider).await
        }
        ProviderKind::Anthropic => anthropic::test_connection(provider).await,
        ProviderKind::Gemini => gemini::test_connection(provider).await,
    }
}

pub async fn stream_step(
    request: GenerationRequest,
    events: &Sender<GenerationEvent>,
    cancellation: CancellationToken,
) -> Result<Message, GenerationError> {
    match request.provider.kind {
        ProviderKind::OpenAi | ProviderKind::OpenAiCompatible => {
            openai::stream(request, events, cancellation).await
        }
        ProviderKind::Anthropic => anthropic::stream(request, events, cancellation).await,
        ProviderKind::Gemini => gemini::stream(request, events, cancellation).await,
    }
}

pub async fn generate(
    request: GenerationRequest,
    events: Sender<GenerationEvent>,
    cancellation: CancellationToken,
) {
    match stream_step(request, &events, cancellation).await {
        Ok(message) if message_tool_calls(&message).is_empty() => {
            let _ = events.send(GenerationEvent::Completed).await;
        }
        Ok(_) => {
            let _ = events
                .send(GenerationEvent::Failed(GenerationError::new(
                    GenerationErrorKind::Unknown,
                    "Unexpected tool call",
                )))
                .await;
        }
        Err(error) => {
            let _ = events.send(GenerationEvent::Failed(error)).await;
        }
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::time::Duration;

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::oneshot,
    };

    pub(crate) async fn server(
        status: &str,
        content_type: &str,
        chunks: Vec<(Duration, String)>,
    ) -> (String, oneshot::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let status = status.to_string();
        let content_type = content_type.to_string();
        let content_length = chunks.iter().map(|(_, chunk)| chunk.len()).sum::<usize>();
        let (request_sender, request_receiver) = oneshot::channel();

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let request = read_request(&mut stream).await;
            let _ = request_sender.send(String::from_utf8_lossy(&request).into_owned());
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {content_length}\r\nConnection: close\r\n\r\n"
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            for (delay, chunk) in chunks {
                tokio::time::sleep(delay).await;
                if stream.write_all(chunk.as_bytes()).await.is_err() {
                    break;
                }
                let _ = stream.flush().await;
            }
        });

        (format!("http://{address}"), request_receiver)
    }

    pub(crate) async fn sequence_server(
        responses: Vec<String>,
    ) -> (String, async_channel::Receiver<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (captured_sender, captured_receiver) = async_channel::bounded(1);

        tokio::spawn(async move {
            let mut captured = Vec::new();
            for response in responses {
                let (mut stream, _) = listener.accept().await.unwrap();
                let request = read_request(&mut stream).await;
                captured.push(String::from_utf8_lossy(&request).into_owned());
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    response.len()
                );
                stream.write_all(header.as_bytes()).await.unwrap();
                stream.write_all(response.as_bytes()).await.unwrap();
                stream.flush().await.unwrap();
            }
            let _ = captured_sender.send(captured).await;
        });

        (format!("http://{address}"), captured_receiver)
    }

    pub(crate) fn fragmented(value: &str, width: usize) -> Vec<(Duration, String)> {
        value
            .as_bytes()
            .chunks(width)
            .map(|chunk| {
                (
                    Duration::from_millis(1),
                    String::from_utf8(chunk.to_vec()).unwrap(),
                )
            })
            .collect()
    }

    async fn read_request(stream: &mut tokio::net::TcpStream) -> Vec<u8> {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let read = stream.read(&mut buffer).await.unwrap_or_default();
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            let Some(header_end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") else {
                continue;
            };
            let body_start = header_end + 4;
            let headers = String::from_utf8_lossy(&request[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or_default();
            if request.len() >= body_start + content_length {
                break;
            }
        }
        request
    }

    pub(crate) fn request_json(request: &str) -> Value {
        let body = request.split_once("\r\n\r\n").unwrap().1;
        serde_json::from_str(body).unwrap()
    }

    use serde_json::Value;
}
