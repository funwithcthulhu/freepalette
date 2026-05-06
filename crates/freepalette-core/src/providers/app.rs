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
use serde::Serialize;
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppIndexReport {
    pub summary: String,
    pub status: AppIndexReportStatus,
    pub entries: Vec<AppIndexEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum AppIndexReportStatus {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppIndexEntry {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub keywords: Vec<String>,
    pub source: AppIndexEntrySource,
    pub source_detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AppIndexEntrySource {
    Config,
    Known,
    WindowsStartMenu,
    Fallback,
}

impl AppLauncherProvider {
    pub fn from_config(config: &Config) -> Self {
        Self::from_config_sources(config, index_platform_apps(), platform_known_apps())
    }

    #[cfg(test)]
    fn from_config_and_index_result(
        config: &Config,
        index_result: Result<AppIndex, AppIndexError>,
    ) -> Self {
        Self::from_config_sources(config, index_result, Vec::new())
    }

    fn from_config_sources(
        config: &Config,
        index_result: Result<AppIndex, AppIndexError>,
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

    pub fn index_report(&self) -> AppIndexReport {
        AppIndexReport {
            summary: self.index_status_summary(),
            status: self.index_status.to_report_status(),
            entries: self.apps.iter().map(app_index_entry).collect(),
        }
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
                launch_command(command, args)?;
                Ok(ActionOutcome::new(format!("launched {command}")))
            }
            Action::OpenPath { path } => {
                open_path_with_default_app(path)?;
                Ok(ActionOutcome::new(format!("opened {path}")))
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

    fn to_report_status(&self) -> AppIndexReportStatus {
        match self {
            Self::Indexed {
                roots_checked,
                discovered,
            } => AppIndexReportStatus::Indexed {
                roots_checked: *roots_checked,
                discovered: *discovered,
            },
            Self::Empty { roots_checked } => AppIndexReportStatus::Empty {
                roots_checked: *roots_checked,
            },
            Self::Unavailable { reason } => AppIndexReportStatus::Unavailable {
                reason: reason.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AppIndex {
    entries: Vec<IndexedApp>,
    roots_checked: usize,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
enum AppIndexError {
    #[cfg(any(test, not(target_os = "windows")))]
    #[error("Windows Start Menu indexing is only available on Windows")]
    UnsupportedPlatform,
    #[cfg(target_os = "windows")]
    #[error("missing Windows Start Menu environment: APPDATA and ProgramData are not set")]
    MissingStartMenuEnvironment,
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
        action_for_app(app),
    )
    .with_subtitle(subtitle)
    .with_keywords(app.entry.keywords.clone())
}

fn action_for_app(app: &IndexedApp) -> Action {
    if matches!(app.source, AppSource::WindowsStartMenu { .. })
        && app.entry.args.is_empty()
        && start_menu_file_extension(Path::new(&app.entry.command))
            .is_some_and(|extension| is_shell_opened_start_menu_extension(&extension))
    {
        Action::OpenPath {
            path: app.entry.command.clone(),
        }
    } else {
        Action::LaunchApp {
            command: app.entry.command.clone(),
            args: app.entry.args.clone(),
        }
    }
}

fn launch_command(command: &str, args: &[String]) -> Result<(), PluginError> {
    Command::new(command).args(args).spawn().map_err(|source| {
        PluginError::Action(format!("failed to launch app '{command}': {source}"))
    })?;

    Ok(())
}

fn open_path_with_default_app(path: &str) -> Result<(), PluginError> {
    platform_open_path_with_default_app(Path::new(path)).map_err(|source| {
        PluginError::Action(format!(
            "failed to open path '{}' with the default app: {source}",
            Path::new(path).display()
        ))
    })
}

#[cfg(target_os = "windows")]
fn platform_open_path_with_default_app(path: &Path) -> Result<(), win_desktop_utils::Error> {
    win_desktop_utils::open_with_default(path)
}

#[cfg(not(target_os = "windows"))]
fn platform_open_path_with_default_app(path: &Path) -> Result<(), String> {
    Err(format!(
        "opening paths with the default app is not implemented on this platform: {}",
        path.display()
    ))
}

fn app_index_entry(app: &IndexedApp) -> AppIndexEntry {
    let (source, source_detail) = app.source.debug_source();
    AppIndexEntry {
        name: app.entry.name.clone(),
        command: app.entry.command.clone(),
        args: app.entry.args.clone(),
        keywords: app.entry.keywords.clone(),
        source,
        source_detail,
    }
}

impl AppSource {
    fn debug_source(&self) -> (AppIndexEntrySource, Option<String>) {
        match self {
            Self::Config => (AppIndexEntrySource::Config, None),
            #[cfg(any(test, target_os = "windows"))]
            Self::Known { label } => (AppIndexEntrySource::Known, Some(label.clone())),
            Self::WindowsStartMenu { path } => (
                AppIndexEntrySource::WindowsStartMenu,
                Some(path.display().to_string()),
            ),
            Self::Fallback { reason } => (AppIndexEntrySource::Fallback, Some(reason.clone())),
        }
    }
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

fn index_platform_apps() -> Result<AppIndex, AppIndexError> {
    let roots = platform_start_menu_roots()?;
    Ok(index_start_menu_roots(&roots))
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
        Err(AppIndexError::MissingStartMenuEnvironment)
    } else {
        Ok(roots)
    }
}

#[cfg(not(target_os = "windows"))]
fn platform_start_menu_roots() -> Result<Vec<PathBuf>, AppIndexError> {
    Err(AppIndexError::UnsupportedPlatform)
}

fn index_start_menu_roots(roots: &[PathBuf]) -> AppIndex {
    let mut entries = Vec::new();

    for root in roots {
        scan_start_menu_root(root, &mut entries);
    }

    dedupe_discovered_apps(&mut entries);
    entries.sort_by(|left, right| {
        left.entry
            .name
            .to_ascii_lowercase()
            .cmp(&right.entry.name.to_ascii_lowercase())
    });

    AppIndex {
        entries,
        roots_checked: roots.len(),
    }
}

fn dedupe_discovered_apps(entries: &mut Vec<IndexedApp>) {
    let mut seen = HashSet::new();
    entries.retain(|app| seen.insert(app.entry.name.to_ascii_lowercase()));
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
                if let Some(app) = indexed_app_from_start_menu_file(&path) {
                    entries.push(app);
                }
            }
        }
    }
}

fn indexed_app_from_start_menu_file(path: &Path) -> Option<IndexedApp> {
    let extension = start_menu_file_extension(path)?;
    if !is_supported_start_menu_extension(&extension) {
        return None;
    }

    let name = app_name_from_path(path)?;
    let (command, args) = launch_command_for_path(path);
    let keywords = keywords_for_discovered_app(path, &extension);
    let entry = AppEntry {
        name,
        command,
        args,
        keywords,
    };

    Some(IndexedApp::discovered(entry, path.to_path_buf()))
}

fn start_menu_file_extension(path: &Path) -> Option<String> {
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

fn launch_command_for_path(path: &Path) -> (String, Vec<String>) {
    (path.to_string_lossy().into_owned(), Vec::new())
}

fn is_supported_start_menu_extension(extension: &str) -> bool {
    extension == "exe" || is_shell_opened_start_menu_extension(extension)
}

fn is_shell_opened_start_menu_extension(extension: &str) -> bool {
    matches!(extension, "lnk" | "appref-ms")
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

        let app_index = index_start_menu_roots(std::slice::from_ref(&root));
        fs::remove_dir_all(&root).expect("test start menu directory should be removed");

        let names = app_index
            .entries
            .iter()
            .map(|app| app.entry.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(app_index.roots_checked, 1);
        assert!(names.contains(&"Example App"));
        assert!(names.contains(&"Helper"));
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn discovered_shortcut_opens_with_default_app() {
        let path = PathBuf::from("App.lnk");
        let app = indexed_app_from_start_menu_file(&path).expect("lnk should be indexed");
        let result = app_result(&app);

        assert_eq!(app.entry.command, path.to_string_lossy().into_owned());
        assert!(app.entry.args.is_empty());
        assert_eq!(app.entry.name, "App");
        assert!(matches!(
            result.action,
            Action::OpenPath { ref path } if path == "App.lnk"
        ));
    }

    #[test]
    fn configured_shortcut_keeps_configured_launch_action() {
        let app = IndexedApp::configured(AppEntry::new("Configured Shortcut", "App.lnk"));
        let result = app_result(&app);

        assert!(matches!(
            result.action,
            Action::LaunchApp { ref command, ref args }
                if command == "App.lnk" && args.is_empty()
        ));
    }

    #[test]
    fn unsupported_start_menu_files_are_ignored() {
        assert!(indexed_app_from_start_menu_file(Path::new("Readme.txt")).is_none());
        assert!(indexed_app_from_start_menu_file(Path::new("NoExtension")).is_none());
    }

    #[test]
    fn duplicate_discovered_apps_keep_first_root() {
        let user_root = temp_root("user-apps");
        let system_root = temp_root("system-apps");
        fs::create_dir_all(&user_root).expect("test user start menu should be created");
        fs::create_dir_all(&system_root).expect("test system start menu should be created");
        fs::write(user_root.join("Same App.lnk"), "")
            .expect("test user shortcut should be written");
        fs::write(system_root.join("Same App.lnk"), "")
            .expect("test system shortcut should be written");

        let app_index = index_start_menu_roots(&[user_root.clone(), system_root.clone()]);
        fs::remove_dir_all(&user_root).expect("test user start menu should be removed");
        fs::remove_dir_all(&system_root).expect("test system start menu should be removed");

        assert_eq!(app_index.entries.len(), 1);
        assert!(matches!(
            &app_index.entries[0].source,
            AppSource::WindowsStartMenu { path } if path.starts_with(&user_root)
        ));
    }

    #[test]
    fn config_entries_win_over_discovered_duplicates() {
        let config = Config {
            apps: vec![AppEntry::new("Notepad", "custom-notepad.exe")],
            ..Default::default()
        };
        let indexed = AppIndex {
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
            Err(AppIndexError::UnsupportedPlatform),
        );

        let results = provider
            .search(&SearchContext::new("notepad", 10))
            .expect("fallback search should succeed");

        assert_eq!(
            provider.index_status_summary(),
            "app indexing unavailable: Windows Start Menu indexing is only available on Windows; using fallback if no configured apps exist"
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
            Ok(AppIndex {
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
            Ok(AppIndex {
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

    #[test]
    fn app_index_report_includes_entries_and_status() {
        let config = Config {
            apps: vec![AppEntry::new("Configured App", "configured.exe")],
            ..Default::default()
        };
        let provider = AppLauncherProvider::from_config_and_index_result(
            &config,
            Ok(AppIndex {
                entries: vec![IndexedApp::discovered(
                    AppEntry::new("Discovered App", "Discovered App.lnk"),
                    PathBuf::from("Discovered App.lnk"),
                )],
                roots_checked: 2,
            }),
        );

        let report = provider.index_report();

        assert_eq!(report.entries.len(), 2);
        assert!(matches!(
            report.status,
            AppIndexReportStatus::Indexed {
                roots_checked: 2,
                discovered: 1
            }
        ));
        assert_eq!(report.entries[0].source, AppIndexEntrySource::Config);
        assert_eq!(
            report.entries[1].source,
            AppIndexEntrySource::WindowsStartMenu
        );
    }
}
