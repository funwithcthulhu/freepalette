use eframe::egui::{self, Color32, Key, RichText, TextEdit};
use freepalette_core::{Action, RankedResult};
use freepalette_ui::{PaletteState, SelectionDirection};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_target(false).init();

    let state = PaletteState::from_default_config()?;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([720.0, 420.0])
            .with_min_inner_size([480.0, 280.0])
            .with_title("freepalette"),
        ..Default::default()
    };

    eframe::run_native(
        "freepalette",
        options,
        Box::new(|_| Ok(Box::new(PaletteApp::new(state)))),
    )
    .map_err(|error| anyhow::anyhow!("failed to run freepalette UI: {error}"))?;

    Ok(())
}

struct PaletteApp {
    state: PaletteState,
    query: String,
    focus_search: bool,
}

impl PaletteApp {
    fn new(state: PaletteState) -> Self {
        let query = state.query().to_string();

        Self {
            state,
            query,
            focus_search: true,
        }
    }

    fn handle_keys(&mut self, context: &egui::Context) {
        if context.input(|input| input.key_pressed(Key::Escape)) {
            context.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if context.input(|input| input.key_pressed(Key::ArrowDown)) {
            self.state.move_selection(SelectionDirection::Next);
        }
        if context.input(|input| input.key_pressed(Key::ArrowUp)) {
            self.state.move_selection(SelectionDirection::Previous);
        }
        if context.input(|input| input.key_pressed(Key::Enter)) {
            self.state.execute_selected();
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
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_keys(context);

        egui::CentralPanel::default().show(context, |ui| {
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
        Action::RunShell { command } => format!("shell {command}"),
        Action::CopyText { text } => format!("copy {text}"),
        Action::Noop { message } => message.clone(),
    }
}
