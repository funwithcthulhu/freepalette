use std::time::Duration;

use eframe::egui::{self, Color32, Key, RichText, TextEdit};
use freepalette_core::{Action, RankedResult};
use freepalette_ui::{
    app_icon_rgba, PaletteState, SelectionDirection, TrayCommand, UiAutostart, UiHotkeyBridge,
    UiTray, APP_ICON_SIZE,
};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_target(false).init();

    let state = PaletteState::from_default_config()?;
    let hotkey_bridge = UiHotkeyBridge::from_state(state.hotkey_state())?;
    if let Some(label) = hotkey_bridge.label() {
        tracing::info!(hotkey = %label, "UI global hotkey registered");
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([720.0, 420.0])
            .with_min_inner_size([480.0, 280.0])
            .with_icon(egui::IconData {
                rgba: app_icon_rgba(),
                width: APP_ICON_SIZE,
                height: APP_ICON_SIZE,
            })
            .with_title("freepalette"),
        ..Default::default()
    };

    eframe::run_native(
        "freepalette",
        options,
        Box::new(|_| Ok(Box::new(PaletteApp::new(state, hotkey_bridge)))),
    )
    .map_err(|error| anyhow::anyhow!("failed to run freepalette UI: {error}"))?;

    Ok(())
}

struct PaletteApp {
    state: PaletteState,
    hotkey_bridge: UiHotkeyBridge,
    tray: Option<UiTray>,
    query: String,
    focus_search: bool,
    exit_requested: bool,
}

impl PaletteApp {
    fn new(mut state: PaletteState, hotkey_bridge: UiHotkeyBridge) -> Self {
        let query = state.query().to_string();
        let tray = create_tray(&mut state);

        Self {
            state,
            hotkey_bridge,
            tray,
            query,
            focus_search: true,
            exit_requested: false,
        }
    }

    fn handle_keys(&mut self, context: &egui::Context) {
        if context.input(|input| input.key_pressed(Key::Escape)) {
            self.close_or_hide(context);
        }
        if context.input(|input| input.key_pressed(Key::ArrowDown)) {
            self.state.move_selection(SelectionDirection::Next);
        }
        if context.input(|input| input.key_pressed(Key::ArrowUp)) {
            self.state.move_selection(SelectionDirection::Previous);
        }
        if context.input(|input| input.key_pressed(Key::Enter)) {
            let execution = self.state.execute_selected();
            if execution.should_hide_palette() {
                self.close_or_hide(context);
            }
        }
    }

