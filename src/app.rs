use std::time::Duration;

use gpui::{
    Context, FocusHandle, Render, ScrollHandle, Task, Timer, Window, div, prelude::*, px, rgb,
};

use crate::ui::composer::{Composer, ComposerEvent};

const FLUSH_INTERVAL: Duration = Duration::from_millis(40);
const PRODUCER_INTERVAL: Duration = Duration::from_millis(6);

pub struct OneChat {
    composer: gpui::Entity<Composer>,
    prompt: String,
    stream: DeltaCoalescer,
    scroll_handle: ScrollHandle,
    generation: u64,
    is_streaming: bool,
    stream_task: Option<Task<()>>,
}

impl OneChat {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let composer = cx.new(Composer::new);
        cx.subscribe(&composer, |this, _, event, cx| match event {
            ComposerEvent::Submit(prompt) => this.start_mock_stream(prompt.clone(), cx),
        })
        .detach();

        Self {
            composer,
            prompt: String::new(),
            stream: DeltaCoalescer::new(FLUSH_INTERVAL),
            scroll_handle: ScrollHandle::new(),
            generation: 0,
            is_streaming: false,
            stream_task: None,
        }
    }

    pub fn composer_focus_handle(&self, cx: &gpui::App) -> FocusHandle {
        self.composer.read(cx).focus_handle(cx)
    }

    fn start_mock_stream(&mut self, prompt: String, cx: &mut Context<Self>) {
        self.generation += 1;
        let generation = self.generation;
        self.prompt = prompt;
        self.stream.reset(Duration::ZERO);
        self.is_streaming = true;
        self.scroll_handle.scroll_to_bottom();
        cx.notify();

        let chunks = mock_chunks(&mock_response(&self.prompt), 6);
        self.stream_task = Some(cx.spawn(async move |this, cx| {
            let mut elapsed = Duration::ZERO;
            for chunk in chunks {
                Timer::after(PRODUCER_INTERVAL).await;
                elapsed += PRODUCER_INTERVAL;

                let keep_running = this
                    .update(cx, |this, cx| {
                        if this.generation != generation {
                            return false;
                        }

                        if this.stream.push(&chunk, elapsed) {
                            this.scroll_handle.scroll_to_bottom();
                            cx.notify();
                        }
                        true
                    })
                    .unwrap_or(false);

                if !keep_running {
                    return;
                }
            }

            let _ = this.update(cx, |this, cx| {
                if this.generation == generation {
                    this.stream.finish();
                    this.is_streaming = false;
                    this.scroll_handle.scroll_to_bottom();
                    cx.notify();
                }
            });
        }));
    }

    fn run_demo(&mut self, cx: &mut Context<Self>) {
        self.start_mock_stream(
            "Explain why coalescing stream deltas keeps a UI responsive.".into(),
            cx,
        );
    }
}

impl Render for OneChat {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let status = if self.is_streaming {
            format!("Streaming · {} UI refreshes", self.stream.flush_count())
        } else if self.stream.visible().is_empty() {
            "Ready".to_string()
        } else {
            format!("Completed · {} UI refreshes", self.stream.flush_count())
        };

        let output = if self.stream.visible().is_empty() {
            "Type a message below and press Enter, or run the built-in demo.\n\nThe mock producer emits a delta every 6ms; OneChat only refreshes this view every 40ms."
                .to_string()
        } else {
            self.stream.visible().to_string()
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xf4f5f7))
            .text_color(rgb(0x202124))
            .child(
                div()
                    .h(px(58.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_6()
                    .border_b_1()
                    .border_color(rgb(0xdfe1e5))
                    .bg(rgb(0xffffff))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child("OneChat"))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0x6b7280))
                                    .child("GPUI capability lab"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .px_3()
                                    .py_1()
                                    .rounded_full()
                                    .bg(if self.is_streaming {
                                        rgb(0xe8f1ff)
                                    } else {
                                        rgb(0xecf7ef)
                                    })
                                    .text_sm()
                                    .child(status),
                            )
                            .child(
                                div()
                                    .id("run-mock-stream")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(rgb(0xc9cdd4))
                                    .bg(rgb(0xffffff))
                                    .cursor_pointer()
                                    .hover(|style| style.bg(rgb(0xf0f2f5)))
                                    .on_click(cx.listener(|this, _, _, cx| this.run_demo(cx)))
                                    .child("Run demo"),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .justify_center()
                    .p_6()
                    .child(
                        div()
                            .w_full()
                            .max_w(px(820.0))
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(
                                div()
                                    .flex_1()
                                    .min_h_0()
                                    .rounded_xl()
                                    .border_1()
                                    .border_color(rgb(0xdfe1e5))
                                    .bg(rgb(0xffffff))
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .id("stream-output")
                                            .size_full()
                                            .overflow_y_scroll()
                                            .track_scroll(&self.scroll_handle)
                                            .p_6()
                                            .line_height(px(24.0))
                                            .child(
                                                div()
                                                    .w_full()
                                                    .text_color(if self.stream.visible().is_empty() {
                                                        rgb(0x6b7280)
                                                    } else {
                                                        rgb(0x202124)
                                                    })
                                                    .child(output),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .child(self.composer.clone())
                                    .child(
                                        div()
                                            .pt_2()
                                            .px_2()
                                            .text_xs()
                                            .text_color(rgb(0x7b8190))
                                            .child("Enter to send · Shift+Enter for a new line · IME and clipboard supported"),
                                    ),
                            ),
                    ),
            )
    }
}

