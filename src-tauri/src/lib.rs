use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    ffi::{OsStr, OsString},
    fs,
    io::{ErrorKind, Read},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Condvar, LazyLock, Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, PhysicalPosition, State, WindowEvent,
};
use tauri_plugin_dialog::DialogExt;

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

const LIBRARY_ROOT_NAME: &str = "虫洞派资料库";
const LEGACY_ORGANIZE_ROOT_NAME: &str = "虫洞派整理";
const ORGANIZE_BATCHES_META_KEY: &str = "organize_batches_v1";
const TRAY_SHOW_ID: &str = "tray_show_main";
const TRAY_EXIT_REST_ID: &str = "tray_exit_rest";
const TRAY_RESTORE_PET_ID: &str = "tray_restore_pet";
const TRAY_QUIT_ID: &str = "tray_quit_app";
const EXIT_REST_EVENT: &str = "ui://exit-rest";
const PET_MENU_OPEN_ID: &str = "pet_menu_open_main";
const PET_MENU_DIALOGUE_ID: &str = "pet_menu_open_dialogue";
const PET_MENU_SETTINGS_ID: &str = "pet_menu_open_settings";
const PET_MENU_LAYER_NORMAL_ID: &str = "pet_menu_layer_normal";
const PET_MENU_LAYER_TOP_ID: &str = "pet_menu_layer_top";
const PET_MENU_LAYER_BOTTOM_ID: &str = "pet_menu_layer_bottom";
const PET_MENU_HIDE_ID: &str = "pet_menu_hide";
const AGENT_TASK_MAX_CHARS: usize = 4_000;
const AGENT_OUTPUT_MAX_BYTES: usize = 128 * 1024;
const AGENT_PROBE_OUTPUT_MAX_BYTES: usize = 8 * 1024;
const AGENT_TASK_TIMEOUT: Duration = Duration::from_secs(12 * 60 * 60);
const AGENT_PROBE_TIMEOUT: Duration = Duration::from_secs(8);
const AGENT_CONNECTOR_CACHE_TTL: Duration = Duration::from_secs(5 * 60);
const AGENT_TASK_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(2);
const AGENT_ATTACHMENT_MAX_COUNT: usize = 8;
const AGENT_ATTACHMENT_MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const AGENT_ATTACHMENT_MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;
const AGENT_ATTACHMENT_DATA_DIR: &str = ".wormhole-pie";
const AGENT_ATTACHMENT_INPUT_DIR: &str = "agent-input";
const AGENT_RESULT_MAX_FILES: usize = 12;
static AGENT_TASK_LOCK: Mutex<()> = Mutex::new(());
static ACTIVE_AGENT_TASK: Mutex<Option<ActiveAgentTask>> = Mutex::new(None);
static LAST_AGENT_TASK_STATUS: Mutex<Option<AgentTaskStatus>> = Mutex::new(None);
static LAST_AGENT_TASK_RESULT: Mutex<Option<AgentTaskResult>> = Mutex::new(None);
static AGENT_TASK_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static AGENT_CONNECTOR_CACHE: LazyLock<(Mutex<AgentConnectorCacheState>, Condvar)> =
    LazyLock::new(|| {
        (
            Mutex::new(AgentConnectorCacheState::default()),
            Condvar::new(),
        )
    });

fn is_wormhole_shortcut_name(name: &OsStr) -> bool {
    let value = name.to_string_lossy();
    value == "虫洞派.lnk" || value.eq_ignore_ascii_case("Wormhole Pie.lnk")
}

#[derive(Clone)]
struct SharedIndex {
    db: Arc<Mutex<Connection>>,
    desktop_path: PathBuf,
    library_path: PathBuf,
    legacy_library_path: PathBuf,
}

