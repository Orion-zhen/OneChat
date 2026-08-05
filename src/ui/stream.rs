use std::time::Duration;

#[derive(Debug)]
pub struct DeltaCoalescer {
    visible: String,
    pending: String,
    interval: Duration,
    last_flush: Duration,
    flush_count: usize,
}

impl DeltaCoalescer {
    pub fn new(interval: Duration) -> Self {
        Self {
            visible: String::new(),
            pending: String::new(),
            interval,
            last_flush: Duration::ZERO,
            flush_count: 0,
        }
    }

    pub fn reset(&mut self, now: Duration) {
        self.visible.clear();
        self.pending.clear();
        self.last_flush = now;
        self.flush_count = 0;
    }

    pub fn push(&mut self, delta: &str, now: Duration) -> bool {
        self.pending.push_str(delta);
        if now.saturating_sub(self.last_flush) >= self.interval {
            self.flush(now)
        } else {
            false
        }
    }

    pub fn finish(&mut self) -> bool {
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

    pub fn visible(&self) -> &str {
        &self.visible
    }

    pub fn flush_count(&self) -> usize {
        self.flush_count
    }
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
    fn reset_discards_visible_and_pending_text() {
        let mut stream = DeltaCoalescer::new(Duration::from_millis(40));
        stream.push("visible", Duration::from_millis(40));
        stream.push("pending", Duration::from_millis(45));
        stream.reset(Duration::from_millis(50));

        assert_eq!(stream.visible(), "");
        assert_eq!(stream.flush_count(), 0);
        assert!(!stream.finish());
    }
}
