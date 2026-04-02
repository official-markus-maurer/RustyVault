use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopMode {
    Running,
    SoftStop,
    HardStop,
}

#[derive(Clone, Default)]
pub struct ProcessControl {
    mode: Arc<AtomicU8>,
}

impl ProcessControl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request_soft_stop(&self) {
        let _ = self.mode.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
            if current >= 2 {
                None
            } else {
                Some(1)
            }
        });
    }

    pub fn request_hard_stop(&self) {
        self.mode.store(2, Ordering::SeqCst);
    }

    pub fn stop_mode(&self) -> StopMode {
        match self.mode.load(Ordering::SeqCst) {
            1 => StopMode::SoftStop,
            2 => StopMode::HardStop,
            _ => StopMode::Running,
        }
    }

    pub fn is_soft_stop_requested(&self) -> bool {
        matches!(self.stop_mode(), StopMode::SoftStop | StopMode::HardStop)
    }

    pub fn is_hard_stop_requested(&self) -> bool {
        self.stop_mode() == StopMode::HardStop
    }
}
