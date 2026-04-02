use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use compress::structured_archive::ZipStructure;

use crate::process_control::ProcessControl;
use crate::torrent_zip::TorrentZip;
use crate::torrent_zip_make::TorrentZipMake;
use crate::trrntzip_status::TrrntZipStatus;

#[derive(Debug, Clone)]
pub struct ProcessItem {
    pub file_id: i32,
    pub path: String,
    pub is_dir: bool,
}

type StartCallback = Arc<dyn Fn(usize, i32, &str) + Send + Sync>;
type EndCallback = Arc<dyn Fn(usize, i32, TrrntZipStatus) + Send + Sync>;

struct BlockingQueue {
    items: Mutex<VecDeque<ProcessItem>>,
    cv: Condvar,
    closed: AtomicBool,
}

impl BlockingQueue {
    fn new() -> Self {
        Self {
            items: Mutex::new(VecDeque::new()),
            cv: Condvar::new(),
            closed: AtomicBool::new(false),
        }
    }

    fn push(&self, item: ProcessItem) {
        if self.closed.load(Ordering::SeqCst) {
            return;
        }
        let mut guard = self.items.lock().unwrap_or_else(|e| e.into_inner());
        guard.push_back(item);
        self.cv.notify_one();
    }

    fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.cv.notify_all();
    }

    fn pop(&self, control: Option<&ProcessControl>) -> Option<ProcessItem> {
        let mut guard = self.items.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if let Some(item) = guard.pop_front() {
                return Some(item);
            }
            if self.closed.load(Ordering::SeqCst) {
                return None;
            }
            if control.is_some_and(|c| c.is_soft_stop_requested()) {
                return None;
            }
            guard = self.cv.wait(guard).unwrap_or_else(|e| e.into_inner());
        }
    }
}

pub struct ProcessZip {
    worker_count: usize,
    queue: Arc<BlockingQueue>,
    torrent_zip: TorrentZip,
    control: Option<ProcessControl>,
    on_start: Option<StartCallback>,
    on_end: Option<EndCallback>,
}

impl ProcessZip {
    pub fn new(worker_count: usize, torrent_zip: TorrentZip) -> Self {
        Self {
            worker_count: worker_count.max(1),
            queue: Arc::new(BlockingQueue::new()),
            torrent_zip,
            control: None,
            on_start: None,
            on_end: None,
        }
    }

    pub fn set_control(&mut self, control: ProcessControl) {
        self.control = Some(control);
    }

    pub fn set_callbacks(&mut self, on_start: Option<StartCallback>, on_end: Option<EndCallback>) {
        self.on_start = on_start;
        self.on_end = on_end;
    }

    pub fn push(&self, item: ProcessItem) {
        self.queue.push(item);
    }

    pub fn close(&self) {
        self.queue.close();
    }

    pub fn run(self) {
        let mut handles = Vec::with_capacity(self.worker_count);
        for thread_id in 0..self.worker_count {
            let queue = Arc::clone(&self.queue);
            let tz = self.torrent_zip;
            let control = self.control.clone();
            let on_start = self.on_start.clone();
            let on_end = self.on_end.clone();

            handles.push(std::thread::spawn(move || loop {
                if let Some(c) = control.as_ref() {
                    c.wait_one();
                    if c.is_soft_stop_requested() {
                        break;
                    }
                }

                let item = queue.pop(control.as_ref());
                let Some(item) = item else {
                    break;
                };

                if let Some(c) = control.as_ref() {
                    if c.is_soft_stop_requested() {
                        if let Some(cb) = on_end.as_ref() {
                            cb(
                                thread_id,
                                item.file_id,
                                if c.is_hard_stop_requested() {
                                    TrrntZipStatus::USER_ABORTED_HARD
                                } else {
                                    TrrntZipStatus::USER_ABORTED
                                },
                            );
                        }
                        continue;
                    }
                }

                if let Some(cb) = on_start.as_ref() {
                    cb(thread_id, item.file_id, &item.path);
                }

                let status = if item.is_dir {
                    if tz.out_zip_type == ZipStructure::ZipTrrnt {
                        TorrentZipMake::zip_directory_with_control(&item.path, ZipStructure::ZipTrrnt, control.as_ref())
                    } else {
                        TrrntZipStatus::CATCH_ERROR
                    }
                } else {
                    tz.process_with_control(&item.path, control.as_ref())
                };

                if let Some(cb) = on_end.as_ref() {
                    cb(thread_id, item.file_id, status);
                }
            }));
        }

        for handle in handles {
            let _ = handle.join();
        }
    }
}

