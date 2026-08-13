use std::time::Duration;

use onechat::speech::{
    AudioCppBackend, SpeechBackend, SpeechErrorKind, SynthesisRequest, TranscriptionRequest,
};
use serde_json::{Map, Value, json};

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

#[derive(Debug, Clone)]
struct RecordedRequest {
    method: String,
    target: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

#[derive(Debug)]
struct MockResponse {
    status: u16,
    content_type: &'static str,
    body: Vec<u8>,
    delay: Duration,
}

impl MockResponse {
    fn json(value: Value) -> Self {
        Self {
            status: 200,
            content_type: "application/json",
            body: serde_json::to_vec(&value).unwrap(),
            delay: Duration::ZERO,
        }
    }

    fn wav() -> Self {
        Self {
            status: 200,
            content_type: "audio/wav",
            body: b"RIFF\x04\x00\x00\x00WAVE".to_vec(),
            delay: Duration::ZERO,
        }
    }
}

async fn server(
    responses: Vec<MockResponse>,
) -> (String, Arc<Mutex<Vec<RecordedRequest>>>, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let responses = Arc::new(Mutex::new(VecDeque::from(responses)));
    let task = tokio::spawn({
        let requests = requests.clone();
        async move {
            while !responses.lock().unwrap().is_empty() {
                let (stream, _) = listener.accept().await.unwrap();
                let response = responses.lock().unwrap().pop_front().unwrap();
                let request = read_request(stream, response, requests.clone());
                request.await;
            }
        }
    });
    (format!("http://{address}"), requests, task)
}

async fn read_request(
    mut stream: TcpStream,
    response: MockResponse,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
) {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    let header_end = loop {
        let count = stream.read(&mut chunk).await.unwrap();
        assert!(count > 0);
        buffer.extend_from_slice(&chunk[..count]);
        if let Some(position) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }
    };
    let header = String::from_utf8(buffer[..header_end].to_vec()).unwrap();
    let mut lines = header.split("\r\n");
    let mut request_line = lines.next().unwrap().split_whitespace();
    let method = request_line.next().unwrap().to_owned();
    let target = request_line.next().unwrap().to_owned();
    let headers: Vec<_> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    let content_length = headers
        .iter()
        .find(|(name, _)| name == "content-length")
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    while buffer.len() - header_end < content_length {
        let count = stream.read(&mut chunk).await.unwrap();
        assert!(count > 0);
        buffer.extend_from_slice(&chunk[..count]);
    }
    requests.lock().unwrap().push(RecordedRequest {
        method,
        target,
        headers,
        body: buffer[header_end..header_end + content_length].to_vec(),
    });

    tokio::time::sleep(response.delay).await;
    let reason = match response.status {
        200 => "OK",
        400 => "Bad Request",
        429 => "Too Many Requests",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let reply = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response.status,
        reason,
        response.content_type,
        response.body.len()
    );
    stream.write_all(reply.as_bytes()).await.unwrap();
    stream.write_all(&response.body).await.unwrap();
}

fn header<'a>(request: &'a RecordedRequest, name: &str) -> Option<&'a str> {
    request
        .headers
        .iter()
        .find(|(header, _)| header == name)
        .map(|(_, value)| value.as_str())
}

fn synthesis(seed: u64) -> SynthesisRequest {
    SynthesisRequest {
        model: "tts-model".into(),
        input: "Hello".into(),
        voice: Some("voice-a".into()),
        seed: Some(seed),
        max_tokens: Some(100),
        speed: Some(1.1),
        temperature: Some(0.7),
        top_p: Some(0.9),
        extra_options: Map::from_iter([("custom".into(), json!(true))]),
    }
}

#[tokio::test]
async fn all_endpoints_use_exact_contract_and_optional_bearer() {
    let responses = vec![
        MockResponse::json(json!({"status":"ok","backend":"metal","models":2})),
        MockResponse::json(json!({
            "object":"list",
            "data":[
                {"id":"tts-model","task":"tts","family":"qwen","loaded":true},
                {"id":"asr-model","task":"asr","mode":"transcription","timing":true}
            ]
        })),
        MockResponse::json(json!({"voices":["voice-a","voice-b"]})),
        MockResponse::wav(),
        MockResponse::json(json!({"text":"Hello","timing":{"duration":1.0}})),
    ];
    let (root, requests, task) = server(responses).await;
    let backend = AudioCppBackend::new(
        &format!("{root}/"),
        Some(" secret "),
        Duration::from_secs(2),
    )
    .unwrap();
    assert_eq!(backend.endpoint(), root);
    assert!(backend.health().await.unwrap().ready);
    let catalog = backend.models().await.unwrap();
    assert_eq!(catalog.tts[0].id, "tts-model");
    assert_eq!(catalog.asr[0].id, "asr-model");
    assert_eq!(
        backend.voices("tts-model").await.unwrap(),
        ["voice-a", "voice-b"]
    );
    assert_eq!(
        backend.synthesize(synthesis(42)).await.unwrap(),
        b"RIFF\x04\x00\x00\x00WAVE"
    );
    assert_eq!(
        backend
            .transcribe(TranscriptionRequest {
                model: "asr-model".into(),
                wav: b"RIFF\x04\x00\x00\x00WAVE".to_vec(),
                language: Some("en".into()),
            })
            .await
            .unwrap(),
        "Hello"
    );
    task.await.unwrap();

    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 5);
    assert!(
        requests
            .iter()
            .all(|request| header(request, "authorization") == Some("Bearer secret"))
    );
    assert_eq!(
        (&requests[0].method[..], &requests[0].target[..]),
        ("GET", "/health")
    );
    assert_eq!(requests[1].target, "/v1/models");
    assert_eq!(requests[2].target, "/v1/audio/voices?model=tts-model");
    let speech: Value = serde_json::from_slice(&requests[3].body).unwrap();
    assert_eq!(speech["response_format"], "wav");
    assert_eq!(speech["seed"], 42);
    assert_eq!(speech["custom"], true);
    let transcription = &requests[4];
    assert!(
        header(transcription, "content-type")
            .unwrap()
            .starts_with("multipart/form-data; boundary=")
    );
    let body = String::from_utf8_lossy(&transcription.body);
    assert!(body.contains("name=\"model\""));
    assert!(body.contains("asr-model"));
    assert!(body.contains("name=\"language\""));
    assert!(body.contains("filename=\"segment.wav\""));
    assert!(body.contains("RIFF"));
}

