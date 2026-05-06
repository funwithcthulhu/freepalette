use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use freepalette_plugin_api::{
    Action, ActionOutcome, PluginError, Provider, ProviderId, ResultKind, SearchContext,
    SearchResult,
};
use thiserror::Error;
use tracing::debug;

use crate::config::{AppEntry, Config};

const PROVIDER_ID: &str = "apps";
#[cfg(target_os = "windows")]
const START_MENU_PROGRAMS: &str = r"Microsoft\Windows\Start Menu\Programs";

pub struct AppLauncherProvider {
    apps: Vec<IndexedApp>,
    index_status: AppIndexStatus,
}

impl AppLauncherProvider {
    pub fn from_config(config: &Config) -> Self {
        Self::from_config_sources(config, discover_platform_apps(), platform_known_apps())
    }

    #[cfg(test)]
    fn from_config_and_index_result(
        config: &Config,
        index_result: Result<AppIndexOutcome, AppIndexError>,
    ) -> Self {
        Self::from_config_sources(config, index_result, Vec::new())
    }

    fn from_config_sources(
        config: &Config,
        index_result: Result<AppIndexOutcome, AppIndexError>,
        known_apps: Vec<IndexedApp>,
    ) -> Self {
        let mut apps = config
            .apps
            .iter()
            .cloned()
            .map(IndexedApp::configured)
            .collect::<Vec<_>>();
        let mut seen = seen_app_names(&apps);
        add_known_apps(&mut apps, &mut seen, known_apps);

        let index_status = match index_result {
            Ok(outcome) if outcome.entries.is_empty() => {
                let reason = format!(
                    "Windows app indexing found no app entries in {} Start Menu root(s)",
                    outcome.roots_checked
                );
                add_fallback_if_needed(&mut apps, &mut seen, &reason);
                AppIndexStatus::Empty {
                    roots_checked: outcome.roots_checked,
                }
            }
            Ok(outcome) => {
                let discovered = outcome.entries.len();
                for app in outcome.entries {
                    push_unique_app(&mut apps, &mut seen, app);
                }
                AppIndexStatus::Indexed {
                    roots_checked: outcome.roots_checked,
                    discovered,
                }
            }
            Err(error) => {
                let reason = error.to_string();
                add_fallback_if_needed(&mut apps, &mut seen, &reason);
                AppIndexStatus::Unavailable { reason }
            }
        };

        Self { apps, index_status }
    }

    pub fn index_status_summary(&self) -> String {
        self.index_status.summary()
    }
}