struct AppState {
    index: SharedIndex,
    watcher: Mutex<Option<RecommendedWatcher>>,
    last_feed: Mutex<Vec<PathBuf>>,
    organize_batches: Mutex<Vec<DesktopOrganizeBatch>>,
    organize_lock: Arc<Mutex<()>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopFile {
    id: i64,
    name: String,
    path: String,
    extension: String,
    kind: String,
    category: String,
    organized_category: String,
    size: u64,
    created_at: u64,
    modified_at: u64,
    is_new: bool,
}

#[derive(Debug, Serialize)]
struct CursorPoint {
    x: f64,
    y: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FeedEvent {
    names: Vec<String>,
    count: usize,
    failed_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DesktopOrganizeMove {
    move_id: String,
    original: PathBuf,
    organized: PathBuf,
    category: String,
    is_dir: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DesktopOrganizeBatch {
    batch_id: String,
    root: PathBuf,
    moves: Vec<DesktopOrganizeMove>,
    created_directories: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopOrganizeItem {
    move_id: String,
    name: String,
    original_path: String,
    organized_path: String,
    category: String,
    kind: String,
    launchable: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopOrganizeResult {
    moved_count: usize,
    new_moved_count: usize,
    migrated_count: usize,
    category_count: usize,
    skipped_count: usize,
    root_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    batch_id: Option<String>,
    items: Vec<DesktopOrganizeItem>,
    excluded_count: usize,
    skipped_items: Vec<DesktopOrganizeSkippedItem>,
    indexed_count: usize,
    public_desktop_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopOrganizeSkippedItem {
    name: String,
    path: String,
    reason_code: String,
    reason: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopOrganizeReviewResult {
    restored_count: usize,
    remembered_count: usize,
    remaining_undo_count: usize,
    conflict_count: usize,
    restored_move_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OrganizeState {
    can_undo: bool,
    batch_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OrganizeExclusion {
    name_key: String,
    display_name: String,
    is_directory: bool,
    created_at: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgramEntry {
    name: String,
    path: String,
    source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentConnectorStatus {
    id: String,
    name: String,
    detected: bool,
    available: bool,
    configured: bool,
    configuration_state: AgentConnectorConfigurationState,
    #[serde(skip_serializing_if = "Option::is_none")]
    config_location_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    executable: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum AgentConnectorConfigurationState {
    Ready,
    NotConfigured,
    PermissionBlocked,
    ProbeFailed,
    NotInstalled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum AgentTaskState {
    Starting,
    Running,
    Cancelling,
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentTaskStatus {
    task_id: String,
    connector_id: String,
    state: AgentTaskState,
    started_at: u64,
    updated_at: u64,
    elapsed_ms: u64,
    cancel_requested: bool,
    detail: String,
}

#[derive(Debug)]
struct ActiveAgentTask {
    status: AgentTaskStatus,
    cancel: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentTaskResult {
    task_id: String,
    connector_id: String,
    success: bool,
    timed_out: bool,
    cancelled: bool,
    output: String,
    exit_code: Option<i32>,
    duration_ms: u64,
    truncated: bool,
    files: Vec<AgentResultFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentResultFile {
    name: String,
    path: String,
    relative_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum AgentConnectorKind {
    Codex,
    Claude,
    Hermes,
}

impl AgentConnectorKind {
    fn id(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Hermes => "hermes",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Claude => "Claude Code",
            Self::Hermes => "Hermes Agent",
        }
    }

    fn command_name(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Hermes => "hermes",
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        match id {
            "codex" => Some(Self::Codex),
            "claude" => Some(Self::Claude),
            "hermes" => Some(Self::Hermes),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
struct ResolvedAgentConnector {
    status: AgentConnectorStatus,
    executable: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct CachedAgentConnector {
    resolved: ResolvedAgentConnector,
    probed_at: Instant,
}

#[derive(Debug, Default)]
struct AgentConnectorCacheState {
    entries: BTreeMap<AgentConnectorKind, CachedAgentConnector>,
    probing: BTreeSet<AgentConnectorKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentConnectorCacheDecision {
    UseCached,
    WaitForProbe,
    StartProbe,
}

#[derive(Debug)]
struct CapturedPipe {
    bytes: Vec<u8>,
    truncated: bool,
}

#[derive(Debug)]
struct BoundedProcessOutput {
    success: bool,
    timed_out: bool,
    cancelled: bool,
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
    duration_ms: u64,
    truncated: bool,
}

#[derive(Debug)]
struct BoundedProcessError {
    message: String,
    permission_denied: bool,
}

#[derive(Debug)]
struct PreparedAgentAttachment {
    display_name: String,
    relative_path: PathBuf,
    snapshot: bool,
}

#[derive(Debug)]
struct PreparedAgentAttachments {
    items: Vec<PreparedAgentAttachment>,
    workspace: PathBuf,
    cleanup_dir: Option<PathBuf>,
}

impl Drop for PreparedAgentAttachments {
    fn drop(&mut self) {
        let Some(cleanup_dir) = self.cleanup_dir.as_deref() else {
            return;
        };
        let expected_root = self
            .workspace
            .join(AGENT_ATTACHMENT_DATA_DIR)
            .join(AGENT_ATTACHMENT_INPUT_DIR);
        if !cleanup_dir.starts_with(&expected_root) {
            return;
        }
        if fs::symlink_metadata(cleanup_dir).is_ok_and(|metadata| {
            metadata.is_dir() && !metadata.file_type().is_symlink() && !is_reparse_point(&metadata)
        }) {
            let _ = fs::remove_dir_all(cleanup_dir);
        }
        let _ = fs::remove_dir(&expected_root);
        let _ = fs::remove_dir(self.workspace.join(AGENT_ATTACHMENT_DATA_DIR));
    }
}

#[derive(Debug, Clone)]
struct ProgramSearchRoot {
    path: PathBuf,
    source: &'static str,
    recursive: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopIconState {
    supported: bool,
    hidden: bool,
    public_desktop_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PetLayerPolicy {
    always_on_top: bool,
    always_on_bottom: bool,
    ignore_cursor_events: bool,
    focusable: bool,
}

struct IndexedPath {
    path: String,
    name: String,
    extension: String,
    kind: String,
    organized_category: String,
    size: u64,
    created_at: u64,
    modified_at: u64,
}

struct OrganizeCandidate {
    original: PathBuf,
    name: OsString,
    category: String,
    is_dir: bool,
}

struct OrganizePlan {
    candidates: Vec<OrganizeCandidate>,
    categories: BTreeSet<String>,
    excluded_count: usize,
    skipped_items: Vec<DesktopOrganizeSkippedItem>,
}

fn organize_category(path: &Path, is_dir: bool) -> &'static str {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if is_dir {
        return if extension == "app" {
            "应用"
        } else {
            "文件夹"
        };
    }

    match extension.as_str() {
        "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "java" | "kt" | "swift" | "c"
        | "cpp" | "h" | "hpp" | "cs" | "rb" | "php" | "html" | "css" | "scss" | "json" | "yaml"
        | "yml" | "toml" | "sql" | "sh" => "代码",
        "doc" | "docx" | "pdf" | "ppt" | "pptx" | "xls" | "xlsx" | "txt" | "md" | "rtf" | "csv"
        | "odt" | "ods" | "odp" | "pages" | "numbers" | "key" | "epub" => "文档",
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" | "heic" | "heif" | "tif"
        | "tiff" | "ico" => "图片",
        "mp4" | "mov" | "avi" | "mkv" | "webm" | "m4v" | "wmv" | "flv" | "mpeg" | "mpg" => "视频",
        "mp3" | "wav" | "m4a" | "flac" | "aac" | "ogg" | "wma" | "aiff" | "mid" | "midi" => "音频",
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "tgz" | "zst" => "压缩包",
        "lnk" | "url" | "webloc" | "desktop" | "alias" => "快捷方式",
        "exe" | "msi" | "msix" | "msixbundle" | "appinstaller" | "bat" | "cmd" | "ps1" | "vbs"
        | "command" | "dmg" | "pkg" => "应用",
        _ => "其他",
    }
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

fn is_hidden_or_system(name: &OsStr, metadata: &fs::Metadata) -> bool {
    if name.to_string_lossy().starts_with('.') {
        return true;
    }

    #[cfg(windows)]
    {
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x0002;
        const FILE_ATTRIBUTE_SYSTEM: u32 = 0x0004;
        let attributes = metadata.file_attributes();
        attributes & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0
    }

    #[cfg(not(windows))]
    {
        false
    }
}

fn path_reservation_key(path: &Path) -> String {
    // Being conservative here also prevents collisions on the default
    // case-insensitive macOS file system.
    path.to_string_lossy().to_lowercase()
}

fn path_is_available(path: &Path, reserved: &HashSet<String>) -> Result<bool, String> {
    if reserved.contains(&path_reservation_key(path)) {
        return Ok(false);
    }

    match fs::symlink_metadata(path) {
        Ok(_) => Ok(false),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(true),
        Err(error) => Err(format!("无法检查目标路径 {}：{error}", path.display())),
    }
}

fn unique_destination(
    directory: &Path,
    original_name: &OsStr,
    is_dir: bool,
    reserved: &mut HashSet<String>,
) -> Result<PathBuf, String> {
    let direct = directory.join(original_name);
    if path_is_available(&direct, reserved)? {
        reserved.insert(path_reservation_key(&direct));
        return Ok(direct);
    }

    let original_path = Path::new(original_name);
    let stem = if is_dir {
        original_name
    } else {
        original_path.file_stem().unwrap_or(original_name)
    };
    let extension = if is_dir {
        None
    } else {
        original_path.extension()
    };

    for number in 2..=10_000 {
        let mut candidate_name = OsString::from(stem);
        candidate_name.push(format!(" ({number})"));
        if let Some(extension) = extension.filter(|value| !value.is_empty()) {
            candidate_name.push(".");
            candidate_name.push(extension);
        }
        let candidate = directory.join(candidate_name);
        if path_is_available(&candidate, reserved)? {
            reserved.insert(path_reservation_key(&candidate));
            return Ok(candidate);
        }
    }

    Err(format!(
        "目标目录 {} 中同名项目过多，无法安全生成新名称",
        directory.display()
    ))
}

fn verify_or_create_directory(path: &Path, created: &mut Vec<PathBuf>) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.is_dir()
                || metadata.file_type().is_symlink()
                || is_reparse_point(&metadata)
            {
                return Err(format!("整理目录不是普通文件夹：{}", path.display()));
            }
            Ok(())
        }
        Err(error) if error.kind() == ErrorKind::NotFound => match fs::create_dir(path) {
            Ok(()) => {
                created.push(path.to_path_buf());
                Ok(())
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                verify_or_create_directory(path, created)
            }
            Err(error) => Err(format!("无法创建整理目录 {}：{error}", path.display())),
        },
        Err(error) => Err(format!("无法检查整理目录 {}：{error}", path.display())),
    }
}

fn remove_created_directories(directories: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut removed = Vec::new();
    for directory in directories.iter().rev() {
        match fs::remove_dir(directory) {
            Ok(()) => removed.push(directory.clone()),
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::NotFound | ErrorKind::DirectoryNotEmpty
                ) => {}
            Err(error) => {
                let recreate_error = recreate_removed_directories(&removed).err();
                return Err(format!(
                    "无法清理空整理目录 {}：{error}{}",
                    directory.display(),
                    recreate_error
                        .map(|value| format!("；恢复已移除目录失败：{value}"))
                        .unwrap_or_default()
                ));
            }
        }
    }
    Ok(removed)
}

fn validate_created_directories(root: &Path, directories: &[PathBuf]) -> Result<(), String> {
    for directory in directories {
        if directory != root && directory.parent() != Some(root) {
            return Err(format!(
                "整理历史包含资料库之外的待清理目录：{}",
                directory.display()
            ));
        }
    }
    Ok(())
}

fn recreate_removed_directories(directories: &[PathBuf]) -> Result<(), String> {
    let mut recreated = Vec::new();
    for directory in directories.iter().rev() {
        verify_or_create_directory(directory, &mut recreated)?;
    }
    Ok(())
}

fn rollback_transfers(transfers: &[(PathBuf, PathBuf, bool)]) -> Result<(), String> {
    let mut reserved = HashSet::new();
    let mut failures = Vec::new();

    for (source, destination, is_dir) in transfers.iter().rev() {
        if !destination.exists() {
            failures.push(format!("回滚来源不存在：{}", destination.display()));
            continue;
        }
        let Some(parent) = source.parent() else {
            failures.push(format!("原始路径无父目录：{}", source.display()));
            continue;
        };
        let Some(name) = source.file_name() else {
            failures.push(format!("原始路径无文件名：{}", source.display()));
            continue;
        };

        let rollback_target = match unique_destination(parent, name, *is_dir, &mut reserved) {
            Ok(path) => path,
            Err(error) => {
                failures.push(error);
                continue;
            }
        };
        if let Err(error) = fs::rename(destination, &rollback_target) {
            failures.push(format!(
                "无法将 {} 回滚到 {}：{error}",
                destination.display(),
                rollback_target.display()
            ));
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("；"))
    }
}

fn execute_transfers(transfers: &[(PathBuf, PathBuf, bool)]) -> Result<(), String> {
    let mut completed = Vec::new();
    for (source, destination, is_dir) in transfers {
        if let Err(error) = fs::rename(source, destination) {
            let rollback = rollback_transfers(&completed);
            return Err(match rollback {
                Ok(()) => format!(
                    "移动 {} 到 {} 失败，已回滚本次整理：{error}",
                    source.display(),
                    destination.display()
                ),
                Err(rollback_error) => format!(
                    "移动 {} 到 {} 失败：{error}；回滚同时失败：{rollback_error}",
                    source.display(),
                    destination.display()
                ),
            });
        }
        completed.push((source.clone(), destination.clone(), *is_dir));
    }
    Ok(())
}

fn reversed_transfers(transfers: &[(PathBuf, PathBuf, bool)]) -> Vec<(PathBuf, PathBuf, bool)> {
    transfers
        .iter()
        .map(|(source, destination, is_dir)| (destination.clone(), source.clone(), *is_dir))
        .collect()
}

fn rollback_exact_restores(restored_moves: &[DesktopOrganizeMove]) -> Result<(), String> {
    let mut failures = Vec::new();
    for item in restored_moves.iter().rev() {
        let metadata = match fs::symlink_metadata(&item.original) {
            Ok(metadata) => metadata,
            Err(error) => {
                failures.push(format!(
                    "无法检查回滚来源 {}：{error}",
                    item.original.display()
                ));
                continue;
            }
        };
        if metadata.file_type().is_symlink()
            || is_reparse_point(&metadata)
            || metadata.is_dir() != item.is_dir
            || (!metadata.is_file() && !metadata.is_dir())
        {
            failures.push(format!("回滚来源已被替换：{}", item.original.display()));
            continue;
        }
        match fs::symlink_metadata(&item.organized) {
            Ok(_) => {
                failures.push(format!("回滚目标已存在：{}", item.organized.display()));
                continue;
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                failures.push(format!(
                    "无法检查回滚目标 {}：{error}",
                    item.organized.display()
                ));
                continue;
            }
        }
        if let Err(error) = fs::rename(&item.original, &item.organized) {
            failures.push(format!(
                "无法将 {} 回滚到 {}：{error}",
                item.original.display(),
                item.organized.display()
            ));
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("；"))
    }
}

fn ensure_restore_parent(
    index: &SharedIndex,
    parent: &Path,
    created_directories: &mut Vec<PathBuf>,
) -> Result<(), String> {
    if parent == index.desktop_path {
        return verify_or_create_directory(parent, created_directories);
    }
    if parent == index.legacy_library_path {
        return verify_or_create_directory(parent, created_directories);
    }
    if parent.starts_with(&index.legacy_library_path)
        && parent.parent() == Some(index.legacy_library_path.as_path())
    {
        verify_or_create_directory(&index.legacy_library_path, created_directories)?;
        return verify_or_create_directory(parent, created_directories);
    }
    Err(format!("撤销目标不在授权目录内：{}", parent.display()))
}

fn refresh_desktop_files(
    app: &tauri::AppHandle,
    index: &SharedIndex,
) -> Result<Vec<DesktopFile>, String> {
    let files = scan_index(index, true)?;
    app.emit("files://changed", ())
        .map_err(|error| format!("无法通知桌面列表刷新：{error}"))?;
    Ok(files)
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn classify(path: &Path, is_dir: bool) -> &'static str {
    if is_dir {
        return if path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("app"))
        {
            "application"
        } else {
            "folder"
        };
    }

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match extension.as_str() {
        "doc" | "docx" | "pdf" | "ppt" | "pptx" | "xls" | "xlsx" | "txt" | "md" | "rtf" => {
            "document"
        }
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" | "heic" => "image",
        "mp4" | "mov" | "avi" | "mkv" | "webm" | "m4v" => "video",
        "mp3" | "wav" | "m4a" | "flac" | "aac" | "ogg" => "audio",
        "zip" | "rar" | "7z" | "tar" | "gz" => "archive",
        "exe" | "msi" | "bat" | "cmd" | "ps1" | "vbs" | "js" | "lnk" | "url" => "application",
        _ => "other",
    }
}

fn normalize_top_level_name(name: &OsStr) -> String {
    let display = name.to_string_lossy();
    display.trim().trim_end_matches([' ', '.']).to_lowercase()
}

fn normalize_name_key(name: &str) -> String {
    normalize_top_level_name(OsStr::new(name))
}

fn is_launchable_program_path(path: &Path) -> bool {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();

    #[cfg(windows)]
    {
        matches!(
            extension.to_ascii_lowercase().as_str(),
            "lnk" | "url" | "exe"
        )
    }

    #[cfg(target_os = "macos")]
    {
        extension.eq_ignore_ascii_case("app")
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = extension;
        false
    }
}

fn is_safe_text_script_extension(extension: &str) -> bool {
    matches!(extension, "js" | "ps1" | "bat" | "cmd" | "vbs" | "sh")
}

fn requires_execution_confirmation(extension: &str) -> bool {
    matches!(
        extension,
        "exe"
            | "com"
            | "scr"
            | "cpl"
            | "hta"
            | "jar"
            | "jnlp"
            | "pif"
            | "scf"
            | "application"
            | "gadget"
            | "reg"
            | "inf"
            | "jse"
            | "vbe"
            | "ws"
            | "wsc"
            | "wsf"
            | "wsh"
            | "msi"
            | "msp"
            | "mst"
            | "msix"
            | "msixbundle"
            | "appinstaller"
            | "dmg"
            | "pkg"
            | "app"
            | "command"
            | "workflow"
            | "lnk"
            | "url"
            | "webloc"
            | "desktop"
            | "alias"
    )
}

fn organize_item(item: &DesktopOrganizeMove) -> DesktopOrganizeItem {
    DesktopOrganizeItem {
        move_id: item.move_id.clone(),
        name: item
            .original
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| item.original.display().to_string()),
        original_path: item.original.to_string_lossy().to_string(),
        organized_path: item.organized.to_string_lossy().to_string(),
        category: item.category.clone(),
        kind: classify(&item.original, item.is_dir).to_string(),
        launchable: is_launchable_program_path(&item.original),
    }
}

fn restored_organize_item(item: &DesktopOrganizeMove, restored_path: &Path) -> DesktopOrganizeItem {
    DesktopOrganizeItem {
        move_id: item.move_id.clone(),
        name: restored_path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| restored_path.display().to_string()),
        original_path: restored_path.to_string_lossy().to_string(),
        organized_path: item.organized.to_string_lossy().to_string(),
        category: item.category.clone(),
        kind: classify(restored_path, item.is_dir).to_string(),
        launchable: is_launchable_program_path(restored_path),
    }
}

fn load_organize_exclusion_keys(index: &SharedIndex) -> Result<HashSet<String>, String> {
    let connection = index
        .db
        .lock()
        .map_err(|_| "索引数据库已锁定".to_string())?;
    let mut statement = connection
        .prepare("SELECT name_key FROM organize_exclusions")
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<HashSet<_>, _>>()
        .map_err(|error| error.to_string())
}

fn organize_skipped_item(
    name: impl Into<String>,
    path: impl Into<String>,
    reason_code: &str,
    reason: &str,
) -> DesktopOrganizeSkippedItem {
    DesktopOrganizeSkippedItem {
        name: name.into(),
        path: path.into(),
        reason_code: reason_code.to_string(),
        reason: reason.to_string(),
    }
}

#[cfg(windows)]
fn public_desktop_visible_paths() -> Vec<(String, PathBuf)> {
    let Some(public_root) = std::env::var_os("PUBLIC") else {
        return Vec::new();
    };
    let public_desktop = PathBuf::from(public_root).join("Desktop");
    let Ok(entries) = fs::read_dir(&public_desktop) else {
        return Vec::new();
    };
    let mut items = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if is_hidden_or_system(&name, &metadata) {
            continue;
        }
        items.push((name.to_string_lossy().to_string(), path));
    }
    items
}

#[cfg(not(windows))]
fn public_desktop_visible_paths() -> Vec<(String, PathBuf)> {
    Vec::new()
}

fn public_desktop_count() -> usize {
    public_desktop_visible_paths().len()
}

fn append_public_desktop_skips(
    desktop: &Path,
    skipped_items: &mut Vec<DesktopOrganizeSkippedItem>,
) {
    for (name, path) in public_desktop_visible_paths() {
        if path
            .parent()
            .is_some_and(|parent| path_reservation_key(parent) == path_reservation_key(desktop))
        {
            continue;
        }
        skipped_items.push(organize_skipped_item(
            name,
            path.to_string_lossy().to_string(),
            "public_desktop",
            "这是所有 Windows 用户共享的公共桌面项目，为避免影响其他账户，虫洞派不会移动它",
        ));
    }
}

fn collect_organize_plan(
    desktop: &Path,
    exclusion_keys: &HashSet<String>,
) -> Result<OrganizePlan, String> {
    let entries = fs::read_dir(desktop)
        .map_err(|error| format!("无法读取桌面目录 {}：{error}", desktop.display()))?;
    let mut plan = OrganizePlan {
        candidates: Vec::new(),
        categories: BTreeSet::new(),
        excluded_count: 0,
        skipped_items: Vec::new(),
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                plan.skipped_items.push(organize_skipped_item(
                    "无法读取的桌面项目",
                    desktop.to_string_lossy().to_string(),
                    "read_error",
                    &format!("无法读取该桌面项目：{error}"),
                ));
                continue;
            }
        };
        let name = entry.file_name();
        let path = entry.path();
        if name == OsStr::new(LEGACY_ORGANIZE_ROOT_NAME) {
            continue;
        }
        if is_wormhole_shortcut_name(&name) {
            plan.skipped_items.push(organize_skipped_item(
                name.to_string_lossy().to_string(),
                path.to_string_lossy().to_string(),
                "wormhole_shortcut",
                "这是虫洞派的桌面入口，会保留在桌面方便你重新打开",
            ));
            continue;
        }

        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                plan.skipped_items.push(organize_skipped_item(
                    name.to_string_lossy().to_string(),
                    path.to_string_lossy().to_string(),
                    "metadata_error",
                    &format!("无法读取项目属性：{error}"),
                ));
                continue;
            }
        };
        let file_type = metadata.file_type();
        if is_hidden_or_system(&name, &metadata) {
            plan.skipped_items.push(organize_skipped_item(
                name.to_string_lossy().to_string(),
                path.to_string_lossy().to_string(),
                "protected_system_file",
                "隐藏或系统文件已保护，不会移动",
            ));
            continue;
        }
        if file_type.is_symlink() || is_reparse_point(&metadata) {
            plan.skipped_items.push(organize_skipped_item(
                name.to_string_lossy().to_string(),
                path.to_string_lossy().to_string(),
                "protected_link_or_cloud_placeholder",
                "符号链接、重解析点或云端占位项目已保护；普通 .lnk/.url 快捷方式仍会正常整理",
            ));
            continue;
        }
        if !file_type.is_file() && !file_type.is_dir() {
            plan.skipped_items.push(organize_skipped_item(
                name.to_string_lossy().to_string(),
                path.to_string_lossy().to_string(),
                "unsupported_type",
                "该项目不是普通文件或文件夹，无法安全移动",
            ));
            continue;
        }

        let is_dir = file_type.is_dir();
        if exclusion_keys.contains(&normalize_top_level_name(&name)) {
            plan.excluded_count += 1;
            plan.skipped_items.push(organize_skipped_item(
                name.to_string_lossy().to_string(),
                path.to_string_lossy().to_string(),
                "excluded_by_user",
                "该项目在你的整理忽略名单中，会继续留在桌面",
            ));
            continue;
        }
        let category = organize_category(&path, is_dir).to_string();
        plan.categories.insert(category.clone());
        plan.candidates.push(OrganizeCandidate {
            original: path,
            name,
            category,
            is_dir,
        });
    }

    append_public_desktop_skips(desktop, &mut plan.skipped_items);
    Ok(plan)
}

fn append_legacy_library_plan(legacy_root: &Path, plan: &mut OrganizePlan) {
    let root_metadata = match fs::symlink_metadata(legacy_root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return,
        Err(error) => {
            plan.skipped_items.push(organize_skipped_item(
                LEGACY_ORGANIZE_ROOT_NAME,
                legacy_root.to_string_lossy().to_string(),
                "legacy_metadata_error",
                &format!("无法读取旧整理库：{error}"),
            ));
            return;
        }
    };
    if !root_metadata.is_dir()
        || root_metadata.file_type().is_symlink()
        || is_reparse_point(&root_metadata)
    {
        plan.skipped_items.push(organize_skipped_item(
            LEGACY_ORGANIZE_ROOT_NAME,
            legacy_root.to_string_lossy().to_string(),
            "protected_legacy_library",
            "旧整理库不是普通文件夹，已保护且不会自动迁移",
        ));
        return;
    }

    let Ok(entries) = fs::read_dir(legacy_root) else {
        return;
    };
    for entry in entries.flatten() {
        let entry_name = entry.file_name();
        let entry_path = entry.path();
        let Ok(entry_metadata) = fs::symlink_metadata(&entry_path) else {
            continue;
        };
        if !is_safe_index_entry(&entry_name, &entry_metadata) {
            plan.skipped_items.push(organize_skipped_item(
                entry_name.to_string_lossy().to_string(),
                entry_path.to_string_lossy().to_string(),
                "protected_legacy_item",
                "旧整理库中的链接、系统项或特殊项目不会迁移",
            ));
            continue;
        }

        if entry_metadata.is_dir() {
            let category = entry_name.to_string_lossy().to_string();
            let Ok(category_items) = fs::read_dir(&entry_path) else {
                continue;
            };
            for item_entry in category_items.flatten() {
                let item_name = item_entry.file_name();
                let item_path = item_entry.path();
                let Ok(item_metadata) = fs::symlink_metadata(&item_path) else {
                    continue;
                };
                if !is_safe_index_entry(&item_name, &item_metadata) {
                    plan.skipped_items.push(organize_skipped_item(
                        item_name.to_string_lossy().to_string(),
                        item_path.to_string_lossy().to_string(),
                        "protected_legacy_item",
                        "旧整理库中的链接、系统项或特殊项目不会迁移",
                    ));
                    continue;
                }
                plan.categories.insert(category.clone());
                plan.candidates.push(OrganizeCandidate {
                    original: item_path,
                    name: item_name,
                    category: category.clone(),
                    is_dir: item_metadata.is_dir(),
                });
            }
        } else {
            let category = organize_category(&entry_path, false).to_string();
            plan.categories.insert(category.clone());
            plan.candidates.push(OrganizeCandidate {
                original: entry_path,
                name: entry_name,
                category,
                is_dir: false,
            });
        }
    }
}

fn legacy_cleanup_directories(legacy_root: &Path) -> Vec<PathBuf> {
    let mut directories = vec![legacy_root.to_path_buf()];
    let Ok(entries) = fs::read_dir(legacy_root) else {
        return directories;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.is_dir() && !metadata.file_type().is_symlink() && !is_reparse_point(&metadata) {
            directories.push(path);
        }
    }
    directories
}

fn is_program_candidate(path: &Path, metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() || is_reparse_point(metadata) {
        return false;
    }

    #[cfg(windows)]
    {
        metadata.is_file() && is_launchable_program_path(path)
    }

    #[cfg(target_os = "macos")]
    {
        metadata.is_dir() && is_launchable_program_path(path)
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = (path, metadata);
        false
    }
}

fn program_name(path: &Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .map(|name| name.to_string_lossy().trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(windows)]
fn program_search_roots(index: &SharedIndex) -> Vec<ProgramSearchRoot> {
    let mut roots = Vec::new();
    if let Some(app_data) = std::env::var_os("APPDATA") {
        roots.push(ProgramSearchRoot {
            path: PathBuf::from(app_data).join("Microsoft/Windows/Start Menu/Programs"),
            source: "用户开始菜单",
            recursive: true,
        });
    }
    if let Some(program_data) = std::env::var_os("PROGRAMDATA") {
        roots.push(ProgramSearchRoot {
            path: PathBuf::from(program_data).join("Microsoft/Windows/Start Menu/Programs"),
            source: "系统开始菜单",
            recursive: true,
        });
    }
    roots.push(ProgramSearchRoot {
        path: index.desktop_path.clone(),
        source: "桌面",
        recursive: false,
    });
    if let Some(public_root) = std::env::var_os("PUBLIC") {
        roots.push(ProgramSearchRoot {
            path: PathBuf::from(public_root).join("Desktop"),
            source: "公共桌面",
            recursive: false,
        });
    }
    for (root, source) in [
        (&index.library_path, "虫洞派资料库"),
        (&index.legacy_library_path, "旧整理库"),
    ] {
        for category in ["快捷方式", "应用"] {
            roots.push(ProgramSearchRoot {
                path: root.join(category),
                source,
                recursive: false,
            });
        }
    }
    roots
}

#[cfg(target_os = "macos")]
fn program_search_roots(index: &SharedIndex) -> Vec<ProgramSearchRoot> {
    let mut roots = vec![ProgramSearchRoot {
        path: PathBuf::from("/Applications"),
        source: "应用程序",
        recursive: true,
    }];
    if let Some(home) = std::env::var_os("HOME") {
        roots.push(ProgramSearchRoot {
            path: PathBuf::from(home).join("Applications"),
            source: "用户应用程序",
            recursive: true,
        });
    }
    for (root, source) in [
        (&index.library_path, "虫洞派资料库"),
        (&index.legacy_library_path, "旧整理库"),
    ] {
        for category in ["快捷方式", "应用"] {
            roots.push(ProgramSearchRoot {
                path: root.join(category),
                source,
                recursive: false,
            });
        }
    }
    roots
}

#[cfg(not(any(windows, target_os = "macos")))]
fn program_search_roots(index: &SharedIndex) -> Vec<ProgramSearchRoot> {
    let _ = index;
    Vec::new()
}

fn path_matches_program_root(path: &Path, root: &Path, recursive: bool) -> bool {
    if recursive {
        path != root && path.starts_with(root)
    } else {
        path.parent() == Some(root)
    }
}

fn canonical_program_roots(index: &SharedIndex) -> Vec<(PathBuf, bool)> {
    program_search_roots(index)
        .into_iter()
        .filter_map(|root| {
            let metadata = fs::symlink_metadata(&root.path).ok()?;
            if !metadata.is_dir()
                || metadata.file_type().is_symlink()
                || is_reparse_point(&metadata)
            {
                return None;
            }
            root.path
                .canonicalize()
                .ok()
                .map(|path| (path, root.recursive))
        })
        .collect()
}

fn path_is_in_program_roots(path: &Path, roots: &[(PathBuf, bool)]) -> bool {
    roots
        .iter()
        .any(|(root, recursive)| path_matches_program_root(path, root, *recursive))
}

fn program_path_is_authorized(index: &SharedIndex, path: &Path) -> bool {
    path_is_in_program_roots(path, &canonical_program_roots(index))
}

fn collect_programs(
    root: &Path,
    source: &str,
    recursive: bool,
    programs: &mut BTreeMap<String, ProgramEntry>,
) {
    const MAX_VISITED_ENTRIES: usize = 12_000;
    const MAX_CANDIDATES: usize = 2_000;
    const MAX_DEPTH: usize = 12;

    let Ok(root_metadata) = fs::symlink_metadata(root) else {
        return;
    };
    if !root_metadata.is_dir()
        || root_metadata.file_type().is_symlink()
        || is_reparse_point(&root_metadata)
    {
        return;
    }

    let mut stack = vec![(root.to_path_buf(), 0usize)];
    let mut visited_entries = 0usize;
    while let Some((directory, depth)) = stack.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            visited_entries += 1;
            if visited_entries > MAX_VISITED_ENTRIES || programs.len() >= MAX_CANDIDATES {
                return;
            }

            let path = entry.path();
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                continue;
            };
            if is_hidden_or_system(&entry.file_name(), &metadata) {
                continue;
            }
            if is_program_candidate(&path, &metadata) {
                let path_text = path.to_string_lossy().to_string();
                programs
                    .entry(path_reservation_key(&path))
                    .or_insert_with(|| ProgramEntry {
                        name: program_name(&path),
                        path: path_text,
                        source: source.to_string(),
                    });
                continue;
            }
            if recursive
                && depth < MAX_DEPTH
                && metadata.is_dir()
                && !metadata.file_type().is_symlink()
                && !is_reparse_point(&metadata)
            {
                stack.push((path, depth + 1));
            }
        }
    }
}

fn parse_hide_icons_registry_output(output: &[u8]) -> Option<bool> {
    let text = String::from_utf8_lossy(output).to_ascii_lowercase();
    text.split_whitespace().find_map(|part| {
        if let Some(value) = part.strip_prefix("0x") {
            u32::from_str_radix(value, 16).ok().map(|value| value != 0)
        } else {
            None
        }
    })
}

#[cfg(windows)]
fn windows_command(program: &str) -> Command {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut command = Command::new(program);
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

#[cfg(windows)]
fn query_desktop_icons_hidden() -> Result<bool, String> {
    const KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
    let output = windows_command("reg.exe")
        .args(["query", KEY, "/v", "HideIcons"])
        .output()
        .map_err(|error| format!("无法读取桌面图标设置：{error}"))?;
    if output.status.success() {
        return parse_hide_icons_registry_output(&output.stdout)
            .ok_or_else(|| "桌面图标设置格式无法识别".to_string());
    }
    // The value is absent on a clean Windows profile, which means icons are visible.
    Ok(false)
}

#[cfg(windows)]
fn apply_desktop_icons_hidden(hidden: bool) -> Result<(), String> {
    const KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
    let value = if hidden { "1" } else { "0" };
    let status = windows_command("reg.exe")
        .args([
            "add",
            KEY,
            "/v",
            "HideIcons",
            "/t",
            "REG_DWORD",
            "/d",
            value,
            "/f",
        ])
        .status()
        .map_err(|error| format!("无法写入桌面图标设置：{error}"))?;
    if !status.success() {
        return Err("Windows 拒绝更新当前用户的桌面图标设置".to_string());
    }

    let _ = windows_command("rundll32.exe")
        .args(["user32.dll,UpdatePerUserSystemParameters", "1", "True"])
        .status();
    let _ = windows_command("ie4uinit.exe").arg("-show").status();
    Ok(())
}

#[tauri::command]
fn get_desktop_icon_state() -> Result<DesktopIconState, String> {
    #[cfg(windows)]
    {
        Ok(DesktopIconState {
            supported: true,
            hidden: query_desktop_icons_hidden()?,
            public_desktop_count: public_desktop_count(),
        })
    }
    #[cfg(not(windows))]
    {
        Ok(DesktopIconState {
            supported: false,
            hidden: false,
            public_desktop_count: 0,
        })
    }
}

#[tauri::command]
fn set_desktop_icons_hidden(hidden: bool) -> Result<DesktopIconState, String> {
    #[cfg(windows)]
    {
        apply_desktop_icons_hidden(hidden)?;
        let actual_hidden = query_desktop_icons_hidden()?;
        Ok(DesktopIconState {
            supported: true,
            hidden: actual_hidden,
            public_desktop_count: public_desktop_count(),
        })
    }
    #[cfg(not(windows))]
    {
        let _ = hidden;
        Err("当前系统不支持通过虫洞派切换桌面图标显示状态".to_string())
    }
}

fn parse_local_speech_output(
    success: bool,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<String, String> {
    let stdout = String::from_utf8_lossy(stdout)
        .trim_start_matches('\u{feff}')
        .trim()
        .to_string();
    let stderr = String::from_utf8_lossy(stderr).trim().to_string();
    if success {
        let text = stdout.strip_prefix("OK:").unwrap_or(&stdout).trim();
        if text.is_empty() {
            return Err("没有识别到清晰语音，请靠近麦克风再说一次".to_string());
        }
        return Ok(text.to_string());
    }
    if stderr.contains("ERR_RECOGNIZER_NOT_FOUND") {
        return Err("未找到已安装的 zh-CN Microsoft Speech Recognizer 8.0".to_string());
    }
    if stderr.contains("ERR_NO_SPEECH") {
        return Err("没有识别到语音，请检查麦克风后重试".to_string());
    }
    if stderr.contains("ERR_AUDIO_DEVICE") {
        return Err("无法使用默认麦克风，请检查 Windows 麦克风权限".to_string());
    }
    Err(if stderr.is_empty() {
        "本地语音识别进程异常退出".to_string()
    } else {
        format!("本地语音识别失败：{stderr}")
    })
}

#[cfg(windows)]
fn recognize_speech_windows() -> Result<String, String> {
    const SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$utf8 = New-Object System.Text.UTF8Encoding($false)
[Console]::OutputEncoding = $utf8
$OutputEncoding = $utf8
$engine = $null
try {
  Add-Type -AssemblyName System.Speech
  $installed = [System.Speech.Recognition.SpeechRecognitionEngine]::InstalledRecognizers()
  $info = $installed | Where-Object { $_.Culture.Name -eq 'zh-CN' -and $_.Name -like '*Microsoft Speech Recognizer 8.0*' } | Select-Object -First 1
  if ($null -eq $info) { $info = $installed | Where-Object { $_.Culture.Name -eq 'zh-CN' } | Select-Object -First 1 }
  if ($null -eq $info) { [Console]::Error.Write('ERR_RECOGNIZER_NOT_FOUND'); exit 21 }
  $engine = [System.Speech.Recognition.SpeechRecognitionEngine]::new($info)
  $engine.LoadGrammar([System.Speech.Recognition.DictationGrammar]::new())
  try { $engine.SetInputToDefaultAudioDevice() } catch { [Console]::Error.Write('ERR_AUDIO_DEVICE'); exit 22 }
  $result = $engine.Recognize([TimeSpan]::FromSeconds(8))
  if ($null -eq $result -or [String]::IsNullOrWhiteSpace($result.Text)) { [Console]::Error.Write('ERR_NO_SPEECH'); exit 23 }
  [Console]::Out.Write('OK:' + $result.Text.Trim())
} catch {
  [Console]::Error.Write('ERR_ENGINE:' + $_.Exception.Message)
  exit 24
} finally {
  if ($null -ne $engine) { $engine.Dispose() }
}
"#;

    let mut child = windows_command("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            SCRIPT,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("无法启动 Windows 本地语音识别：{error}"))?;
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if child
            .try_wait()
            .map_err(|error| format!("无法读取语音识别进程状态：{error}"))?
            .is_some()
        {
            let output = child
                .wait_with_output()
                .map_err(|error| format!("无法读取语音识别结果：{error}"))?;
            return parse_local_speech_output(
                output.status.success(),
                &output.stdout,
                &output.stderr,
            );
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("本地语音识别超时，请在提示后 8 秒内说完".to_string());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[tauri::command]
async fn recognize_speech_local() -> Result<String, String> {
    #[cfg(windows)]
    {
        tauri::async_runtime::spawn_blocking(recognize_speech_windows)
            .await
            .map_err(|error| format!("本地语音识别任务异常：{error}"))?
    }
    #[cfg(not(windows))]
    {
        Err("当前系统尚未配置可用的本地语音识别组件".to_string())
    }
}

fn init_database(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            CREATE TABLE IF NOT EXISTS files (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL UNIQUE COLLATE NOCASE,
                name TEXT NOT NULL,
                extension TEXT NOT NULL,
                kind TEXT NOT NULL,
                category TEXT NOT NULL DEFAULT '待整理',
                organized_category TEXT NOT NULL DEFAULT '',
                size INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL DEFAULT 0,
                modified_at INTEGER NOT NULL,
                indexed_at INTEGER NOT NULL,
                present INTEGER NOT NULL DEFAULT 1
            );
            CREATE INDEX IF NOT EXISTS idx_files_name ON files(name COLLATE NOCASE);
            CREATE INDEX IF NOT EXISTS idx_files_modified_at ON files(modified_at DESC);
            CREATE TABLE IF NOT EXISTS operation_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                action TEXT NOT NULL,
                file_id INTEGER,
                target TEXT,
                result TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS app_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS organize_exclusions (
                name_key TEXT PRIMARY KEY COLLATE NOCASE,
                display_name TEXT NOT NULL,
                is_directory INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_organize_exclusions_created_at
                ON organize_exclusions(created_at DESC);
            ",
        )
        .map_err(|error| error.to_string())?;
    let has_organized_category = {
        let mut statement = connection
            .prepare("PRAGMA table_info(files)")
            .map_err(|error| error.to_string())?;
        let columns = statement
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|error| error.to_string())?;
        let mut found = false;
        for column in columns {
            if column.map_err(|error| error.to_string())? == "organized_category" {
                found = true;
                break;
            }
        }
        found
    };
    if !has_organized_category {
        connection
            .execute(
                "ALTER TABLE files ADD COLUMN organized_category TEXT NOT NULL DEFAULT ''",
                [],
            )
            .map_err(|error| error.to_string())?;
    }
    let has_created_at = {
        let mut statement = connection
            .prepare("PRAGMA table_info(files)")
            .map_err(|error| error.to_string())?;
        let columns = statement
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|error| error.to_string())?;
        let mut found = false;
        for column in columns {
            if column.map_err(|error| error.to_string())? == "created_at" {
                found = true;
                break;
            }
        }
        found
    };
    if !has_created_at {
        connection
            .execute(
                "ALTER TABLE files ADD COLUMN created_at INTEGER NOT NULL DEFAULT 0",
                [],
            )
            .map_err(|error| error.to_string())?;
    }
    connection
        .execute(
            "CREATE INDEX IF NOT EXISTS idx_files_created_at ON files(created_at DESC)",
            [],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn serialize_organize_batches(batches: &[DesktopOrganizeBatch]) -> Result<String, String> {
    serde_json::to_string(batches).map_err(|error| format!("无法保存整理历史：{error}"))
}

fn persist_organize_batches(
    index: &SharedIndex,
    batches: &[DesktopOrganizeBatch],
) -> Result<(), String> {
    let serialized = serialize_organize_batches(batches)?;
    let connection = index
        .db
        .lock()
        .map_err(|_| "索引数据库已锁定".to_string())?;
    connection
        .execute(
            "INSERT OR REPLACE INTO app_meta(key, value) VALUES (?1, ?2)",
            params![ORGANIZE_BATCHES_META_KEY, serialized],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn load_organize_batches(connection: &Connection) -> Result<Vec<DesktopOrganizeBatch>, String> {
    let serialized = connection.query_row(
        "SELECT value FROM app_meta WHERE key = ?1",
        [ORGANIZE_BATCHES_META_KEY],
        |row| row.get::<_, String>(0),
    );
    match serialized {
        Ok(value) => {
            serde_json::from_str(&value).map_err(|error| format!("无法读取整理历史：{error}"))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(Vec::new()),
        Err(error) => Err(error.to_string()),
    }
}

fn indexed_path(path: &Path, metadata: &fs::Metadata, organized_category: &str) -> IndexedPath {
    IndexedPath {
        path: path.to_string_lossy().to_string(),
        name: path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string()),
        extension: path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase(),
        kind: classify(path, metadata.is_dir()).to_string(),
        organized_category: organized_category.to_string(),
        size: if metadata.is_file() {
            metadata.len()
        } else {
            0
        },
        created_at: metadata
            .created()
            .ok()
            .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
            .map(|value| value.as_millis() as u64)
            .unwrap_or_default(),
        modified_at: metadata
            .modified()
            .ok()
            .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
            .map(|value| value.as_millis() as u64)
            .unwrap_or_default(),
    }
}

fn is_safe_index_entry(name: &OsStr, metadata: &fs::Metadata) -> bool {
    let file_type = metadata.file_type();
    !is_hidden_or_system(name, metadata)
        && !file_type.is_symlink()
        && !is_reparse_point(metadata)
        && (file_type.is_file() || file_type.is_dir())
}

fn collect_categorized_index_paths(root: &Path, items: &mut BTreeMap<String, IndexedPath>) {
    let root_is_safe = fs::symlink_metadata(root)
        .map(|metadata| {
            metadata.is_dir() && !metadata.file_type().is_symlink() && !is_reparse_point(&metadata)
        })
        .unwrap_or(false);
    if !root_is_safe {
        return;
    }

    let Ok(categories) = fs::read_dir(root) else {
        return;
    };
    for category_entry in categories.flatten() {
        let category_name = category_entry.file_name();
        let category_path = category_entry.path();
        let Ok(category_metadata) = fs::symlink_metadata(&category_path) else {
            continue;
        };
        if !category_metadata.is_dir()
            || category_metadata.file_type().is_symlink()
            || is_reparse_point(&category_metadata)
            || is_hidden_or_system(&category_name, &category_metadata)
        {
            continue;
        }
        let organized_category = category_name.to_string_lossy().to_string();
        let Ok(category_items) = fs::read_dir(&category_path) else {
            continue;
        };
        for item_entry in category_items.flatten() {
            let item_name = item_entry.file_name();
            let item_path = item_entry.path();
            let Ok(item_metadata) = fs::symlink_metadata(&item_path) else {
                continue;
            };
            if !is_safe_index_entry(&item_name, &item_metadata) {
                continue;
            }
            items.insert(
                path_reservation_key(&item_path),
                indexed_path(&item_path, &item_metadata, &organized_category),
            );
        }
    }
}

fn collect_index_paths(
    desktop: &Path,
    library: &Path,
    legacy_library: &Path,
) -> Result<Vec<IndexedPath>, String> {
    let entries = fs::read_dir(desktop)
        .map_err(|error| format!("无法读取桌面目录 {}: {error}", desktop.display()))?;
    let mut items = BTreeMap::new();

    for entry in entries.flatten() {
        let name = entry.file_name();
        if name == OsStr::new(LEGACY_ORGANIZE_ROOT_NAME) {
            continue;
        }
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if !is_safe_index_entry(&name, &metadata) {
            continue;
        }
        items.insert(
            path_reservation_key(&path),
            indexed_path(&path, &metadata, ""),
        );
    }

    collect_categorized_index_paths(library, &mut items);
    collect_categorized_index_paths(legacy_library, &mut items);

    Ok(items.into_values().collect())
}

fn scan_index(index: &SharedIndex, mark_new: bool) -> Result<Vec<DesktopFile>, String> {
    let items = collect_index_paths(
        &index.desktop_path,
        &index.library_path,
        &index.legacy_library_path,
    )?;

    let now = now_millis();
    {
        let mut connection = index
            .db
            .lock()
            .map_err(|_| "索引数据库已锁定".to_string())?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        transaction
            .execute("UPDATE files SET present = 0", [])
            .map_err(|error| error.to_string())?;

        for item in items {
            transaction
                .execute(
                    "
                    INSERT INTO files(
                        path, name, extension, kind, category, organized_category,
                        size, created_at, modified_at, indexed_at, present
                    )
                    VALUES (?1, ?2, ?3, ?4, '待整理', ?5, ?6, ?7, ?8, ?9, 1)
                    ON CONFLICT(path) DO UPDATE SET
                        name = excluded.name,
                        extension = excluded.extension,
                        kind = excluded.kind,
                        organized_category = excluded.organized_category,
                        size = excluded.size,
                        created_at = excluded.created_at,
                        modified_at = excluded.modified_at,
                        present = 1
                    ",
                    params![
                        item.path,
                        item.name,
                        item.extension,
                        item.kind,
                        item.organized_category,
                        item.size,
                        item.created_at,
                        item.modified_at,
                        if mark_new { now } else { 0 }
                    ],
                )
                .map_err(|error| error.to_string())?;
        }

        transaction
            .execute("DELETE FROM files WHERE present = 0", [])
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())?;
    }

    list_indexed_files(index)
}

fn physical_category_for_path(index: &SharedIndex, path: &Path) -> String {
    if path.starts_with(&index.library_path) || path.starts_with(&index.legacy_library_path) {
        return path
            .parent()
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
    }
    String::new()
}

fn repath_index_transfers(
    index: &SharedIndex,
    transfers: &[(PathBuf, PathBuf, bool)],
) -> Result<(), String> {
    let mut indexed_destinations = Vec::with_capacity(transfers.len());
    for (source, destination, is_dir) in transfers {
        let metadata = fs::symlink_metadata(destination).map_err(|error| {
            format!(
                "无法读取已移动项目 {} 的属性：{error}",
                destination.display()
            )
        })?;
        if metadata.file_type().is_symlink()
            || is_reparse_point(&metadata)
            || metadata.is_dir() != *is_dir
            || (!metadata.is_file() && !metadata.is_dir())
        {
            return Err(format!(
                "已移动项目不是安全的普通文件或文件夹：{}",
                destination.display()
            ));
        }
        indexed_destinations.push((
            source.clone(),
            indexed_path(
                destination,
                &metadata,
                &physical_category_for_path(index, destination),
            ),
        ));
    }

    let mut connection = index
        .db
        .lock()
        .map_err(|_| "索引数据库已锁定".to_string())?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    for (source, destination) in indexed_destinations {
        let source_id = transaction
            .query_row(
                "SELECT id FROM files WHERE path = ?1",
                [source.to_string_lossy().to_string()],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;

        if let Some(source_id) = source_id {
            transaction
                .execute(
                    "DELETE FROM files WHERE path = ?1 AND id <> ?2",
                    params![destination.path, source_id],
                )
                .map_err(|error| error.to_string())?;
            transaction
                .execute(
                    "
                    UPDATE files SET
                        path = ?1,
                        name = ?2,
                        extension = ?3,
                        kind = ?4,
                        organized_category = ?5,
                        size = ?6,
                        created_at = ?7,
                        modified_at = ?8,
                        present = 1
                    WHERE id = ?9
                    ",
                    params![
                        destination.path,
                        destination.name,
                        destination.extension,
                        destination.kind,
                        destination.organized_category,
                        destination.size,
                        destination.created_at,
                        destination.modified_at,
                        source_id
                    ],
                )
                .map_err(|error| error.to_string())?;
        } else {
            transaction
                .execute(
                    "
                    INSERT INTO files(
                        path, name, extension, kind, category, organized_category,
                        size, created_at, modified_at, indexed_at, present
                    ) VALUES (?1, ?2, ?3, ?4, '待整理', ?5, ?6, ?7, ?8, ?9, 1)
                    ON CONFLICT(path) DO UPDATE SET
                        name = excluded.name,
                        extension = excluded.extension,
                        kind = excluded.kind,
                        organized_category = excluded.organized_category,
                        size = excluded.size,
                        created_at = excluded.created_at,
                        modified_at = excluded.modified_at,
                        present = 1
                    ",
                    params![
                        destination.path,
                        destination.name,
                        destination.extension,
                        destination.kind,
                        destination.organized_category,
                        destination.size,
                        destination.created_at,
                        destination.modified_at,
                        now_millis()
                    ],
                )
                .map_err(|error| error.to_string())?;
        }
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(())
}

fn list_indexed_files(index: &SharedIndex) -> Result<Vec<DesktopFile>, String> {
    let connection = index
        .db
        .lock()
        .map_err(|_| "索引数据库已锁定".to_string())?;
    let mut statement = connection
        .prepare(
            "
            SELECT id, name, path, extension, kind, category, organized_category,
                   size, created_at, modified_at, indexed_at
            FROM files
            WHERE present = 1
            ORDER BY modified_at DESC, name COLLATE NOCASE ASC
            ",
        )
        .map_err(|error| error.to_string())?;

    let now = now_millis();
    let rows = statement
        .query_map([], |row| {
            let indexed_at: u64 = row.get(10)?;
            Ok(DesktopFile {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                extension: row.get(3)?,
                kind: row.get(4)?,
                category: row.get(5)?,
                organized_category: row.get(6)?,
                size: row.get(7)?,
                created_at: row.get(8)?,
                modified_at: row.get(9)?,
                is_new: indexed_at > 0
                    && now.saturating_sub(indexed_at)
                        < Duration::from_secs(24 * 60 * 60).as_millis() as u64,
            })
        })
        .map_err(|error| error.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn append_log(index: &SharedIndex, action: &str, file_id: Option<i64>, target: &str, result: &str) {
    if let Ok(connection) = index.db.lock() {
        let _ = connection.execute(
            "INSERT INTO operation_log(action, file_id, target, result, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![action, file_id, target, result, now_millis()],
        );
    }
}

fn path_is_authorized(index: &SharedIndex, path: &Path) -> bool {
    path.starts_with(&index.desktop_path)
        || path.starts_with(&index.library_path)
        || path.starts_with(&index.legacy_library_path)
}

fn validate_feed_entry_flags(
    is_symlink: bool,
    is_reparse: bool,
    hidden_or_system: bool,
    is_file: bool,
    is_dir: bool,
) -> Result<(), &'static str> {
    if is_symlink || is_reparse {
        return Err("宠物不会删除符号链接、重解析点或云端占位项目");
    }
    if hidden_or_system {
        return Err("宠物不会删除隐藏或系统项目");
    }
    if !is_file && !is_dir {
        return Err("宠物只能删除普通文件或文件夹");
    }
    Ok(())
}

fn validate_feed_entry(path: &Path, metadata: &fs::Metadata) -> Result<(), String> {
    let name = path
        .file_name()
        .ok_or_else(|| "无法识别待删除项目的名称".to_string())?;
    validate_feed_entry_flags(
        metadata.file_type().is_symlink(),
        is_reparse_point(metadata),
        is_hidden_or_system(name, metadata),
        metadata.is_file(),
        metadata.is_dir(),
    )
    .map_err(str::to_string)
}

fn is_protected_feed_root(index: &SharedIndex, path: &Path, metadata: &fs::Metadata) -> bool {
    if path == index.desktop_path || path == index.library_path || path == index.legacy_library_path
    {
        return true;
    }
    metadata.is_dir()
        && [
            index.library_path.as_path(),
            index.legacy_library_path.as_path(),
        ]
        .iter()
        .any(|root| path.parent() == Some(*root))
}

fn validate_feed_path(index: &SharedIndex, raw_path: &Path) -> Result<PathBuf, String> {
    let raw_metadata = fs::symlink_metadata(raw_path)
        .map_err(|_| format!("文件不存在：{}", raw_path.display()))?;
    validate_feed_entry(raw_path, &raw_metadata)?;

    let canonical = raw_path
        .canonicalize()
        .map_err(|_| format!("文件不存在：{}", raw_path.display()))?;
    if !path_is_authorized(index, &canonical) {
        return Err("宠物目前只吃桌面或虫洞派资料库里的文件".to_string());
    }

    let canonical_metadata = fs::symlink_metadata(&canonical)
        .map_err(|_| format!("文件不存在：{}", raw_path.display()))?;
    validate_feed_entry(&canonical, &canonical_metadata)?;
    if is_protected_feed_root(index, &canonical, &canonical_metadata) {
        return Err("桌面、资料库及其分类根目录受保护，不能喂给宠物".to_string());
    }
    Ok(canonical)
}

#[tauri::command]
fn list_files(state: State<'_, AppState>) -> Result<Vec<DesktopFile>, String> {
    list_indexed_files(&state.index)
}

#[tauri::command]
fn scan_desktop(state: State<'_, AppState>) -> Result<Vec<DesktopFile>, String> {
    let _operation_guard = state
        .organize_lock
        .lock()
        .map_err(|_| "桌面整理操作已锁定".to_string())?;
    scan_index(&state.index, true)
}

#[tauri::command]
fn get_organize_state(state: State<'_, AppState>) -> Result<OrganizeState, String> {
    let batches = state
        .organize_batches
        .lock()
        .map_err(|_| "桌面整理历史已锁定".to_string())?;
    Ok(OrganizeState {
        can_undo: !batches.is_empty(),
        batch_count: batches.len(),
    })
}

#[tauri::command]
fn organize_desktop(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<DesktopOrganizeResult, String> {
    let _operation_guard = state
        .organize_lock
        .lock()
        .map_err(|_| "桌面整理操作已锁定".to_string())?;
    let mut batches = state
        .organize_batches
        .lock()
        .map_err(|_| "桌面整理历史已锁定".to_string())?;

    let desktop = state
        .index
        .desktop_path
        .canonicalize()
        .map_err(|error| format!("桌面目录不可访问：{error}"))?;
    let organize_root = state.index.library_path.clone();
    let legacy_root = state.index.legacy_library_path.clone();
    let exclusion_keys = load_organize_exclusion_keys(&state.index)?;
    let mut plan = collect_organize_plan(&desktop, &exclusion_keys)?;
    append_legacy_library_plan(&legacy_root, &mut plan);
    let skipped_count = plan.skipped_items.len();
    let public_desktop_count = public_desktop_count();

    if plan.candidates.is_empty() {
        let legacy_directories = legacy_cleanup_directories(&legacy_root);
        let _ = remove_created_directories(&legacy_directories);
        let indexed_count = scan_index(&state.index, true)?.len();
        append_log(
            &state.index,
            "organize_desktop",
            None,
            &organize_root.to_string_lossy(),
            "success:0",
        );
        return Ok(DesktopOrganizeResult {
            moved_count: 0,
            new_moved_count: 0,
            migrated_count: 0,
            category_count: 0,
            skipped_count,
            root_path: organize_root.to_string_lossy().to_string(),
            batch_id: None,
            items: Vec::new(),
            excluded_count: plan.excluded_count,
            skipped_items: plan.skipped_items,
            indexed_count,
            public_desktop_count,
        });
    }

    let mut created_directories = Vec::new();
    if let Err(error) = verify_or_create_directory(&organize_root, &mut created_directories) {
        let _ = remove_created_directories(&created_directories);
        return Err(error);
    }
    for category in &plan.categories {
        if let Err(error) =
            verify_or_create_directory(&organize_root.join(category), &mut created_directories)
        {
            let cleanup_error = remove_created_directories(&created_directories).err();
            return Err(match cleanup_error {
                Some(cleanup_error) => format!("{error}；清理失败：{cleanup_error}"),
                None => error,
            });
        }
    }

    let mut reserved = HashSet::new();
    let mut moves = Vec::with_capacity(plan.candidates.len());
    let batch_id = format!("organize-{}-{}", now_millis(), std::process::id());
    for (move_index, candidate) in plan.candidates.into_iter().enumerate() {
        let organized = match unique_destination(
            &organize_root.join(&candidate.category),
            &candidate.name,
            candidate.is_dir,
            &mut reserved,
        ) {
            Ok(path) => path,
            Err(error) => {
                let cleanup_error = remove_created_directories(&created_directories).err();
                return Err(match cleanup_error {
                    Some(cleanup_error) => format!("{error}；清理失败：{cleanup_error}"),
                    None => error,
                });
            }
        };
        moves.push(DesktopOrganizeMove {
            move_id: format!("{batch_id}-{move_index}"),
            original: candidate.original,
            organized,
            category: candidate.category,
            is_dir: candidate.is_dir,
        });
    }

    let transfers = moves
        .iter()
        .map(|item| (item.original.clone(), item.organized.clone(), item.is_dir))
        .collect::<Vec<_>>();
    if let Err(error) = execute_transfers(&transfers) {
        let cleanup_error = remove_created_directories(&created_directories).err();
        let _ = refresh_desktop_files(&app, &state.index);
        return Err(match cleanup_error {
            Some(cleanup_error) => format!("{error}；清理失败：{cleanup_error}"),
            None => error,
        });
    }

    if let Err(error) = repath_index_transfers(&state.index, &transfers) {
        let rollback_error = rollback_transfers(&transfers).err();
        let cleanup_error = remove_created_directories(&created_directories).err();
        let _ = refresh_desktop_files(&app, &state.index);
        return Err(format!(
            "整理后的索引路径更新失败：{error}{}{}",
            rollback_error
                .map(|value| format!("；文件回滚失败：{value}"))
                .unwrap_or_default(),
            cleanup_error
                .map(|value| format!("；新库清理失败：{value}"))
                .unwrap_or_default()
        ));
    }
    let reverse_index_transfers = reversed_transfers(&transfers);

    let legacy_directories = legacy_cleanup_directories(&legacy_root);
    let removed_legacy_directories = match remove_created_directories(&legacy_directories) {
        Ok(removed) => removed,
        Err(error) => {
            let rollback_error = rollback_transfers(&transfers).err();
            let index_rollback_error = if rollback_error.is_none() {
                repath_index_transfers(&state.index, &reverse_index_transfers).err()
            } else {
                None
            };
            let cleanup_error = remove_created_directories(&created_directories).err();
            let _ = refresh_desktop_files(&app, &state.index);
            return Err(format!(
                "旧整理库清理失败：{error}{}{}{}",
                rollback_error
                    .map(|value| format!("；文件回滚失败：{value}"))
                    .unwrap_or_default(),
                index_rollback_error
                    .map(|value| format!("；索引回滚失败：{value}"))
                    .unwrap_or_default(),
                cleanup_error
                    .map(|value| format!("；新库清理失败：{value}"))
                    .unwrap_or_default()
            ));
        }
    };

    let batch = DesktopOrganizeBatch {
        batch_id: batch_id.clone(),
        root: organize_root.clone(),
        moves: moves.clone(),
        created_directories: created_directories.clone(),
    };
    let previous_batches = batches.clone();
    let mut next_batches = previous_batches.clone();
    next_batches.push(batch);
    if let Err(error) = persist_organize_batches(&state.index, &next_batches) {
        let recreate_error = recreate_removed_directories(&removed_legacy_directories).err();
        let rollback_error = rollback_transfers(&transfers).err();
        let index_rollback_error = if rollback_error.is_none() {
            repath_index_transfers(&state.index, &reverse_index_transfers).err()
        } else {
            None
        };
        let cleanup_error = remove_created_directories(&created_directories).err();
        let _ = refresh_desktop_files(&app, &state.index);
        return Err(format!(
            "整理历史保存失败：{error}{}{}{}{}",
            recreate_error
                .map(|value| format!("；旧库目录恢复失败：{value}"))
                .unwrap_or_default(),
            rollback_error
                .map(|value| format!("；文件回滚失败：{value}"))
                .unwrap_or_default(),
            index_rollback_error
                .map(|value| format!("；索引回滚失败：{value}"))
                .unwrap_or_default(),
            cleanup_error
                .map(|value| format!("；新库清理失败：{value}"))
                .unwrap_or_default()
        ));
    }
    *batches = next_batches;

    let indexed_files = match refresh_desktop_files(&app, &state.index) {
        Ok(files) => files,
        Err(error) => {
            let history_error = persist_organize_batches(&state.index, &previous_batches).err();
            *batches = previous_batches;
            let recreate_error = recreate_removed_directories(&removed_legacy_directories).err();
            let rollback_error = rollback_transfers(&transfers).err();
            let index_rollback_error = if rollback_error.is_none() {
                repath_index_transfers(&state.index, &reverse_index_transfers).err()
            } else {
                None
            };
            let cleanup_error = remove_created_directories(&created_directories).err();
            let _ = refresh_desktop_files(&app, &state.index);
            return Err(format!(
                "整理后的索引刷新失败：{error}{}{}{}{}{}",
                history_error
                    .map(|value| format!("；整理历史回滚失败：{value}"))
                    .unwrap_or_default(),
                recreate_error
                    .map(|value| format!("；旧库目录恢复失败：{value}"))
                    .unwrap_or_default(),
                rollback_error
                    .map(|value| format!("；文件回滚失败：{value}"))
                    .unwrap_or_default(),
                index_rollback_error
                    .map(|value| format!("；索引回滚失败：{value}"))
                    .unwrap_or_default(),
                cleanup_error
                    .map(|value| format!("；新库清理失败：{value}"))
                    .unwrap_or_default()
            ));
        }
    };

    let moved_count = moves.len();
    let new_moved_count = moves
        .iter()
        .filter(|item| item.original.parent() == Some(desktop.as_path()))
        .count();
    let migrated_count = moved_count.saturating_sub(new_moved_count);
    let category_count = plan.categories.len();
    let items = moves
        .iter()
        .filter(|item| item.original.parent() == Some(desktop.as_path()))
        .map(organize_item)
        .collect();
    append_log(
        &state.index,
        "organize_desktop",
        None,
        &organize_root.to_string_lossy(),
        &format!("success:{moved_count}"),
    );

    Ok(DesktopOrganizeResult {
        moved_count,
        new_moved_count,
        migrated_count,
        category_count,
        skipped_count,
        root_path: organize_root.to_string_lossy().to_string(),
        batch_id: Some(batch_id),
        items,
        excluded_count: plan.excluded_count,
        skipped_items: plan.skipped_items,
        indexed_count: indexed_files.len(),
        public_desktop_count,
    })
}

#[tauri::command]
fn review_desktop_organize(
    batch_id: String,
    excluded_move_ids: Vec<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<DesktopOrganizeReviewResult, String> {
    let _operation_guard = state
        .organize_lock
        .lock()
        .map_err(|_| "桌面整理操作已锁定".to_string())?;
    let mut batches = state
        .organize_batches
        .lock()
        .map_err(|_| "桌面整理历史已锁定".to_string())?;
    let batch_index = batches
        .iter()
        .position(|batch| batch.batch_id == batch_id)
        .ok_or_else(|| "整理批次不存在或已经撤销".to_string())?;
    let mut batch = batches[batch_index].clone();

    let desktop = state
        .index
        .desktop_path
        .canonicalize()
        .map_err(|error| format!("桌面目录不可访问：{error}"))?;
    let expected_root = state.index.library_path.clone();
    if batch.root != expected_root {
        return Err("整理记录的根目录与当前桌面不一致，已拒绝复核".to_string());
    }
    validate_created_directories(&batch.root, &batch.created_directories)?;

    let requested_ids = excluded_move_ids.into_iter().collect::<HashSet<_>>();
    if requested_ids.is_empty() {
        return Ok(DesktopOrganizeReviewResult {
            restored_count: 0,
            remembered_count: 0,
            remaining_undo_count: batch.moves.len(),
            conflict_count: 0,
            restored_move_ids: Vec::new(),
        });
    }
    let known_ids = batch
        .moves
        .iter()
        .map(|item| item.move_id.as_str())
        .collect::<HashSet<_>>();
    if requested_ids
        .iter()
        .any(|move_id| !known_ids.contains(move_id.as_str()))
    {
        return Err("排除选择包含不属于当前整理批次的项目".to_string());
    }

    let mut candidate_rules = BTreeMap::<String, (String, String, bool)>::new();
    for item in batch
        .moves
        .iter()
        .filter(|item| requested_ids.contains(&item.move_id))
    {
        if item.original.parent() != Some(desktop.as_path())
            || !item.organized.starts_with(&batch.root)
        {
            return Err("整理记录包含授权目录之外的项目，已拒绝复核".to_string());
        }
        let name = item
            .original
            .file_name()
            .ok_or_else(|| "整理记录中的原始项目缺少文件名".to_string())?;
        let name_key = normalize_top_level_name(name);
        if name_key.is_empty() {
            return Err("无法为排除项生成安全名称".to_string());
        }
        candidate_rules.insert(
            item.move_id.clone(),
            (name_key, name.to_string_lossy().to_string(), item.is_dir),
        );
    }

    let mut restored_ids = HashSet::new();
    let mut restored_moves = Vec::new();
    let mut restored_rules = BTreeMap::<String, (String, bool)>::new();
    let mut conflict_count = 0;
    for item in batch
        .moves
        .iter()
        .filter(|item| requested_ids.contains(&item.move_id))
    {
        let metadata = match fs::symlink_metadata(&item.organized) {
            Ok(metadata) => metadata,
            Err(_) => {
                conflict_count += 1;
                continue;
            }
        };
        if metadata.file_type().is_symlink()
            || is_reparse_point(&metadata)
            || metadata.is_dir() != item.is_dir
            || (!metadata.is_file() && !metadata.is_dir())
        {
            conflict_count += 1;
            continue;
        }
        let Some(category_directory) = item.organized.parent() else {
            conflict_count += 1;
            continue;
        };
        let category_is_safe = fs::symlink_metadata(category_directory)
            .map(|metadata| {
                metadata.is_dir()
                    && !metadata.file_type().is_symlink()
                    && !is_reparse_point(&metadata)
            })
            .unwrap_or(false);
        if !category_is_safe || fs::symlink_metadata(&item.original).is_ok() {
            conflict_count += 1;
            continue;
        }
        match fs::rename(&item.organized, &item.original) {
            Ok(()) => {
                restored_ids.insert(item.move_id.clone());
                restored_moves.push(item.clone());
                if let Some((name_key, display_name, is_directory)) =
                    candidate_rules.get(&item.move_id)
                {
                    restored_rules.insert(name_key.clone(), (display_name.clone(), *is_directory));
                }
            }
            Err(_) => conflict_count += 1,
        }
    }

    batch
        .moves
        .retain(|item| !restored_ids.contains(&item.move_id));
    let remaining_undo_count = batch.moves.len();
    let mut next_batches = batches.clone();
    if remaining_undo_count == 0 {
        next_batches.remove(batch_index);
    } else {
        next_batches[batch_index] = batch.clone();
    }
    let serialized_batches = match serialize_organize_batches(&next_batches) {
        Ok(serialized) => serialized,
        Err(error) => {
            let rollback_error = rollback_exact_restores(&restored_moves).err();
            let _ = refresh_desktop_files(&app, &state.index);
            return Err(format!(
                "整理历史序列化失败：{error}{}",
                rollback_error
                    .map(|value| format!("；文件回滚失败：{value}"))
                    .unwrap_or_default()
            ));
        }
    };
    let restored_transfers = restored_moves
        .iter()
        .map(|item| (item.organized.clone(), item.original.clone(), item.is_dir))
        .collect::<Vec<_>>();
    if let Err(index_error) = repath_index_transfers(&state.index, &restored_transfers) {
        let rollback_error = rollback_exact_restores(&restored_moves).err();
        let _ = refresh_desktop_files(&app, &state.index);
        return Err(format!(
            "恢复后的索引路径更新失败：{index_error}{}",
            rollback_error
                .map(|value| format!("；文件回滚失败：{value}"))
                .unwrap_or_default()
        ));
    }
    let reverse_restored_index = reversed_transfers(&restored_transfers);

    let database_result = if restored_rules.is_empty() {
        Ok(0usize)
    } else {
        (|| -> Result<usize, String> {
            let mut connection = state
                .index
                .db
                .lock()
                .map_err(|_| "索引数据库已锁定".to_string())?;
            let transaction = connection
                .transaction()
                .map_err(|error| error.to_string())?;
            for (name_key, (display_name, is_directory)) in &restored_rules {
                transaction
                    .execute(
                        "
                        INSERT INTO organize_exclusions(name_key, display_name, is_directory, created_at)
                        VALUES (?1, ?2, ?3, ?4)
                        ON CONFLICT(name_key) DO UPDATE SET
                            display_name = excluded.display_name,
                            is_directory = excluded.is_directory
                        ",
                        params![
                            name_key,
                            display_name,
                            if *is_directory { 1 } else { 0 },
                            now_millis()
                        ],
                    )
                    .map_err(|error| error.to_string())?;
            }
            transaction
                .execute(
                    "INSERT OR REPLACE INTO app_meta(key, value) VALUES (?1, ?2)",
                    params![ORGANIZE_BATCHES_META_KEY, serialized_batches],
                )
                .map_err(|error| error.to_string())?;
            transaction.commit().map_err(|error| error.to_string())?;
            Ok(restored_rules.len())
        })()
    };

    let remembered_count = match database_result {
        Ok(count) => count,
        Err(database_error) => {
            let rollback_error = rollback_exact_restores(&restored_moves).err();
            let index_rollback_error = if rollback_error.is_none() {
                repath_index_transfers(&state.index, &reverse_restored_index).err()
            } else {
                None
            };
            let _ = refresh_desktop_files(&app, &state.index);
            return Err(format!(
                "保存忽略规则失败：{database_error}{}{}",
                rollback_error
                    .map(|value| format!("；文件回滚失败：{value}"))
                    .unwrap_or_else(|| "；已恢复整理状态".to_string()),
                index_rollback_error
                    .map(|value| format!("；索引回滚失败：{value}"))
                    .unwrap_or_default()
            ));
        }
    };

    let restored_count = restored_ids.len();
    let mut restored_move_ids = restored_ids.into_iter().collect::<Vec<_>>();
    restored_move_ids.sort();
    *batches = next_batches;

    if let Err(error) = remove_created_directories(&batch.created_directories) {
        append_log(
            &state.index,
            "review_desktop_organize_cleanup",
            None,
            &batch.root.to_string_lossy(),
            &format!("failed:{error}"),
        );
    }
    let refresh_error = if restored_count > 0 {
        refresh_desktop_files(&app, &state.index).err()
    } else {
        None
    };
    if let Some(error) = &refresh_error {
        append_log(
            &state.index,
            "review_desktop_organize_refresh",
            None,
            &batch.root.to_string_lossy(),
            &format!("failed:{error}"),
        );
    }
    append_log(
        &state.index,
        "review_desktop_organize",
        None,
        &batch.root.to_string_lossy(),
        &format!(
            "restored:{restored_count},remembered:{remembered_count},conflicts:{conflict_count},refresh:{}",
            if refresh_error.is_some() { "failed" } else { "ok" }
        ),
    );

    Ok(DesktopOrganizeReviewResult {
        restored_count,
        remembered_count,
        remaining_undo_count,
        conflict_count,
        restored_move_ids,
    })
}

#[tauri::command]
fn list_organize_exclusions(state: State<'_, AppState>) -> Result<Vec<OrganizeExclusion>, String> {
    let connection = state
        .index
        .db
        .lock()
        .map_err(|_| "索引数据库已锁定".to_string())?;
    let mut statement = connection
        .prepare(
            "
            SELECT name_key, display_name, is_directory, created_at
            FROM organize_exclusions
            ORDER BY created_at DESC, display_name COLLATE NOCASE ASC
            ",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(OrganizeExclusion {
                name_key: row.get(0)?,
                display_name: row.get(1)?,
                is_directory: row.get::<_, i64>(2)? != 0,
                created_at: row.get(3)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn remove_organize_exclusion(name_key: String, state: State<'_, AppState>) -> Result<(), String> {
    let normalized = normalize_name_key(&name_key);
    if normalized.is_empty() {
        return Err("排除项名称不能为空".to_string());
    }
    let connection = state
        .index
        .db
        .lock()
        .map_err(|_| "索引数据库已锁定".to_string())?;
    connection
        .execute(
            "DELETE FROM organize_exclusions WHERE name_key = ?1",
            [normalized],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn undo_desktop_organize(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<DesktopOrganizeResult, String> {
    let _operation_guard = state
        .organize_lock
        .lock()
        .map_err(|_| "桌面整理操作已锁定".to_string())?;
    let mut batches = state
        .organize_batches
        .lock()
        .map_err(|_| "桌面整理历史已锁定".to_string())?;
    let batch = batches
        .last()
        .cloned()
        .ok_or_else(|| "没有可以撤销的桌面整理记录".to_string())?;

    let expected_root = state.index.library_path.clone();
    if batch.root != expected_root {
        return Err("整理记录的根目录与当前资料库不一致，已拒绝撤销".to_string());
    }
    validate_created_directories(&batch.root, &batch.created_directories)?;
    match fs::symlink_metadata(&batch.root) {
        Ok(metadata)
            if !metadata.is_dir()
                || metadata.file_type().is_symlink()
                || is_reparse_point(&metadata) =>
        {
            return Err("整理根目录已被替换为非普通文件夹，已拒绝撤销".to_string())
        }
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(format!("无法检查整理根目录：{error}")),
    }

    let mut reserved = HashSet::new();
    let mut restore_transfers = Vec::new();
    let mut restored_items = Vec::new();
    let mut restored_categories = HashSet::new();
    let mut skipped_items = Vec::new();
    let mut created_restore_directories = Vec::new();

    for item in &batch.moves {
        if !item.organized.starts_with(&batch.root) {
            return Err("整理记录包含根目录之外的项目，已拒绝撤销".to_string());
        }

        let metadata = match fs::symlink_metadata(&item.organized) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                skipped_items.push(organize_skipped_item(
                    item.original
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                        .unwrap_or_else(|| item.original.display().to_string()),
                    item.organized.to_string_lossy().to_string(),
                    "undo_source_missing",
                    "已整理项目已被移动或删除，本次撤销无法找到它",
                ));
                continue;
            }
            Err(error) => {
                return Err(format!(
                    "无法检查待撤销项目 {}：{error}",
                    item.organized.display()
                ))
            }
        };
        if metadata.file_type().is_symlink()
            || is_reparse_point(&metadata)
            || (!metadata.is_file() && !metadata.is_dir())
        {
            skipped_items.push(organize_skipped_item(
                item.original
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| item.original.display().to_string()),
                item.organized.to_string_lossy().to_string(),
                "undo_source_unsafe",
                "已整理项目已变成链接或特殊项目，已保护且不会移动",
            ));
            continue;
        }
        let Some(category_directory) = item.organized.parent() else {
            return Err("整理记录中的项目缺少分类目录".to_string());
        };
        let category_metadata = fs::symlink_metadata(category_directory).map_err(|error| {
            format!("无法检查分类目录 {}：{error}", category_directory.display())
        })?;
        if !category_metadata.is_dir()
            || category_metadata.file_type().is_symlink()
            || is_reparse_point(&category_metadata)
        {
            return Err("分类目录已被替换为非普通文件夹，已拒绝撤销".to_string());
        }

        let name = item
            .original
            .file_name()
            .ok_or_else(|| "整理记录中的原始项目缺少文件名".to_string())?;
        let restore_parent = item
            .original
            .parent()
            .ok_or_else(|| "整理记录中的原始项目缺少父目录".to_string())?;
        if let Err(error) = ensure_restore_parent(
            &state.index,
            restore_parent,
            &mut created_restore_directories,
        ) {
            let _ = remove_created_directories(&created_restore_directories);
            return Err(error);
        }
        let restored = match unique_destination(restore_parent, name, item.is_dir, &mut reserved) {
            Ok(path) => path,
            Err(error) => {
                let _ = remove_created_directories(&created_restore_directories);
                return Err(error);
            }
        };
        restored_items.push(restored_organize_item(item, &restored));
        restore_transfers.push((item.organized.clone(), restored, item.is_dir));
        restored_categories.insert(item.category.clone());
    }

    if let Err(error) = execute_transfers(&restore_transfers) {
        let _ = remove_created_directories(&created_restore_directories);
        let _ = refresh_desktop_files(&app, &state.index);
        return Err(error);
    }

    if let Err(error) = repath_index_transfers(&state.index, &restore_transfers) {
        let rollback_error = rollback_transfers(&restore_transfers).err();
        let cleanup_error = remove_created_directories(&created_restore_directories).err();
        let _ = refresh_desktop_files(&app, &state.index);
        return Err(format!(
            "撤销后的索引路径更新失败：{error}{}{}",
            rollback_error
                .map(|value| format!("；文件回滚失败：{value}"))
                .unwrap_or_default(),
            cleanup_error
                .map(|value| format!("；恢复目录清理失败：{value}"))
                .unwrap_or_default()
        ));
    }
    let reverse_index_transfers = reversed_transfers(&restore_transfers);

    let removed_directories = match remove_created_directories(&batch.created_directories) {
        Ok(removed) => removed,
        Err(error) => {
            let rollback_error = rollback_transfers(&restore_transfers).err();
            let index_rollback_error = if rollback_error.is_none() {
                repath_index_transfers(&state.index, &reverse_index_transfers).err()
            } else {
                None
            };
            let cleanup_error = remove_created_directories(&created_restore_directories).err();
            let _ = refresh_desktop_files(&app, &state.index);
            return Err(format!(
                "撤销后的目录清理失败：{error}{}{}{}",
                rollback_error
                    .map(|value| format!("；回滚失败：{value}"))
                    .unwrap_or_default(),
                index_rollback_error
                    .map(|value| format!("；索引回滚失败：{value}"))
                    .unwrap_or_default(),
                cleanup_error
                    .map(|value| format!("；恢复目录清理失败：{value}"))
                    .unwrap_or_default()
            ));
        }
    };

    let previous_batches = batches.clone();
    let mut next_batches = previous_batches.clone();
    next_batches.pop();
    if let Err(error) = persist_organize_batches(&state.index, &next_batches) {
        let recreate_error = recreate_removed_directories(&removed_directories).err();
        let rollback_error = rollback_transfers(&restore_transfers).err();
        let index_rollback_error = if rollback_error.is_none() {
            repath_index_transfers(&state.index, &reverse_index_transfers).err()
        } else {
            None
        };
        let cleanup_error = remove_created_directories(&created_restore_directories).err();
        let _ = refresh_desktop_files(&app, &state.index);
        return Err(format!(
            "撤销历史保存失败：{error}{}{}{}{}",
            recreate_error
                .map(|value| format!("；资料库目录恢复失败：{value}"))
                .unwrap_or_default(),
            rollback_error
                .map(|value| format!("；文件回滚失败：{value}"))
                .unwrap_or_default(),
            index_rollback_error
                .map(|value| format!("；索引回滚失败：{value}"))
                .unwrap_or_default(),
            cleanup_error
                .map(|value| format!("；恢复目录清理失败：{value}"))
                .unwrap_or_default()
        ));
    }
    *batches = next_batches;

    let indexed_files = match refresh_desktop_files(&app, &state.index) {
        Ok(files) => files,
        Err(error) => {
            let history_error = persist_organize_batches(&state.index, &previous_batches).err();
            *batches = previous_batches;
            let recreate_error = recreate_removed_directories(&removed_directories).err();
            let rollback_error = rollback_transfers(&restore_transfers).err();
            let index_rollback_error = if rollback_error.is_none() {
                repath_index_transfers(&state.index, &reverse_index_transfers).err()
            } else {
                None
            };
            let cleanup_error = remove_created_directories(&created_restore_directories).err();
            let _ = refresh_desktop_files(&app, &state.index);
            return Err(format!(
                "撤销后的索引刷新失败：{error}{}{}{}{}{}",
                history_error
                    .map(|value| format!("；撤销历史回滚失败：{value}"))
                    .unwrap_or_default(),
                recreate_error
                    .map(|value| format!("；资料库目录恢复失败：{value}"))
                    .unwrap_or_default(),
                rollback_error
                    .map(|value| format!("；文件回滚失败：{value}"))
                    .unwrap_or_default(),
                index_rollback_error
                    .map(|value| format!("；索引回滚失败：{value}"))
                    .unwrap_or_default(),
                cleanup_error
                    .map(|value| format!("；恢复目录清理失败：{value}"))
                    .unwrap_or_default()
            ));
        }
    };

    let moved_count = restore_transfers.len();
    let category_count = restored_categories.len();
    let skipped_count = skipped_items.len();
    append_log(
        &state.index,
        "undo_desktop_organize",
        None,
        &batch.root.to_string_lossy(),
        &format!("success:{moved_count}"),
    );

    Ok(DesktopOrganizeResult {
        moved_count,
        new_moved_count: moved_count,
        migrated_count: 0,
        category_count,
        skipped_count,
        root_path: batch.root.to_string_lossy().to_string(),
        batch_id: Some(batch.batch_id.clone()),
        items: restored_items,
        excluded_count: 0,
        skipped_items,
        indexed_count: indexed_files.len(),
        public_desktop_count: public_desktop_count(),
    })
}

#[tauri::command]
fn update_file_category(
    file_id: i64,
    category: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    const ALLOWED: [&str; 4] = ["待整理", "工作项目", "小红书素材", "个人资料"];
    if !ALLOWED.contains(&category.as_str()) {
        return Err("不支持的分类".to_string());
    }
    let connection = state
        .index
        .db
        .lock()
        .map_err(|_| "索引数据库已锁定".to_string())?;
    let changed = connection
        .execute(
            "UPDATE files SET category = ?1 WHERE id = ?2",
            params![category, file_id],
        )
        .map_err(|error| error.to_string())?;
    if changed == 1 {
        Ok(())
    } else {
        Err("文件已经不在索引中，请刷新后重试".to_string())
    }
}

#[tauri::command]
fn open_file(file_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let path: String = {
        let connection = state
            .index
            .db
            .lock()
            .map_err(|_| "索引数据库已锁定".to_string())?;
        connection
            .query_row(
                "SELECT path FROM files WHERE id = ?1 AND present = 1",
                [file_id],
                |row| row.get(0),
            )
            .map_err(|_| "文件不在索引中".to_string())?
    };

    let requested = PathBuf::from(&path);
    let canonical_file = requested
        .canonicalize()
        .map_err(|_| "文件不存在或已被移动".to_string())?;
    if !path_is_authorized(&state.index, &canonical_file) {
        append_log(
            &state.index,
            "open_file",
            Some(file_id),
            &path,
            "denied_outside_root",
        );
        return Err("拒绝打开授权目录之外的文件".to_string());
    }

    let extension = canonical_file
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if is_safe_text_script_extension(&extension) {
        #[cfg(windows)]
        {
            Command::new("notepad.exe")
                .arg(&canonical_file)
                .spawn()
                .map_err(|error| format!("无法用记事本查看脚本：{error}"))?;
        }
        #[cfg(target_os = "macos")]
        {
            Command::new("open")
                .args(["-a", "TextEdit"])
                .arg(&canonical_file)
                .spawn()
                .map_err(|error| format!("无法用 TextEdit 查看脚本：{error}"))?;
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            return Err("当前系统尚未配置安全的脚本文本查看器".to_string());
        }
        append_log(
            &state.index,
            "open_file_as_text",
            Some(file_id),
            &path,
            "success",
        );
        return Ok(());
    }

    if requires_execution_confirmation(&extension) {
        append_log(
            &state.index,
            "open_file",
            Some(file_id),
            &path,
            "confirmation_required",
        );
        return Err("可执行文件需要二次确认，当前 MVP 不会直接运行".to_string());
    }

    open::that(&canonical_file).map_err(|error| {
        append_log(&state.index, "open_file", Some(file_id), &path, "failed");
        error.to_string()
    })?;
    append_log(&state.index, "open_file", Some(file_id), &path, "success");
    Ok(())
}

#[tauri::command]
fn list_programs(state: State<'_, AppState>) -> Result<Vec<ProgramEntry>, String> {
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = state;
        Err("当前系统暂不支持常用程序扫描".to_string())
    }

    #[cfg(any(windows, target_os = "macos"))]
    {
        let mut programs = BTreeMap::new();
        for root in program_search_roots(&state.index) {
            collect_programs(&root.path, root.source, root.recursive, &mut programs);
        }
        let mut result = programs.into_values().collect::<Vec<_>>();
        result.sort_by(|left, right| {
            left.name
                .to_lowercase()
                .cmp(&right.name.to_lowercase())
                .then_with(|| left.source.cmp(&right.source))
                .then_with(|| left.path.cmp(&right.path))
        });
        result.truncate(80);
        Ok(result)
    }
}

#[tauri::command]
fn launch_program(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut requested = PathBuf::from(&path);
    if !requested.is_absolute() {
        return Err("程序路径必须是绝对路径".to_string());
    }
    if !requested.exists() {
        if let Ok(batches) = state.organize_batches.lock() {
            if let Some(migrated) = batches
                .iter()
                .flat_map(|batch| batch.moves.iter())
                .find(|item| item.original == requested && item.organized.exists())
                .map(|item| item.organized.clone())
            {
                requested = migrated;
            }
        }
    }
    if !requested.exists() {
        if let Ok(relative) = requested.strip_prefix(&state.index.legacy_library_path) {
            if relative
                .components()
                .all(|component| matches!(component, Component::Normal(_)))
            {
                let migrated = state.index.library_path.join(relative);
                if migrated.exists() {
                    requested = migrated;
                }
            }
        }
    }
    let requested_metadata =
        fs::symlink_metadata(&requested).map_err(|_| "程序不存在或已被移动".to_string())?;
    if !is_program_candidate(&requested, &requested_metadata) {
        return Err("仅允许打开已存在的 .lnk、.url、.exe 或 .app 程序".to_string());
    }
    let canonical = requested
        .canonicalize()
        .map_err(|_| "程序不存在或已被移动".to_string())?;
    let metadata = fs::symlink_metadata(&canonical).map_err(|_| "无法读取程序信息".to_string())?;
    if !is_program_candidate(&canonical, &metadata) {
        append_log(
            &state.index,
            "launch_program",
            None,
            &path,
            "denied_invalid_program",
        );
        return Err("仅允许打开已存在的 .lnk、.url、.exe 或 .app 程序".to_string());
    }
    if !program_path_is_authorized(&state.index, &canonical) {
        append_log(
            &state.index,
            "launch_program",
            None,
            &path,
            "denied_outside_program_roots",
        );
        return Err("拒绝启动常用程序目录之外的文件".to_string());
    }

    open::that(&canonical).map_err(|error| {
        append_log(&state.index, "launch_program", None, &path, "failed");
        error.to_string()
    })?;
    append_log(&state.index, "launch_program", None, &path, "success");
    Ok(())
}

fn agent_candidate_key(path: &Path) -> String {
    #[cfg(windows)]
    {
        path.to_string_lossy().to_lowercase()
    }
    #[cfg(not(windows))]
    {
        path.to_string_lossy().to_string()
    }
}

fn push_agent_candidate(
    candidates: &mut Vec<PathBuf>,
    seen: &mut BTreeSet<String>,
    candidate: PathBuf,
) {
    let Ok(metadata) = fs::metadata(&candidate) else {
        return;
    };
    if !metadata.is_file() {
        return;
    }
    let resolved = candidate.canonicalize().unwrap_or(candidate);
    if seen.insert(agent_candidate_key(&resolved)) {
        candidates.push(resolved);
    }
}

fn agent_executable_file_name(kind: AgentConnectorKind) -> String {
    #[cfg(windows)]
    {
        format!("{}.exe", kind.command_name())
    }
    #[cfg(not(windows))]
    {
        kind.command_name().to_string()
    }
}

fn agent_executable_candidates(kind: AgentConnectorKind) -> Vec<PathBuf> {
    let file_name = agent_executable_file_name(kind);
    let mut candidates = Vec::new();
    let mut seen = BTreeSet::new();

    if let Some(path_value) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&path_value) {
            push_agent_candidate(&mut candidates, &mut seen, directory.join(&file_name));
        }
    }

    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        let home = PathBuf::from(home);
        push_agent_candidate(
            &mut candidates,
            &mut seen,
            home.join(".local").join("bin").join(&file_name),
        );
    }

    #[cfg(windows)]
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let local_app_data = PathBuf::from(local_app_data);
        match kind {
            AgentConnectorKind::Codex => push_agent_candidate(
                &mut candidates,
                &mut seen,
                local_app_data
                    .join("Programs")
                    .join("Codex")
                    .join(&file_name),
            ),
            AgentConnectorKind::Claude => push_agent_candidate(
                &mut candidates,
                &mut seen,
                local_app_data
                    .join("Programs")
                    .join("Claude")
                    .join(&file_name),
            ),
            AgentConnectorKind::Hermes => push_agent_candidate(
                &mut candidates,
                &mut seen,
                local_app_data
                    .join("hermes")
                    .join("hermes-agent")
                    .join("venv")
                    .join("Scripts")
                    .join(&file_name),
            ),
        }
    }

    #[cfg(target_os = "macos")]
    for directory in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
        push_agent_candidate(
            &mut candidates,
            &mut seen,
            Path::new(directory).join(&file_name),
        );
    }

    candidates
}

fn agent_user_home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

fn push_agent_config_candidate(
    candidates: &mut Vec<(PathBuf, String)>,
    root: &Path,
    marker: &str,
    location_label: &str,
) {
    candidates.push((root.join(marker), location_label.to_string()));
}

fn agent_configuration_candidates(kind: AgentConnectorKind) -> Vec<(PathBuf, String)> {
    let mut candidates = Vec::new();
    let home = agent_user_home();
    match kind {
        AgentConnectorKind::Codex => {
            if let Some(root) = std::env::var_os("CODEX_HOME").map(PathBuf::from) {
                for marker in ["config.toml", "auth.json"] {
                    push_agent_config_candidate(&mut candidates, &root, marker, "$CODEX_HOME");
                }
            }
            if let Some(root) = home.as_ref().map(|home| home.join(".codex")) {
                for marker in ["config.toml", "auth.json"] {
                    push_agent_config_candidate(&mut candidates, &root, marker, "~/.codex");
                }
            }
        }
        AgentConnectorKind::Claude => {
            if let Some(root) = std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from) {
                for marker in ["settings.json", "settings.local.json", ".credentials.json"] {
                    push_agent_config_candidate(
                        &mut candidates,
                        &root,
                        marker,
                        "$CLAUDE_CONFIG_DIR",
                    );
                }
            }
            if let Some(home) = home.as_ref() {
                let root = home.join(".claude");
                for marker in ["settings.json", "settings.local.json", ".credentials.json"] {
                    push_agent_config_candidate(&mut candidates, &root, marker, "~/.claude");
                }
                candidates.push((home.join(".claude.json"), "~/.claude.json".to_string()));
            }
        }
        AgentConnectorKind::Hermes => {
            if let Some(root) = std::env::var_os("HERMES_HOME").map(PathBuf::from) {
                for marker in [
                    "config.yaml",
                    "config.yml",
                    "config.json",
                    ".env",
                    "auth.json",
                ] {
                    push_agent_config_candidate(&mut candidates, &root, marker, "$HERMES_HOME");
                }
            }
            if let Some(root) = home.as_ref().map(|home| home.join(".hermes")) {
                for marker in [
                    "config.yaml",
                    "config.yml",
                    "config.json",
                    ".env",
                    "auth.json",
                ] {
                    push_agent_config_candidate(&mut candidates, &root, marker, "~/.hermes");
                }
            }
        }
    }
    candidates
}

fn first_existing_agent_configuration(
    candidates: impl IntoIterator<Item = (PathBuf, String)>,
) -> Option<String> {
    candidates.into_iter().find_map(|(path, label)| {
        fs::metadata(path)
            .ok()
            .filter(|metadata| metadata.is_file())
            .map(|_| label)
    })
}

fn agent_configuration_location(kind: AgentConnectorKind) -> Option<String> {
    first_existing_agent_configuration(agent_configuration_candidates(kind))
}

fn agent_connector_configuration_state(
    detected: bool,
    available: bool,
    configured: bool,
) -> AgentConnectorConfigurationState {
    if !detected {
        AgentConnectorConfigurationState::NotInstalled
    } else if !available {
        AgentConnectorConfigurationState::ProbeFailed
    } else if configured {
        AgentConnectorConfigurationState::Ready
    } else {
        AgentConnectorConfigurationState::NotConfigured
    }
}

fn read_pipe_limited<R: Read>(mut reader: R, limit: usize) -> CapturedPipe {
    let mut bytes = Vec::with_capacity(limit.min(8 * 1024));
    let mut truncated = false;
    let mut buffer = [0u8; 8 * 1024];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                let remaining = limit.saturating_sub(bytes.len());
                let accepted = remaining.min(count);
                bytes.extend_from_slice(&buffer[..accepted]);
                if accepted < count {
                    truncated = true;
                }
            }
            Err(_) => {
                truncated = true;
                break;
            }
        }
    }
    CapturedPipe { bytes, truncated }
}

fn sanitize_process_output(bytes: &[u8], limit: usize) -> String {
    let mut output = String::from_utf8_lossy(bytes)
        .chars()
        .filter(|character| matches!(character, '\n' | '\r' | '\t') || !character.is_control())
        .collect::<String>()
        .trim()
        .to_string();
    if output.len() > limit {
        let mut boundary = limit;
        while boundary > 0 && !output.is_char_boundary(boundary) {
            boundary -= 1;
        }
        output.truncate(boundary);
    }
    output
}

fn bounded_process_command(executable: &Path) -> Command {
    let mut command = Command::new(executable);
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command
}

fn terminate_child_tree(child: &mut std::process::Child) {
    #[cfg(windows)]
    {
        let system_root = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        let taskkill = system_root.join("System32").join("taskkill.exe");
        if taskkill.is_file() {
            let mut command = Command::new(taskkill);
            command
                .args(["/PID", &child.id().to_string(), "/T", "/F"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(0x0800_0000);
            let _ = command.status();
        }
    }
    #[cfg(unix)]
    {
        let process_group = format!("-{}", child.id());
        let _ = Command::new("/bin/kill")
            .args(["-TERM", &process_group])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        thread::sleep(Duration::from_millis(50));
        let _ = Command::new("/bin/kill")
            .args(["-KILL", &process_group])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
}

fn run_bounded_process(
    executable: &Path,
    arguments: &[OsString],
    workspace: Option<&Path>,
    timeout: Duration,
    output_limit: usize,
) -> Result<BoundedProcessOutput, BoundedProcessError> {
    run_bounded_process_with_control(
        executable,
        arguments,
        workspace,
        timeout,
        output_limit,
        None,
        |_| {},
    )
}

fn run_bounded_process_with_control<F>(
    executable: &Path,
    arguments: &[OsString],
    workspace: Option<&Path>,
    timeout: Duration,
    output_limit: usize,
    cancellation: Option<&AtomicBool>,
    mut on_heartbeat: F,
) -> Result<BoundedProcessOutput, BoundedProcessError>
where
    F: FnMut(Duration),
{
    let started = Instant::now();
    let mut command = bounded_process_command(executable);
    command
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("NO_COLOR", "1")
        .env("TERM", "dumb");
    if let Some(workspace) = workspace {
        command.current_dir(workspace);
    }

    let mut child = command.spawn().map_err(|error| BoundedProcessError {
        message: format!("无法启动连接器：{error}"),
        permission_denied: error.kind() == ErrorKind::PermissionDenied,
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        terminate_child_tree(&mut child);
        let _ = child.wait();
        BoundedProcessError {
            message: "无法读取连接器标准输出".to_string(),
            permission_denied: false,
        }
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        terminate_child_tree(&mut child);
        let _ = child.wait();
        BoundedProcessError {
            message: "无法读取连接器错误输出".to_string(),
            permission_denied: false,
        }
    })?;
    let stdout_reader = thread::spawn(move || read_pipe_limited(stdout, output_limit));
    let stderr_reader = thread::spawn(move || read_pipe_limited(stderr, output_limit));

    let mut timed_out = false;
    let mut cancelled = false;
    let mut next_heartbeat = Duration::ZERO;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                let elapsed = started.elapsed();
                if elapsed >= next_heartbeat {
                    on_heartbeat(elapsed);
                    next_heartbeat = elapsed.saturating_add(AGENT_TASK_HEARTBEAT_INTERVAL);
                }
                if cancellation.is_some_and(|flag| flag.load(Ordering::Acquire)) {
                    cancelled = true;
                    terminate_child_tree(&mut child);
                    break child.wait().ok();
                }
                if elapsed >= timeout {
                    timed_out = true;
                    terminate_child_tree(&mut child);
                    break child.wait().ok();
                }
                thread::sleep(Duration::from_millis(25));
            }
            Err(error) => {
                terminate_child_tree(&mut child);
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(BoundedProcessError {
                    message: format!("无法等待连接器结束：{error}"),
                    permission_denied: error.kind() == ErrorKind::PermissionDenied,
                });
            }
        }
    };

    let stdout = stdout_reader.join().map_err(|_| BoundedProcessError {
        message: "读取连接器标准输出时发生异常".to_string(),
        permission_denied: false,
    })?;
    let stderr = stderr_reader.join().map_err(|_| BoundedProcessError {
        message: "读取连接器错误输出时发生异常".to_string(),
        permission_denied: false,
    })?;
    let truncated = stdout.truncated;
    let stdout = sanitize_process_output(&stdout.bytes, output_limit);
    let stderr = sanitize_process_output(&stderr.bytes, output_limit);
    let success = !timed_out && !cancelled && status.is_some_and(|status| status.success());
    Ok(BoundedProcessOutput {
        success,
        timed_out,
        cancelled,
        stdout,
        stderr,
        exit_code: status.and_then(|status| status.code()),
        duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        truncated,
    })
}

fn connector_version(output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .find(|line| {
            !line.is_empty()
                && line.chars().any(|character| character.is_ascii_digit())
                && !line.contains('\\')
                && !line.contains('/')
                && !line.contains('=')
                && !line.to_ascii_lowercase().contains("token")
        })
        .map(|line| line.chars().take(160).collect())
}

fn resolve_agent_connector(kind: AgentConnectorKind) -> ResolvedAgentConnector {
    let config_location_label = agent_configuration_location(kind);
    let configured = config_location_label.is_some();
    let candidates = agent_executable_candidates(kind);
    if candidates.is_empty() {
        return ResolvedAgentConnector {
            status: AgentConnectorStatus {
                id: kind.id().to_string(),
                name: kind.name().to_string(),
                detected: false,
                available: false,
                configured,
                configuration_state: agent_connector_configuration_state(false, false, configured),
                config_location_label,
                executable: None,
                version: None,
                detail: if configured {
                    format!(
                        "已识别本机配置，但未在 PATH 或常见安装目录中发现 {} 命令",
                        kind.command_name()
                    )
                } else {
                    format!(
                        "未在 PATH 或常见安装目录中发现 {} 命令或配置",
                        kind.command_name()
                    )
                },
            },
            executable: None,
        };
    }

    let first_path = candidates[0].clone();
    let mut first_failure = None;
    let mut saw_permission_denied = false;
    let mut saw_other_failure = false;
    for executable in candidates {
        let version_arguments = [OsString::from("--version")];
        match run_bounded_process(
            &executable,
            &version_arguments,
            None,
            AGENT_PROBE_TIMEOUT,
            AGENT_PROBE_OUTPUT_MAX_BYTES,
        ) {
            Ok(result) if result.success => {
                let version =
                    connector_version(&result.stdout).or_else(|| connector_version(&result.stderr));
                let executable_label = executable.to_string_lossy().to_string();
                return ResolvedAgentConnector {
                    status: AgentConnectorStatus {
                        id: kind.id().to_string(),
                        name: kind.name().to_string(),
                        detected: true,
                        available: true,
                        configured,
                        configuration_state: agent_connector_configuration_state(
                            true, true, configured,
                        ),
                        config_location_label,
                        executable: Some(executable_label),
                        version,
                        detail: if configured {
                            "本机命令与配置均已识别，可在已存在的工作区内执行任务".to_string()
                        } else {
                            "本机命令可用，但尚未识别到已知配置文件".to_string()
                        },
                    },
                    executable: Some(executable),
                };
            }
            Ok(result) => {
                saw_other_failure = true;
                first_failure.get_or_insert_with(|| {
                    if result.timed_out {
                        "版本探测超时".to_string()
                    } else {
                        format!("版本探测未成功（退出码 {:?}）", result.exit_code)
                    }
                });
            }
            Err(error) => {
                if error.permission_denied {
                    saw_permission_denied = true;
                    first_failure.get_or_insert_with(|| "系统拒绝执行版本探测".to_string());
                } else {
                    saw_other_failure = true;
                    first_failure.get_or_insert_with(|| "无法完成版本探测".to_string());
                }
            }
        }
    }

    let permission_blocked = saw_permission_denied && !saw_other_failure;

    ResolvedAgentConnector {
        status: AgentConnectorStatus {
            id: kind.id().to_string(),
            name: kind.name().to_string(),
            detected: true,
            available: false,
            configured,
            configuration_state: if permission_blocked {
                AgentConnectorConfigurationState::PermissionBlocked
            } else {
                AgentConnectorConfigurationState::ProbeFailed
            },
            config_location_label,
            executable: Some(first_path.to_string_lossy().to_string()),
            version: None,
            detail: first_failure.unwrap_or_else(|| "本机命令版本探测未成功".to_string()),
        },
        executable: None,
    }
}

fn agent_connector_cache_decision(
    has_fresh_entry: bool,
    probe_in_flight: bool,
    force: bool,
    waited_for_probe: bool,
) -> AgentConnectorCacheDecision {
    if probe_in_flight {
        AgentConnectorCacheDecision::WaitForProbe
    } else if has_fresh_entry && (!force || waited_for_probe) {
        AgentConnectorCacheDecision::UseCached
    } else {
        AgentConnectorCacheDecision::StartProbe
    }
}

fn resolve_agent_connector_cached(kind: AgentConnectorKind, force: bool) -> ResolvedAgentConnector {
    let (cache_lock, cache_ready) = &*AGENT_CONNECTOR_CACHE;
    let mut waited_for_probe = false;
    loop {
        let now = Instant::now();
        let mut cache = cache_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let has_fresh_entry = cache.entries.get(&kind).is_some_and(|entry| {
            now.saturating_duration_since(entry.probed_at) <= AGENT_CONNECTOR_CACHE_TTL
        });
        match agent_connector_cache_decision(
            has_fresh_entry,
            cache.probing.contains(&kind),
            force,
            waited_for_probe,
        ) {
            AgentConnectorCacheDecision::UseCached => {
                return cache
                    .entries
                    .get(&kind)
                    .expect("fresh connector cache entry must exist")
                    .resolved
                    .clone();
            }
            AgentConnectorCacheDecision::WaitForProbe => {
                drop(
                    cache_ready
                        .wait(cache)
                        .unwrap_or_else(std::sync::PoisonError::into_inner),
                );
                waited_for_probe = true;
            }
            AgentConnectorCacheDecision::StartProbe => {
                cache.probing.insert(kind);
                drop(cache);
                let resolved = resolve_agent_connector(kind);
                let mut cache = cache_lock
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                cache.entries.insert(
                    kind,
                    CachedAgentConnector {
                        resolved: resolved.clone(),
                        probed_at: Instant::now(),
                    },
                );
                cache.probing.remove(&kind);
                cache_ready.notify_all();
                return resolved;
            }
        }
    }
}

fn invalidate_agent_connector_cache(kind: AgentConnectorKind) {
    let (cache_lock, cache_ready) = &*AGENT_CONNECTOR_CACHE;
    let mut cache = cache_lock
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    cache.entries.remove(&kind);
    cache_ready.notify_all();
}

fn list_resolved_agent_connectors(force: bool) -> Result<Vec<ResolvedAgentConnector>, String> {
    let kinds = [
        AgentConnectorKind::Codex,
        AgentConnectorKind::Claude,
        AgentConnectorKind::Hermes,
    ];
    let handles =
        kinds.map(|kind| thread::spawn(move || resolve_agent_connector_cached(kind, force)));
    handles
        .into_iter()
        .map(|handle| {
            handle
                .join()
                .map_err(|_| "连接器探测线程异常结束".to_string())
        })
        .collect()
}

fn validate_agent_task(task: &str) -> Result<String, String> {
    let task = task.trim();
    if task.is_empty() {
        return Err("任务内容不能为空".to_string());
    }
    if task.chars().count() > AGENT_TASK_MAX_CHARS {
        return Err(format!("任务内容最多 {AGENT_TASK_MAX_CHARS} 个字符"));
    }
    if task.contains('\0') {
        return Err("任务内容包含不支持的空字符".to_string());
    }
    Ok(task.to_string())
}

fn validate_agent_workspace(workspace: &str) -> Result<PathBuf, String> {
    if workspace.contains('\0') {
        return Err("工作区路径包含不支持的空字符".to_string());
    }
    let requested = PathBuf::from(workspace.trim());
    if !requested.is_absolute() {
        return Err("工作区必须使用绝对路径".to_string());
    }
    let requested_metadata =
        fs::symlink_metadata(&requested).map_err(|_| "工作区不存在或无法访问".to_string())?;
    if !requested_metadata.is_dir()
        || requested_metadata.file_type().is_symlink()
        || is_reparse_point(&requested_metadata)
    {
        return Err("工作区必须是普通文件夹，不能使用链接或重解析目录".to_string());
    }
    let canonical = requested
        .canonicalize()
        .map_err(|_| "工作区不存在或无法访问".to_string())?;
    let canonical_metadata =
        fs::symlink_metadata(&canonical).map_err(|_| "工作区不存在或无法访问".to_string())?;
    if !canonical_metadata.is_dir()
        || canonical_metadata.file_type().is_symlink()
        || is_reparse_point(&canonical_metadata)
    {
        return Err("工作区必须是已存在的目录".to_string());
    }
    Ok(canonical)
}

fn ensure_plain_agent_directory(
    parent: &Path,
    name: &str,
    require_new: bool,
) -> Result<PathBuf, String> {
    let candidate = parent.join(name);
    match fs::symlink_metadata(&candidate) {
        Ok(metadata) => {
            if require_new {
                return Err("附件临时目录发生冲突，请重新发送".to_string());
            }
            if !metadata.is_dir()
                || metadata.file_type().is_symlink()
                || is_reparse_point(&metadata)
            {
                return Err("附件临时目录不是安全的普通文件夹".to_string());
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            fs::create_dir(&candidate).map_err(|_| "无法创建附件临时目录".to_string())?;
        }
        Err(_) => return Err("无法检查附件临时目录".to_string()),
    }
    let metadata =
        fs::symlink_metadata(&candidate).map_err(|_| "无法检查附件临时目录".to_string())?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err("附件临时目录不是安全的普通文件夹".to_string());
    }
    let canonical = candidate
        .canonicalize()
        .map_err(|_| "无法访问附件临时目录".to_string())?;
    if !canonical.starts_with(parent) {
        return Err("附件临时目录超出 Agent 工作区".to_string());
    }
    Ok(canonical)
}

fn create_agent_attachment_snapshot_dir(
    workspace: &Path,
    task_id: &str,
) -> Result<PathBuf, String> {
    let data_dir = ensure_plain_agent_directory(workspace, AGENT_ATTACHMENT_DATA_DIR, false)?;
    let input_dir = ensure_plain_agent_directory(&data_dir, AGENT_ATTACHMENT_INPUT_DIR, false)?;
    ensure_plain_agent_directory(&input_dir, task_id, true)
}

fn agent_attachment_display_name(path: &Path) -> String {
    let value = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "附件".to_string());
    let sanitized = value
        .chars()
        .filter(|character| !character.is_control())
        .take(120)
        .collect::<String>();
    if sanitized.trim().is_empty() {
        "附件".to_string()
    } else {
        sanitized
    }
}

fn agent_attachment_safe_extension(path: &Path) -> String {
    path.extension()
        .and_then(OsStr::to_str)
        .filter(|extension| {
            !extension.is_empty()
                && extension.len() <= 16
                && extension
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric())
        })
        .map(|extension| format!(".{}", extension.to_ascii_lowercase()))
        .unwrap_or_default()
}

fn validate_agent_attachment(
    raw_path: &str,
    attachment_number: usize,
) -> Result<(PathBuf, u64, String), String> {
    if raw_path.contains('\0') {
        return Err(format!("附件 {attachment_number} 的路径无效"));
    }
    let requested = PathBuf::from(raw_path.trim());
    if !requested.is_absolute() {
        return Err(format!("附件 {attachment_number} 必须使用绝对路径"));
    }
    let metadata = fs::symlink_metadata(&requested)
        .map_err(|_| format!("附件 {attachment_number} 不存在或无法访问"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err(format!("附件 {attachment_number} 不是可读取的普通文件"));
    }
    let canonical = requested
        .canonicalize()
        .map_err(|_| format!("附件 {attachment_number} 无法安全读取"))?;
    let canonical_metadata = fs::symlink_metadata(&canonical)
        .map_err(|_| format!("附件 {attachment_number} 无法安全读取"))?;
    if !canonical_metadata.is_file()
        || canonical_metadata.file_type().is_symlink()
        || is_reparse_point(&canonical_metadata)
    {
        return Err(format!("附件 {attachment_number} 不是可读取的普通文件"));
    }
    let size = canonical_metadata.len();
    if size > AGENT_ATTACHMENT_MAX_FILE_BYTES {
        return Err(format!("附件 {attachment_number} 超过 64 MB"));
    }
    let display_name = agent_attachment_display_name(&canonical);
    Ok((canonical, size, display_name))
}

fn prepare_agent_attachments(
    workspace: &Path,
    attachment_paths: Vec<String>,
    task_id: &str,
) -> Result<PreparedAgentAttachments, String> {
    if attachment_paths.len() > AGENT_ATTACHMENT_MAX_COUNT {
        return Err(format!("一次最多发送 {AGENT_ATTACHMENT_MAX_COUNT} 个附件"));
    }
    let mut prepared = PreparedAgentAttachments {
        items: Vec::new(),
        workspace: workspace.to_path_buf(),
        cleanup_dir: None,
    };
    let mut canonical_paths = HashSet::new();
    let mut total_bytes = 0u64;

    for (index, raw_path) in attachment_paths.iter().enumerate() {
        let attachment_number = index + 1;
        let (canonical, size, display_name) =
            validate_agent_attachment(raw_path, attachment_number)?;
        if !canonical_paths.insert(canonical.clone()) {
            continue;
        }
        total_bytes = total_bytes
            .checked_add(size)
            .ok_or_else(|| "附件总大小无效".to_string())?;
        if total_bytes > AGENT_ATTACHMENT_MAX_TOTAL_BYTES {
            return Err("附件总大小不能超过 256 MB".to_string());
        }

        if let Ok(relative_path) = canonical.strip_prefix(workspace) {
            prepared.items.push(PreparedAgentAttachment {
                display_name,
                relative_path: relative_path.to_path_buf(),
                snapshot: false,
            });
            continue;
        }

        let snapshot_dir = match prepared.cleanup_dir.clone() {
            Some(path) => path,
            None => {
                let path = create_agent_attachment_snapshot_dir(workspace, task_id)?;
                prepared.cleanup_dir = Some(path.clone());
                path
            }
        };
        let extension = agent_attachment_safe_extension(&canonical);
        let snapshot_name = format!("attachment-{attachment_number:02}{extension}");
        let snapshot_path = snapshot_dir.join(snapshot_name);
        let copied = fs::copy(&canonical, &snapshot_path)
            .map_err(|_| format!("附件 {attachment_number} 无法复制到 Agent 工作区"))?;
        if copied != size {
            return Err(format!("附件 {attachment_number} 复制不完整"));
        }
        let snapshot_metadata = fs::symlink_metadata(&snapshot_path)
            .map_err(|_| format!("附件 {attachment_number} 的工作区快照无效"))?;
        if !snapshot_metadata.is_file()
            || snapshot_metadata.file_type().is_symlink()
            || is_reparse_point(&snapshot_metadata)
            || snapshot_metadata.len() != size
        {
            return Err(format!("附件 {attachment_number} 的工作区快照无效"));
        }
        let relative_path = snapshot_path
            .strip_prefix(workspace)
            .map_err(|_| "附件快照超出 Agent 工作区".to_string())?
            .to_path_buf();
        prepared.items.push(PreparedAgentAttachment {
            display_name,
            relative_path,
            snapshot: true,
        });
    }
    Ok(prepared)
}

fn compose_agent_task_with_attachments(
    task: &str,
    attachments: &PreparedAgentAttachments,
) -> String {
    let mut prompt = String::with_capacity(task.len() + attachments.items.len() * 220);
    prompt.push_str(task);
    if !attachments.items.is_empty() {
        prompt.push_str("\n\n[虫洞派提供的附件]\n");
        prompt.push_str("请先读取下列文件，再执行上面的任务。路径均相对于当前 Agent 工作区。\n");
        for (index, attachment) in attachments.items.iter().enumerate() {
            let display_name = serde_json::to_string(&attachment.display_name)
                .unwrap_or_else(|_| "\"附件\"".to_string());
            let relative_path = attachment
                .relative_path
                .to_string_lossy()
                .replace('\\', "/");
            let relative_path =
                serde_json::to_string(&relative_path).unwrap_or_else(|_| "\"附件\"".to_string());
            let mode = if attachment.snapshot {
                "外部文件的安全快照"
            } else {
                "工作区内文件"
            };
            prompt.push_str(&format!(
                "- 附件 {}：名称 {}；路径 {}；模式 {}\n",
                index + 1,
                display_name,
                relative_path,
                mode
            ));
        }
        if attachments
            .items
            .iter()
            .any(|attachment| attachment.snapshot)
        {
            prompt.push_str("外部文件已复制成临时快照；如需产生修改结果，请在工作区另存新文件，不要声称已经覆盖原文件。\n");
        }
    }
    prompt.push_str("\n[虫洞派交付约定]\n如果本次任务创建或修改了需要交付给用户的文件，请在最终回复末尾为每个文件单独输出一行：WORMHOLE_FILE: 工作区相对路径。只列出当前工作区内确实存在的普通文件。\n");
    prompt
}

fn resolve_agent_result_file(workspace: &Path, raw_path: &str) -> Option<AgentResultFile> {
    let mut value = raw_path.trim();
    value = value
        .strip_prefix('<')
        .and_then(|value| value.strip_suffix('>'))
        .unwrap_or(value);
    value = value.trim_matches(|character| matches!(character, '`' | '"' | '\''));
    if value.is_empty()
        || value.contains('\0')
        || value.starts_with("http://")
        || value.starts_with("https://")
    {
        return None;
    }
    let requested = if let Some(file_url_path) = value.strip_prefix("file://") {
        PathBuf::from(file_url_path)
    } else {
        PathBuf::from(value)
    };
    let candidate = if requested.is_absolute() {
        requested
    } else {
        workspace.join(requested)
    };
    let canonical = candidate.canonicalize().ok()?;
    if !canonical.starts_with(workspace)
        || canonical.starts_with(
            workspace
                .join(AGENT_ATTACHMENT_DATA_DIR)
                .join(AGENT_ATTACHMENT_INPUT_DIR),
        )
    {
        return None;
    }
    let metadata = fs::symlink_metadata(&canonical).ok()?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return None;
    }
    let relative_path = canonical.strip_prefix(workspace).ok()?;
    Some(AgentResultFile {
        name: agent_attachment_display_name(&canonical),
        path: canonical.to_string_lossy().to_string(),
        relative_path: relative_path.to_string_lossy().replace('\\', "/"),
    })
}

fn collect_agent_result_file(
    workspace: &Path,
    raw_path: &str,
    seen: &mut HashSet<PathBuf>,
    files: &mut Vec<AgentResultFile>,
) {
    if files.len() >= AGENT_RESULT_MAX_FILES {
        return;
    }
    let Some(file) = resolve_agent_result_file(workspace, raw_path) else {
        return;
    };
    let key = PathBuf::from(&file.path);
    if seen.insert(key) {
        files.push(file);
    }
}

fn extract_agent_result_files(output: &str, workspace: &Path) -> (String, Vec<AgentResultFile>) {
    let mut files = Vec::new();
    let mut seen = HashSet::new();
    let mut visible_lines = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();
        let marker_candidate = trimmed
            .trim_start_matches(|character: char| {
                character.is_whitespace() || matches!(character, '-' | '*')
            })
            .strip_prefix("WORMHOLE_FILE:")
            .or_else(|| trimmed.strip_prefix("[虫洞派文件]"))
            .or_else(|| trimmed.strip_prefix("虫洞派文件:"));
        if let Some(candidate) = marker_candidate {
            collect_agent_result_file(workspace, candidate, &mut seen, &mut files);
            continue;
        }

        let mut remaining = line;
        while let Some(start) = remaining.find("](") {
            let after = &remaining[start + 2..];
            let Some(end) = after.find(')') else {
                break;
            };
            collect_agent_result_file(workspace, &after[..end], &mut seen, &mut files);
            remaining = &after[end + 1..];
        }
        for (index, segment) in line.split('`').enumerate() {
            if index % 2 == 1 {
                collect_agent_result_file(workspace, segment, &mut seen, &mut files);
            }
        }
        collect_agent_result_file(workspace, trimmed, &mut seen, &mut files);
        visible_lines.push(line);
    }

    (visible_lines.join("\n").trim().to_string(), files)
}

fn safe_existing_agent_workspace(path: &Path) -> Option<PathBuf> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return None;
    }
    path.canonicalize().ok().filter(|path| path.is_dir())
}

fn looks_like_agent_project(path: &Path) -> bool {
    [
        ".git",
        ".hg",
        "package.json",
        "Cargo.toml",
        "pyproject.toml",
        "requirements.txt",
        "go.mod",
        "pom.xml",
        "build.gradle",
        "settings.gradle",
        "Gemfile",
        "composer.json",
        "AGENTS.md",
    ]
    .into_iter()
    .any(|marker| fs::symlink_metadata(path.join(marker)).is_ok())
}

fn ensure_dedicated_agent_workspace(path: &Path) -> Result<PathBuf, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata)
            if metadata.is_dir()
                && !metadata.file_type().is_symlink()
                && !is_reparse_point(&metadata) => {}
        Ok(_) => return Err("专用 Agent 工作区不是普通文件夹".to_string()),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            fs::create_dir(path).map_err(|error| format!("无法创建专用 Agent 工作区：{error}"))?;
        }
        Err(error) => return Err(format!("无法检查专用 Agent 工作区：{error}")),
    }
    safe_existing_agent_workspace(path).ok_or_else(|| "专用 Agent 工作区无法安全访问".to_string())
}

fn select_agent_default_workspace(
    current_directory: Option<PathBuf>,
    documents_directory: &Path,
) -> Result<PathBuf, String> {
    if let Some(current) = current_directory
        .as_deref()
        .and_then(safe_existing_agent_workspace)
        .filter(|path| looks_like_agent_project(path))
    {
        return Ok(current);
    }

    let documents = safe_existing_agent_workspace(documents_directory)
        .ok_or_else(|| "文档目录无法安全访问".to_string())?;
    if let Some(codex) = safe_existing_agent_workspace(&documents.join("Codex")) {
        return Ok(codex);
    }

    ensure_dedicated_agent_workspace(&documents.join("虫洞派 Agent 工作区"))
}

fn agent_task_arguments(kind: AgentConnectorKind, task: &str) -> Vec<OsString> {
    match kind {
        AgentConnectorKind::Codex => [
            "--ask-for-approval",
            "never",
            "--sandbox",
            "workspace-write",
            "exec",
            "--skip-git-repo-check",
            "--color",
            "never",
            "--",
            task,
        ]
        .into_iter()
        .map(OsString::from)
        .collect(),
        AgentConnectorKind::Claude => [
            "--print",
            "--output-format",
            "text",
            "--permission-mode",
            "acceptEdits",
            "--no-session-persistence",
            "--no-chrome",
            "--",
            task,
        ]
        .into_iter()
        .map(OsString::from)
        .collect(),
        AgentConnectorKind::Hermes => [
            "chat",
            "-Q",
            "--checkpoints",
            "--max-turns",
            "24",
            "--source",
            "tool",
            "-q",
            task,
        ]
        .into_iter()
        .map(OsString::from)
        .collect(),
    }
}

fn next_agent_task_id(kind: AgentConnectorKind) -> String {
    let sequence = AGENT_TASK_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{}-{}-{sequence}", kind.id(), now_millis())
}

fn emit_agent_task_status(app: &tauri::AppHandle, status: &AgentTaskStatus) {
    let _ = app.emit("agent://task-status", status.clone());
}

fn publish_agent_task_result(app: &tauri::AppHandle, result: &AgentTaskResult) {
    if let Ok(mut last) = LAST_AGENT_TASK_RESULT.lock() {
        *last = Some(result.clone());
    }
    let _ = app.emit("agent://task-result", result.clone());
}

fn begin_agent_task(
    app: &tauri::AppHandle,
    kind: AgentConnectorKind,
    task_id: String,
) -> Result<(String, Arc<AtomicBool>), String> {
    let cancel = Arc::new(AtomicBool::new(false));
    let now = now_millis();
    let status = AgentTaskStatus {
        task_id: task_id.clone(),
        connector_id: kind.id().to_string(),
        state: AgentTaskState::Starting,
        started_at: now,
        updated_at: now,
        elapsed_ms: 0,
        cancel_requested: false,
        detail: "正在安全启动本机 Agent".to_string(),
    };
    let mut active = ACTIVE_AGENT_TASK
        .lock()
        .map_err(|_| "Agent 任务状态已锁定".to_string())?;
    if active.is_some() {
        return Err("已有 Agent 任务正在执行，请稍后再试".to_string());
    }
    *active = Some(ActiveAgentTask {
        status: status.clone(),
        cancel: cancel.clone(),
    });
    drop(active);
    if let Ok(mut last_result) = LAST_AGENT_TASK_RESULT.lock() {
        *last_result = None;
    }
    emit_agent_task_status(app, &status);
    Ok((task_id, cancel))
}

fn heartbeat_agent_task(app: &tauri::AppHandle, task_id: &str, elapsed: Duration) {
    let status = ACTIVE_AGENT_TASK.lock().ok().and_then(|mut active| {
        let task = active.as_mut()?;
        if task.status.task_id != task_id {
            return None;
        }
        let cancel_requested = task.cancel.load(Ordering::Acquire);
        task.status.state = if cancel_requested {
            AgentTaskState::Cancelling
        } else {
            AgentTaskState::Running
        };
        task.status.updated_at = now_millis();
        task.status.elapsed_ms = elapsed.as_millis().min(u128::from(u64::MAX)) as u64;
        task.status.cancel_requested = cancel_requested;
        task.status.detail = if cancel_requested {
            "正在停止本机 Agent".to_string()
        } else {
            "本机 Agent 仍在运行".to_string()
        };
        Some(task.status.clone())
    });
    if let Some(status) = status {
        emit_agent_task_status(app, &status);
    }
}

fn completed_agent_task_state(result: &BoundedProcessOutput) -> AgentTaskState {
    if result.cancelled {
        AgentTaskState::Cancelled
    } else if result.timed_out {
        AgentTaskState::TimedOut
    } else if result.success {
        AgentTaskState::Succeeded
    } else {
        AgentTaskState::Failed
    }
}

fn agent_task_state_detail(state: AgentTaskState) -> &'static str {
    match state {
        AgentTaskState::Starting => "正在安全启动本机 Agent",
        AgentTaskState::Running => "本机 Agent 仍在运行",
        AgentTaskState::Cancelling => "正在停止本机 Agent",
        AgentTaskState::Succeeded => "本机 Agent 已完成任务",
        AgentTaskState::Failed => "本机 Agent 执行失败",
        AgentTaskState::Cancelled => "任务已由用户停止",
        AgentTaskState::TimedOut => "任务达到长时运行安全上限，进程已停止",
    }
}

fn finish_agent_task(
    app: &tauri::AppHandle,
    task_id: &str,
    state: AgentTaskState,
    elapsed_ms: u64,
) {
    let status = ACTIVE_AGENT_TASK.lock().ok().and_then(|mut active| {
        let current = active.as_ref()?;
        if current.status.task_id != task_id {
            return None;
        }
        let mut task = active.take()?;
        task.status.state = state;
        task.status.updated_at = now_millis();
        task.status.elapsed_ms = elapsed_ms;
        task.status.cancel_requested = task.cancel.load(Ordering::Acquire);
        task.status.detail = agent_task_state_detail(state).to_string();
        Some(task.status)
    });
    if let Some(status) = status {
        if let Ok(mut last) = LAST_AGENT_TASK_STATUS.lock() {
            *last = Some(status.clone());
        }
        emit_agent_task_status(app, &status);
    }
}

fn request_agent_task_cancellation(
    active: &mut Option<ActiveAgentTask>,
    requested_task_id: Option<&str>,
    updated_at: u64,
) -> Result<Option<AgentTaskStatus>, String> {
    let Some(task) = active.as_mut() else {
        return Ok(None);
    };
    if requested_task_id
        .map(str::trim)
        .is_some_and(|requested| !requested.is_empty() && requested != task.status.task_id)
    {
        return Err("指定的 Agent 任务已结束或不是当前任务".to_string());
    }
    task.cancel.store(true, Ordering::Release);
    task.status.state = AgentTaskState::Cancelling;
    task.status.updated_at = updated_at;
    task.status.cancel_requested = true;
    task.status.detail = agent_task_state_detail(AgentTaskState::Cancelling).to_string();
    Ok(Some(task.status.clone()))
}

fn agent_task_public_output(result: &BoundedProcessOutput) -> String {
    if result.success {
        if result.stdout.is_empty() {
            "任务已完成，连接器没有返回文字结果。".to_string()
        } else {
            result.stdout.clone()
        }
    } else if result.cancelled {
        "任务已由用户停止。".to_string()
    } else if result.timed_out {
        "任务达到长时运行安全上限，已经安全停止。".to_string()
    } else {
        "本机 Agent 没有完成任务，请检查连接状态或稍后再试。".to_string()
    }
}

fn execute_agent_task(
    app: tauri::AppHandle,
    connector_id: String,
    task: String,
    workspace: String,
    attachment_paths: Option<Vec<String>>,
) -> Result<AgentTaskResult, String> {
    let kind = AgentConnectorKind::from_id(&connector_id)
        .ok_or_else(|| "仅支持 codex、claude 或 hermes 连接器".to_string())?;
    let task = validate_agent_task(&task)?;
    let workspace = validate_agent_workspace(&workspace)?;
    let connector = resolve_agent_connector_cached(kind, false);
    let executable = connector
        .executable
        .ok_or_else(|| connector.status.detail.clone())?;
    let _guard = AGENT_TASK_LOCK
        .try_lock()
        .map_err(|_| "已有 Agent 任务正在执行，请稍后再试".to_string())?;
    let task_id = next_agent_task_id(kind);
    let attachments =
        prepare_agent_attachments(&workspace, attachment_paths.unwrap_or_default(), &task_id)?;
    let task = compose_agent_task_with_attachments(&task, &attachments);
    let (task_id, cancellation) = begin_agent_task(&app, kind, task_id)?;
    let arguments = agent_task_arguments(kind, &task);
    let heartbeat_app = app.clone();
    let heartbeat_task_id = task_id.clone();
    let result = run_bounded_process_with_control(
        &executable,
        &arguments,
        Some(&workspace),
        AGENT_TASK_TIMEOUT,
        AGENT_OUTPUT_MAX_BYTES,
        Some(cancellation.as_ref()),
        move |elapsed| heartbeat_agent_task(&heartbeat_app, &heartbeat_task_id, elapsed),
    );
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            invalidate_agent_connector_cache(kind);
            let elapsed_ms = ACTIVE_AGENT_TASK
                .lock()
                .ok()
                .and_then(|active| active.as_ref().map(|task| task.status.started_at))
                .map(|started_at| now_millis().saturating_sub(started_at))
                .unwrap_or_default();
            finish_agent_task(&app, &task_id, AgentTaskState::Failed, elapsed_ms);
            let output = if error.permission_denied {
                "系统没有允许启动这个本机 Agent，请重新识别安装状态。".to_string()
            } else {
                "本机 Agent 启动或运行异常，请稍后重新识别后再试。".to_string()
            };
            let _internal_diagnostic = error.message;
            let result = AgentTaskResult {
                task_id,
                connector_id: kind.id().to_string(),
                success: false,
                timed_out: false,
                cancelled: false,
                output,
                exit_code: None,
                duration_ms: elapsed_ms,
                truncated: false,
                files: Vec::new(),
            };
            publish_agent_task_result(&app, &result);
            return Ok(result);
        }
    };
    let final_state = completed_agent_task_state(&result);
    finish_agent_task(&app, &task_id, final_state, result.duration_ms);
    let output = agent_task_public_output(&result);
    let (output, files) = extract_agent_result_files(&output, &workspace);
    let task_result = AgentTaskResult {
        task_id,
        connector_id: kind.id().to_string(),
        success: result.success,
        timed_out: result.timed_out,
        cancelled: result.cancelled,
        output,
        exit_code: result.exit_code,
        duration_ms: result.duration_ms,
        truncated: result.truncated,
        files,
    };
    publish_agent_task_result(&app, &task_result);
    Ok(task_result)
}