#[derive(Debug)]
struct DeltaCoalescer {
    visible: String,
    pending: String,
    interval: Duration,
    last_flush: Duration,
    flush_count: usize,
}

impl DeltaCoalescer {
    fn new(interval: Duration) -> Self {
        Self {
            visible: String::new(),
            pending: String::new(),
            interval,
            last_flush: Duration::ZERO,
            flush_count: 0,
        }
    }

    fn reset(&mut self, now: Duration) {
        self.visible.clear();
        self.pending.clear();
        self.last_flush = now;
        self.flush_count = 0;
    }

    fn push(&mut self, delta: &str, now: Duration) -> bool {
        self.pending.push_str(delta);
        if now.saturating_sub(self.last_flush) >= self.interval {
            self.flush(now)
        } else {
            false
        }
    }

    fn finish(&mut self) -> bool {
        self.flush(self.last_flush)
    }

    fn flush(&mut self, now: Duration) -> bool {
        if self.pending.is_empty() {
            return false;
        }
        self.visible.push_str(&self.pending);
        self.pending.clear();
        self.last_flush = now;
        self.flush_count += 1;
        true
    }

    fn visible(&self) -> &str {
        &self.visible
    }

    fn flush_count(&self) -> usize {
        self.flush_count
    }
}

fn mock_chunks(text: &str, chars_per_chunk: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        current.push(ch);
        if current.chars().count() == chars_per_chunk {
            chunks.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

fn mock_response(prompt: &str) -> String {
    let section = "A responsive streaming UI separates producer cadence from render cadence. Network chunks may arrive every few milliseconds, but laying out the document for every chunk creates unnecessary work. OneChat therefore appends incoming deltas to a pending buffer and exposes them to GPUI roughly every 40ms. This preserves ordering while reducing layout churn.\n\nThe output view is scrollable and follows the newest merged update. The input below is a real EntityInputHandler prototype: it keeps UTF-8 storage, translates the UTF-16 ranges used by macOS text services, tracks marked text during IME composition, and grows as wrapped lines are added.\n\n";

    format!(
        "Prompt\n{prompt}\n\nMock response\n\n{section}{section}{section}End of simulated stream. The complete text was delivered without scheduling a render for every producer delta."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coalesces_deltas_at_the_configured_interval() {
        let mut stream = DeltaCoalescer::new(Duration::from_millis(40));

        assert!(!stream.push("a", Duration::from_millis(5)));
        assert!(!stream.push("b", Duration::from_millis(20)));
        assert_eq!(stream.visible(), "");
        assert!(stream.push("c", Duration::from_millis(40)));
        assert_eq!(stream.visible(), "abc");
        assert_eq!(stream.flush_count(), 1);
    }

    #[test]
    fn finish_forces_the_last_partial_batch_without_reordering() {
        let mut stream = DeltaCoalescer::new(Duration::from_millis(40));
        stream.push("first ", Duration::from_millis(40));
        stream.push("second", Duration::from_millis(45));

        assert!(stream.finish());
        assert_eq!(stream.visible(), "first second");
        assert_eq!(stream.flush_count(), 2);
        assert!(!stream.finish());
    }

    #[test]
    fn mock_chunking_preserves_unicode_text() {
        let text = "Hello 世界 👋🏽";
        assert_eq!(mock_chunks(text, 3).concat(), text);
    }
}
