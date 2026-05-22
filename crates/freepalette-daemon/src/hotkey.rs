use freepalette_core::HotkeyConfig;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotkeyState {
    Disabled,
    ReadyForWindowsMessageLoop(HotkeyBinding),
    UnsupportedPlatform { platform: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotkeyLoopStatus {
    Disabled,
    UnsupportedPlatform { platform: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeyBinding {
    pub key: HotkeyKey,
    pub modifiers: HotkeyModifiers,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotkeyKey {
    Space,
    Character(char),
    Function(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HotkeyModifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

#[derive(Debug, Error)]
pub enum HotkeyError {
    #[error("global hotkey key is empty")]
    EmptyKey,
    #[error("global hotkey key '{0}' is not supported yet")]
    UnsupportedKey(String),
    #[error("global hotkey must include at least one modifier")]
    MissingModifier,
}

#[derive(Debug, Error)]
pub enum HotkeyLoopError {
    #[error("configured global hotkey {binding} cannot be registered: {reason}")]
    InvalidBinding { binding: String, reason: String },
    #[error("failed to initialize global hotkey manager for {binding}: {details}")]
    ManagerInit { binding: String, details: String },
    #[error("failed to register global hotkey {binding}: {details}")]
    Registration { binding: String, details: String },
}

impl HotkeyState {
    pub fn from_config(config: &HotkeyConfig) -> Result<Self, HotkeyError> {
        if !config.enabled {
            return Ok(Self::Disabled);
        }

        let binding = HotkeyBinding::from_config(config)?;

        if cfg!(target_os = "windows") {
            Ok(Self::ReadyForWindowsMessageLoop(binding))
        } else {
            Ok(Self::UnsupportedPlatform {
                platform: std::env::consts::OS.to_string(),
            })
        }
    }

    pub fn summary(&self) -> String {
        match self {
            Self::Disabled => "global hotkey disabled".to_string(),
            Self::ReadyForWindowsMessageLoop(binding) => {
                format!(
                    "global hotkey {} can be registered on Windows",
                    binding.display()
                )
            }
            Self::UnsupportedPlatform { platform } => {
                format!("global hotkey unsupported on {platform}")
            }
        }
    }

    pub fn windows_binding(&self) -> Option<&HotkeyBinding> {
        match self {
            Self::ReadyForWindowsMessageLoop(binding) => Some(binding),
            Self::Disabled | Self::UnsupportedPlatform { .. } => None,
        }
    }
}

pub fn run_hotkey_loop(state: &HotkeyState) -> Result<HotkeyLoopStatus, HotkeyLoopError> {
    match state {
        HotkeyState::Disabled => Ok(HotkeyLoopStatus::Disabled),
        HotkeyState::UnsupportedPlatform { platform } => {
            Ok(HotkeyLoopStatus::UnsupportedPlatform {
                platform: platform.clone(),
            })
        }
        HotkeyState::ReadyForWindowsMessageLoop(binding) => run_platform_hotkey_loop(binding),
    }
}

impl HotkeyBinding {
    fn from_config(config: &HotkeyConfig) -> Result<Self, HotkeyError> {
        let modifiers = HotkeyModifiers {
            ctrl: config.ctrl,
            alt: config.alt,
            shift: config.shift,
            meta: config.meta,
        };
        if !modifiers.has_any() {
            return Err(HotkeyError::MissingModifier);
        }

        Ok(Self {
            key: HotkeyKey::parse(&config.key)?,
            modifiers,
        })
    }

    pub fn display(&self) -> String {
        let mut parts = Vec::new();
        if self.modifiers.ctrl {
            parts.push("Ctrl".to_string());
        }
        if self.modifiers.alt {
            parts.push("Alt".to_string());
        }
        if self.modifiers.shift {
            parts.push("Shift".to_string());
        }
        if self.modifiers.meta {
            parts.push("Meta".to_string());
        }
        parts.push(self.key.display());
        parts.join("+")
    }
}

impl HotkeyKey {
    fn parse(input: &str) -> Result<Self, HotkeyError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(HotkeyError::EmptyKey);
        }

        if trimmed.eq_ignore_ascii_case("space") {
            return Ok(Self::Space);
        }

        if let Some(function_number) = parse_function_key(trimmed) {
            return Ok(Self::Function(function_number));
        }

        let mut chars = trimmed.chars();
        let Some(character) = chars.next() else {
            return Err(HotkeyError::EmptyKey);
        };
        if chars.next().is_none() && character.is_ascii_alphanumeric() {
            return Ok(Self::Character(character.to_ascii_uppercase()));
        }

        Err(HotkeyError::UnsupportedKey(trimmed.to_string()))
    }

    fn display(&self) -> String {
        match self {
            Self::Space => "Space".to_string(),
            Self::Character(character) => character.to_string(),
            Self::Function(number) => format!("F{number}"),
        }
    }
}

impl HotkeyModifiers {
    fn has_any(self) -> bool {
        self.ctrl || self.alt || self.shift || self.meta
    }
}

fn parse_function_key(input: &str) -> Option<u8> {
    let uppercase = input.to_ascii_uppercase();
    let number = uppercase.strip_prefix('F')?.parse::<u8>().ok()?;
    if (1..=24).contains(&number) {
        Some(number)
    } else {
        None
    }
}

#[cfg(windows)]
fn hotkey_code(key: &HotkeyKey) -> Option<global_hotkey::hotkey::Code> {
    use global_hotkey::hotkey::Code;

    match key {
        HotkeyKey::Space => Some(Code::Space),
        HotkeyKey::Character(character) => match character.to_ascii_uppercase() {
            'A' => Some(Code::KeyA),
            'B' => Some(Code::KeyB),
            'C' => Some(Code::KeyC),
            'D' => Some(Code::KeyD),
            'E' => Some(Code::KeyE),
            'F' => Some(Code::KeyF),
            'G' => Some(Code::KeyG),
            'H' => Some(Code::KeyH),
            'I' => Some(Code::KeyI),
            'J' => Some(Code::KeyJ),
            'K' => Some(Code::KeyK),
            'L' => Some(Code::KeyL),
            'M' => Some(Code::KeyM),
            'N' => Some(Code::KeyN),
            'O' => Some(Code::KeyO),
            'P' => Some(Code::KeyP),
            'Q' => Some(Code::KeyQ),
            'R' => Some(Code::KeyR),
            'S' => Some(Code::KeyS),
            'T' => Some(Code::KeyT),
            'U' => Some(Code::KeyU),
            'V' => Some(Code::KeyV),
            'W' => Some(Code::KeyW),
            'X' => Some(Code::KeyX),
            'Y' => Some(Code::KeyY),
            'Z' => Some(Code::KeyZ),
            '0' => Some(Code::Digit0),
            '1' => Some(Code::Digit1),
            '2' => Some(Code::Digit2),
            '3' => Some(Code::Digit3),
            '4' => Some(Code::Digit4),
            '5' => Some(Code::Digit5),
            '6' => Some(Code::Digit6),
            '7' => Some(Code::Digit7),
            '8' => Some(Code::Digit8),
            '9' => Some(Code::Digit9),
            _ => None,
        },
        HotkeyKey::Function(1) => Some(Code::F1),
        HotkeyKey::Function(2) => Some(Code::F2),
        HotkeyKey::Function(3) => Some(Code::F3),
        HotkeyKey::Function(4) => Some(Code::F4),
        HotkeyKey::Function(5) => Some(Code::F5),
        HotkeyKey::Function(6) => Some(Code::F6),
        HotkeyKey::Function(7) => Some(Code::F7),
        HotkeyKey::Function(8) => Some(Code::F8),
        HotkeyKey::Function(9) => Some(Code::F9),
        HotkeyKey::Function(10) => Some(Code::F10),
        HotkeyKey::Function(11) => Some(Code::F11),
        HotkeyKey::Function(12) => Some(Code::F12),
        HotkeyKey::Function(13) => Some(Code::F13),
        HotkeyKey::Function(14) => Some(Code::F14),
        HotkeyKey::Function(15) => Some(Code::F15),
        HotkeyKey::Function(16) => Some(Code::F16),
        HotkeyKey::Function(17) => Some(Code::F17),
        HotkeyKey::Function(18) => Some(Code::F18),
        HotkeyKey::Function(19) => Some(Code::F19),
        HotkeyKey::Function(20) => Some(Code::F20),
        HotkeyKey::Function(21) => Some(Code::F21),
        HotkeyKey::Function(22) => Some(Code::F22),
        HotkeyKey::Function(23) => Some(Code::F23),
        HotkeyKey::Function(24) => Some(Code::F24),
        HotkeyKey::Function(_) => None,
    }
}

#[cfg(windows)]
pub fn windows_global_hotkey(
    binding: &HotkeyBinding,
) -> Result<global_hotkey::hotkey::HotKey, HotkeyLoopError> {
    use global_hotkey::hotkey::{HotKey, Modifiers};

    let binding_label = binding.display();
    if !binding.modifiers.has_any() {
        return Err(HotkeyLoopError::InvalidBinding {
            binding: binding_label,
            reason: "at least one modifier is required".to_string(),
        });
    }

    let mut modifiers = Modifiers::empty();
    if binding.modifiers.ctrl {
        modifiers |= Modifiers::CONTROL;
    }
    if binding.modifiers.alt {
        modifiers |= Modifiers::ALT;
    }
    if binding.modifiers.shift {
        modifiers |= Modifiers::SHIFT;
    }
    if binding.modifiers.meta {
        modifiers |= Modifiers::SUPER;
    }

    let code = hotkey_code(&binding.key).ok_or_else(|| HotkeyLoopError::InvalidBinding {
        binding: binding_label.clone(),
        reason: "key is outside the supported launcher hotkey set".to_string(),
    })?;

    Ok(HotKey::new(Some(modifiers), code))
}

#[cfg(windows)]
fn run_platform_hotkey_loop(binding: &HotkeyBinding) -> Result<HotkeyLoopStatus, HotkeyLoopError> {
    use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState as GlobalHotKeyState};
    use tao::event_loop::{ControlFlow, EventLoop};

    let binding_label = binding.display();
    let event_loop = EventLoop::new();
    let manager = GlobalHotKeyManager::new().map_err(|source| HotkeyLoopError::ManagerInit {
        binding: binding_label.clone(),
        details: source.to_string(),
    })?;
    let hotkey = windows_global_hotkey(binding)?;

    manager
        .register(hotkey)
        .map_err(|source| HotkeyLoopError::Registration {
            binding: binding_label.clone(),
            details: source.to_string(),
        })?;

    let receiver = GlobalHotKeyEvent::receiver();
    tracing::info!(hotkey = %binding_label, "global hotkey registered");
    println!("global hotkey {binding_label} registered; press Ctrl+C to stop the daemon");

    event_loop.run(move |_event, _, control_flow| {
        let _registered_hotkey_manager = &manager;
        *control_flow = ControlFlow::Wait;

        while let Ok(event) = receiver.try_recv() {
            if event.id == hotkey.id() && event.state == GlobalHotKeyState::Pressed {
                tracing::info!(hotkey = %binding_label, "global hotkey pressed");
                println!(
                    "global hotkey {binding_label} pressed; palette activation is not wired yet"
                );
            }
        }
    });
}

#[cfg(not(windows))]
fn run_platform_hotkey_loop(binding: &HotkeyBinding) -> Result<HotkeyLoopStatus, HotkeyLoopError> {
    let _binding = binding;
    Ok(HotkeyLoopStatus::UnsupportedPlatform {
        platform: std::env::consts::OS.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_hotkey_is_disabled() {
        let state = HotkeyState::from_config(&HotkeyConfig::default())
            .expect("default hotkey config should be valid");

        assert_eq!(state, HotkeyState::Disabled);
    }

    #[test]
    fn parses_conservative_hotkey_keys() {
        assert_eq!(
            HotkeyKey::parse("Space").expect("space should parse"),
            HotkeyKey::Space
        );
        assert_eq!(
            HotkeyKey::parse("k").expect("single letter should parse"),
            HotkeyKey::Character('K')
        );
        assert_eq!(
            HotkeyKey::parse("F12").expect("function key should parse"),
            HotkeyKey::Function(12)
        );
    }

    #[test]
    fn rejects_unsupported_hotkey_keys() {
        let error = HotkeyKey::parse("PageDown").expect_err("unsupported key should fail");

        assert!(matches!(error, HotkeyError::UnsupportedKey(_)));
    }

    #[test]
    fn enabled_hotkey_requires_modifier() {
        let error = HotkeyState::from_config(&HotkeyConfig {
            enabled: true,
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
            ..Default::default()
        })
        .expect_err("hotkey without modifiers should fail");

        assert!(matches!(error, HotkeyError::MissingModifier));
    }

    #[test]
    fn ready_hotkey_state_exposes_configured_windows_binding() {
        let state = HotkeyState::ReadyForWindowsMessageLoop(HotkeyBinding {
            key: HotkeyKey::Space,
            modifiers: HotkeyModifiers {
                ctrl: true,
                alt: true,
                shift: false,
                meta: false,
            },
        });

        let binding = state
            .windows_binding()
            .expect("ready hotkey state should expose its binding");

        assert_eq!(binding.display(), "Ctrl+Alt+Space");
    }

    #[test]
    fn disabled_hotkey_loop_exits_without_registering() {
        let status =
            run_hotkey_loop(&HotkeyState::Disabled).expect("disabled hotkey loop should not fail");

        assert_eq!(status, HotkeyLoopStatus::Disabled);
    }

    #[cfg(not(windows))]
    #[test]
    fn enabled_hotkey_loop_reports_unsupported_platform() {
        let state = HotkeyState::ReadyForWindowsMessageLoop(HotkeyBinding {
            key: HotkeyKey::Space,
            modifiers: HotkeyModifiers {
                ctrl: true,
                alt: true,
                shift: false,
                meta: false,
            },
        });

        let status = run_hotkey_loop(&state)
            .expect("unsupported platform should be reported without registering");

        assert_eq!(
            status,
            HotkeyLoopStatus::UnsupportedPlatform {
                platform: std::env::consts::OS.to_string()
            }
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_hotkey_mapping_accepts_supported_binding() {
        use global_hotkey::hotkey::{Code, Modifiers};

        let binding = HotkeyBinding {
            key: HotkeyKey::Character('P'),
            modifiers: HotkeyModifiers {
                ctrl: true,
                alt: true,
                shift: false,
                meta: false,
            },
        };

        let hotkey = windows_global_hotkey(&binding)
            .expect("supported binding should map to a global hotkey");

        assert_eq!(hotkey.key, Code::KeyP);
        assert!(hotkey.mods.contains(Modifiers::CONTROL));
        assert!(hotkey.mods.contains(Modifiers::ALT));
        assert!(!hotkey.mods.contains(Modifiers::SHIFT));
    }

    #[cfg(windows)]
    #[test]
    fn windows_hotkey_mapping_rejects_invalid_public_binding() {
        let binding = HotkeyBinding {
            key: HotkeyKey::Function(25),
            modifiers: HotkeyModifiers {
                ctrl: true,
                alt: false,
                shift: false,
                meta: false,
            },
        };

        let error = windows_global_hotkey(&binding)
            .expect_err("out-of-range function key should fail before registration");

        assert!(matches!(error, HotkeyLoopError::InvalidBinding { .. }));
    }

    #[cfg(windows)]
    #[test]
    fn windows_hotkey_mapping_rejects_modifierless_public_binding() {
        let binding = HotkeyBinding {
            key: HotkeyKey::Character('P'),
            modifiers: HotkeyModifiers {
                ctrl: false,
                alt: false,
                shift: false,
                meta: false,
            },
        };

        let error = windows_global_hotkey(&binding)
            .expect_err("modifierless hotkey should fail before registration");

        assert!(matches!(error, HotkeyLoopError::InvalidBinding { .. }));
    }
}