#[tauri::command]
async fn list_agent_connectors(force: Option<bool>) -> Result<Vec<AgentConnectorStatus>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        list_resolved_agent_connectors(force.unwrap_or(false))
            .map(|connectors| connectors.into_iter().map(|item| item.status).collect())
    })
    .await
    .map_err(|error| format!("连接器探测任务异常：{error}"))?
}

#[tauri::command]
fn get_agent_default_workspace(app: tauri::AppHandle) -> Result<String, String> {
    let documents = app
        .path()
        .document_dir()
        .map_err(|error| format!("无法定位文档目录：{error}"))?;
    select_agent_default_workspace(std::env::current_dir().ok(), &documents)
        .map(|path| path.to_string_lossy().to_string())
}

#[tauri::command]
fn pick_dialogue_files(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let files = app
        .dialog()
        .file()
        .set_title("选择要交给 Agent 的文件")
        .blocking_pick_files()
        .unwrap_or_default();
    files
        .into_iter()
        .map(|file| {
            file.into_path()
                .map(|path| path.to_string_lossy().to_string())
                .map_err(|_| "只支持选择本机文件".to_string())
        })
        .collect()
}

#[tauri::command]
fn get_agent_task_status() -> Result<Option<AgentTaskStatus>, String> {
    if let Some(status) = ACTIVE_AGENT_TASK
        .lock()
        .map_err(|_| "Agent 任务状态已锁定".to_string())?
        .as_ref()
        .map(|task| task.status.clone())
    {
        return Ok(Some(status));
    }
    LAST_AGENT_TASK_STATUS
        .lock()
        .map(|status| status.clone())
        .map_err(|_| "Agent 历史任务状态已锁定".to_string())
}

