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
const NOISY_START_MENU_ENTRY_SCORE_HINT: i64 = -220;
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
        AppSource::Config => "Configured app".to_string(),
        #[cfg(any(test, target_os = "windows"))]
        AppSource::Known { label } => label.clone(),
        AppSource::WindowsStartMenu { .. } => "Windows Start Menu".to_string(),
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
    .with_score_hint(app_score_hint(app))
}

fn app_score_hint(app: &IndexedApp) -> i64 {
    if matches!(app.source, AppSource::WindowsStartMenu { .. })
        && is_noisy_start_menu_entry(&app.entry.name)
    {
        NOISY_START_MENU_ENTRY_SCORE_HINT
    } else {
        0
    }
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
    ensure_launch_command_is_complete(command)?;
    ensure_launch_command_has_no_embedded_arguments(command)?;

    if command_is_explicit_path(command) {
        ensure_app_target_exists(Path::new(command))?;
    }

    Command::new(command).args(args).spawn().map_err(|source| {
        PluginError::Action(format!("failed to launch app '{command}': {source}"))
    })?;

    Ok(())
}

fn ensure_launch_command_is_complete(command: &str) -> Result<(), PluginError> {
    if command.trim().is_empty() {
        Err(PluginError::Action(
            "app launch command is empty; check the app index entry or config".to_string(),
        ))
    } else {
        Ok(())
    }
}

fn ensure_launch_command_has_no_embedded_arguments(command: &str) -> Result<(), PluginError> {
    if command.contains('"') {
        Err(PluginError::Action(
            "app launch command must be an executable path without embedded quotes or arguments; store arguments separately".to_string(),
        ))
    } else {
        Ok(())
    }
}

fn open_path_with_default_app(path: &str) -> Result<(), PluginError> {
    ensure_app_target_exists(Path::new(path))?;

    platform_open_path_with_default_app(Path::new(path)).map_err(|source| {
        PluginError::Action(format!(
            "failed to open path '{}' with the default app: {source}",
            Path::new(path).display()
        ))
    })
}

fn command_is_explicit_path(command: &str) -> bool {
    Path::new(command).is_absolute()
}

