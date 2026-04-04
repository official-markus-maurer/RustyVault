use std::io::Read;
use std::sync::mpsc::{channel, TryRecvError};
use std::thread;

use trrntzip::ProcessControl;

use crate::RomVaultApp;

#[path = "app_sam/worker.rs"]
mod worker;

#[derive(Clone)]
pub(crate) struct SamJobRequest {
    pub(crate) sources: Vec<String>,
    pub(crate) output_directory: String,
    pub(crate) use_origin_output: bool,
    pub(crate) input_kind: crate::dialogs::SamInputKind,
    pub(crate) output_kind: crate::dialogs::SamOutputKind,
    pub(crate) recurse_subdirs: bool,
    pub(crate) rebuild_existing: bool,
    pub(crate) remove_source: bool,
    pub(crate) verify_output: bool,
}

pub(crate) enum SamWorkerEvent {
    Started {
        total_items: usize,
    },
    ItemStarted {
        item: String,
        index: usize,
        total: usize,
    },
    Log(String),
    ItemFinished {
        item: String,
        status: String,
    },
    Finished {
        status: String,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SamSourceKind {
    Directory,
    Zip,
    SevenZip,
}

struct SamInterruptReader<R> {
    inner: R,
    control: ProcessControl,
}

impl<R: Read> Read for SamInterruptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.control.is_hard_stop_requested() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "USER_ABORTED_HARD",
            ));
        }
        self.inner.read(buf)
    }
}

impl RomVaultApp {
    pub(crate) const SAM_7Z_ZSTD_LEVEL: u32 = 19;

    pub(crate) fn sam_output_extension(
        output_kind: crate::dialogs::SamOutputKind,
    ) -> Option<&'static str> {
        match output_kind {
            crate::dialogs::SamOutputKind::TorrentZip
            | crate::dialogs::SamOutputKind::Zip
            | crate::dialogs::SamOutputKind::ZipZstd => Some("zip"),
            crate::dialogs::SamOutputKind::SevenZipLzma
            | crate::dialogs::SamOutputKind::SevenZipZstd => Some("7z"),
        }
    }

    pub(crate) fn sam_output_kind_supported(output_kind: crate::dialogs::SamOutputKind) -> bool {
        let _ = output_kind;
        true
    }

    pub(crate) fn sam_output_kind_support_message(
        output_kind: crate::dialogs::SamOutputKind,
    ) -> Option<&'static str> {
        let _ = output_kind;
        None
    }

    pub(crate) fn sam_has_usable_output_target(&self) -> bool {
        self.sam_use_origin_output || !self.sam_output_directory.trim().is_empty()
    }

    pub(crate) fn start_sam_job(&mut self) {
        if self.sam_running {
            return;
        }
        if !self.sam_has_usable_output_target() {
            self.task_logs.push(
                "SAM requires either an output directory or origin-location output mode."
                    .to_string(),
            );
            return;
        }

        let request = SamJobRequest {
            sources: self.sam_source_items.clone(),
            output_directory: self.sam_output_directory.clone(),
            use_origin_output: self.sam_use_origin_output,
            input_kind: self.sam_input_kind,
            output_kind: self.sam_output_kind,
            recurse_subdirs: self.sam_recurse_subdirs,
            rebuild_existing: self.sam_rebuild_existing,
            remove_source: self.sam_remove_source,
            verify_output: self.sam_verify_output,
        };
        let control = ProcessControl::new();
        let worker_control = control.clone();
        let (tx, rx) = channel();

        self.sam_running = true;
        self.sam_soft_stop_requested = false;
        self.sam_hard_stop_requested = false;
        self.sam_status_text = "Running".to_string();
        self.sam_current_item = None;
        self.sam_completed_items = 0;
        self.sam_total_items = 0;
        self.sam_stop_control = Some(control);
        self.sam_worker_rx = Some(rx);
        self.task_logs.push(format!(
            "Starting SAM with {} queued source path(s).",
            request.sources.len()
        ));

        thread::spawn(move || Self::run_sam_job(request, worker_control, tx));
    }

    pub(crate) fn request_sam_soft_stop(&mut self) {
        if let Some(control) = self.sam_stop_control.as_ref() {
            control.request_soft_stop();
            self.sam_soft_stop_requested = true;
            self.sam_status_text = "Soft stop requested".to_string();
            self.task_logs.push("SAM soft stop requested.".to_string());
        }
    }

    pub(crate) fn request_sam_hard_stop(&mut self) {
        if let Some(control) = self.sam_stop_control.as_ref() {
            control.request_hard_stop();
            self.sam_hard_stop_requested = true;
            self.sam_status_text = "Hard stop requested".to_string();
            self.task_logs.push("SAM hard stop requested.".to_string());
        }
    }

    pub(crate) fn poll_sam_worker(&mut self) {
        let mut finished = false;

        if let Some(rx) = self.sam_worker_rx.as_ref() {
            loop {
                match rx.try_recv() {
                    Ok(SamWorkerEvent::Started { total_items }) => {
                        self.sam_total_items = total_items;
                        self.sam_status_text = format!("Running {} item(s)", total_items);
                    }
                    Ok(SamWorkerEvent::ItemStarted { item, index, total }) => {
                        self.sam_current_item = Some(item.clone());
                        self.sam_status_text = format!("Processing {}/{}", index, total);
                        self.task_logs.push(format!("SAM processing {}", item));
                    }
                    Ok(SamWorkerEvent::Log(message)) => {
                        self.task_logs.push(message);
                    }
                    Ok(SamWorkerEvent::ItemFinished { item, status }) => {
                        self.sam_completed_items += 1;
                        self.task_logs
                            .push(format!("SAM finished {} with {}", item, status));
                    }
                    Ok(SamWorkerEvent::Finished { status }) => {
                        self.sam_status_text = status.clone();
                        self.task_logs.push(status);
                        finished = true;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        finished = true;
                        break;
                    }
                }
            }
        }

        if finished {
            self.sam_running = false;
            self.sam_current_item = None;
            self.sam_stop_control = None;
            self.sam_worker_rx = None;
            self.sam_soft_stop_requested = false;
            self.sam_hard_stop_requested = false;
        }
    }
}