#[tokio::test]
async fn large_seed_is_a_string_and_empty_token_sends_no_authorization() {
    let (root, requests, task) = server(vec![MockResponse::wav()]).await;
    let backend = AudioCppBackend::new(&root, Some(" "), Duration::from_secs(2)).unwrap();
    backend.synthesize(synthesis(1_u64 << 60)).await.unwrap();
    task.await.unwrap();
    let requests = requests.lock().unwrap();
    let payload: Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(payload["seed"], (1_u64 << 60).to_string());
    assert_eq!(header(&requests[0], "authorization"), None);
}

#[tokio::test]
async fn classifies_http_protocol_connection_and_timeout_errors() {
    let (root, _, task) = server(vec![MockResponse {
        status: 503,
        content_type: "application/json",
        body: serde_json::to_vec(&json!({"error":{"message":"busy","type":"server_busy"}}))
            .unwrap(),
        delay: Duration::ZERO,
    }])
    .await;
    let backend = AudioCppBackend::new(&root, None, Duration::from_secs(2)).unwrap();
    let error = backend.health().await.unwrap_err();
    assert_eq!(error.kind, SpeechErrorKind::Http);
    assert_eq!(error.status_code, Some(503));
    assert_eq!(error.service_message.as_deref(), Some("busy"));
    assert!(error.retryable);
    task.await.unwrap();

    let (root, _, task) = server(vec![MockResponse::json(json!({"data":[{"id":"x"}]}))]).await;
    let backend = AudioCppBackend::new(&root, None, Duration::from_secs(2)).unwrap();
    let error = backend.models().await.unwrap_err();
    assert_eq!(error.kind, SpeechErrorKind::Protocol);
    task.await.unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let root = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let backend = AudioCppBackend::new(&root, None, Duration::from_millis(100)).unwrap();
    let error = backend.health().await.unwrap_err();
    assert_eq!(error.kind, SpeechErrorKind::Transport);
    assert!(error.retryable);

    let (root, _, task) = server(vec![MockResponse {
        delay: Duration::from_millis(200),
        ..MockResponse::json(json!({"status":"ok"}))
    }])
    .await;
    let backend = AudioCppBackend::new(&root, None, Duration::from_millis(20)).unwrap();
    let error = backend.health().await.unwrap_err();
    assert_eq!(error.kind, SpeechErrorKind::Transport);
    assert!(error.message.contains("timed out"));
    task.abort();
}

#[tokio::test]
async fn rejects_invalid_shapes_speech_content_and_transcription_text() {
    let responses = vec![
        MockResponse::json(json!(["not-an-object"])),
        MockResponse {
            status: 200,
            content_type: "application/json",
            body: b"{}".to_vec(),
            delay: Duration::ZERO,
        },
        MockResponse::json(json!({"transcript":"wrong field"})),
    ];
    let (root, _, task) = server(responses).await;
    let backend = AudioCppBackend::new(&root, None, Duration::from_secs(2)).unwrap();
    assert_eq!(
        backend.health().await.unwrap_err().kind,
        SpeechErrorKind::Protocol
    );
    assert_eq!(
        backend.synthesize(synthesis(1)).await.unwrap_err().kind,
        SpeechErrorKind::Protocol
    );
    assert_eq!(
        backend
            .transcribe(TranscriptionRequest {
                model: "asr".into(),
                wav: vec![1],
                language: None,
            })
            .await
            .unwrap_err()
            .kind,
        SpeechErrorKind::Protocol
    );
    task.await.unwrap();
}

#[tokio::test]
async fn validates_client_and_method_arguments() {
    assert!(AudioCppBackend::new("localhost:8080", None, Duration::from_secs(1)).is_err());
    assert!(AudioCppBackend::new("http://localhost?a=1", None, Duration::from_secs(1)).is_err());
    assert!(AudioCppBackend::new("http://localhost", None, Duration::ZERO).is_err());

    let (root, _, task) = server(Vec::new()).await;
    let backend = AudioCppBackend::new(&root, None, Duration::from_secs(1)).unwrap();
    assert!(backend.voices(" ").await.is_err());
    assert!(
        backend
            .synthesize(SynthesisRequest {
                input: " ".into(),
                ..synthesis(1)
            })
            .await
            .is_err()
    );
    assert!(
        backend
            .transcribe(TranscriptionRequest {
                model: "asr".into(),
                wav: Vec::new(),
                language: None,
            })
            .await
            .is_err()
    );
    task.await.unwrap();
}
