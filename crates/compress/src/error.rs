use std::sync::{OnceLock, RwLock};

#[derive(Debug, Clone, Default)]
pub struct ErrorState {
    pub error_message: String,
    pub error_code: i32,
}

static ERROR_STATE: OnceLock<RwLock<ErrorState>> = OnceLock::new();

fn state() -> &'static RwLock<ErrorState> {
    ERROR_STATE.get_or_init(|| RwLock::new(ErrorState::default()))
}

pub fn set_error(error_message: impl Into<String>, error_code: i32) {
    if let Ok(mut s) = state().write() {
        s.error_message = error_message.into();
        s.error_code = error_code;
    }
}

pub fn clear_error() {
    if let Ok(mut s) = state().write() {
        *s = ErrorState::default();
    }
}

pub fn get_error() -> ErrorState {
    state().read().map(|s| s.clone()).unwrap_or_default()
}