#[tauri::command]
fn get_agent_task_result(task_id: Option<String>) -> Result<Option<AgentTaskResult>, String> {
    let result = LAST_AGENT_TASK_RESULT
        .lock()
        .map_err(|_| "Agent 任务结果已锁定".to_string())?
        .clone();
    Ok(result.filter(|result| {
        task_id
            .as_deref()
            .map(str::trim)
            .is_none_or(|requested| requested.is_empty() || requested == result.task_id)
    }))
}

#[tauri::command]
fn stop_agent_task(app: tauri::AppHandle, task_id: Option<String>) -> Result<bool, String> {
    let status = {
        let mut active = ACTIVE_AGENT_TASK
            .lock()
            .map_err(|_| "Agent 任务状态已锁定".to_string())?;
        let Some(status) =
            request_agent_task_cancellation(&mut active, task_id.as_deref(), now_millis())?
        else {
            return Ok(false);
        };
        status
    };
    emit_agent_task_status(&app, &status);
    Ok(true)
}

#[tauri::command]
async fn run_agent_task(
    app: tauri::AppHandle,
    connector_id: String,
    task: String,
    workspace: String,
    attachment_paths: Option<Vec<String>>,
) -> Result<AgentTaskResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        execute_agent_task(app, connector_id, task, workspace, attachment_paths)
    })
    .await
    .map_err(|error| format!("Agent 执行任务异常：{error}"))?
}

