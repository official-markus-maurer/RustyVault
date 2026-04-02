use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopMode {
    Running,
    SoftStop,
    HardStop,
}

#[derive(Clone, Default)]
pub struct ProcessControl {
    inner: Arc<Inner>,
}

#[derive(Default)]
struct Inner {
    mode: AtomicU8,
    paused: AtomicBool,
    pause_lock: Mutex<()>,
    pause_cv: Condvar,
}

impl ProcessControl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request_soft_stop(&self) {
        let _ = self.inner.mode.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
            if current >= 2 {
                None
            } else {
                Some(1)
            }
        });
        self.unpause();
    }

    pub fn request_hard_stop(&self) {
        self.inner.mode.store(2, Ordering::SeqCst);
        self.unpause();
    }

    pub fn stop_mode(&self) -> StopMode {
        match self.inner.mode.load(Ordering::SeqCst) {
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

    pub fn pause(&self) {
        self.inner.paused.store(true, Ordering::SeqCst);
    }

    pub fn unpause(&self) {
        self.inner.paused.store(false, Ordering::SeqCst);
        self.inner.pause_cv.notify_all();
    }

    pub fn is_paused(&self) -> bool {
        self.inner.paused.load(Ordering::SeqCst)
    }

    pub fn wait_one(&self) {
        let mut guard = self.inner.pause_lock.lock().unwrap_or_else(|e| e.into_inner());
        while self.is_paused() && !self.is_soft_stop_requested() {
            guard = self.inner.pause_cv.wait(guard).unwrap_or_else(|e| e.into_inner());
        }
    }

    pub fn reset(&self) {
        self.inner.mode.store(0, Ordering::SeqCst);
        self.unpause();
    }
}