impl Provider for AppLauncherProvider {
    fn id(&self) -> ProviderId {
        ProviderId::from(PROVIDER_ID)
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct IndexedApp {
    entry: AppEntry,
    source: AppSource,
}

impl IndexedApp {
    fn configured(entry: AppEntry) -> Self {
        Self {
            entry,
            source: AppSource::Config,
        }
    }

    fn discovered(entry: AppEntry, path: PathBuf) -> Self {
        Self {
            entry,
            source: AppSource::WindowsStartMenu { path },
        }
    }

    fn fallback(reason: &str) -> Self {
        let mut entry = AppEntry::new("Notepad", "notepad.exe");
        entry.keywords = vec![
            "editor".to_string(),
            "text".to_string(),
            "sample".to_string(),
            "fallback".to_string(),
        ];

        Self {
            entry,
            source: AppSource::Fallback {
                reason: reason.to_string(),
            },
        }
    }

    #[cfg(any(test, target_os = "windows"))]
    fn known(entry: AppEntry, label: &str) -> Self {
        Self {
            entry,
            source: AppSource::Known {
                label: label.to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AppSource {
    Config,
    #[cfg(any(test, target_os = "windows"))]
    Known {
        label: String,
    },
    WindowsStartMenu {
        path: PathBuf,
    },
    Fallback {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AppIndexStatus {
    Indexed {
        roots_checked: usize,
        discovered: usize,
    },
    Empty {
        roots_checked: usize,
    },
    Unavailable {
        reason: String,
    },
}

impl AppIndexStatus {
    fn summary(&self) -> String {
        match self {
            Self::Indexed {
                roots_checked,
                discovered,
            } => format!(
                "indexed {discovered} app(s) from {roots_checked} Windows Start Menu root(s)"
            ),
            Self::Empty { roots_checked } => {
                format!("indexed 0 app(s) from {roots_checked} Windows Start Menu root(s); using fallback if no configured apps exist")
            }
            Self::Unavailable { reason } => {
                format!("app indexing unavailable: {reason}; using fallback if no configured apps exist")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AppIndexOutcome {
    entries: Vec<IndexedApp>,
    roots_checked: usize,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
enum AppIndexError {
    #[error("{0}")]
    Unavailable(String),
}

fn app_result(app: &IndexedApp) -> SearchResult {
    let subtitle = match &app.source {
        AppSource::Config => command_summary(&app.entry),
        #[cfg(any(test, target_os = "windows"))]
        AppSource::Known { label } => format!("{label}: {}", command_summary(&app.entry)),
        AppSource::WindowsStartMenu { path } => format!("Windows Start Menu: {}", path.display()),
        AppSource::Fallback { reason } => format!("Fallback sample: {reason}"),
    };

    SearchResult::new(
        ProviderId::from(PROVIDER_ID),
        stable_app_id(&app.entry.name),
        app.entry.name.clone(),
        ResultKind::App,
        Action::LaunchApp {
            command: app.entry.command.clone(),
            args: app.entry.args.clone(),
        },
    )
    .with_subtitle(subtitle)
    .with_keywords(app.entry.keywords.clone())
}

fn command_summary(entry: &AppEntry) -> String {
    if entry.args.is_empty() {
        entry.command.clone()
    } else {
        format!("{} {}", entry.command, entry.args.join(" "))
    }
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

fn seen_app_names(apps: &[IndexedApp]) -> HashSet<String> {
    apps.iter()
        .map(|app| app.entry.name.to_ascii_lowercase())
        .collect()
}

fn push_unique_app(apps: &mut Vec<IndexedApp>, seen: &mut HashSet<String>, app: IndexedApp) {
    if seen.insert(app.entry.name.to_ascii_lowercase()) {
        apps.push(app);
    }
}

fn add_fallback_if_needed(apps: &mut Vec<IndexedApp>, seen: &mut HashSet<String>, reason: &str) {
    if apps.is_empty() {
        push_unique_app(apps, seen, IndexedApp::fallback(reason));
    }
}

fn add_known_apps(
    apps: &mut Vec<IndexedApp>,
    seen: &mut HashSet<String>,
    known_apps: Vec<IndexedApp>,
) {
    for app in known_apps {
        push_unique_app(apps, seen, app);
    }
}

#[cfg(target_os = "windows")]
fn platform_known_apps() -> Vec<IndexedApp> {
    let mut notepad = AppEntry::new("Notepad", "notepad.exe");
    notepad.keywords = vec![
        "editor".to_string(),
        "text".to_string(),
        "windows".to_string(),
        "built-in".to_string(),
    ];
    vec![IndexedApp::known(notepad, "Windows built-in app")]
}

#[cfg(not(target_os = "windows"))]
fn platform_known_apps() -> Vec<IndexedApp> {
    Vec::new()
}

fn discover_platform_apps() -> Result<AppIndexOutcome, AppIndexError> {
    let roots = platform_start_menu_roots()?;
    Ok(discover_from_roots(&roots))
}

#[cfg(target_os = "windows")]
fn platform_start_menu_roots() -> Result<Vec<PathBuf>, AppIndexError> {
    let mut roots = Vec::new();

    if let Some(appdata) = std::env::var_os("APPDATA") {
        roots.push(PathBuf::from(appdata).join(START_MENU_PROGRAMS));
    }
    if let Some(program_data) = std::env::var_os("ProgramData") {
        roots.push(PathBuf::from(program_data).join(START_MENU_PROGRAMS));
    }

    if roots.is_empty() {
        Err(AppIndexError::Unavailable(
            "APPDATA and ProgramData are not set".to_string(),
        ))
    } else {
        Ok(roots)
    }
}

#[cfg(not(target_os = "windows"))]
fn platform_start_menu_roots() -> Result<Vec<PathBuf>, AppIndexError> {
    Err(AppIndexError::Unavailable(
        "Windows Start Menu indexing is only available on Windows".to_string(),
    ))
}

fn discover_from_roots(roots: &[PathBuf]) -> AppIndexOutcome {
    let mut entries = Vec::new();

    for root in roots {
        scan_start_menu_root(root, &mut entries);
    }

    entries.sort_by(|left, right| {
        left.entry
            .name
            .to_ascii_lowercase()
            .cmp(&right.entry.name.to_ascii_lowercase())
    });
    entries.dedup_by(|left, right| left.entry.name.eq_ignore_ascii_case(&right.entry.name));

    AppIndexOutcome {
        entries,
        roots_checked: roots.len(),
    }
}

fn scan_start_menu_root(root: &Path, entries: &mut Vec<IndexedApp>) {
    let mut pending = vec![root.to_path_buf()];

    while let Some(directory) = pending.pop() {
        let Ok(children) = fs::read_dir(&directory) else {
            debug!(path = %directory.display(), "skipping unreadable app index directory");
            continue;
        };

        for child in children {
            let Ok(child) = child else {
                continue;
            };
            let path = child.path();
            let Ok(file_type) = child.file_type() else {
                continue;
            };

            if file_type.is_dir() {
                pending.push(path);
            } else if file_type.is_file() {
                if let Some(app) = app_from_start_menu_file(&path) {
                    entries.push(app);
                }
            }
        }
    }
}

fn app_from_start_menu_file(path: &Path) -> Option<IndexedApp> {
    let extension = normalized_extension(path)?;
    if !matches!(extension.as_str(), "exe" | "lnk" | "appref-ms") {
        return None;
    }

    let name = app_name_from_path(path)?;
    let (command, args) = launch_command_for_path(path, &extension);
    let keywords = keywords_for_discovered_app(path, &extension);
    let entry = AppEntry {
        name,
        command,
        args,
        keywords,
    };

    Some(IndexedApp::discovered(entry, path.to_path_buf()))
}

fn normalized_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
}

fn app_name_from_path(path: &Path) -> Option<String> {
    let name = path.file_stem()?.to_str()?.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn launch_command_for_path(path: &Path, extension: &str) -> (String, Vec<String>) {
    let path = path.to_string_lossy().into_owned();
    if extension == "exe" {
        (path, Vec::new())
    } else {
        ("explorer.exe".to_string(), vec![path])
    }
}

fn keywords_for_discovered_app(path: &Path, extension: &str) -> Vec<String> {
    let mut keywords = vec![
        "app".to_string(),
        "launcher".to_string(),
        "windows".to_string(),
        extension.to_string(),
    ];

    keywords.extend(
        path.parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .into_iter()
            .map(ToString::to_string),
    );

    keywords
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("freepalette-{name}-{unique}"))
    }

    #[test]
    fn app_result_contains_keywords_for_fuzzy_search() {
        let mut entry = AppEntry::new("Plain Text", "editor");
        entry.keywords = vec!["notepad".to_string()];
        let app = IndexedApp::configured(entry);

        let result = app_result(&app);

        assert_eq!(result.provider.as_str(), "apps");
        assert_eq!(result.title, "Plain Text");
        assert_eq!(result.keywords, ["notepad"]);
    }

    #[test]
    fn discovers_supported_start_menu_entries() {
        let root = temp_root("apps");
        let tools = root.join("Tools");
        fs::create_dir_all(&tools).expect("test start menu directory should be created");
        fs::write(tools.join("Example App.lnk"), "").expect("test shortcut should be written");
        fs::write(tools.join("Helper.exe"), "").expect("test exe should be written");
        fs::write(tools.join("ignore.txt"), "").expect("test text file should be written");

        let outcome = discover_from_roots(std::slice::from_ref(&root));
        fs::remove_dir_all(&root).expect("test start menu directory should be removed");

        let names = outcome
            .entries
            .iter()
            .map(|app| app.entry.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(outcome.roots_checked, 1);
        assert!(names.contains(&"Example App"));
        assert!(names.contains(&"Helper"));
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn discovered_shortcut_launches_via_explorer() {
        let path = PathBuf::from("App.lnk");
        let app = app_from_start_menu_file(&path).expect("lnk should be indexed");

        assert_eq!(app.entry.command, "explorer.exe");
        assert_eq!(app.entry.args, vec![path.to_string_lossy().into_owned()]);
        assert_eq!(app.entry.name, "App");
    }

    #[test]
    fn config_entries_win_over_discovered_duplicates() {
        let config = Config {
            apps: vec![AppEntry::new("Notepad", "custom-notepad.exe")],
            ..Default::default()
        };
        let indexed = AppIndexOutcome {
            entries: vec![IndexedApp::discovered(
                AppEntry::new("Notepad", "notepad.exe"),
                PathBuf::from("Notepad.lnk"),
            )],
            roots_checked: 1,
        };

        let provider = AppLauncherProvider::from_config_and_index_result(&config, Ok(indexed));
        let result = provider
            .search(&SearchContext::new("notepad", 10))
            .expect("app search should succeed")
            .into_iter()
            .find(|result| result.title == "Notepad")
            .expect("configured Notepad should be present");

        assert!(matches!(
            result.action,
            Action::LaunchApp { ref command, .. } if command == "custom-notepad.exe"
        ));
    }

    #[test]
    fn fallback_sample_is_used_when_indexing_is_unavailable_and_no_config_exists() {
        let provider = AppLauncherProvider::from_config_and_index_result(
            &Config::default(),
            Err(AppIndexError::Unavailable("not windows".to_string())),
        );

        let results = provider
            .search(&SearchContext::new("notepad", 10))
            .expect("fallback search should succeed");

        assert_eq!(
            provider.index_status_summary(),
            "app indexing unavailable: not windows; using fallback if no configured apps exist"
        );
        assert!(results.iter().any(|result| {
            result.title == "Notepad"
                && result
                    .subtitle
                    .as_deref()
                    .is_some_and(|subtitle| subtitle.contains("Fallback sample"))
        }));
    }

    #[test]
    fn empty_index_does_not_add_fallback_when_config_apps_exist() {
        let config = Config {
            apps: vec![AppEntry::new("Configured App", "configured.exe")],
            ..Default::default()
        };
        let provider = AppLauncherProvider::from_config_and_index_result(
            &config,
            Ok(AppIndexOutcome {
                entries: Vec::new(),
                roots_checked: 2,
            }),
        );

        let results = provider
            .search(&SearchContext::new("", 10))
            .expect("app search should succeed");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Configured App");
    }

    #[test]
    fn known_platform_apps_are_added_without_using_fallback() {
        let mut notepad = AppEntry::new("Notepad", "notepad.exe");
        notepad.keywords = vec!["built-in".to_string()];
        let provider = AppLauncherProvider::from_config_sources(
            &Config::default(),
            Ok(AppIndexOutcome {
                entries: Vec::new(),
                roots_checked: 1,
            }),
            vec![IndexedApp::known(notepad, "Windows built-in app")],
        );

        let results = provider
            .search(&SearchContext::new("notepad", 10))
            .expect("known app search should succeed");

        assert!(results.iter().any(|result| {
            result.title == "Notepad"
                && result
                    .subtitle
                    .as_deref()
                    .is_some_and(|subtitle| subtitle.contains("Windows built-in app"))
        }));
    }
}