#[tauri::command]
fn open_agent_result_file(path: String, workspace: String) -> Result<(), String> {
    let workspace = validate_agent_workspace(&workspace)?;
    let file = resolve_agent_result_file(&workspace, &path)
        .ok_or_else(|| "这个交付文件已移动、已删除或不在当前 Agent 工作区".to_string())?;
    let canonical = PathBuf::from(&file.path);
    let extension = canonical
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if is_safe_text_script_extension(&extension) {
        #[cfg(windows)]
        Command::new("notepad.exe")
            .arg(&canonical)
            .spawn()
            .map_err(|error| format!("无法用记事本查看交付脚本：{error}"))?;
        #[cfg(target_os = "macos")]
        Command::new("open")
            .args(["-a", "TextEdit"])
            .arg(&canonical)
            .spawn()
            .map_err(|error| format!("无法用 TextEdit 查看交付脚本：{error}"))?;
        #[cfg(not(any(windows, target_os = "macos")))]
        return Err("当前系统尚未配置安全的脚本文本查看器".to_string());
        return Ok(());
    }
    if requires_execution_confirmation(&extension) {
        return Err("可执行交付文件需要二次确认，虫洞派不会直接运行".to_string());
    }
    open::that(&canonical).map_err(|error| format!("无法打开交付文件：{error}"))
}

