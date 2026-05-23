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
        let icon = tray_icon::Icon::from_rgba(freepalette_icon_rgba(), ICON_SIZE, ICON_SIZE)?;
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

#[cfg(windows)]
const ICON_SIZE: u32 = 32;

#[cfg(windows)]
fn freepalette_icon_rgba() -> Vec<u8> {
    let mut pixels = vec![0_u8; (ICON_SIZE * ICON_SIZE * 4) as usize];

    draw_ellipse(&mut pixels, 14, 16, 12, 10, [78, 54, 38, 255]);
    draw_ellipse(&mut pixels, 14, 16, 10, 8, [224, 174, 96, 255]);
    draw_circle(&mut pixels, 10, 13, 3, [245, 235, 211, 255]);
    draw_circle(&mut pixels, 15, 10, 2, [210, 73, 65, 255]);
    draw_circle(&mut pixels, 20, 13, 2, [61, 139, 91, 255]);
    draw_circle(&mut pixels, 17, 20, 2, [57, 101, 188, 255]);
    draw_circle(&mut pixels, 9, 19, 2, [236, 206, 82, 255]);

    draw_brush(&mut pixels);
    pixels
}

#[cfg(windows)]
fn draw_brush(pixels: &mut [u8]) {
    for step in 0..11 {
        let x = 18 + step;
        let y = 22 + step / 2;
        draw_circle(pixels, x, y, 1, [112, 72, 42, 255]);
        draw_circle(pixels, x + 1, y + 1, 1, [112, 72, 42, 255]);
    }
    draw_circle(pixels, 18, 22, 2, [42, 45, 52, 255]);
    draw_circle(pixels, 17, 21, 1, [238, 238, 230, 255]);
}

#[cfg(windows)]
fn draw_ellipse(
    pixels: &mut [u8],
    center_x: i32,
    center_y: i32,
    radius_x: i32,
    radius_y: i32,
    color: [u8; 4],
) {
    let radius_x_sq = radius_x * radius_x;
    let radius_y_sq = radius_y * radius_y;
    let limit = radius_x_sq * radius_y_sq;

    for y in (center_y - radius_y)..=(center_y + radius_y) {
        for x in (center_x - radius_x)..=(center_x + radius_x) {
            let dx = x - center_x;
            let dy = y - center_y;
            if dx * dx * radius_y_sq + dy * dy * radius_x_sq <= limit {
                set_pixel(pixels, x, y, color);
            }
        }
    }
}

#[cfg(windows)]
fn draw_circle(pixels: &mut [u8], center_x: i32, center_y: i32, radius: i32, color: [u8; 4]) {
    let radius_sq = radius * radius;
    for y in (center_y - radius)..=(center_y + radius) {
        for x in (center_x - radius)..=(center_x + radius) {
            let dx = x - center_x;
            let dy = y - center_y;
            if dx * dx + dy * dy <= radius_sq {
                set_pixel(pixels, x, y, color);
            }
        }
    }
}

#[cfg(windows)]
fn set_pixel(pixels: &mut [u8], x: i32, y: i32, color: [u8; 4]) {
    if x < 0 || y < 0 || x >= ICON_SIZE as i32 || y >= ICON_SIZE as i32 {
        return;
    }

    let offset = ((y as u32 * ICON_SIZE + x as u32) * 4) as usize;
    pixels[offset..offset + 4].copy_from_slice(&color);
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    #[cfg(windows)]
    fn generated_tray_icon_has_expected_rgba_shape() {
        let pixels = freepalette_icon_rgba();

        assert_eq!(pixels.len(), (ICON_SIZE * ICON_SIZE * 4) as usize);
        assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
    }
}