    fn close_or_hide(&mut self, context: &egui::Context) {
        if self.background_lifecycle_active() {
            self.hide_palette(context);
        } else {
            context.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn background_lifecycle_active(&self) -> bool {
        self.hotkey_bridge.is_active()
            || self
                .tray
                .as_ref()
                .map(|tray| tray.is_active())
                .unwrap_or(false)
    }

    fn hide_palette(&mut self, context: &egui::Context) {
        self.state.reset_for_next_activation();
        self.query.clear();
        self.focus_search = true;
        context.send_viewport_cmd(egui::ViewportCommand::Visible(false));
    }

    fn show_palette(&mut self, context: &egui::Context) {
        context.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        context.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        context.send_viewport_cmd(egui::ViewportCommand::Focus);
        self.focus_search = true;
    }

    fn quit(&mut self, context: &egui::Context) {
        self.exit_requested = true;
        context.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn handle_close_request(&mut self, context: &egui::Context) {
        let close_requested = context.input(|input| input.viewport().close_requested());
        if close_requested && !self.exit_requested && self.background_lifecycle_active() {
            context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.hide_palette(context);
        }
    }

    fn handle_hotkey_activation(&mut self, context: &egui::Context) {
        if !self.hotkey_bridge.is_active() {
            return;
        }

        context.request_repaint_after(Duration::from_millis(100));
        if self.hotkey_bridge.take_activation_request() {
            self.show_palette(context);
        }
    }

    fn handle_tray_events(&mut self, context: &egui::Context) {
        if self.tray.is_none() {
            return;
        }

        context.request_repaint_after(Duration::from_millis(100));
        let command = self.tray.as_ref().and_then(UiTray::poll_command);
        if let Some(command) = command {
            self.handle_tray_command(command, context);
        }
    }

    fn handle_tray_command(&mut self, command: TrayCommand, context: &egui::Context) {
        match command {
            TrayCommand::Show => self.show_palette(context),
            TrayCommand::Hide => self.hide_palette(context),
            TrayCommand::ReloadConfig => {
                self.state.reload_config();
                self.query = self.state.query().to_string();
            }
            TrayCommand::EnableAutostart => self.enable_autostart(),
            TrayCommand::DisableAutostart => self.disable_autostart(),
            TrayCommand::Quit => self.quit(context),
        }
    }

    fn enable_autostart(&mut self) {
        match UiAutostart::enable() {
            Ok(shortcut) => {
                self.state
                    .set_status_info(format!("Launch at sign-in enabled: {}", shortcut.display()));
                self.refresh_tray_autostart_menu();
            }
            Err(error) => {
                self.state
                    .set_status_error(format!("Could not enable launch at sign-in: {error}"));
            }
        }
    }

    fn disable_autostart(&mut self) {
        match UiAutostart::disable() {
            Ok(shortcut) => {
                self.state.set_status_info(format!(
                    "Launch at sign-in disabled: {}",
                    shortcut.display()
                ));
                self.refresh_tray_autostart_menu();
            }
            Err(error) => {
                self.state
                    .set_status_error(format!("Could not disable launch at sign-in: {error}"));
            }
        }
    }

    fn refresh_tray_autostart_menu(&self) {
        if let Some(tray) = &self.tray {
            tray.refresh_autostart_menu();
        }
    }

    fn show_search(&mut self, ui: &mut egui::Ui) {
        let response = ui.add(
            TextEdit::singleline(&mut self.query)
                .hint_text("Search")
                .desired_width(f32::INFINITY),
        );

        if self.focus_search {
            response.request_focus();
            self.focus_search = false;
        }

        if response.changed() {
            self.state.set_query(self.query.clone());
        }
    }

    fn show_results(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (index, ranked) in self.state.results().iter().enumerate() {
                    let selected = self.state.selected_index() == Some(index);
                    show_result_row(ui, ranked, selected);
                    ui.separator();
                }
            });
    }

    fn show_status(&self, ui: &mut egui::Ui) {
        let Some(message) = self.state.status().message() else {
            return;
        };

        let color = if self.state.status().is_error() {
            Color32::from_rgb(220, 88, 88)
        } else {
            ui.visuals().weak_text_color()
        };

        ui.label(RichText::new(message).color(color));
    }
}

impl eframe::App for PaletteApp {
    fn logic(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_close_request(context);
        self.handle_hotkey_activation(context);
        self.handle_tray_events(context);
        self.handle_keys(context);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.vertical_centered_justified(|ui| {
                ui.add_space(8.0);
                self.show_search(ui);
                ui.add_space(8.0);
            });

            self.show_results(ui);

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                self.show_status(ui);
            });
        });
    }
}

#[cfg(windows)]
fn create_tray(state: &mut PaletteState) -> Option<UiTray> {
    match UiTray::new() {
        Ok(tray) => Some(tray),
        Err(error) => {
            let message = format!("Tray unavailable: {error}");
            tracing::warn!("{message}");
            state.set_status_error(message);
            None
        }
    }
}

#[cfg(not(windows))]
fn create_tray(_state: &mut PaletteState) -> Option<UiTray> {
    None
}

fn show_result_row(ui: &mut egui::Ui, ranked: &RankedResult, selected: bool) {
    let result = &ranked.result;
    let text_color = if selected {
        ui.visuals().selection.stroke.color
    } else {
        ui.visuals().strong_text_color()
    };

    let fill = if selected {
        ui.visuals().selection.bg_fill
    } else {
        Color32::TRANSPARENT
    };

    egui::Frame::NONE.fill(fill).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(&result.title).color(text_color).strong());
                if let Some(subtitle) = &result.subtitle {
                    ui.label(RichText::new(subtitle).color(ui.visuals().weak_text_color()));
                }
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(describe_action(&result.action)).small());
            });
        });
    });
}

fn describe_action(action: &Action) -> String {
    match action {
        Action::LaunchApp { command, args } if args.is_empty() => {
            format!("launch {command}")
        }
        Action::LaunchApp { command, args } => format!("launch {command} {}", args.join(" ")),
        Action::OpenPath { path } => format!("open {path}"),
        Action::RunShell { .. } => "shell command blocked".to_string(),
        Action::CopyText { .. } => "copy text".to_string(),
        Action::Noop { message } => message.clone(),
    }
}
