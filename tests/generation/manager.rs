use super::*;

#[test]
fn generation_manager_prevents_parallel_runs_and_stops_the_active_run() {
    let mut manager = GenerationManager::default();
    let cancellation = CancellationToken::new();
    assert!(manager.start(
        "conversation".into(),
        "request-1".into(),
        "response-1".into(),
        cancellation.clone(),
    ));
    assert!(!manager.start(
        "conversation".into(),
        "request-2".into(),
        "response-2".into(),
        CancellationToken::new(),
    ));
    assert!(manager.stop("conversation"));
    assert!(cancellation.is_cancelled());

    manager.finish("conversation", "another-request");
    assert!(manager.is_active("conversation"));
    manager.finish("conversation", "request-1");
    assert!(!manager.is_active("conversation"));
}
