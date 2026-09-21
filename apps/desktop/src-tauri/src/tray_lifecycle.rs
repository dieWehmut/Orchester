//! Process exit is a tray action. Closing the window only hides its view.

use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
pub struct ExitGate {
    requested_from_tray: AtomicBool,
}

impl ExitGate {
    pub fn request_tray_exit(&self) {
        self.requested_from_tray.store(true, Ordering::SeqCst);
    }

    pub fn on_exit_requested(&self, prevent: impl FnOnce()) {
        if !self.requested_from_tray.load(Ordering::SeqCst) {
            prevent();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_and_application_quit_requests_are_prevented_until_tray_quit() {
        let gate = ExitGate::default();
        let mut prevented = 0;
        gate.on_exit_requested(|| prevented += 1);
        gate.on_exit_requested(|| prevented += 1);
        assert_eq!(prevented, 2);
        gate.request_tray_exit();
        gate.on_exit_requested(|| prevented += 1);
        assert_eq!(prevented, 2);
    }
}
