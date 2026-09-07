use std::time::Duration;

use onechat::{
    domain::{
        GenerationConfig, GenerationRequest, Message, Model, ModelReasoningConfig, Provider,
        ProviderKind,
    },
    providers::{list_models, stream_step},
};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
    time::timeout,
};
use tokio_util::sync::CancellationToken;

async fn server(
    responses: Vec<(&'static str, Value)>,
) -> (String, JoinHandle<Vec<(String, Value)>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        timeout(Duration::from_secs(10), async move {
            let mut requests = Vec::new();
            for (status, response) in responses {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                let mut chunk = [0; 4096];
                let header_end = loop {
                    let count = stream.read(&mut chunk).await.unwrap();
                    assert!(count > 0);
                    bytes.extend_from_slice(&chunk[..count]);
                    if let Some(index) = bytes.windows(4).position(|value| value == b"\r\n\r\n") {
                        break index + 4;
                    }
                };
                let header = String::from_utf8(bytes[..header_end].to_vec()).unwrap();
                let length = header.lines().filter_map(|line| line.split_once(':'))
                    .find(|(key, _)| key.eq_ignore_ascii_case("content-length"))
                    .map_or(0, |(_, value)| value.trim().parse::<usize>().unwrap());
                while bytes.len() < header_end + length {
                    let count = stream.read(&mut chunk).await.unwrap();
                    assert!(count > 0);
                    bytes.extend_from_slice(&chunk[..count]);
                }
                let body = if length == 0 { Value::Null } else {
                    serde_json::from_slice(&bytes[header_end..header_end + length]).unwrap()
                };
                requests.push((header.lines().next().unwrap().to_string(), body));
                let body = response.to_string();
                let reply = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                stream.write_all(reply.as_bytes()).await.unwrap();
            }
            requests
        }).await.unwrap()
    });
    (endpoint, task)
}

#[tokio::test]
async fn discovered_reasoning_presets_select_wire_model_ids_for_every_provider() {
    for kind in ProviderKind::ALL {
        let metadata = if kind == ProviderKind::Gemini {
            json!({"models": [{"name": "models/model"}, {"name": "models/model:Off"}, {"name": "models/model:HIGH"}]})
        } else {
            json!({"data": [{"id": "model"}, {"id": "model:Off"}, {"id": "model:HIGH"}]})
        };
        let mut responses = vec![("200 OK", metadata)];
        responses.extend((0..3).map(|_| {
            (
                "400 Bad Request",
                json!({"error": {"message": "request recorded"}}),
            )
        }));
        let (endpoint, server) = server(responses).await;
        let mut provider = Provider::new("Test", kind);
        provider.endpoint = endpoint;
        provider.api_key = "test".into();
        let models = list_models(&provider).await.unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "model");
        let mut model = Model::new(&provider.id, &models[0].id, "Model", kind);
        model.reasoning = Some(ModelReasoningConfig::ModelIdSuffix(
            models[0].reasoning.clone().unwrap(),
        ));
        let cases = [
            (None, "model"),
            (Some("off"), "model:Off"),
            (Some("high"), "model:HIGH"),
        ];
        for (selected, _) in cases {
            let request = GenerationRequest {
                provider: provider.clone(),
                model: model.clone(),
                config: GenerationConfig {
                    reasoning_preset: selected.map(str::to_string),
                    ..GenerationConfig::default()
                },
                system_prompt: String::new(),
                messages: vec![Message::user("Hello")],
                audio_duration_ms: 0,
                tools: Vec::new(),
            };
            let (events, _receiver) = async_channel::unbounded();
            let result = timeout(
                Duration::from_secs(10),
                stream_step(request, &events, CancellationToken::new()),
            )
            .await
            .unwrap();
            assert!(
                result.is_err(),
                "the recording server intentionally rejects generation"
            );
        }
        let requests = server.await.unwrap();
        assert_eq!(requests.len(), 4);
        for ((path, body), (_, expected)) in requests.iter().skip(1).zip(cases) {
            if kind == ProviderKind::Gemini {
                assert!(
                    path.contains(&format!("/models/{expected}:streamGenerateContent")),
                    "{path}"
                );
            } else {
                assert_eq!(body["model"], expected, "{kind:?}: {body}");
            }
            for key in ["reasoning", "reasoning_effort", "thinking"] {
                assert!(body.get(key).is_none(), "{kind:?}: {body}");
            }
            assert!(body["generationConfig"].get("thinkingConfig").is_none());
        }
    }
}

#[tokio::test]
async fn reasoning_groups_span_all_model_list_pages() {
    for kind in [ProviderKind::Anthropic, ProviderKind::Gemini] {
        let pages = if kind == ProviderKind::Anthropic {
            vec![
                json!({"data": [{"id": "model:low"}], "has_more": true, "last_id": "model:low"}),
                json!({"data": [{"id": "model:medium"}, {"id": "model:high"}], "has_more": false}),
            ]
        } else {
            vec![
                json!({"models": [{"name": "models/model:low"}], "nextPageToken": "next"}),
                json!({"models": [{"name": "models/model:medium"}, {"name": "models/model:high"}]}),
            ]
        };
        let (endpoint, server) =
            server(pages.into_iter().map(|body| ("200 OK", body)).collect()).await;
        let mut provider = Provider::new("Test", kind);
        provider.endpoint = endpoint;
        let models = list_models(&provider).await.unwrap();
        assert_eq!(models.len(), 1);
        let config = models[0].reasoning.as_ref().unwrap();
        assert_eq!(config.presets.len(), 3);
        assert_eq!(config.default_preset, "medium");
        assert_eq!(
            config.resolve_preset(None).unwrap().model_id,
            "model:medium"
        );
        assert!(config.presets.iter().all(|preset| preset.level.is_some()));
        assert_eq!(server.await.unwrap().len(), 2);
    }
}
