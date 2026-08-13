use std::collections::HashSet;

use gpui::{Context, ScrollHandle, Task, Window};
use tokio_util::sync::CancellationToken;

use super::controls::TtsControls;
use crate::{
    desktop::app::{DrawerMotion, OneChat},
    speech::{
        HealthInfo, ModelCatalog, RunSnapshot, RunStatus, SegmentResult, SegmentStatus,
        SpeechConfig, SpeechError, SpeechEvent, SpeechRun,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TtsOperationKind {
    Discovery,
    Generate,
    Regenerate(usize),
    RetryFailed,
}

#[derive(Debug)]
pub(crate) struct ActiveTtsOperation {
    pub(crate) id: u64,
    pub(crate) kind: TtsOperationKind,
    pub(crate) cancellation: CancellationToken,
}

#[derive(Debug, Default)]
pub(crate) struct TtsOperationManager {
    next_id: u64,
    active: Option<ActiveTtsOperation>,
}

impl TtsOperationManager {
    pub(crate) fn start(&mut self, kind: TtsOperationKind) -> Option<(u64, CancellationToken)> {
        if self.active.is_some() {
            return None;
        }
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let cancellation = CancellationToken::new();
        let id = self.next_id;
        self.active = Some(ActiveTtsOperation {
            id,
            kind,
            cancellation: cancellation.clone(),
        });
        Some((id, cancellation))
    }

    pub(crate) fn is_current(&self, id: u64) -> bool {
        self.active.as_ref().is_some_and(|active| active.id == id)
    }

    pub(crate) fn finish(&mut self, id: u64) -> bool {
        if !self.is_current(id) {
            return false;
        }
        self.active = None;
        true
    }

    pub(crate) fn cancel(&self) -> bool {
        let Some(active) = &self.active else {
            return false;
        };
        active.cancellation.cancel();
        true
    }

    pub(crate) fn active(&self) -> Option<&ActiveTtsOperation> {
        self.active.as_ref()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct TtsDiscovery {
    pub(crate) loading: bool,
    pub(crate) health: Option<HealthInfo>,
    pub(crate) catalog: ModelCatalog,
    pub(crate) voices: Vec<String>,
    pub(crate) error: Option<SpeechError>,
}

#[derive(Debug, Default)]
pub(crate) struct TtsController {
    pub(crate) config: SpeechConfig,
    pub(crate) source: String,
    pub(crate) discovery: TtsDiscovery,
    pub(crate) run: Option<SpeechRun>,
    pub(crate) operation: TtsOperationManager,
    pub(crate) error: Option<SpeechError>,
    pub(crate) audio_revision: u64,
    source_revision: u64,
    config_revision: u64,
    run_source_revision: Option<u64>,
    run_config_revision: Option<u64>,
}

impl TtsController {
    pub(crate) fn set_source(&mut self, source: String) {
        if self.source != source {
            self.source = source;
            self.source_revision = self.source_revision.wrapping_add(1);
        }
    }

    pub(crate) fn update_config(&mut self, update: impl FnOnce(&mut SpeechConfig)) {
        let before = self.config.clone();
        update(&mut self.config);
        if self.config != before {
            self.config_revision = self.config_revision.wrapping_add(1);
        }
    }

    pub(crate) fn bump_audio_revision(&mut self) {
        self.audio_revision = self.audio_revision.wrapping_add(1);
    }

    pub(crate) fn run_is_stale(&self) -> bool {
        self.run.is_some()
            && (self.run_source_revision != Some(self.source_revision)
                || self.run_config_revision != Some(self.config_revision))
    }

    pub(crate) fn begin_discovery(&mut self) {
        self.discovery.loading = true;
        self.discovery.error = None;
        self.error = None;
    }

    pub(crate) fn apply_discovery(
        &mut self,
        health: HealthInfo,
        catalog: ModelCatalog,
        voices: Vec<String>,
    ) {
        self.discovery = TtsDiscovery {
            loading: false,
            health: Some(health),
            catalog,
            voices,
            error: None,
        };
    }

    pub(crate) fn fail_discovery(&mut self, error: SpeechError) {
        self.discovery.loading = false;
        self.discovery.error = Some(error.clone());
        self.error = Some(error);
    }

    pub(crate) fn apply_speech_event(&mut self, event: SpeechEvent) {
        match event {
            SpeechEvent::RunStarted { snapshot } => {
                let starts_new_run = self.operation.active().is_none_or(|active| {
                    active.kind == TtsOperationKind::Generate || self.run.is_none()
                });
                if starts_new_run {
                    self.run_source_revision = Some(self.source_revision);
                    self.run_config_revision = Some(self.config_revision);
                    self.run = Some(started_run(*snapshot));
                }
                self.error = None;
            }
            SpeechEvent::SegmentChanged {
                index,
                status,
                attempt,
            } => {
                if let Some(result) = self.run.as_mut().and_then(|run| {
                    run.segments
                        .iter_mut()
                        .find(|result| result.segment.index == index)
                }) {
                    result.status = status;
                    result.attempt = attempt;
                }
            }
            SpeechEvent::SegmentFinished { result } => {
                replace_result(&mut self.run, *result);
            }
            SpeechEvent::RunFinished { run } => self.run = Some(*run),
        }
    }

    pub(crate) fn finish_speech(&mut self, result: Result<SpeechRun, SpeechError>) {
        match result {
            Ok(run) => self.run = Some(run),
            Err(error) => self.error = Some(error),
        }
    }
}

fn started_run(snapshot: RunSnapshot) -> SpeechRun {
    let segments = snapshot
        .segments
        .iter()
        .cloned()
        .map(|segment| SegmentResult {
            segment,
            status: SegmentStatus::Waiting,
            attempt: 0,
            seed: None,
            clip: None,
            error: None,
            audio_validation: None,
            transcript_validation: None,
        })
        .collect();
    SpeechRun {
        snapshot,
        status: RunStatus::Running,
        segments,
        combined_clip: None,
        final_validation: None,
        error: None,
    }
}

fn replace_result(run: &mut Option<SpeechRun>, result: SegmentResult) {
    let Some(current) = run.as_mut() else {
        return;
    };
    if let Some(position) = current
        .segments
        .iter()
        .position(|stored| stored.segment.index == result.segment.index)
    {
        current.segments[position] = result;
    }
}

#[derive(Debug, Default)]
pub(crate) struct TtsViewState {
    pub(crate) expanded_segments: HashSet<usize>,
    pub(crate) technical_segments: HashSet<usize>,
    pub(crate) connection_popover_open: bool,
    pub(crate) inspector_open: bool,
    pub(crate) audio_thresholds_expanded: bool,
    pub(crate) transcript_details_expanded: bool,
}

pub(crate) struct TtsState {
    pub(crate) controller: TtsController,
    pub(crate) controls: TtsControls,
    pub(crate) output_scroll: ScrollHandle,
    pub(crate) view: TtsViewState,
    pub(crate) inspector_motion: DrawerMotion,
    pub(crate) completion_notice: Option<String>,
    pub(super) event_task: Task<()>,
}

impl TtsState {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<OneChat>) -> Self {
        Self {
            controller: TtsController::default(),
            controls: TtsControls::new(window, cx),
            output_scroll: ScrollHandle::new(),
            view: TtsViewState::default(),
            inspector_motion: DrawerMotion::new(false),
            completion_notice: None,
            event_task: Task::ready(()),
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