#[tauri::command]
fn open_social(platform: String, state: State<'_, AppState>) -> Result<(), String> {
    let url = match platform.as_str() {
        "xiaohongshu" => "https://creator.xiaohongshu.com/publish/publish",
        "x" => "https://x.com/compose/post",
        "douyin" => "https://creator.douyin.com/creator-micro/content/upload",
        _ => return Err("不支持的社交平台".to_string()),
    };

    open::that(url).map_err(|error| error.to_string())?;
    append_log(&state.index, "open_social", None, url, "success");
    Ok(())
}

#[tauri::command]
fn open_external_url(url: String, state: State<'_, AppState>) -> Result<(), String> {
    let value = url.trim();
    let lower = value.to_ascii_lowercase();
    if value.is_empty()
        || value.len() > 2048
        || value.chars().any(char::is_control)
        || !(lower.starts_with("https://") || lower.starts_with("http://"))
    {
        return Err("只支持安全的 http 或 https 链接".to_string());
    }
    open::that(value).map_err(|error| error.to_string())?;
    append_log(&state.index, "open_external_url", None, value, "success");
    Ok(())
}

#[tauri::command]
fn start_watching(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let mut watcher_slot = state
        .watcher
        .lock()
        .map_err(|_| "文件监听器已锁定".to_string())?;
    if watcher_slot.is_some() {
        return Ok(());
    }

    let (change_tx, change_rx) = std::sync::mpsc::sync_channel::<()>(1);
    let worker_index = state.index.clone();
    let worker_app = app.clone();
    let worker_organize_lock = state.organize_lock.clone();
    std::thread::spawn(move || {
        while change_rx.recv().is_ok() {
            while change_rx.recv_timeout(Duration::from_millis(300)).is_ok() {}
            let Ok(_operation_guard) = worker_organize_lock.lock() else {
                continue;
            };
            if scan_index(&worker_index, true).is_ok() {
                let _ = worker_app.emit("files://changed", ());
            }
        }
    });
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
        if result.is_ok() {
            let _ = change_tx.try_send(());
        }
    })
    .map_err(|error| error.to_string())?;

    watcher
        .watch(&state.index.desktop_path, RecursiveMode::NonRecursive)
        .map_err(|error| error.to_string())?;
    watcher
        .watch(&state.index.library_path, RecursiveMode::Recursive)
        .map_err(|error| error.to_string())?;
    if state.index.legacy_library_path.exists() {
        watcher
            .watch(&state.index.legacy_library_path, RecursiveMode::Recursive)
            .map_err(|error| error.to_string())?;
    }
    *watcher_slot = Some(watcher);
    Ok(())
}

fn position_main_top_right(window: &tauri::WebviewWindow) -> Result<(), String> {
    let monitor = window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or(window
            .primary_monitor()
            .map_err(|error| error.to_string())?)
        .ok_or_else(|| "无法获取当前显示器".to_string())?;
    let outer_size = window.outer_size().map_err(|error| error.to_string())?;
    let work_area = monitor.work_area();
    let margin = (16.0 * monitor.scale_factor()).round() as i64;
    let x = i64::from(work_area.position.x) + i64::from(work_area.size.width)
        - i64::from(outer_size.width)
        - margin;
    let y = i64::from(work_area.position.y) + margin;
    window
        .set_position(PhysicalPosition::new(
            x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
            y.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        ))
        .map_err(|error| error.to_string())
}

trait RestWindowOps {
    fn rest_show(&self) -> Result<(), String>;
    fn rest_unminimize(&self) -> Result<(), String>;
    fn rest_set_always_on_top(&self, enabled: bool) -> Result<(), String>;
    fn rest_set_fullscreen(&self, enabled: bool) -> Result<(), String>;
    fn rest_focus(&self) -> Result<(), String>;
}

impl RestWindowOps for tauri::WebviewWindow {
    fn rest_show(&self) -> Result<(), String> {
        self.show().map_err(|error| error.to_string())
    }

    fn rest_unminimize(&self) -> Result<(), String> {
        self.unminimize().map_err(|error| error.to_string())
    }

    fn rest_set_always_on_top(&self, enabled: bool) -> Result<(), String> {
        self.set_always_on_top(enabled)
            .map_err(|error| error.to_string())
    }

    fn rest_set_fullscreen(&self, enabled: bool) -> Result<(), String> {
        self.set_fullscreen(enabled)
            .map_err(|error| error.to_string())
    }

    fn rest_focus(&self) -> Result<(), String> {
        self.set_focus().map_err(|error| error.to_string())
    }
}

impl RestWindowOps for tauri::Window {
    fn rest_show(&self) -> Result<(), String> {
        self.show().map_err(|error| error.to_string())
    }

    fn rest_unminimize(&self) -> Result<(), String> {
        self.unminimize().map_err(|error| error.to_string())
    }

    fn rest_set_always_on_top(&self, enabled: bool) -> Result<(), String> {
        self.set_always_on_top(enabled)
            .map_err(|error| error.to_string())
    }

    fn rest_set_fullscreen(&self, enabled: bool) -> Result<(), String> {
        self.set_fullscreen(enabled)
            .map_err(|error| error.to_string())
    }

    fn rest_focus(&self) -> Result<(), String> {
        self.set_focus().map_err(|error| error.to_string())
    }
}

fn combined_window_result(context: &str, errors: Vec<String>) -> Result<(), String> {
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!("{context}：{}", errors.join("；")))
    }
}

fn enter_rest_window<W: RestWindowOps>(window: &W) -> Result<(), String> {
    let entered = (|| {
        window.rest_show()?;
        window.rest_unminimize()?;
        window.rest_set_always_on_top(true)?;
        window.rest_set_fullscreen(true)?;
        window.rest_focus()
    })();
    let Err(error) = entered else {
        return Ok(());
    };

    let mut errors = vec![format!("进入失败：{error}")];
    if let Err(rollback_error) = window.rest_set_fullscreen(false) {
        errors.push(format!("回滚全屏失败：{rollback_error}"));
    }
    if let Err(rollback_error) = window.rest_set_always_on_top(false) {
        errors.push(format!("回滚置顶失败：{rollback_error}"));
    }
    combined_window_result("无法进入休息模式", errors)
}

fn exit_rest_window<W: RestWindowOps>(window: &W) -> Result<(), String> {
    let mut errors = Vec::new();
    if let Err(error) = window.rest_set_fullscreen(false) {
        errors.push(format!("退出全屏失败：{error}"));
    }
    if let Err(error) = window.rest_set_always_on_top(false) {
        errors.push(format!("取消置顶失败：{error}"));
    }
    if let Err(error) = window.rest_show() {
        errors.push(format!("显示窗口失败：{error}"));
    }
    if let Err(error) = window.rest_unminimize() {
        errors.push(format!("恢复最小化失败：{error}"));
    }
    if let Err(error) = window.rest_focus() {
        errors.push(format!("聚焦窗口失败：{error}"));
    }
    combined_window_result("无法完整退出休息模式", errors)
}

fn exit_rest_and_show_main(app: &tauri::AppHandle) -> Result<(), String> {
    let mut errors = Vec::new();
    match app.get_webview_window("main") {
        Some(main) => {
            if let Err(error) = exit_rest_window(&main) {
                errors.push(error);
            }
        }
        None => errors.push("主窗口不存在".to_string()),
    }
    if let Err(error) = app.emit(EXIT_REST_EVENT, ()) {
        errors.push(format!("通知界面退出休息失败：{error}"));
    }
    combined_window_result("结束休息并显示虫洞派失败", errors)
}

