use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[cfg(windows)]
use freepalette_daemon::windows_global_hotkey;
use freepalette_daemon::{HotkeyBinding, HotkeyLoopError, HotkeyState};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UiHotkeyError {
    #[error(transparent)]
    Hotkey(#[from] HotkeyLoopError),
}

pub struct UiHotkeyBridge {
    label: Option<String>,
    activation_requested: Arc<AtomicBool>,
    #[cfg(windows)]
    _registration: Option<WindowsHotkeyRegistration>,
}

impl UiHotkeyBridge {
    pub fn from_state(state: &HotkeyState) -> Result<Self, UiHotkeyError> {
        let Some(binding) = state.windows_binding() else {
            return Ok(Self::disabled());
        };

        register_hotkey(binding)
    }

    pub fn disabled() -> Self {
        Self {
            label: None,
            activation_requested: Arc::new(AtomicBool::new(false)),
            #[cfg(windows)]
            _registration: None,
        }
    }

    pub fn is_active(&self) -> bool {
        self.label.is_some()
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn take_activation_request(&self) -> bool {
        self.activation_requested.swap(false, Ordering::SeqCst)
    }
}

#[cfg(windows)]
struct WindowsHotkeyRegistration {
    stop_requested: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    _manager: global_hotkey::GlobalHotKeyManager,
    _hotkey: global_hotkey::hotkey::HotKey,
}

#[cfg(windows)]
impl Drop for WindowsHotkeyRegistration {
    fn drop(&mut self) {
        self.stop_requested.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(windows)]
fn register_hotkey(binding: &HotkeyBinding) -> Result<UiHotkeyBridge, UiHotkeyError> {
    use std::time::Duration;

    use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

    let binding_label = binding.display();
    let manager = GlobalHotKeyManager::new().map_err(|source| HotkeyLoopError::ManagerInit {
        binding: binding_label.clone(),
        details: source.to_string(),
    })?;
    let hotkey = windows_global_hotkey(binding)?;
    let hotkey_id = hotkey.id();

    manager
        .register(hotkey)
        .map_err(|source| HotkeyLoopError::Registration {
            binding: binding_label.clone(),
            details: source.to_string(),
        })?;

    let activation_requested = Arc::new(AtomicBool::new(false));
    let thread_activation_requested = Arc::clone(&activation_requested);
    let stop_requested = Arc::new(AtomicBool::new(false));
    let thread_stop_requested = Arc::clone(&stop_requested);
    let receiver = GlobalHotKeyEvent::receiver();

    let thread = std::thread::spawn(move || {
        while !thread_stop_requested.load(Ordering::SeqCst) {
            while let Ok(event) = receiver.try_recv() {
                if event.id == hotkey_id && event.state == HotKeyState::Pressed {
                    thread_activation_requested.store(true, Ordering::SeqCst);
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    });

    Ok(UiHotkeyBridge {
        label: Some(binding_label),
        activation_requested,
        _registration: Some(WindowsHotkeyRegistration {
            stop_requested,
            thread: Some(thread),
            _manager: manager,
            _hotkey: hotkey,
        }),
    })
}

#[cfg(not(windows))]
fn register_hotkey(binding: &HotkeyBinding) -> Result<UiHotkeyBridge, UiHotkeyError> {
    let _binding = binding;
    Ok(UiHotkeyBridge::disabled())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_bridge_does_not_request_activation() {
        let bridge = UiHotkeyBridge::disabled();

        assert!(!bridge.is_active());
        assert_eq!(bridge.label(), None);
        assert!(!bridge.take_activation_request());
    }

    #[test]
    fn bridge_is_disabled_when_hotkey_state_is_disabled() {
        let bridge = UiHotkeyBridge::from_state(&HotkeyState::Disabled)
            .expect("disabled hotkey state should be accepted");

        assert!(!bridge.is_active());
    }
}