fn ensure_app_target_exists(path: &Path) -> Result<(), PluginError> {
    if path.exists() {
        Ok(())
    } else {
        Err(PluginError::Action(format!(
            "app target does not exist: {}",
            path.display()
        )))
    }
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

fn is_noisy_start_menu_entry(name: &str) -> bool {
    let normalized = name.trim().to_ascii_lowercase();
    normalized.starts_with("uninstall ")
        || normalized.starts_with("uninstall-")
        || normalized.starts_with("uninstall_")
        || normalized.starts_with("documentation ")
        || normalized.starts_with("install additional tools ")
        || normalized.starts_with("samples for ")
        || normalized.starts_with("tools for ")
        || normalized.contains(" documentation")
        || normalized.ends_with(" uninstaller")
        || normalized.ends_with(" release notes")
        || normalized.ends_with(" help")
        || normalized.ends_with(" documentation")
        || normalized.ends_with(" manual")
        || normalized.ends_with(" readme")
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
    use crate::{providers::CalculatorProvider, ProviderRegistry};

    use super::*;

    fn stale_indexed_app() -> (AppLauncherProvider, PathBuf) {
        let missing_target = temp_root("stale-app").join("Missing App.exe");
        let missing_target_string = missing_target.to_string_lossy().into_owned();
        let indexed = AppIndex {
            entries: vec![IndexedApp::discovered(
                AppEntry::new("Stale App", missing_target_string),
                missing_target.clone(),
            )],
            roots_checked: 1,
        };

        (
            AppLauncherProvider::from_config_and_index_result(&Config::default(), Ok(indexed)),
            missing_target,
        )
    }

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
    fn app_result_uses_source_specific_subtitle() {
        let configured = app_result(&IndexedApp::configured(AppEntry::new(
            "Configured Editor",
            "editor.exe",
        )));
        let discovered = app_result(&IndexedApp::discovered(
            AppEntry::new("Discovered Editor", "editor.lnk"),
            PathBuf::from("Tools/Discovered Editor.lnk"),
        ));

        assert_eq!(configured.subtitle.as_deref(), Some("Configured app"));
        assert_eq!(discovered.subtitle.as_deref(), Some("Windows Start Menu"));
    }

    #[test]
    fn configured_app_keyword_alias_is_searchable() {
        let mut entry = AppEntry::new("Visual Studio Code", "code.exe");
        entry.keywords = vec!["code".to_string(), "editor".to_string()];
        let provider = AppLauncherProvider::from_config_and_index_result(
            &Config {
                apps: vec![entry],
                ..Default::default()
            },
            Ok(AppIndex {
                entries: Vec::new(),
                roots_checked: 1,
            }),
        );
        let mut registry = ProviderRegistry::new();
        registry
            .register(provider)
            .expect("app provider should register");

        let results = registry
            .search("code", 10)
            .expect("alias search should succeed");

        assert_eq!(results[0].result.title, "Visual Studio Code");
        assert_eq!(
            results[0].result.subtitle.as_deref(),
            Some("Configured app")
        );
    }

    #[test]
    fn noisy_start_menu_entries_are_demoted_but_remain_searchable() {
        let indexed = AppIndex {
            entries: vec![
                IndexedApp::discovered(
                    AppEntry::new("Node.js", "Node.js.lnk"),
                    PathBuf::from("Node.js.lnk"),
                ),
                IndexedApp::discovered(
                    AppEntry::new("Uninstall Node.js", "Uninstall Node.js.lnk"),
                    PathBuf::from("Uninstall Node.js.lnk"),
                ),
                IndexedApp::discovered(
                    AppEntry::new("Node.js Documentation", "Node.js Documentation.lnk"),
                    PathBuf::from("Node.js Documentation.lnk"),
                ),
                IndexedApp::discovered(
                    AppEntry::new(
                        "Documentation for Desktop Apps",
                        "Documentation for Desktop Apps.lnk",
                    ),
                    PathBuf::from("Documentation for Desktop Apps.lnk"),
                ),
                IndexedApp::discovered(
                    AppEntry::new(
                        "Install Additional Tools for Node.js",
                        "Install Additional Tools for Node.js.lnk",
                    ),
                    PathBuf::from("Install Additional Tools for Node.js.lnk"),
                ),
            ],
            roots_checked: 1,
        };
        let provider =
            AppLauncherProvider::from_config_and_index_result(&Config::default(), Ok(indexed));
        let mut registry = ProviderRegistry::new();
        registry
            .register(provider)
            .expect("app provider should register");

        let node_results = registry
            .search("node", 10)
            .expect("node search should succeed");
        let uninstall_results = registry
            .search("uninstall node", 10)
            .expect("uninstall search should succeed");

        assert_eq!(node_results[0].result.title, "Node.js");
        assert!(node_results
            .iter()
            .any(|ranked| ranked.result.title == "Uninstall Node.js"));
        let node_position = node_results
            .iter()
            .position(|ranked| ranked.result.title == "Node.js")
            .expect("normal app should remain visible");
        let docs_position = node_results
            .iter()
            .position(|ranked| ranked.result.title == "Documentation for Desktop Apps")
            .expect("noisy docs shortcut should remain searchable");

        assert!(node_position < docs_position);
        assert_eq!(uninstall_results[0].result.title, "Uninstall Node.js");
    }

    #[test]
    fn stale_indexed_app_searches_without_panicking() {
        let (provider, missing_target) = stale_indexed_app();

        let results = provider
            .search(&SearchContext::new("stale", 10))
            .expect("stale indexed app search should not fail");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Stale App");
        let missing_target_string = missing_target.to_string_lossy().into_owned();
        assert!(matches!(
            &results[0].action,
            Action::LaunchApp { command, args }
                if command == &missing_target_string && args.is_empty()
        ));

        let report = provider.index_report();
        assert_eq!(report.entries.len(), 1);
        assert_eq!(
            report.entries[0].source,
            AppIndexEntrySource::WindowsStartMenu
        );
        assert_eq!(
            report.entries[0].source_detail.as_deref(),
            Some(missing_target.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn stale_indexed_app_execution_fails_clearly() {
        let (provider, missing_target) = stale_indexed_app();
        let result = provider
            .search(&SearchContext::new("stale", 10))
            .expect("stale indexed app search should not fail")
            .into_iter()
            .next()
            .expect("stale app result should be present");

        let error = provider
            .execute(&result.action)
            .expect_err("stale app launch should fail before spawning");
        let message = error.to_string();

        assert!(message.contains("app target does not exist"));
        assert!(message.contains(&missing_target.to_string_lossy().to_string()));
    }

    #[test]
    fn stale_shortcut_open_path_execution_fails_clearly() {
        let missing_shortcut = temp_root("stale-shortcut").join("Missing Shortcut.lnk");

        let error = open_path_with_default_app(&missing_shortcut.to_string_lossy())
            .expect_err("stale shortcut should fail before opening with the platform shell");
        let message = error.to_string();

        assert!(message.contains("app target does not exist"));
        assert!(message.contains(&missing_shortcut.to_string_lossy().to_string()));
    }

    #[test]
    fn stale_indexed_app_does_not_displace_calculator_results() {
        let (provider, _missing_target) = stale_indexed_app();
        let mut registry = ProviderRegistry::new();
        registry
            .register(provider)
            .expect("stale app provider should register");
        registry
            .register(CalculatorProvider)
            .expect("calculator provider should register");

        let results = registry
            .search("calc 2+2", 10)
            .expect("search with stale app should succeed");

        assert_eq!(results[0].result.provider, ProviderId::from("calculator"));
        assert_eq!(results[0].result.title, "2+2 = 4");
    }

    #[test]
    fn indexed_app_with_display_name_but_empty_command_fails_before_launch() {
        let indexed = AppIndex {
            entries: vec![IndexedApp::discovered(
                AppEntry::new("Empty Target", ""),
                PathBuf::from("Empty Target.lnk"),
            )],
            roots_checked: 1,
        };
        let provider =
            AppLauncherProvider::from_config_and_index_result(&Config::default(), Ok(indexed));

        let result = provider
            .search(&SearchContext::new("empty target", 10))
            .expect("search with incomplete indexed app should succeed")
            .into_iter()
            .next()
            .expect("incomplete indexed app should remain visible");

        assert_eq!(result.title, "Empty Target");
        assert!(matches!(
            result.action,
            Action::LaunchApp { ref command, ref args } if command.is_empty() && args.is_empty()
        ));

        let error = provider
            .execute(&result.action)
            .expect_err("empty app command should fail before spawning");

        assert!(error.to_string().contains("app launch command is empty"));
    }

    #[test]
    fn parsed_shortcut_target_with_spaces_preserves_command_and_args() {
        let executable = r"C:\Program Files\Example App\app.exe".to_string();
        let args = vec![
            "--profile".to_string(),
            "German News".to_string(),
            "--safe-mode".to_string(),
        ];
        let indexed = IndexedApp::discovered(
            AppEntry {
                name: "Example App".to_string(),
                command: executable.clone(),
                args: args.clone(),
                keywords: Vec::new(),
            },
            PathBuf::from(r"C:\Start Menu\Example App.lnk"),
        );

        let result = app_result(&indexed);
        let report_entry = app_index_entry(&indexed);

        assert!(matches!(
            &result.action,
            Action::LaunchApp {
                command,
                args: action_args,
            }
                if command == &executable && action_args == &args
        ));
        assert!(!matches!(&result.action, Action::RunShell { .. }));
        assert_eq!(report_entry.command, executable);
        assert_eq!(report_entry.args, args);
        assert_eq!(
            report_entry.source_detail.as_deref(),
            Some(r"C:\Start Menu\Example App.lnk")
        );
    }

    #[test]
    fn flattened_quoted_shortcut_target_is_rejected_before_launch() {
        let flattened_command = r#""C:\Program Files\Example App\app.exe" --profile "German News""#;

        let error = launch_command(flattened_command, &[])
            .expect_err("flattened shortcut command should be rejected");

        assert!(error
            .to_string()
            .contains("without embedded quotes or arguments"));
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
    fn fallback_sample_report_stays_separate_from_indexed_apps() {
        let provider = AppLauncherProvider::from_config_and_index_result(
            &Config::default(),
            Err(AppIndexError::UnsupportedPlatform),
        );

        let report = provider.index_report();

        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].name, "Notepad");
        assert_eq!(report.entries[0].source, AppIndexEntrySource::Fallback);
        assert!(report.entries[0]
            .source_detail
            .as_deref()
            .is_some_and(|detail| {
                detail.contains("Windows Start Menu indexing is only available on Windows")
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
