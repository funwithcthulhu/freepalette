use thiserror::Error;

#[derive(Debug, Error)]
pub enum UiTrayError {
    #[error("tray integration is only implemented on Windows")]
    UnsupportedPlatform,
    #[cfg(windows)]
    #[error("failed to create tray icon image: {0}")]
    Icon(#[from] tray_icon::BadIcon),
    #[cfg(windows)]
    #[error("failed to create tray menu: {0}")]
    Menu(#[from] tray_icon::menu::Error),
    #[cfg(windows)]
    #[error("failed to create tray icon: {0}")]
    Tray(#[from] tray_icon::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    Show,
    Hide,
    ReloadConfig,
    EnableAutostart,
    DisableAutostart,
    Quit,
}

#[cfg(windows)]
pub struct UiTray {
    _menu: tray_icon::menu::Menu,
    _tray_icon: tray_icon::TrayIcon,
    tray_id: tray_icon::TrayIconId,
    show_id: tray_icon::menu::MenuId,
    hide_id: tray_icon::menu::MenuId,
    reload_id: tray_icon::menu::MenuId,
    enable_autostart_id: tray_icon::menu::MenuId,
    disable_autostart_id: tray_icon::menu::MenuId,
    quit_id: tray_icon::menu::MenuId,
    enable_autostart_item: tray_icon::menu::MenuItem,
    disable_autostart_item: tray_icon::menu::MenuItem,
}

#[cfg(windows)]
impl UiTray {
    pub fn new() -> Result<Self, UiTrayError> {
        use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};

        let show_item = MenuItem::new("Show freepalette", true, None);
        let hide_item = MenuItem::new("Hide freepalette", true, None);
        let reload_item = MenuItem::new("Reload config", true, None);
        let enable_autostart_item = MenuItem::new("Enable launch at sign-in", true, None);
        let disable_autostart_item = MenuItem::new("Disable launch at sign-in", false, None);
        let quit_item = MenuItem::new("Quit freepalette", true, None);
        let first_separator = PredefinedMenuItem::separator();
        let second_separator = PredefinedMenuItem::separator();
        let third_separator = PredefinedMenuItem::separator();
        let menu = Menu::with_items(&[
            &show_item,
            &hide_item,
            &first_separator,
            &reload_item,
            &second_separator,
            &enable_autostart_item,
            &disable_autostart_item,
            &third_separator,
            &quit_item,
        ])?;
        let icon = tray_icon::Icon::from_rgba(
            crate::app_icon_rgba(),
            crate::APP_ICON_SIZE,
            crate::APP_ICON_SIZE,
        )?;
        let tray_icon = tray_icon::TrayIconBuilder::new()
            .with_menu(Box::new(menu.clone()))
            .with_menu_on_left_click(false)
            .with_menu_on_right_click(true)
            .with_tooltip("freepalette")
            .with_icon(icon)
            .build()?;
        let tray_id = tray_icon.id().clone();

        let tray = Self {
            _menu: menu,
            _tray_icon: tray_icon,
            tray_id,
            show_id: show_item.id().clone(),
            hide_id: hide_item.id().clone(),
            reload_id: reload_item.id().clone(),
            enable_autostart_id: enable_autostart_item.id().clone(),
            disable_autostart_id: disable_autostart_item.id().clone(),
            quit_id: quit_item.id().clone(),
            enable_autostart_item,
            disable_autostart_item,
        };
        tray.refresh_autostart_menu();
        Ok(tray)
    }

    pub fn is_active(&self) -> bool {
        true
    }

    pub fn poll_command(&self) -> Option<TrayCommand> {
        let mut command = None;
        while let Ok(event) = tray_icon::TrayIconEvent::receiver().try_recv() {
            if event.id() == &self.tray_id && is_activation_click(&event) {
                command = Some(TrayCommand::Show);
            }
        }

        while let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
            if let Some(next) = self.command_for_menu_id(event.id()) {
                command = Some(next);
            }
        }

        command
    }

    pub fn refresh_autostart_menu(&self) {
        match crate::UiAutostart::status() {
            Ok(crate::UiAutostartStatus::Enabled { .. }) => {
                self.enable_autostart_item.set_enabled(false);
                self.disable_autostart_item.set_enabled(true);
            }
            Ok(crate::UiAutostartStatus::Disabled { .. }) => {
                self.enable_autostart_item.set_enabled(true);
                self.disable_autostart_item.set_enabled(false);
            }
            Ok(crate::UiAutostartStatus::Unsupported) | Err(_) => {
                self.enable_autostart_item.set_enabled(false);
                self.disable_autostart_item.set_enabled(false);
            }
        }
    }

    fn command_for_menu_id(&self, id: &tray_icon::menu::MenuId) -> Option<TrayCommand> {
        if id == &self.show_id {
            Some(TrayCommand::Show)
        } else if id == &self.hide_id {
            Some(TrayCommand::Hide)
        } else if id == &self.reload_id {
            Some(TrayCommand::ReloadConfig)
        } else if id == &self.enable_autostart_id {
            Some(TrayCommand::EnableAutostart)
        } else if id == &self.disable_autostart_id {
            Some(TrayCommand::DisableAutostart)
        } else if id == &self.quit_id {
            Some(TrayCommand::Quit)
        } else {
            None
        }
    }
}

#[cfg(windows)]
fn is_activation_click(event: &tray_icon::TrayIconEvent) -> bool {
    matches!(
        event,
        tray_icon::TrayIconEvent::Click {
            button: tray_icon::MouseButton::Left,
            ..
        } | tray_icon::TrayIconEvent::DoubleClick {
            button: tray_icon::MouseButton::Left,
            ..
        }
    )
}

#[cfg(not(windows))]
pub struct UiTray;

#[cfg(not(windows))]
impl UiTray {
    pub fn new() -> Result<Self, UiTrayError> {
        Err(UiTrayError::UnsupportedPlatform)
    }

    pub fn is_active(&self) -> bool {
        false
    }

    pub fn poll_command(&self) -> Option<TrayCommand> {
        None
    }

    pub fn refresh_autostart_menu(&self) {}
}
