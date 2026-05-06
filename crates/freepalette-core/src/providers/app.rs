use std::process::Command;

use freepalette_plugin_api::{
    Action, ActionOutcome, PluginError, Provider, ProviderId, ResultKind, SearchContext,
    SearchResult,
};

use crate::config::{AppEntry, Config};

pub struct AppLauncherProvider {
    apps: Vec<AppEntry>,
}

impl AppLauncherProvider {
    pub fn from_config(config: &Config) -> Self {
        let mut apps = config.apps.clone();
        add_stub_defaults(&mut apps);
        Self { apps }
    }
}

impl Provider for AppLauncherProvider {
    fn id(&self) -> ProviderId {
        ProviderId::from("apps")
    }

    fn search(&self, _context: &SearchContext) -> Result<Vec<SearchResult>, PluginError> {
        Ok(self.apps.iter().map(app_result).collect())
    }

    fn execute(&self, action: &Action) -> Result<ActionOutcome, PluginError> {
        match action {
            Action::LaunchApp { command, args } => {
                Command::new(command)
                    .args(args)
                    .spawn()
                    .map_err(|source| PluginError::Action(source.to_string()))?;

                Ok(ActionOutcome::new(format!("launched {command}")))
            }
            _ => Err(PluginError::UnsupportedAction),
        }
    }
}

fn app_result(app: &AppEntry) -> SearchResult {
    let subtitle = if app.args.is_empty() {
        app.command.clone()
    } else {
        format!("{} {}", app.command, app.args.join(" "))
    };

    SearchResult::new(
        ProviderId::from("apps"),
        stable_app_id(&app.name),
        app.name.clone(),
        ResultKind::App,
        Action::LaunchApp {
            command: app.command.clone(),
            args: app.args.clone(),
        },
    )
    .with_subtitle(subtitle)
    .with_keywords(app.keywords.clone())
}

fn stable_app_id(name: &str) -> String {
    let mut id = name
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();

    while id.contains("--") {
        id = id.replace("--", "-");
    }

    id.trim_matches('-').to_string()
}

fn add_stub_defaults(apps: &mut Vec<AppEntry>) {
    for app in stub_default_apps() {
        if !apps
            .iter()
            .any(|existing| existing.name.eq_ignore_ascii_case(&app.name))
        {
            apps.push(app);
        }
    }
}

fn stub_default_apps() -> Vec<AppEntry> {
    let mut notepad = AppEntry::new("Notepad", "notepad.exe");
    notepad.keywords = vec![
        "editor".to_string(),
        "text".to_string(),
        "sample".to_string(),
    ];
    vec![notepad]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_result_contains_keywords_for_fuzzy_search() {
        let mut app = AppEntry::new("Plain Text", "editor");
        app.keywords = vec!["notepad".to_string()];

        let result = app_result(&app);

        assert_eq!(result.provider.as_str(), "apps");
        assert_eq!(result.title, "Plain Text");
        assert_eq!(result.keywords, ["notepad"]);
    }

    #[test]
    fn default_stub_contains_notepad() {
        let provider = AppLauncherProvider::from_config(&Config::default());
        let results = provider
            .search(&SearchContext::new("notepad", 10))
            .expect("app provider search should succeed");

        assert!(results.iter().any(|result| result.title == "Notepad"));
    }
}
