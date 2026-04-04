use std::sync::atomic::{AtomicUsize, Ordering};

static ZSTD_THREADS: AtomicUsize = AtomicUsize::new(0);

pub fn set_zstd_threads(threads: usize) {
    ZSTD_THREADS.store(threads, Ordering::Relaxed);
}

pub fn zstd_threads() -> usize {
    ZSTD_THREADS.load(Ordering::Relaxed)
}
