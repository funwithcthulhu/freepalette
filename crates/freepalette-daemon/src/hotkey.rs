use freepalette_core::HotkeyConfig;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotkeyState {
    Disabled,
    ReadyForWindowsMessageLoop(HotkeyBinding),
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
                    "global hotkey {} is ready for a Windows message loop",
                    binding.display()
                )
            }
            Self::UnsupportedPlatform { platform } => {
                format!("global hotkey unsupported on {platform}")
            }
        }
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

    fn display(&self) -> String {
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
}