fn show_main_window(app: &tauri::AppHandle) -> Result<(), String> {
    let main = app
        .get_webview_window("main")
        .ok_or_else(|| "主窗口不存在".to_string())?;
    #[cfg(not(target_os = "macos"))]
    main.set_skip_taskbar(true)
        .map_err(|error| error.to_string())?;
    main.show().map_err(|error| error.to_string())?;
    main.unminimize().map_err(|error| error.to_string())?;
    main.set_focus().map_err(|error| error.to_string())?;
    app.emit("main://shown", ())
        .map_err(|error| error.to_string())
}

fn hide_main_window(app: &tauri::AppHandle) -> Result<(), String> {
    let main = app
        .get_webview_window("main")
        .ok_or_else(|| "主窗口不存在".to_string())?;
    main.hide().map_err(|error| error.to_string())
}

fn restore_pet_to_primary_work_area(app: &tauri::AppHandle) -> Result<(), String> {
    set_pet_layer(app, "normal")?;
    let pet = app
        .get_webview_window("pet")
        .ok_or_else(|| "宠物窗口不存在".to_string())?;
    let monitor = pet
        .primary_monitor()
        .map_err(|error| error.to_string())?
        .or(pet.current_monitor().map_err(|error| error.to_string())?)
        .ok_or_else(|| "无法获取可用显示器".to_string())?;
    let work = monitor.work_area();
    let size = pet.outer_size().map_err(|error| error.to_string())?;
    let current = pet.outer_position().map_err(|error| error.to_string())?;
    let min_x = i64::from(work.position.x);
    let min_y = i64::from(work.position.y);
    let max_x = min_x + i64::from(work.size.width).saturating_sub(i64::from(size.width));
    let max_y = min_y + i64::from(work.size.height).saturating_sub(i64::from(size.height));
    pet.set_position(PhysicalPosition::new(
        i64::from(current.x).clamp(min_x, max_x.max(min_x)) as i32,
        i64::from(current.y).clamp(min_y, max_y.max(min_y)) as i32,
    ))
    .map_err(|error| error.to_string())?;
    set_pet_visibility_internal(app, true)?;
    pet.set_focus().map_err(|error| error.to_string())
}

fn pet_layer_policy(layer: &str) -> Result<PetLayerPolicy, String> {
    match layer {
        "normal" => Ok(PetLayerPolicy {
            always_on_top: false,
            always_on_bottom: false,
            ignore_cursor_events: false,
            focusable: true,
        }),
        "bottom" => Ok(PetLayerPolicy {
            always_on_top: false,
            always_on_bottom: true,
            ignore_cursor_events: false,
            focusable: true,
        }),
        "top" => Ok(PetLayerPolicy {
            always_on_top: true,
            always_on_bottom: false,
            ignore_cursor_events: false,
            focusable: true,
        }),
        _ => Err("不支持的宠物层级".to_string()),
    }
}

fn set_pet_layer(app: &tauri::AppHandle, layer: &str) -> Result<(), String> {
    let pet = app
        .get_webview_window("pet")
        .ok_or_else(|| "宠物窗口不存在".to_string())?;
    let policy = pet_layer_policy(layer)?;
    // Always clear the previous exclusive layer first. Switching directly from
    // bottom to top (or top to bottom) can otherwise leave the native window in
    // the neutral layer after the second flag cancels the first one.
    pet.set_always_on_top(false)
        .map_err(|error| error.to_string())?;
    pet.set_always_on_bottom(false)
        .map_err(|error| error.to_string())?;
    if policy.always_on_top {
        pet.set_always_on_top(true)
            .map_err(|error| error.to_string())?;
    } else if policy.always_on_bottom {
        pet.set_always_on_bottom(true)
            .map_err(|error| error.to_string())?;
    }
    pet.set_ignore_cursor_events(policy.ignore_cursor_events)
        .map_err(|error| error.to_string())?;
    pet.set_focusable(policy.focusable)
        .map_err(|error| error.to_string())?;
    app.emit("pet://layer-changed", layer.to_string())
        .map_err(|error| error.to_string())
}

fn set_pet_visibility_internal(app: &tauri::AppHandle, visible: bool) -> Result<(), String> {
    let pet = app
        .get_webview_window("pet")
        .ok_or_else(|| "宠物窗口不存在".to_string())?;
    if visible {
        pet.show().map_err(|error| error.to_string())?;
    } else {
        pet.hide().map_err(|error| error.to_string())?;
        if let Some(dialogue) = app.get_webview_window("pet-dialogue") {
            let _ = dialogue.hide();
        }
    }
    app.emit("pet://visibility-changed", visible)
        .map_err(|error| error.to_string())
}

fn show_pet_dialogue_window(app: &tauri::AppHandle) -> Result<(), String> {
    let pet = app
        .get_webview_window("pet")
        .ok_or_else(|| "宠物窗口不存在".to_string())?;
    let dialogue = app
        .get_webview_window("pet-dialogue")
        .ok_or_else(|| "宠物对话窗口不存在".to_string())?;
    let monitor = pet
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or(pet.primary_monitor().map_err(|error| error.to_string())?)
        .ok_or_else(|| "无法获取宠物所在显示器".to_string())?;
    let pet_position = pet.outer_position().map_err(|error| error.to_string())?;
    let pet_size = pet.outer_size().map_err(|error| error.to_string())?;
    let dialogue_size = dialogue.outer_size().map_err(|error| error.to_string())?;
    let work = monitor.work_area();
    let gap = (12.0 * monitor.scale_factor()).round() as i64;
    let min_x = i64::from(work.position.x);
    let min_y = i64::from(work.position.y);
    let max_x = min_x + i64::from(work.size.width) - i64::from(dialogue_size.width);
    let max_y = min_y + i64::from(work.size.height) - i64::from(dialogue_size.height);
    let right_x = i64::from(pet_position.x) + i64::from(pet_size.width) + gap;
    let left_x = i64::from(pet_position.x) - i64::from(dialogue_size.width) - gap;
    let desired_x = if right_x <= max_x { right_x } else { left_x };
    let desired_y = i64::from(pet_position.y)
        + (i64::from(pet_size.height) - i64::from(dialogue_size.height)) / 2;
    dialogue
        .set_position(PhysicalPosition::new(
            desired_x.clamp(min_x, max_x.max(min_x)) as i32,
            desired_y.clamp(min_y, max_y.max(min_y)) as i32,
        ))
        .map_err(|error| error.to_string())?;
    #[cfg(not(target_os = "macos"))]
    dialogue
        .set_skip_taskbar(true)
        .map_err(|error| error.to_string())?;
    dialogue.show().map_err(|error| error.to_string())?;
    dialogue.unminimize().map_err(|error| error.to_string())?;
    dialogue.set_focus().map_err(|error| error.to_string())
}

fn handle_native_menu_event(app: &tauri::AppHandle, id: &str) {
    match id {
        TRAY_SHOW_ID | PET_MENU_OPEN_ID => {
            let _ = show_main_window(app);
        }
        TRAY_EXIT_REST_ID => {
            let _ = exit_rest_and_show_main(app);
        }
        TRAY_RESTORE_PET_ID => {
            let _ = restore_pet_to_primary_work_area(app);
        }
        TRAY_QUIT_ID => app.exit(0),
        PET_MENU_DIALOGUE_ID => {
            let _ = app.emit("ui://open-dialogue", ());
        }
        PET_MENU_SETTINGS_ID => {
            if show_main_window(app).is_ok() {
                let _ = app.emit("ui://open-pet-settings", ());
            }
        }
        PET_MENU_LAYER_NORMAL_ID => {
            let _ = set_pet_layer(app, "normal");
        }
        PET_MENU_LAYER_TOP_ID => {
            let _ = set_pet_layer(app, "top");
        }
        PET_MENU_LAYER_BOTTOM_ID => {
            let _ = set_pet_layer(app, "bottom");
        }
        PET_MENU_HIDE_ID => {
            let _ = set_pet_visibility_internal(app, false);
        }
        _ => {}
    }
}

#[tauri::command]
fn window_action(action: String, window: tauri::WebviewWindow) -> Result<(), String> {
    match action.as_str() {
        "minimize" if window.label() == "main" => window.hide().map_err(|error| error.to_string()),
        "minimize" => window.minimize().map_err(|error| error.to_string()),
        "close" => window.close().map_err(|error| error.to_string()),
        "enter_rest" => enter_rest_window(&window),
        "exit_rest" => exit_rest_window(&window),
        _ => Err("不支持的窗口操作".to_string()),
    }
}

#[tauri::command]
fn hide_main_to_tray(app: tauri::AppHandle) -> Result<(), String> {
    hide_main_window(&app)
}

#[tauri::command]
fn show_main_from_tray(app: tauri::AppHandle) -> Result<(), String> {
    show_main_window(&app)
}

#[tauri::command]
fn quit_app(app: tauri::AppHandle) -> Result<(), String> {
    app.exit(0);
    Ok(())
}

#[tauri::command]
fn show_pet_dialogue(app: tauri::AppHandle) -> Result<(), String> {
    show_pet_dialogue_window(&app)
}

#[tauri::command]
fn hide_pet_dialogue(app: tauri::AppHandle) -> Result<(), String> {
    let dialogue = app
        .get_webview_window("pet-dialogue")
        .ok_or_else(|| "宠物对话窗口不存在".to_string())?;
    dialogue.hide().map_err(|error| error.to_string())
}

#[tauri::command]
fn show_pet_context_menu(app: tauri::AppHandle) -> Result<(), String> {
    let pet = app
        .get_webview_window("pet")
        .ok_or_else(|| "宠物窗口不存在".to_string())?;
    let open_main = MenuItem::with_id(&app, PET_MENU_OPEN_ID, "打开虫洞派", true, None::<&str>)
        .map_err(|error| error.to_string())?;
    let dialogue = MenuItem::with_id(&app, PET_MENU_DIALOGUE_ID, "开始对话", true, None::<&str>)
        .map_err(|error| error.to_string())?;
    let settings = MenuItem::with_id(&app, PET_MENU_SETTINGS_ID, "宠物设置", true, None::<&str>)
        .map_err(|error| error.to_string())?;
    let normal = MenuItem::with_id(
        &app,
        PET_MENU_LAYER_NORMAL_ID,
        "普通层级",
        true,
        None::<&str>,
    )
    .map_err(|error| error.to_string())?;
    let top = MenuItem::with_id(&app, PET_MENU_LAYER_TOP_ID, "置顶", true, None::<&str>)
        .map_err(|error| error.to_string())?;
    let bottom = MenuItem::with_id(
        &app,
        PET_MENU_LAYER_BOTTOM_ID,
        "桌面底层",
        true,
        None::<&str>,
    )
    .map_err(|error| error.to_string())?;
    let hide = MenuItem::with_id(&app, PET_MENU_HIDE_ID, "隐藏宠物", true, None::<&str>)
        .map_err(|error| error.to_string())?;
    let separator_one = PredefinedMenuItem::separator(&app).map_err(|error| error.to_string())?;
    let separator_two = PredefinedMenuItem::separator(&app).map_err(|error| error.to_string())?;
    let menu = Menu::with_items(
        &app,
        &[
            &open_main,
            &dialogue,
            &settings,
            &separator_one,
            &normal,
            &top,
            &bottom,
            &separator_two,
            &hide,
        ],
    )
    .map_err(|error| error.to_string())?;
    pet.popup_menu(&menu).map_err(|error| error.to_string())
}

#[tauri::command]
fn cursor_position(window: tauri::WebviewWindow) -> Result<CursorPoint, String> {
    let position = window
        .cursor_position()
        .map_err(|error| error.to_string())?;
    let window_position = window.outer_position().map_err(|error| error.to_string())?;
    let scale_factor = window.scale_factor().map_err(|error| error.to_string())?;
    Ok(relative_logical_cursor(
        position,
        window_position,
        scale_factor,
    ))
}

fn relative_logical_cursor(
    position: PhysicalPosition<f64>,
    window_position: PhysicalPosition<i32>,
    scale_factor: f64,
) -> CursorPoint {
    let scale_factor = scale_factor.max(f64::EPSILON);
    CursorPoint {
        x: (position.x - f64::from(window_position.x)) / scale_factor,
        y: (position.y - f64::from(window_position.y)) / scale_factor,
    }
}

#[tauri::command]
fn pet_visibility(visible: bool, app: tauri::AppHandle) -> Result<(), String> {
    set_pet_visibility_internal(&app, visible)
}

#[tauri::command]
fn pet_layer(layer: String, app: tauri::AppHandle) -> Result<(), String> {
    set_pet_layer(&app, &layer)
}

#[cfg(any(
    target_os = "windows",
    all(
        unix,
        not(target_os = "macos"),
        not(target_os = "ios"),
        not(target_os = "android")
    )
))]
fn restore_paths_from_trash(paths: &[PathBuf]) -> Result<(), String> {
    let trash_items = trash::os_limited::list().map_err(|error| error.to_string())?;
    let mut selected = Vec::new();
    for path in paths {
        if let Some(item) = trash_items
            .iter()
            .filter(|item| item.original_path() == *path)
            .max_by_key(|item| item.time_deleted)
        {
            selected.push(item.clone());
        }
    }
    if selected.len() != paths.len() {
        return Err("回收站中找不到完整的待恢复记录".to_string());
    }
    trash::os_limited::restore_all(selected).map_err(|error| error.to_string())
}

#[cfg(not(any(
    target_os = "windows",
    all(
        unix,
        not(target_os = "macos"),
        not(target_os = "ios"),
        not(target_os = "android")
    )
)))]
fn restore_paths_from_trash(_paths: &[PathBuf]) -> Result<(), String> {
    Err("当前系统暂不支持从废纸篓自动恢复，请在废纸篓中手动恢复".to_string())
}

#[tauri::command]
fn feed_files(
    paths: Vec<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<FeedEvent, String> {
    if paths.is_empty() {
        return Err("没有收到文件".to_string());
    }
    if paths.len() > 20 {
        return Err("一次最多喂给宠物 20 个项目".to_string());
    }
    let _operation_guard = state
        .organize_lock
        .lock()
        .map_err(|_| "桌面整理操作已锁定".to_string())?;

    let mut approved = Vec::new();
    for raw_path in paths {
        approved.push(validate_feed_path(&state.index, Path::new(&raw_path))?);
    }

    let requested_count = approved.len();
    let mut deleted = Vec::new();
    let mut partial_error = None;
    for path in approved {
        match trash::delete(&path) {
            Ok(()) => deleted.push(path),
            Err(error) => {
                if deleted.is_empty() {
                    return Err(format!("送入回收站失败：{error}"));
                }
                if restore_paths_from_trash(&deleted).is_ok() {
                    let _ = scan_index(&state.index, true);
                    return Err(format!("送入回收站失败，已恢复本次先前项目：{error}"));
                }
                partial_error = Some(format!(
                    "送入回收站时部分失败，且自动恢复未完成；已有 {} 个项目可能仍在回收站，请立即检查或尝试撤销：{error}",
                    deleted.len()
                ));
                break;
            }
        }
    }

    let names = deleted
        .iter()
        .map(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| path.display().to_string())
        })
        .collect::<Vec<_>>();
    if let Ok(mut last_feed) = state.last_feed.lock() {
        *last_feed = deleted;
    }
    let _ = scan_index(&state.index, true);
    let payload = FeedEvent {
        count: names.len(),
        failed_count: requested_count.saturating_sub(names.len()),
        names,
        warning: partial_error.clone(),
    };
    let _ = app.emit("pet://fed", payload.clone());
    Ok(payload)
}

