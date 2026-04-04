use std::cell::RefCell;
use std::sync::mpsc::Sender;

thread_local! {
    static TASK_REPORTER: RefCell<Option<Sender<String>>> = const { RefCell::new(None) };
}

pub fn set_task_reporter(tx: Sender<String>) {
    TASK_REPORTER.with(|r| {
        *r.borrow_mut() = Some(tx);
    });
}

pub fn clear_task_reporter() {
    TASK_REPORTER.with(|r| {
        *r.borrow_mut() = None;
    });
}

pub fn task_log(message: impl Into<String>) {
    TASK_REPORTER.with(|r| {
        if let Some(tx) = r.borrow().as_ref() {
            let _ = tx.send(message.into());
        }
    });
}

