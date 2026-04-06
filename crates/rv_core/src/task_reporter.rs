use std::cell::RefCell;
use std::sync::mpsc::{Sender, SyncSender};

thread_local! {
    static TASK_REPORTER: RefCell<Option<TaskReporter>> = const { RefCell::new(None) };
}

#[derive(Clone)]
enum TaskReporter {
    Unbounded(Sender<String>),
    Bounded(SyncSender<String>),
}

pub fn set_task_reporter(tx: Sender<String>) {
    TASK_REPORTER.with(|r| {
        *r.borrow_mut() = Some(TaskReporter::Unbounded(tx));
    });
}

pub fn set_task_reporter_bounded(tx: SyncSender<String>) {
    TASK_REPORTER.with(|r| {
        *r.borrow_mut() = Some(TaskReporter::Bounded(tx));
    });
}

pub fn clear_task_reporter() {
    TASK_REPORTER.with(|r| {
        *r.borrow_mut() = None;
    });
}

pub fn task_log(message: impl Into<String>) {
    TASK_REPORTER.with(|r| {
        let binding = r.borrow();
        let Some(tx) = binding.as_ref() else {
            return;
        };
        match tx {
            TaskReporter::Unbounded(tx) => {
                let _ = tx.send(message.into());
            }
            TaskReporter::Bounded(tx) => {
                let _ = tx.try_send(message.into());
            }
        }
    });
}