#[tauri::command]
fn undo_last_feed(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<FeedEvent, String> {
    let _operation_guard = state
        .organize_lock
        .lock()
        .map_err(|_| "桌面整理操作已锁定".to_string())?;
    let paths = state
        .last_feed
        .lock()
        .map_err(|_| "回收站记录已锁定".to_string())?
        .clone();
    if paths.is_empty() {
        return Err("没有可以撤销的喂食记录".to_string());
    }

    restore_paths_from_trash(&paths)?;
    if let Ok(mut last_feed) = state.last_feed.lock() {
        last_feed.clear();
    }
    let _ = scan_index(&state.index, true);
    let names = paths
        .iter()
        .map(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| path.display().to_string())
        })
        .collect::<Vec<_>>();
    let payload = FeedEvent {
        count: names.len(),
        failed_count: 0,
        names,
        warning: None,
    };
    let _ = app.emit("pet://restored", payload.clone());
    Ok(payload)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = show_main_window(app);
            let _ = restore_pet_to_primary_work_area(app);
        }))
        .on_menu_event(|app, event| handle_native_menu_event(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                let _ = show_main_window(tray.app_handle());
            }
        })
        .on_window_event(|window, event| {
            if matches!(window.label(), "main" | "pet" | "pet-dialogue") {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    if window.label() == "main" {
                        match window.is_fullscreen() {
                            Ok(false) => {
                                let _ = window.hide();
                            }
                            Ok(true) | Err(_) => {
                                let _ = exit_rest_window(window);
                                let _ = window.app_handle().emit(EXIT_REST_EVENT, ());
                            }
                        }
                    } else {
                        let _ = window.hide();
                    }
                }
            }
        })
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&app_data_dir)?;
            let desktop_path = app.path().desktop_dir()?.canonicalize()?;
            let documents_path = app.path().document_dir()?;
            fs::create_dir_all(&documents_path)?;
            let documents_path = documents_path.canonicalize()?;
            let library_path = documents_path.join(LIBRARY_ROOT_NAME);
            let legacy_library_path = desktop_path.join(LEGACY_ORGANIZE_ROOT_NAME);
            let mut initial_directories = Vec::new();
            verify_or_create_directory(&library_path, &mut initial_directories)
                .map_err(std::io::Error::other)?;
            let connection = Connection::open(app_data_dir.join("zuo-index.sqlite3"))?;
            init_database(&connection).map_err(std::io::Error::other)?;
            let organize_batches =
                load_organize_batches(&connection).map_err(std::io::Error::other)?;

            let baseline_complete = connection
                .query_row(
                    "SELECT value FROM app_meta WHERE key = 'baseline_complete'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .is_ok();
            if !baseline_complete {
                connection.execute("UPDATE files SET indexed_at = 0", [])?;
            }

            let index = SharedIndex {
                db: Arc::new(Mutex::new(connection)),
                desktop_path,
                library_path,
                legacy_library_path,
            };
            let _ = scan_index(&index, baseline_complete);
            if !baseline_complete {
                if let Ok(connection) = index.db.lock() {
                    let _ = connection.execute(
                        "INSERT OR REPLACE INTO app_meta(key, value) VALUES ('baseline_complete', '1')",
                        [],
                    );
                }
            }
            app.manage(AppState {
                index,
                watcher: Mutex::new(None),
                last_feed: Mutex::new(Vec::new()),
                organize_batches: Mutex::new(organize_batches),
                organize_lock: Arc::new(Mutex::new(())),
            });

            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let main = app
                .get_webview_window("main")
                .ok_or_else(|| std::io::Error::other("主窗口不存在"))?;
            #[cfg(not(target_os = "macos"))]
            main.set_skip_taskbar(true)?;
            position_main_top_right(&main).map_err(std::io::Error::other)?;

            let pet = app
                .get_webview_window("pet")
                .ok_or_else(|| std::io::Error::other("宠物窗口不存在"))?;
            #[cfg(not(target_os = "macos"))]
            pet.set_skip_taskbar(true)?;

            let pet_dialogue = app
                .get_webview_window("pet-dialogue")
                .ok_or_else(|| std::io::Error::other("宠物对话窗口不存在"))?;
            #[cfg(not(target_os = "macos"))]
            pet_dialogue.set_skip_taskbar(true)?;

            let tray_show = MenuItem::with_id(
                app,
                TRAY_SHOW_ID,
                "显示虫洞派",
                true,
                None::<&str>,
            )?;
            let tray_exit_rest = MenuItem::with_id(
                app,
                TRAY_EXIT_REST_ID,
                "结束休息并显示虫洞派",
                true,
                None::<&str>,
            )?;
            let tray_restore_pet = MenuItem::with_id(
                app,
                TRAY_RESTORE_PET_ID,
                "找回宠物（普通层级）",
                true,
                None::<&str>,
            )?;
            let tray_separator = PredefinedMenuItem::separator(app)?;
            let tray_quit =
                MenuItem::with_id(app, TRAY_QUIT_ID, "退出虫洞派", true, None::<&str>)?;
            let tray_menu = Menu::with_items(
                app,
                &[
                    &tray_show,
                    &tray_exit_rest,
                    &tray_restore_pet,
                    &tray_separator,
                    &tray_quit,
                ],
            )?;
            let mut tray = TrayIconBuilder::with_id("wormhole-pie-tray")
                .menu(&tray_menu)
                .tooltip("虫洞派")
                .show_menu_on_left_click(false);
            if let Some(icon) = app.default_window_icon().cloned() {
                tray = tray.icon(icon);
            }
            tray.build(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_files,
            scan_desktop,
            get_organize_state,
            organize_desktop,
            review_desktop_organize,
            list_organize_exclusions,
            remove_organize_exclusion,
            undo_desktop_organize,
            update_file_category,
            open_file,
            list_programs,
            launch_program,
            list_agent_connectors,
            get_agent_default_workspace,
            pick_dialogue_files,
            get_agent_task_status,
            get_agent_task_result,
            stop_agent_task,
            run_agent_task,
            open_agent_result_file,
            get_desktop_icon_state,
            set_desktop_icons_hidden,
            recognize_speech_local,
            open_social,
            open_external_url,
            start_watching,
            window_action,
            hide_main_to_tray,
            show_main_from_tray,
            quit_app,
            show_pet_dialogue,
            hide_pet_dialogue,
            show_pet_context_menu,
            cursor_position,
            pet_visibility,
            pet_layer,
            feed_files,
            undo_last_feed
        ])
        .run(tauri::generate_context!())
        .expect("error while running Wormhole Pie");
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeRestWindow {
        calls: std::cell::RefCell<Vec<String>>,
        failures: std::collections::HashSet<String>,
    }

    impl FakeRestWindow {
        fn failing(operations: impl IntoIterator<Item = &'static str>) -> Self {
            Self {
                calls: std::cell::RefCell::new(Vec::new()),
                failures: operations.into_iter().map(str::to_string).collect(),
            }
        }

        fn perform(&self, operation: &str) -> Result<(), String> {
            self.calls.borrow_mut().push(operation.to_string());
            if self.failures.contains(operation) {
                Err(format!("{operation} failed"))
            } else {
                Ok(())
            }
        }
    }

    impl RestWindowOps for FakeRestWindow {
        fn rest_show(&self) -> Result<(), String> {
            self.perform("show")
        }

        fn rest_unminimize(&self) -> Result<(), String> {
            self.perform("unminimize")
        }

        fn rest_set_always_on_top(&self, enabled: bool) -> Result<(), String> {
            self.perform(if enabled { "top:true" } else { "top:false" })
        }

        fn rest_set_fullscreen(&self, enabled: bool) -> Result<(), String> {
            self.perform(if enabled {
                "fullscreen:true"
            } else {
                "fullscreen:false"
            })
        }

        fn rest_focus(&self) -> Result<(), String> {
            self.perform("focus")
        }
    }

    struct TempWorkspace(PathBuf);

    impl TempWorkspace {
        fn new(label: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "wormhole-pie-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn test_index(workspace: &TempWorkspace) -> SharedIndex {
        let desktop = workspace.0.join("Desktop");
        let library = workspace.0.join("Documents").join(LIBRARY_ROOT_NAME);
        fs::create_dir_all(&desktop).unwrap();
        fs::create_dir_all(&library).unwrap();
        let desktop = desktop.canonicalize().unwrap();
        let library = library.canonicalize().unwrap();
        let connection = Connection::open_in_memory().unwrap();
        init_database(&connection).unwrap();
        SharedIndex {
            db: Arc::new(Mutex::new(connection)),
            legacy_library_path: desktop.join(LEGACY_ORGANIZE_ROOT_NAME),
            desktop_path: desktop,
            library_path: library,
        }
    }

    #[test]
    fn scan_includes_desktop_and_direct_library_items_without_deep_recursion() {
        let workspace = TempWorkspace::new("scan");
        let index = test_index(&workspace);
        fs::write(index.desktop_path.join("loose.txt"), b"desktop").unwrap();
        let documents = index.library_path.join("文档");
        let folders = index.library_path.join("文件夹");
        fs::create_dir_all(&documents).unwrap();
        fs::create_dir_all(folders.join("Project")).unwrap();
        fs::write(documents.join("report.pdf"), b"pdf").unwrap();
        fs::write(folders.join("Project").join("nested.txt"), b"nested").unwrap();

        let files = scan_index(&index, false).unwrap();
        assert_eq!(files.len(), 3);
        assert!(files
            .iter()
            .any(|file| { file.name == "report.pdf" && file.organized_category == "文档" }));
        assert!(files
            .iter()
            .any(|file| { file.name == "Project" && file.organized_category == "文件夹" }));
        assert!(!files.iter().any(|file| file.name == "nested.txt"));
    }

    #[test]
    fn repeated_plans_move_only_new_desktop_items_and_preserve_index_identity() {
        let workspace = TempWorkspace::new("repeat");
        let index = test_index(&workspace);
        let first = index.desktop_path.join("first.txt");
        let ignored = index.desktop_path.join("keep.lnk");
        fs::write(&first, b"first").unwrap();
        fs::write(&ignored, b"shortcut").unwrap();
        let initial = scan_index(&index, false).unwrap();
        let first_id = initial
            .iter()
            .find(|file| file.name == "first.txt")
            .unwrap()
            .id;
        {
            let connection = index.db.lock().unwrap();
            connection
                .execute(
                    "UPDATE files SET category = '工作项目', indexed_at = 1234 WHERE id = ?1",
                    [first_id],
                )
                .unwrap();
        }

        let exclusions = HashSet::from([normalize_top_level_name(OsStr::new("keep.lnk"))]);
        let first_plan = collect_organize_plan(&index.desktop_path, &exclusions).unwrap();
        assert_eq!(first_plan.candidates.len(), 1);
        assert_eq!(first_plan.excluded_count, 1);
        assert_eq!(first_plan.candidates[0].category, "文档");
        let category = index.library_path.join("文档");
        fs::create_dir_all(&category).unwrap();
        let first_destination = category.join("first.txt");
        let first_transfer = vec![(first.clone(), first_destination.clone(), false)];
        execute_transfers(&first_transfer).unwrap();
        repath_index_transfers(&index, &first_transfer).unwrap();
        let after_first = scan_index(&index, true).unwrap();
        let indexed_first = after_first
            .iter()
            .find(|file| file.name == "first.txt")
            .unwrap();
        assert_eq!(indexed_first.id, first_id);
        assert_eq!(indexed_first.category, "工作项目");
        assert_eq!(indexed_first.organized_category, "文档");
        assert_eq!(PathBuf::from(&indexed_first.path), first_destination);

        let second = index.desktop_path.join("second.png");
        fs::write(&second, b"png").unwrap();
        let second_plan = collect_organize_plan(&index.desktop_path, &exclusions).unwrap();
        assert_eq!(second_plan.candidates.len(), 1);
        assert_eq!(second_plan.candidates[0].name, OsStr::new("second.png"));
        assert_eq!(second_plan.candidates[0].category, "图片");
        let second_category = index.library_path.join("图片");
        fs::create_dir_all(&second_category).unwrap();
        let second_transfer = vec![(second, second_category.join("second.png"), false)];
        execute_transfers(&second_transfer).unwrap();
        repath_index_transfers(&index, &second_transfer).unwrap();
        let after_second = scan_index(&index, true).unwrap();
        assert!(after_second.iter().any(|file| file.name == "first.txt"));
        assert!(after_second.iter().any(|file| file.name == "second.png"));
        assert!(ignored.exists());
    }

    #[test]
    fn legacy_migration_can_restore_original_category_structure() {
        let workspace = TempWorkspace::new("legacy");
        let index = test_index(&workspace);
        let legacy_category = index.legacy_library_path.join("文档");
        fs::create_dir_all(&legacy_category).unwrap();
        let original = legacy_category.join("old.txt");
        fs::write(&original, b"old").unwrap();
        let mut plan = collect_organize_plan(&index.desktop_path, &HashSet::new()).unwrap();
        append_legacy_library_plan(&index.legacy_library_path, &mut plan);
        assert_eq!(plan.candidates.len(), 1);
        let destination_category = index.library_path.join("文档");
        fs::create_dir_all(&destination_category).unwrap();
        let destination = destination_category.join("old.txt");
        let forward = vec![(original.clone(), destination.clone(), false)];
        execute_transfers(&forward).unwrap();
        let legacy_directories = legacy_cleanup_directories(&index.legacy_library_path);
        remove_created_directories(&legacy_directories).unwrap();
        assert!(!index.legacy_library_path.exists());

        let mut recreated = Vec::new();
        ensure_restore_parent(&index, original.parent().unwrap(), &mut recreated).unwrap();
        execute_transfers(&[(destination, original.clone(), false)]).unwrap();
        assert!(original.exists());
        assert!(legacy_category.is_dir());
    }

    #[test]
    fn three_batches_survive_database_reopen() {
        let workspace = TempWorkspace::new("history");
        let db_path = workspace.0.join("history.sqlite3");
        let desktop = workspace.0.join("Desktop");
        let library = workspace.0.join("Documents").join(LIBRARY_ROOT_NAME);
        fs::create_dir_all(&desktop).unwrap();
        fs::create_dir_all(&library).unwrap();
        {
            let connection = Connection::open(&db_path).unwrap();
            init_database(&connection).unwrap();
            let index = SharedIndex {
                db: Arc::new(Mutex::new(connection)),
                desktop_path: desktop.clone(),
                library_path: library.clone(),
                legacy_library_path: desktop.join(LEGACY_ORGANIZE_ROOT_NAME),
            };
            let batches = (0..3)
                .map(|number| DesktopOrganizeBatch {
                    batch_id: format!("batch-{number}"),
                    root: library.clone(),
                    moves: vec![DesktopOrganizeMove {
                        move_id: format!("move-{number}"),
                        original: desktop.join(format!("file-{number}.txt")),
                        organized: library.join("文档").join(format!("file-{number}.txt")),
                        category: "文档".to_string(),
                        is_dir: false,
                    }],
                    created_directories: Vec::new(),
                })
                .collect::<Vec<_>>();
            persist_organize_batches(&index, &batches).unwrap();
        }
        let reopened = Connection::open(&db_path).unwrap();
        init_database(&reopened).unwrap();
        let loaded = load_organize_batches(&reopened).unwrap();
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded.last().unwrap().batch_id, "batch-2");
    }

    #[test]
    fn category_and_pet_layer_policies_are_stable() {
        assert_eq!(organize_category(Path::new("main.rs"), false), "代码");
        assert_eq!(organize_category(Path::new("app.tsx"), false), "代码");
        assert_eq!(organize_category(Path::new("photo.png"), false), "图片");
        assert_eq!(organize_category(Path::new("资料"), true), "文件夹");
        assert_eq!(organize_category(Path::new("Example.app"), true), "应用");
        assert!(is_wormhole_shortcut_name(OsStr::new("虫洞派.lnk")));
        assert!(is_wormhole_shortcut_name(OsStr::new("wormhole pie.LNK")));
        let bottom = pet_layer_policy("bottom").unwrap();
        assert!(bottom.always_on_bottom);
        assert!(!bottom.ignore_cursor_events);
        assert!(bottom.focusable);
    }

    #[test]
    fn cursor_coordinates_are_relative_logical_pixels() {
        let point = relative_logical_cursor(
            PhysicalPosition::new(450.0, 300.0),
            PhysicalPosition::new(300, 150),
            1.5,
        );
        assert!((point.x - 100.0).abs() < f64::EPSILON);
        assert!((point.y - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn feed_validation_rejects_roots_and_managed_category_directories() {
        let workspace = TempWorkspace::new("feed-roots");
        let index = test_index(&workspace);
        fs::create_dir_all(&index.legacy_library_path).unwrap();
        let category = index.library_path.join("文档");
        fs::create_dir_all(&category).unwrap();
        let ordinary = index.desktop_path.join("note.txt");
        fs::write(&ordinary, b"safe temporary fixture").unwrap();

        assert!(validate_feed_path(&index, &index.desktop_path).is_err());
        assert!(validate_feed_path(&index, &index.library_path).is_err());
        assert!(validate_feed_path(&index, &index.legacy_library_path).is_err());
        assert!(validate_feed_path(&index, &category).is_err());
        assert_eq!(
            validate_feed_path(&index, &ordinary).unwrap(),
            ordinary.canonicalize().unwrap()
        );
    }

    #[test]
    fn feed_validation_rejects_symlink_and_reparse_flags() {
        assert!(validate_feed_entry_flags(true, false, false, true, false).is_err());
        assert!(validate_feed_entry_flags(false, true, false, true, false).is_err());
        assert!(validate_feed_entry_flags(false, false, true, true, false).is_err());
        assert!(validate_feed_entry_flags(false, false, false, false, false).is_err());
        assert!(validate_feed_entry_flags(false, false, false, true, false).is_ok());
        assert!(validate_feed_entry_flags(false, false, false, false, true).is_ok());
    }

    #[test]
    fn active_content_extensions_require_confirmation_while_scripts_are_text_safe() {
        for extension in [
            "com",
            "scr",
            "cpl",
            "hta",
            "jar",
            "jnlp",
            "pif",
            "scf",
            "application",
            "gadget",
            "reg",
            "inf",
            "jse",
            "vbe",
            "ws",
            "wsc",
            "wsf",
            "wsh",
            "msp",
            "mst",
            "app",
        ] {
            assert!(requires_execution_confirmation(extension), "{extension}");
        }
        for extension in ["js", "ps1", "bat", "cmd", "vbs", "sh"] {
            assert!(is_safe_text_script_extension(extension), "{extension}");
        }
        assert!(!requires_execution_confirmation("pdf"));
        assert!(!is_safe_text_script_extension("pdf"));
    }

    #[test]
    fn program_authorization_respects_recursive_and_direct_roots() {
        let workspace = TempWorkspace::new("program-roots");
        let direct_root = workspace.0.join("Desktop");
        let recursive_root = workspace.0.join("StartMenu");
        let outside = workspace.0.join("Downloads").join("tool.exe");
        let roots = vec![(direct_root.clone(), false), (recursive_root.clone(), true)];

        assert!(path_is_in_program_roots(
            &direct_root.join("tool.exe"),
            &roots
        ));
        assert!(!path_is_in_program_roots(
            &direct_root.join("folder").join("tool.exe"),
            &roots
        ));
        assert!(path_is_in_program_roots(
            &recursive_root.join("folder").join("tool.exe"),
            &roots
        ));
        assert!(!path_is_in_program_roots(&outside, &roots));
    }

    #[test]
    fn undo_result_uses_the_actual_unique_restore_path() {
        let move_record = DesktopOrganizeMove {
            move_id: "move-1".to_string(),
            original: PathBuf::from(r"C:\Users\owner\Desktop\Tool.lnk"),
            organized: PathBuf::from(r"C:\Users\owner\Documents\虫洞派资料库\快捷方式\Tool.lnk"),
            category: "快捷方式".to_string(),
            is_dir: false,
        };
        let actual = PathBuf::from(r"C:\Users\owner\Desktop\Tool (2).lnk");
        let item = restored_organize_item(&move_record, &actual);
        assert_eq!(item.original_path, actual.to_string_lossy());
        assert_eq!(item.name, "Tool (2).lnk");
        assert_eq!(item.organized_path, move_record.organized.to_string_lossy());
    }

    #[test]
    fn legacy_library_is_read_authorized_and_cleanup_paths_stay_bounded() {
        let workspace = TempWorkspace::new("legacy-auth");
        let index = test_index(&workspace);
        let legacy_file = index.legacy_library_path.join("文档").join("old.pdf");
        assert!(path_is_authorized(&index, &legacy_file));
        assert!(!path_is_authorized(
            &index,
            &workspace.0.join("Downloads").join("outside.pdf")
        ));

        let allowed = vec![index.library_path.clone(), index.library_path.join("文档")];
        assert!(validate_created_directories(&index.library_path, &allowed).is_ok());
        assert!(validate_created_directories(
            &index.library_path,
            &[workspace.0.join("unrelated")]
        )
        .is_err());
    }

    #[test]
    fn speech_and_registry_outputs_are_parsed_without_external_services() {
        assert_eq!(
            parse_local_speech_output(true, "OK:打开项目报告".as_bytes(), b"").unwrap(),
            "打开项目报告"
        );
        assert!(parse_local_speech_output(false, b"", b"ERR_NO_SPEECH").is_err());
        assert_eq!(
            parse_hide_icons_registry_output(b"HideIcons    REG_DWORD    0x1"),
            Some(true)
        );
    }

    #[test]
    fn agent_tasks_and_workspaces_are_strictly_validated() {
        assert!(validate_agent_task("   ").is_err());
        assert!(validate_agent_task("bad\0task").is_err());
        assert!(validate_agent_task(&"好".repeat(AGENT_TASK_MAX_CHARS)).is_ok());
        assert!(validate_agent_task(&"好".repeat(AGENT_TASK_MAX_CHARS + 1)).is_err());

        let workspace = TempWorkspace::new("agent-workspace");
        let canonical = validate_agent_workspace(&workspace.0.to_string_lossy()).unwrap();
        assert_eq!(canonical, workspace.0.canonicalize().unwrap());
        let file = workspace.0.join("not-a-directory.txt");
        fs::write(&file, b"fixture").unwrap();
        assert!(validate_agent_workspace(&file.to_string_lossy()).is_err());
        assert!(validate_agent_workspace("relative/workspace").is_err());
        assert!(validate_agent_workspace(&workspace.0.join("missing").to_string_lossy()).is_err());
    }

    #[test]
    fn agent_attachments_are_bounded_staged_and_cleaned_without_leaking_source_paths() {
        let workspace = TempWorkspace::new("agent-attachment-workspace");
        let external = TempWorkspace::new("agent-attachment-external");
        let external_file = external.0.join("用户资料.txt");
        fs::write(&external_file, b"attachment fixture").unwrap();
        let workspace_path = workspace.0.canonicalize().unwrap();

        let prepared = prepare_agent_attachments(
            &workspace_path,
            vec![external_file.to_string_lossy().to_string()],
            "claude-test-1",
        )
        .unwrap();
        assert_eq!(prepared.items.len(), 1);
        assert!(prepared.items[0].snapshot);
        let snapshot = workspace_path.join(&prepared.items[0].relative_path);
        assert_eq!(fs::read(&snapshot).unwrap(), b"attachment fixture");
        let prompt = compose_agent_task_with_attachments("读取附件", &prepared);
        assert!(prompt.contains("WORMHOLE_FILE"));
        assert!(prompt.contains("用户资料.txt"));
        assert!(!prompt.contains(&external.0.to_string_lossy().to_string()));
        let cleanup_dir = prepared.cleanup_dir.clone().unwrap();
        drop(prepared);
        assert!(!cleanup_dir.exists());
    }

    #[test]
    fn agent_attachments_reject_directories_and_excess_counts() {
        let workspace = TempWorkspace::new("agent-attachment-limits");
        let workspace_path = workspace.0.canonicalize().unwrap();
        assert!(prepare_agent_attachments(
            &workspace_path,
            vec![workspace_path.to_string_lossy().to_string()],
            "codex-test-dir",
        )
        .is_err());
        assert!(prepare_agent_attachments(
            &workspace_path,
            (0..=AGENT_ATTACHMENT_MAX_COUNT)
                .map(|index| workspace_path
                    .join(format!("missing-{index}.txt"))
                    .to_string_lossy()
                    .to_string())
                .collect(),
            "codex-test-count",
        )
        .is_err());
    }

    #[test]
    fn agent_result_files_are_verified_deduplicated_and_hidden_markers_are_removed() {
        let workspace = TempWorkspace::new("agent-result-files");
        let workspace_path = workspace.0.canonicalize().unwrap();
        let report_dir = workspace_path.join("reports");
        fs::create_dir(&report_dir).unwrap();
        fs::write(report_dir.join("final report.pdf"), b"report").unwrap();
        let outside = TempWorkspace::new("agent-result-outside");
        let outside_file = outside.0.join("secret.txt");
        fs::write(&outside_file, b"secret").unwrap();
        let output = format!(
            "文件已经做好。\nWORMHOLE_FILE: reports/final report.pdf\n[再次打开](reports/final report.pdf)\nWORMHOLE_FILE: {}",
            outside_file.to_string_lossy()
        );
        let (visible, files) = extract_agent_result_files(&output, &workspace_path);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "final report.pdf");
        assert_eq!(files[0].relative_path, "reports/final report.pdf");
        assert!(!visible.contains("WORMHOLE_FILE"));
        assert!(visible.contains("文件已经做好"));
    }

    #[test]
    fn default_agent_workspace_prefers_project_then_codex_then_dedicated_directory() {
        let workspace = TempWorkspace::new("agent-default-workspace");
        let documents = workspace.0.join("Documents");
        let current = workspace.0.join("plain-current");
        fs::create_dir(&documents).unwrap();
        fs::create_dir(&current).unwrap();

        let dedicated = select_agent_default_workspace(Some(current.clone()), &documents).unwrap();
        assert_eq!(
            dedicated,
            documents
                .join("虫洞派 Agent 工作区")
                .canonicalize()
                .unwrap()
        );
        assert_ne!(dedicated, documents.canonicalize().unwrap());

        let codex = documents.join("Codex");
        fs::create_dir(&codex).unwrap();
        assert_eq!(
            select_agent_default_workspace(Some(current.clone()), &documents).unwrap(),
            codex.canonicalize().unwrap()
        );

        fs::write(current.join("package.json"), b"{}").unwrap();
        assert_eq!(
            select_agent_default_workspace(Some(current.clone()), &documents).unwrap(),
            current.canonicalize().unwrap()
        );
    }

    #[test]
    fn agent_arguments_keep_the_task_in_one_argument_without_dangerous_bypasses() {
        let task = r#"修复按钮；$(Remove-Item *) & echo done"#;
        for kind in [
            AgentConnectorKind::Codex,
            AgentConnectorKind::Claude,
            AgentConnectorKind::Hermes,
        ] {
            let arguments = agent_task_arguments(kind, task);
            assert_eq!(arguments.last(), Some(&OsString::from(task)));
            assert_eq!(
                arguments
                    .iter()
                    .filter(|argument| argument.as_os_str() == OsStr::new(task))
                    .count(),
                1
            );
            let normalized = arguments
                .iter()
                .map(|argument| argument.to_string_lossy().to_lowercase())
                .collect::<Vec<_>>();
            assert!(!normalized.iter().any(|argument| {
                argument == "--dangerously-skip-permissions"
                    || argument == "--dangerously-bypass-approvals-and-sandbox"
                    || argument == "--yolo"
                    || argument == "--accept-hooks"
                    || argument == "cmd.exe"
                    || argument == "powershell.exe"
            }));
        }

        let codex = agent_task_arguments(AgentConnectorKind::Codex, task);
        assert!(codex.iter().any(|argument| argument == "workspace-write"));
        let hermes = agent_task_arguments(AgentConnectorKind::Hermes, task);
        assert!(hermes.iter().any(|argument| argument == "--checkpoints"));
    }

    #[test]
    fn connector_cache_decision_preserves_force_refresh_and_singleflight() {
        assert_eq!(
            agent_connector_cache_decision(true, false, false, false),
            AgentConnectorCacheDecision::UseCached
        );
        assert_eq!(
            agent_connector_cache_decision(true, false, true, false),
            AgentConnectorCacheDecision::StartProbe
        );
        assert_eq!(
            agent_connector_cache_decision(false, true, true, false),
            AgentConnectorCacheDecision::WaitForProbe
        );
        assert_eq!(
            agent_connector_cache_decision(true, true, false, false),
            AgentConnectorCacheDecision::WaitForProbe
        );
        assert_eq!(
            agent_connector_cache_decision(true, false, true, true),
            AgentConnectorCacheDecision::UseCached
        );
    }

    #[test]
    fn agent_output_capture_enforces_limits_and_sanitizes_control_bytes() {
        let captured = read_pipe_limited(std::io::Cursor::new(vec![b'a'; 20]), 8);
        assert_eq!(captured.bytes.len(), 8);
        assert!(captured.truncated);
        let sanitized = sanitize_process_output(b"ok\0\x1b[31m\nnext", 64);
        assert!(!sanitized.contains('\0'));
        assert!(!sanitized.contains('\x1b'));
        assert!(sanitized.contains("next"));
    }

    #[test]
    fn agent_configuration_probe_only_reports_existing_marker_labels() {
        let workspace = TempWorkspace::new("agent-config-probe");
        let marker = workspace.0.join("config.toml");
        assert_eq!(
            first_existing_agent_configuration([
                (workspace.0.join("missing.json"), "hidden-a".to_string()),
                (marker.clone(), "safe-label".to_string()),
            ]),
            None
        );
        fs::write(&marker, b"fixture-not-read-by-probe").unwrap();
        assert_eq!(
            first_existing_agent_configuration([(marker, "safe-label".to_string())]),
            Some("safe-label".to_string())
        );
    }

    #[test]
    fn agent_connector_configuration_states_are_unambiguous() {
        assert_eq!(
            agent_connector_configuration_state(false, false, true),
            AgentConnectorConfigurationState::NotInstalled
        );
        assert_eq!(
            agent_connector_configuration_state(true, false, true),
            AgentConnectorConfigurationState::ProbeFailed
        );
        assert_eq!(
            agent_connector_configuration_state(true, true, false),
            AgentConnectorConfigurationState::NotConfigured
        );
        assert_eq!(
            agent_connector_configuration_state(true, true, true),
            AgentConnectorConfigurationState::Ready
        );
    }

    #[test]
    fn agent_task_cancellation_is_explicit_and_id_scoped() {
        let cancel = Arc::new(AtomicBool::new(false));
        let mut active = Some(ActiveAgentTask {
            status: AgentTaskStatus {
                task_id: "task-1".to_string(),
                connector_id: "claude".to_string(),
                state: AgentTaskState::Running,
                started_at: 10,
                updated_at: 10,
                elapsed_ms: 0,
                cancel_requested: false,
                detail: "running".to_string(),
            },
            cancel: cancel.clone(),
        });
        assert!(request_agent_task_cancellation(&mut active, Some("other-task"), 20).is_err());
        assert!(!cancel.load(Ordering::Acquire));

        let status = request_agent_task_cancellation(&mut active, Some("task-1"), 30)
            .unwrap()
            .unwrap();
        assert!(cancel.load(Ordering::Acquire));
        assert_eq!(status.state, AgentTaskState::Cancelling);
        assert!(status.cancel_requested);
        assert_eq!(status.updated_at, 30);
    }

    #[test]
    fn agent_task_completion_distinguishes_exit_cancel_and_timeout() {
        let result = |success, timed_out, cancelled| BoundedProcessOutput {
            success,
            timed_out,
            cancelled,
            stdout: String::new(),
            stderr: String::new(),
            exit_code: None,
            duration_ms: 0,
            truncated: false,
        };
        assert_eq!(
            completed_agent_task_state(&result(true, false, false)),
            AgentTaskState::Succeeded
        );
        assert_eq!(
            completed_agent_task_state(&result(false, false, false)),
            AgentTaskState::Failed
        );
        assert_eq!(
            completed_agent_task_state(&result(false, true, false)),
            AgentTaskState::TimedOut
        );
        assert_eq!(
            completed_agent_task_state(&result(false, false, true)),
            AgentTaskState::Cancelled
        );
    }

    #[test]
    fn failed_agent_output_never_exposes_stdout_stderr_or_paths() {
        let failed = BoundedProcessOutput {
            success: false,
            timed_out: false,
            cancelled: false,
            stdout: "partial workflow output".to_string(),
            stderr: r#"stack trace C:\Users\owner\.claude\settings.json token=secret"#.to_string(),
            exit_code: Some(1),
            duration_ms: 10,
            truncated: false,
        };
        let public = agent_task_public_output(&failed);
        assert!(!public.contains("workflow"));
        assert!(!public.contains("settings.json"));
        assert!(!public.contains("secret"));

        let succeeded = BoundedProcessOutput {
            success: true,
            stdout: "最终结果".to_string(),
            ..failed
        };
        assert_eq!(agent_task_public_output(&succeeded), "最终结果");
    }

    #[test]
    fn entering_rest_rolls_back_fullscreen_and_top_after_every_step_failure() {
        for failed_operation in ["show", "unminimize", "top:true", "fullscreen:true", "focus"] {
            let window = FakeRestWindow::failing([failed_operation]);
            let error = enter_rest_window(&window).unwrap_err();
            let calls = window.calls.borrow();
            assert!(
                calls.iter().any(|call| call == "fullscreen:false"),
                "missing fullscreen rollback after {failed_operation}: {calls:?}"
            );
            assert!(
                calls.iter().any(|call| call == "top:false"),
                "missing top rollback after {failed_operation}: {calls:?}"
            );
            assert!(error.contains("进入失败"));
        }
    }

    #[test]
    fn exiting_rest_attempts_every_recovery_step_and_combines_errors() {
        let window = FakeRestWindow::failing(["fullscreen:false", "show"]);
        let error = exit_rest_window(&window).unwrap_err();
        assert_eq!(
            *window.calls.borrow(),
            [
                "fullscreen:false",
                "top:false",
                "show",
                "unminimize",
                "focus",
            ]
        );
        assert!(error.contains("退出全屏失败"));
        assert!(error.contains("显示窗口失败"));
    }
}
