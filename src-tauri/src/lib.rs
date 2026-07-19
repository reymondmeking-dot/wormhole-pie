use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    ffi::{OsStr, OsString},
    fs,
    io::{ErrorKind, Read},
    net::{TcpStream, ToSocketAddrs},
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
use std::os::windows::ffi::{OsStrExt, OsStringExt};
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
use std::{
    io::{BufRead, BufReader, Write},
    os::windows::io::{AsRawHandle, FromRawHandle, IntoRawHandle, OwnedHandle, RawHandle},
};
#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{
        SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
        WAIT_TIMEOUT,
    },
    Security::SECURITY_ATTRIBUTES,
    Storage::FileSystem::{
        CreateFileW, FileRenameInfo, GetFileInformationByHandle, GetFinalPathNameByHandleW,
        SetFileAttributesW, SetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, DELETE,
        FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_REPARSE_POINT,
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES,
        FILE_RENAME_INFO, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TRAVERSE,
        OPEN_EXISTING,
    },
    System::{
        Com::CoTaskMemFree,
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        },
        Pipes::CreatePipe,
        Threading::{
            CreateProcessW, GetExitCodeProcess, ResumeThread, TerminateProcess,
            WaitForSingleObject, CREATE_SUSPENDED, PROCESS_INFORMATION, STARTUPINFOW,
        },
    },
    UI::{
        Shell::{
            FOLDERID_Documents, FOLDERID_ProgramFiles, FOLDERID_ProgramFilesX64,
            FOLDERID_ProgramFilesX86, FOLDERID_PublicDesktop, SHGetKnownFolderPath,
            ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
        },
        WindowsAndMessaging::SW_SHOWNORMAL,
    },
};

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
const AGENT_INSTALL_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const AGENT_INSTALL_OUTPUT_MAX_BYTES: usize = 64 * 1024;
const AGENT_CONNECTOR_CACHE_TTL: Duration = Duration::from_secs(5 * 60);
const AGENT_TASK_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(2);
const AGENT_ATTACHMENT_MAX_COUNT: usize = 8;
const AGENT_ATTACHMENT_MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const AGENT_ATTACHMENT_MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;
const AGENT_ATTACHMENT_DATA_DIR: &str = ".wormhole-pie";
const AGENT_ATTACHMENT_INPUT_DIR: &str = "agent-input";
const AGENT_RESULT_MAX_FILES: usize = 12;
#[cfg(windows)]
const SOCIAL_CDP_COMMAND_TIMEOUT: Duration = Duration::from_secs(8);
#[cfg(windows)]
const SOCIAL_CDP_MAX_MESSAGE_BYTES: usize = 1024 * 1024;
#[cfg(windows)]
const SOCIAL_EDGE_SHUTDOWN_TIMEOUT_MS: u32 = 5_000;
#[cfg(windows)]
const ELEVATED_FILE_PLAN_ARG: &str = "--elevated-file-plan";
#[cfg(windows)]
const ELEVATED_FILE_PLAN_VERSION: u8 = 2;
#[cfg(windows)]
const ELEVATED_FILE_PLAN_TIMEOUT: Duration = Duration::from_secs(120);
#[cfg(windows)]
const PUBLIC_DESKTOP_CONFIRMATION_TTL: Duration = Duration::from_secs(10);
#[cfg(windows)]
const PUBLIC_DESKTOP_ACTION_ORGANIZE: &str = "organize";
#[cfg(windows)]
const PUBLIC_DESKTOP_ACTION_REVIEW: &str = "review";
#[cfg(windows)]
const PUBLIC_DESKTOP_ACTION_UNDO: &str = "undo";
static AGENT_TASK_LOCK: Mutex<()> = Mutex::new(());
static ACTIVE_AGENT_TASK: Mutex<Option<ActiveAgentTask>> = Mutex::new(None);
static LAST_AGENT_TASK_STATUS: Mutex<Option<AgentTaskStatus>> = Mutex::new(None);
static LAST_AGENT_TASK_RESULT: Mutex<Option<AgentTaskResult>> = Mutex::new(None);
static AGENT_TASK_SEQUENCE: AtomicU64 = AtomicU64::new(1);
#[cfg(windows)]
static PUBLIC_DESKTOP_CONFIRMATION_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static APP_LOCALE: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new("zh-CN".to_string()));
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
    #[cfg(windows)]
    public_desktop_confirmations: Mutex<BTreeMap<String, PublicDesktopConfirmation>>,
    #[cfg(windows)]
    social_sessions: Mutex<BTreeMap<String, ManagedSocialSession>>,
}

#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct PublicDesktopConfirmation {
    action: String,
    batch_id: Option<String>,
    move_ids: Vec<String>,
    organize_snapshot_digest: Option<String>,
    expires_at: u64,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SocialAccountSnapshot {
    platform: String,
    display_name: String,
    followers: u64,
    unread_messages: u64,
    unread_notifications: u64,
    #[serde(default)]
    followers_source: String,
    #[serde(default)]
    unread_messages_source: String,
    #[serde(default)]
    unread_notifications_source: String,
    #[serde(default)]
    account_identity: String,
    connected: bool,
    updated_at: u64,
    #[serde(default = "social_session_persistence")]
    session_persistence: String,
    #[serde(default)]
    raw_cookie_accessed: bool,
}

#[cfg(windows)]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CdpTargetInfo {
    target_id: String,
    #[serde(rename = "type")]
    target_type: String,
    url: String,
}

#[cfg(windows)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VisibleSocialMetrics {
    page_confirmed: bool,
    selectors_confirmed: bool,
    display_name: Option<String>,
    account_identity: Option<String>,
    followers: Option<u64>,
    followers_confirmed: bool,
    unread_messages: Option<u64>,
    messages_confirmed: bool,
    unread_notifications: Option<u64>,
    notifications_confirmed: bool,
}

#[cfg(windows)]
struct ManagedSocialSession {
    platform: String,
    process: OwnedHandle,
    job: OwnedHandle,
    command_input: Option<fs::File>,
    responses: std::sync::mpsc::Receiver<Result<serde_json::Value, String>>,
    next_command_id: u64,
    active_target_id: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    public_identity: Option<DesktopOrganizeIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct DesktopOrganizeIdentity {
    volume_serial: u32,
    file_index: u64,
    digest: String,
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
    personal_moved_count: usize,
    public_moved_count: usize,
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
    latest_batch_touches_public_desktop: bool,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentInstallRequest {
    connector_id: String,
    locale: String,
    install_directory: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentInstallResult {
    connector_id: String,
    success: bool,
    dependency_installed: bool,
    executable: Option<String>,
    detail: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentApiConfig {
    connector_id: String,
    api_key: String,
    base_url: String,
    model: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentApiTestResult {
    reachable: bool,
    detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CcSwitchProviderSummary {
    id: String,
    app_type: String,
    name: String,
    is_current: bool,
    model: Option<String>,
    endpoint: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CcSwitchStatus {
    detected: bool,
    database_path: Option<String>,
    providers: Vec<CcSwitchProviderSummary>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CcSwitchApplyRequest {
    app_type: String,
    provider_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LibraryRestoreResult {
    restored_count: usize,
    conflict_count: usize,
    restored_paths: Vec<String>,
}

#[derive(Debug)]
struct ActiveAgentTask {
    status: AgentTaskStatus,
    cancel: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum AgentTaskFailureCode {
    Authentication,
    Quota,
    ModelUnavailable,
    Network,
    SessionNotFound,
    Permission,
    ProcessLaunch,
    ExitFailure,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentTaskResult {
    task_id: String,
    connector_id: String,
    app_session_id: String,
    success: bool,
    timed_out: bool,
    cancelled: bool,
    output: String,
    failure_code: Option<AgentTaskFailureCode>,
    provider_session_id: Option<String>,
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

#[derive(Debug)]
struct FileTransferError {
    message: String,
    permission_denied: bool,
}

struct OrganizePlan {
    candidates: Vec<OrganizeCandidate>,
    categories: BTreeSet<String>,
    excluded_count: usize,
    skipped_items: Vec<DesktopOrganizeSkippedItem>,
}

#[cfg(windows)]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ElevatedFileTransfer {
    source: PathBuf,
    destination: PathBuf,
    is_dir: bool,
    source_digest: String,
    source_volume_serial: u32,
    source_file_index: u64,
    source_size: u64,
    source_last_write_time: u64,
}

#[cfg(windows)]
#[derive(Debug, Serialize, Deserialize)]
struct ElevatedFilePlan {
    version: u8,
    created_at_millis: u64,
    library_root: PathBuf,
    transfers: Vec<ElevatedFileTransfer>,
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

fn is_hidden_or_system(name: &OsStr, _metadata: &fs::Metadata) -> bool {
    if name.to_string_lossy().starts_with('.') {
        return true;
    }

    #[cfg(windows)]
    {
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x0002;
        const FILE_ATTRIBUTE_SYSTEM: u32 = 0x0004;
        let attributes = _metadata.file_attributes();
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

#[cfg(windows)]
fn windows_known_folder_path(
    folder_id: &windows_sys::core::GUID,
    label: &str,
) -> Result<PathBuf, String> {
    let mut raw_path = std::ptr::null_mut();
    let result = unsafe { SHGetKnownFolderPath(folder_id, 0, std::ptr::null_mut(), &mut raw_path) };
    if result < 0 || raw_path.is_null() {
        return Err(format!("无法定位 Windows {label}：HRESULT {result:#x}"));
    }
    let length = unsafe {
        let mut length = 0usize;
        while *raw_path.add(length) != 0 {
            length += 1;
        }
        length
    };
    let value = OsString::from_wide(unsafe { std::slice::from_raw_parts(raw_path, length) });
    unsafe { CoTaskMemFree(raw_path.cast()) };
    Ok(PathBuf::from(value))
}

#[cfg(windows)]
fn known_documents_directory() -> Result<PathBuf, String> {
    windows_known_folder_path(&FOLDERID_Documents, "文档目录")
}

#[cfg(windows)]
fn public_desktop_path() -> Option<PathBuf> {
    windows_known_folder_path(&FOLDERID_PublicDesktop, "公共桌面").ok()
}

#[cfg(not(windows))]
fn public_desktop_path() -> Option<PathBuf> {
    None
}

fn same_path(left: &Path, right: &Path) -> bool {
    path_reservation_key(left) == path_reservation_key(right)
}

fn is_desktop_source_parent(parent: &Path, user_desktop: &Path) -> bool {
    same_path(parent, user_desktop)
        || public_desktop_path().is_some_and(|public| same_path(parent, &public))
}

#[cfg(any(windows, test))]
fn elevated_parent_pair_allowed(
    source_parent: &Path,
    destination_parent: &Path,
    public_desktop: &Path,
    library_root: &Path,
) -> bool {
    let source_in_public = same_path(source_parent, public_desktop);
    let destination_in_public = same_path(destination_parent, public_desktop);
    let source_in_library = source_parent
        .parent()
        .is_some_and(|parent| same_path(parent, library_root));
    let destination_in_library = destination_parent
        .parent()
        .is_some_and(|parent| same_path(parent, library_root));
    (source_in_public && destination_in_library) || (source_in_library && destination_in_public)
}

#[cfg(windows)]
fn canonical_safe_directory(path: &Path, label: &str) -> Result<PathBuf, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("无法检查{label} {}：{error}", path.display()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err(format!("{label}不是普通目录：{}", path.display()));
    }
    path.canonicalize()
        .map_err(|error| format!("无法访问{label} {}：{error}", path.display()))
}

#[cfg(windows)]
fn update_elevated_path_digest(
    root: &Path,
    path: &Path,
    hasher: &mut Sha256,
) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("无法读取提权项目摘要 {}：{error}", path.display()))?;
    if metadata.file_type().is_symlink()
        || is_reparse_point(&metadata)
        || (!metadata.is_file() && !metadata.is_dir())
    {
        return Err(format!(
            "提权项目包含不安全的链接或特殊节点：{}",
            path.display()
        ));
    }
    let relative = path.strip_prefix(root).unwrap_or(path);
    hasher.update(if metadata.is_dir() { b"D" } else { b"F" });
    hasher.update(relative.as_os_str().to_string_lossy().as_bytes());
    hasher.update(metadata.file_size().to_le_bytes());
    if metadata.is_file() {
        let mut file = fs::File::open(path)
            .map_err(|error| format!("无法读取提权文件摘要 {}：{error}", path.display()))?;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = file
                .read(&mut buffer)
                .map_err(|error| format!("无法读取提权文件摘要 {}：{error}", path.display()))?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        return Ok(());
    }
    let mut children = fs::read_dir(path)
        .map_err(|error| format!("无法遍历提权目录 {}：{error}", path.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("无法遍历提权目录 {}：{error}", path.display()))?;
    children.sort_by_key(|entry| entry.file_name());
    for child in children {
        update_elevated_path_digest(root, &child.path(), hasher)?;
    }
    Ok(())
}

#[cfg(windows)]
fn elevated_path_digest(path: &Path) -> Result<String, String> {
    let mut hasher = Sha256::new();
    update_elevated_path_digest(path, path, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(windows)]
fn elevated_plan_digest(serialized: &[u8]) -> String {
    format!("{:x}", Sha256::digest(serialized))
}

#[cfg(windows)]
fn elevated_file_information(
    handle: HANDLE,
    label: &str,
) -> Result<BY_HANDLE_FILE_INFORMATION, String> {
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(handle, &mut information) } == 0 {
        return Err(format!("无法读取{label}文件身份"));
    }
    Ok(information)
}

#[cfg(windows)]
fn elevated_identity_from_information(information: &BY_HANDLE_FILE_INFORMATION) -> (u32, u64) {
    let file_index = ((information.nFileIndexHigh as u64) << 32) | information.nFileIndexLow as u64;
    (information.dwVolumeSerialNumber, file_index)
}

#[cfg(windows)]
fn elevated_file_identity(path: &Path) -> Result<(u32, u64), String> {
    let wide = wide_null(path.as_os_str());
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(format!("无法打开提权项目身份句柄：{}", path.display()));
    }
    let handle = owned_windows_handle(handle);
    let information = elevated_file_information(handle.as_raw_handle() as HANDLE, "提权项目")?;
    Ok(elevated_identity_from_information(&information))
}

#[cfg(windows)]
fn desktop_organize_identity(path: &Path) -> Result<DesktopOrganizeIdentity, String> {
    let (volume_serial, file_index) = elevated_file_identity(path)?;
    Ok(DesktopOrganizeIdentity {
        volume_serial,
        file_index,
        digest: elevated_path_digest(path)?,
    })
}

#[cfg(windows)]
fn verify_desktop_organize_identity(
    path: &Path,
    expected: &DesktopOrganizeIdentity,
) -> Result<(), String> {
    let actual = desktop_organize_identity(path)?;
    if &actual == expected {
        Ok(())
    } else {
        Err(format!("项目文件身份或内容摘要不匹配：{}", path.display()))
    }
}

#[cfg(windows)]
fn elevated_transfer_from_paths(
    source: PathBuf,
    destination: PathBuf,
    is_dir: bool,
) -> Result<ElevatedFileTransfer, String> {
    let metadata =
        fs::symlink_metadata(&source).map_err(|error| format!("提权移动来源不可访问：{error}"))?;
    let (source_volume_serial, source_file_index) = elevated_file_identity(&source)?;
    Ok(ElevatedFileTransfer {
        source_digest: elevated_path_digest(&source)?,
        source_volume_serial,
        source_file_index,
        source_size: metadata.file_size(),
        source_last_write_time: metadata.last_write_time(),
        source,
        destination,
        is_dir,
    })
}

#[cfg(windows)]
fn elevated_identity_matches(transfer: &ElevatedFileTransfer, metadata: &fs::Metadata) -> bool {
    let identity_matches = elevated_file_identity(&transfer.source).is_ok_and(|identity| {
        identity == (transfer.source_volume_serial, transfer.source_file_index)
    });
    metadata.is_dir() == transfer.is_dir
        && metadata.file_size() == transfer.source_size
        && metadata.last_write_time() == transfer.source_last_write_time
        && identity_matches
}

#[cfg(windows)]
fn validate_elevated_transfer(
    transfer: &ElevatedFileTransfer,
    public_desktop: &Path,
    library_root: &Path,
) -> Result<(), String> {
    if !transfer.source.is_absolute() || !transfer.destination.is_absolute() {
        return Err("提权文件计划只能包含绝对路径".to_string());
    }
    let metadata = fs::symlink_metadata(&transfer.source)
        .map_err(|error| format!("提权移动来源不可访问：{error}"))?;
    if metadata.file_type().is_symlink()
        || is_reparse_point(&metadata)
        || metadata.is_dir() != transfer.is_dir
        || (!metadata.is_file() && !metadata.is_dir())
    {
        return Err(format!(
            "提权移动来源不是安全的普通项目：{}",
            transfer.source.display()
        ));
    }
    if !elevated_identity_matches(transfer, &metadata)
        || elevated_path_digest(&transfer.source)? != transfer.source_digest
    {
        return Err(format!(
            "提权移动来源在确认后发生变化，已拒绝继续：{}",
            transfer.source.display()
        ));
    }
    let source_name = transfer
        .source
        .file_name()
        .ok_or_else(|| "提权移动来源缺少文件名".to_string())?;
    if is_hidden_or_system(source_name, &metadata) || is_wormhole_shortcut_name(source_name) {
        return Err(format!(
            "提权移动来源属于受保护项目：{}",
            transfer.source.display()
        ));
    }
    if fs::symlink_metadata(&transfer.destination).is_ok() {
        return Err(format!(
            "提权移动目标已经存在：{}",
            transfer.destination.display()
        ));
    }
    let source_parent = transfer
        .source
        .parent()
        .ok_or_else(|| "提权移动来源缺少父目录".to_string())?;
    let destination_parent = transfer
        .destination
        .parent()
        .ok_or_else(|| "提权移动目标缺少父目录".to_string())?;
    let source_parent = canonical_safe_directory(source_parent, "来源目录")?;
    let destination_parent = canonical_safe_directory(destination_parent, "目标目录")?;
    if !elevated_parent_pair_allowed(
        &source_parent,
        &destination_parent,
        public_desktop,
        library_root,
    ) {
        return Err(format!(
            "提权移动只允许在公共桌面与虫洞派资料库分类之间进行：{} -> {}",
            transfer.source.display(),
            transfer.destination.display()
        ));
    }
    Ok(())
}

#[cfg(windows)]
struct ElevatedDirectoryGuard {
    handle: OwnedHandle,
    final_path: PathBuf,
    volume_serial: u32,
}

#[cfg(windows)]
struct PreparedElevatedMove {
    source: OwnedHandle,
    destination_parent: ElevatedDirectoryGuard,
    destination_name: Vec<u16>,
}

#[cfg(windows)]
fn final_path_from_handle(handle: HANDLE, label: &str) -> Result<PathBuf, String> {
    let required = unsafe { GetFinalPathNameByHandleW(handle, std::ptr::null_mut(), 0, 0) };
    if required == 0 {
        return Err(format!("无法读取{label}最终路径"));
    }
    let mut buffer = vec![0u16; required as usize + 1];
    let written =
        unsafe { GetFinalPathNameByHandleW(handle, buffer.as_mut_ptr(), buffer.len() as u32, 0) };
    if written == 0 || written as usize >= buffer.len() {
        return Err(format!("无法读取{label}最终路径"));
    }
    Ok(PathBuf::from(OsString::from_wide(
        &buffer[..written as usize],
    )))
}

#[cfg(windows)]
fn open_elevated_directory_guard(
    path: &Path,
    label: &str,
) -> Result<ElevatedDirectoryGuard, String> {
    let wide = wide_null(path.as_os_str());
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_READ_ATTRIBUTES | FILE_TRAVERSE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(format!("无法锁定{label} {}", path.display()));
    }
    let handle = owned_windows_handle(handle);
    let information = elevated_file_information(handle.as_raw_handle() as HANDLE, label)?;
    if information.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0
        || information.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(format!("{label}不是安全的普通目录：{}", path.display()));
    }
    Ok(ElevatedDirectoryGuard {
        final_path: final_path_from_handle(handle.as_raw_handle() as HANDLE, label)?,
        volume_serial: information.dwVolumeSerialNumber,
        handle,
    })
}

#[cfg(windows)]
fn elevated_last_write_time(information: &BY_HANDLE_FILE_INFORMATION) -> u64 {
    ((information.ftLastWriteTime.dwHighDateTime as u64) << 32)
        | information.ftLastWriteTime.dwLowDateTime as u64
}

#[cfg(windows)]
fn elevated_file_size(information: &BY_HANDLE_FILE_INFORMATION) -> u64 {
    ((information.nFileSizeHigh as u64) << 32) | information.nFileSizeLow as u64
}

#[cfg(windows)]
fn safe_elevated_destination_name(path: &Path) -> Result<Vec<u16>, String> {
    let name = path
        .file_name()
        .ok_or_else(|| "提权移动目标缺少文件名".to_string())?;
    let name_text = name.to_string_lossy();
    if matches!(name_text.as_ref(), "." | "..")
        || name_text.contains(['\\', '/', ':'])
        || name_text.contains('\0')
    {
        return Err("提权移动目标文件名不安全".to_string());
    }
    let wide = name.encode_wide().collect::<Vec<_>>();
    if wide.is_empty() || wide.len() > 255 {
        return Err("提权移动目标文件名为空或过长".to_string());
    }
    Ok(wide)
}

#[cfg(windows)]
fn ensure_public_elevated_same_volume(
    source_volume_serial: u32,
    destination_volume_serial: u32,
) -> Result<(), String> {
    if source_volume_serial == destination_volume_serial {
        Ok(())
    } else {
        Err("公共桌面跨卷暂不自动处理，请手工处理该项目后刷新资料库。".to_string())
    }
}

#[cfg(windows)]
fn prepare_elevated_move(
    transfer: &ElevatedFileTransfer,
    public_desktop: &Path,
    library_root: &Path,
) -> Result<PreparedElevatedMove, String> {
    let source_parent = transfer
        .source
        .parent()
        .ok_or_else(|| "提权移动来源缺少父目录".to_string())?;
    let destination_parent = transfer
        .destination
        .parent()
        .ok_or_else(|| "提权移动目标缺少父目录".to_string())?;
    let public_guard = open_elevated_directory_guard(public_desktop, "公共桌面")?;
    let library_guard = open_elevated_directory_guard(library_root, "虫洞派资料库")?;
    let source_parent_guard = open_elevated_directory_guard(source_parent, "来源目录")?;
    let destination_parent_guard = open_elevated_directory_guard(destination_parent, "目标目录")?;
    if !elevated_parent_pair_allowed(
        &source_parent_guard.final_path,
        &destination_parent_guard.final_path,
        &public_guard.final_path,
        &library_guard.final_path,
    ) {
        return Err(format!(
            "提权移动目录在执行前发生变化，已拒绝继续：{} -> {}",
            transfer.source.display(),
            transfer.destination.display()
        ));
    }
    let source_wide = wide_null(transfer.source.as_os_str());
    let source = unsafe {
        CreateFileW(
            source_wide.as_ptr(),
            DELETE | FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        )
    };
    if source == INVALID_HANDLE_VALUE {
        return Err(format!(
            "无法锁定提权移动来源，项目可能正在使用：{}",
            transfer.source.display()
        ));
    }
    let source = owned_windows_handle(source);
    let information = elevated_file_information(source.as_raw_handle() as HANDLE, "提权移动来源")?;
    let source_is_dir = information.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0;
    if information.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || source_is_dir != transfer.is_dir
        || elevated_identity_from_information(&information)
            != (transfer.source_volume_serial, transfer.source_file_index)
        || elevated_file_size(&information) != transfer.source_size
        || elevated_last_write_time(&information) != transfer.source_last_write_time
        || elevated_path_digest(&transfer.source)? != transfer.source_digest
    {
        return Err(format!(
            "提权移动来源在执行前发生变化，已拒绝继续：{}",
            transfer.source.display()
        ));
    }
    ensure_public_elevated_same_volume(
        transfer.source_volume_serial,
        destination_parent_guard.volume_serial,
    )?;
    Ok(PreparedElevatedMove {
        source,
        destination_parent: destination_parent_guard,
        destination_name: safe_elevated_destination_name(&transfer.destination)?,
    })
}

#[cfg(windows)]
fn rename_prepared_elevated_move(prepared: &PreparedElevatedMove) -> Result<(), String> {
    let header_size = std::mem::offset_of!(FILE_RENAME_INFO, FileName);
    let byte_len = header_size + prepared.destination_name.len() * std::mem::size_of::<u16>();
    let word_len = byte_len.div_ceil(std::mem::size_of::<u64>());
    let mut storage = vec![0u64; word_len];
    let information = storage.as_mut_ptr().cast::<FILE_RENAME_INFO>();
    unsafe {
        (*information).Anonymous.ReplaceIfExists = false;
        (*information).RootDirectory = prepared.destination_parent.handle.as_raw_handle() as HANDLE;
        (*information).FileNameLength =
            (prepared.destination_name.len() * std::mem::size_of::<u16>()) as u32;
        std::ptr::copy_nonoverlapping(
            prepared.destination_name.as_ptr(),
            (*information).FileName.as_mut_ptr(),
            prepared.destination_name.len(),
        );
    }
    if unsafe {
        SetFileInformationByHandle(
            prepared.source.as_raw_handle() as HANDLE,
            FileRenameInfo,
            information.cast(),
            byte_len as u32,
        )
    } == 0
    {
        return Err(format!(
            "受控句柄重命名失败：{}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn remove_elevated_copy_checked(path: &Path) -> Result<(), String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("无法检查待清理项目 {}：{error}", path.display())),
    };
    if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err(format!("拒绝清理链接或重解析点：{}", path.display()));
    }
    if metadata.is_dir() {
        fs::remove_dir_all(path)
            .map_err(|error| format!("无法清理目录 {}：{error}", path.display()))?;
    } else if metadata.is_file() {
        fs::remove_file(path)
            .map_err(|error| format!("无法清理文件 {}：{error}", path.display()))?;
    } else {
        return Err(format!("拒绝清理特殊项目：{}", path.display()));
    }
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(format!("清理后项目仍然存在：{}", path.display())),
        Err(error) => Err(format!("无法复核清理结果 {}：{error}", path.display())),
    }
}

#[cfg(windows)]
fn remove_elevated_copy(path: &Path) {
    let _ = remove_elevated_copy_checked(path);
}

#[cfg(windows)]
fn copy_elevated_path(source: &Path, destination: &Path) -> Result<(), String> {
    let metadata =
        fs::symlink_metadata(source).map_err(|error| format!("跨卷复制来源不可访问：{error}"))?;
    if metadata.file_type().is_symlink()
        || is_reparse_point(&metadata)
        || (!metadata.is_file() && !metadata.is_dir())
    {
        return Err(format!("跨卷复制拒绝链接或特殊项目：{}", source.display()));
    }
    if metadata.is_file() {
        let mut input =
            fs::File::open(source).map_err(|error| format!("无法打开跨卷复制来源：{error}"))?;
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        let mut output = options
            .open(destination)
            .map_err(|error| format!("无法创建跨卷复制目标：{error}"))?;
        std::io::copy(&mut input, &mut output)
            .and_then(|_| output.sync_all())
            .map_err(|error| format!("跨卷复制文件失败：{error}"))?;
        return Ok(());
    }
    fs::create_dir(destination).map_err(|error| format!("无法创建跨卷目标目录：{error}"))?;
    let mut children = fs::read_dir(source)
        .map_err(|error| format!("无法读取跨卷来源目录：{error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("无法读取跨卷来源目录：{error}"))?;
    children.sort_by_key(|entry| entry.file_name());
    for child in children {
        if let Err(error) = copy_elevated_path(&child.path(), &destination.join(child.file_name()))
        {
            remove_elevated_copy(destination);
            return Err(error);
        }
    }
    Ok(())
}

#[cfg(windows)]
fn cross_volume_elevated_move(transfer: &ElevatedFileTransfer) -> Result<(), String> {
    if let Err(error) = copy_elevated_path(&transfer.source, &transfer.destination) {
        remove_elevated_copy(&transfer.destination);
        return Err(error);
    }
    let destination_digest = match elevated_path_digest(&transfer.destination) {
        Ok(digest) => digest,
        Err(error) => {
            remove_elevated_copy(&transfer.destination);
            return Err(error);
        }
    };
    let source_digest = match elevated_path_digest(&transfer.source) {
        Ok(digest) => digest,
        Err(error) => {
            remove_elevated_copy(&transfer.destination);
            return Err(error);
        }
    };
    if destination_digest != transfer.source_digest || source_digest != transfer.source_digest {
        remove_elevated_copy(&transfer.destination);
        return Err("跨卷复制摘要校验失败，来源保持不变".to_string());
    }
    if !transfer.is_dir {
        if let Err(error) = fs::remove_file(&transfer.source) {
            remove_elevated_copy(&transfer.destination);
            return Err(format!("跨卷复制完成但无法移除原文件：{error}"));
        }
        return Ok(());
    }
    let source_parent = transfer
        .source
        .parent()
        .ok_or_else(|| "跨卷来源缺少父目录".to_string())?;
    let quarantine = source_parent.join(format!(
        ".wormhole-pie-quarantine-{}-{}",
        std::process::id(),
        now_millis()
    ));
    if fs::symlink_metadata(&quarantine).is_ok() {
        remove_elevated_copy(&transfer.destination);
        return Err("跨卷隔离目录已存在，已拒绝继续".to_string());
    }
    if let Err(error) = fs::rename(&transfer.source, &quarantine) {
        remove_elevated_copy(&transfer.destination);
        return Err(format!("跨卷复制完成但无法隔离原目录：{error}"));
    }
    let quarantine_wide = wide_null(quarantine.as_os_str());
    unsafe {
        SetFileAttributesW(quarantine_wide.as_ptr(), FILE_ATTRIBUTE_HIDDEN);
    }
    remove_elevated_copy_checked(&quarantine).map_err(|error| {
        format!(
            "recovery_required：跨卷来源已隔离但未能完全清理 {}：{error}",
            quarantine.display()
        )
    })
}

#[cfg(windows)]
fn move_elevated_transfer(
    transfer: &ElevatedFileTransfer,
    public_desktop: &Path,
    library_root: &Path,
) -> Result<(), String> {
    let prepared = prepare_elevated_move(transfer, public_desktop, library_root)?;
    rename_prepared_elevated_move(&prepared)
}

#[cfg(windows)]
fn verify_elevated_transfer_completed(transfer: &ElevatedFileTransfer) -> Result<(), String> {
    match fs::symlink_metadata(&transfer.source) {
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Ok(_) => {
            return Err(format!(
                "提权移动完成后来源仍然存在：{}",
                transfer.source.display()
            ))
        }
        Err(error) => return Err(format!("无法复核提权移动来源：{error}")),
    }
    let metadata = fs::symlink_metadata(&transfer.destination)
        .map_err(|error| format!("提权移动目标不可访问：{error}"))?;
    let (destination_volume_serial, destination_file_index) =
        elevated_file_identity(&transfer.destination)?;
    if metadata.file_type().is_symlink()
        || is_reparse_point(&metadata)
        || metadata.is_dir() != transfer.is_dir
        || (destination_volume_serial == transfer.source_volume_serial
            && destination_file_index != transfer.source_file_index)
        || elevated_path_digest(&transfer.destination)? != transfer.source_digest
    {
        return Err(format!(
            "提权移动目标与已确认来源不一致：{}",
            transfer.destination.display()
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn rollback_elevated_transfers(
    completed: &[ElevatedFileTransfer],
    public_desktop: &Path,
    library_root: &Path,
) -> Result<(), String> {
    let mut failures = Vec::new();
    for transfer in completed.iter().rev() {
        if fs::symlink_metadata(&transfer.source).is_ok() {
            failures.push(format!("回滚目标已被占用：{}", transfer.source.display()));
            continue;
        }
        let destination_metadata = match fs::symlink_metadata(&transfer.destination) {
            Ok(metadata) => metadata,
            Err(error) => {
                failures.push(format!("回滚来源不可访问：{error}"));
                continue;
            }
        };
        if destination_metadata.file_type().is_symlink()
            || is_reparse_point(&destination_metadata)
            || destination_metadata.is_dir() != transfer.is_dir
        {
            failures.push(format!(
                "回滚来源已被替换，已停止提权回滚：{}",
                transfer.destination.display()
            ));
            continue;
        }
        match elevated_file_identity(&transfer.destination) {
            Ok((volume_serial, file_index))
                if volume_serial != transfer.source_volume_serial
                    || file_index == transfer.source_file_index => {}
            Ok(_) => {
                failures.push(format!(
                    "回滚来源文件身份不一致，已停止提权回滚：{}",
                    transfer.destination.display()
                ));
                continue;
            }
            Err(error) => {
                failures.push(error);
                continue;
            }
        }
        match elevated_path_digest(&transfer.destination) {
            Ok(digest) if digest == transfer.source_digest => {}
            Ok(_) => {
                failures.push(format!(
                    "回滚来源摘要不一致，已停止提权回滚：{}",
                    transfer.destination.display()
                ));
                continue;
            }
            Err(error) => {
                failures.push(error);
                continue;
            }
        }
        let reverse = match elevated_transfer_from_paths(
            transfer.destination.clone(),
            transfer.source.clone(),
            transfer.is_dir,
        ) {
            Ok(reverse) => reverse,
            Err(error) => {
                failures.push(error);
                continue;
            }
        };
        if let Err(error) = validate_elevated_transfer(&reverse, public_desktop, library_root) {
            failures.push(error);
            continue;
        }
        if let Err(error) = move_elevated_transfer(&reverse, public_desktop, library_root) {
            failures.push(format!("受控提权回滚失败：{error}"));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!("recovery_required：{}", failures.join("；")))
    }
}

#[cfg(windows)]
fn execute_elevated_file_plan(plan_path: &Path, expected_digest: &str) -> Result<(), String> {
    let helper_root = std::env::temp_dir().join("wormhole-pie-elevated-plans");
    let helper_root = canonical_safe_directory(&helper_root, "提权计划目录")?;
    let plan_parent = plan_path
        .parent()
        .ok_or_else(|| "提权计划缺少父目录".to_string())?;
    let plan_parent = canonical_safe_directory(plan_parent, "提权计划目录")?;
    if !same_path(&helper_root, &plan_parent) {
        return Err("提权计划不在虫洞派受控临时目录中".to_string());
    }
    let plan_metadata =
        fs::symlink_metadata(plan_path).map_err(|error| format!("无法读取提权计划：{error}"))?;
    if !plan_metadata.is_file()
        || plan_metadata.file_type().is_symlink()
        || is_reparse_point(&plan_metadata)
        || plan_metadata.len() > 1024 * 1024
    {
        return Err("提权计划文件不安全或过大".to_string());
    }
    let serialized = fs::read(plan_path).map_err(|error| format!("无法读取提权计划：{error}"))?;
    if expected_digest.len() != 64
        || !expected_digest
            .bytes()
            .all(|value| value.is_ascii_hexdigit())
        || elevated_plan_digest(&serialized) != expected_digest.to_ascii_lowercase()
    {
        return Err("提权计划摘要不匹配，已拒绝执行".to_string());
    }
    let plan: ElevatedFilePlan = serde_json::from_slice(&serialized)
        .map_err(|error| format!("提权计划格式无效：{error}"))?;
    let plan_age = now_millis().saturating_sub(plan.created_at_millis);
    if plan.version != ELEVATED_FILE_PLAN_VERSION
        || plan.created_at_millis > now_millis().saturating_add(30_000)
        || plan_age > 5 * 60 * 1_000
        || plan.transfers.is_empty()
        || plan.transfers.len() > 500
    {
        return Err("提权计划版本无效或项目数量超限".to_string());
    }
    let public_desktop =
        public_desktop_path().ok_or_else(|| "无法定位 Windows 公共桌面".to_string())?;
    let public_desktop = canonical_safe_directory(&public_desktop, "公共桌面")?;
    let expected_library = known_documents_directory()?.join(LIBRARY_ROOT_NAME);
    let expected_library = canonical_safe_directory(&expected_library, "虫洞派资料库")?;
    let requested_library = canonical_safe_directory(&plan.library_root, "计划资料库")?;
    if !same_path(&expected_library, &requested_library) {
        return Err("提权计划中的资料库不是当前用户的虫洞派资料库".to_string());
    }
    for transfer in &plan.transfers {
        validate_elevated_transfer(transfer, &public_desktop, &expected_library)?;
    }

    let mut completed: Vec<ElevatedFileTransfer> = Vec::new();
    for transfer in &plan.transfers {
        validate_elevated_transfer(transfer, &public_desktop, &expected_library)?;
        if let Err(error) = move_elevated_transfer(transfer, &public_desktop, &expected_library) {
            let rollback_error =
                rollback_elevated_transfers(&completed, &public_desktop, &expected_library).err();
            return Err(format!(
                "受控提权移动失败：{error}{}",
                rollback_error
                    .map(|value| format!("；{value}"))
                    .unwrap_or_default()
            ));
        }
        if let Err(error) = verify_elevated_transfer_completed(transfer) {
            let mut rollback_items = completed.clone();
            rollback_items.push(transfer.clone());
            let rollback_error =
                rollback_elevated_transfers(&rollback_items, &public_desktop, &expected_library)
                    .err();
            return Err(format!(
                "受控提权移动复核失败：{error}{}",
                rollback_error
                    .map(|value| format!("；{value}"))
                    .unwrap_or_default()
            ));
        }
        completed.push(transfer.clone());
    }
    Ok(())
}

#[cfg(windows)]
fn maybe_run_elevated_file_plan() -> Option<i32> {
    let mut arguments = std::env::args_os();
    let _ = arguments.next();
    let flag = arguments.next()?;
    if flag != OsStr::new(ELEVATED_FILE_PLAN_ARG) {
        return None;
    }
    let Some(plan_path) = arguments.next().map(PathBuf::from) else {
        return Some(2);
    };
    let Some(expected_digest) = arguments.next().and_then(|value| value.into_string().ok()) else {
        return Some(2);
    };
    if arguments.next().is_some() {
        return Some(2);
    }
    match execute_elevated_file_plan(&plan_path, &expected_digest) {
        Ok(()) => Some(0),
        Err(_) => Some(1),
    }
}

#[cfg(windows)]
fn wide_null(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn protected_program_files_roots() -> Vec<PathBuf> {
    let mut roots = [
        (&FOLDERID_ProgramFiles, "Program Files"),
        (&FOLDERID_ProgramFilesX64, "Program Files x64"),
        (&FOLDERID_ProgramFilesX86, "Program Files x86"),
    ]
    .into_iter()
    .filter_map(|(folder_id, label)| windows_known_folder_path(folder_id, label).ok())
    .filter_map(|root| root.canonicalize().ok())
    .collect::<Vec<_>>();
    roots.sort_by_key(|root| path_reservation_key(root));
    roots.dedup_by(|left, right| same_path(left, right));
    roots
}

#[cfg(windows)]
fn path_is_under_protected_install_root(executable: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| executable.starts_with(root))
}

#[cfg(windows)]
fn directory_is_writable_by_current_process(directory: &Path) -> Result<bool, String> {
    for attempt in 0..8u64 {
        let probe = directory.join(format!(
            ".wormhole-pie-write-probe-{}-{}-{attempt}.tmp",
            std::process::id(),
            now_millis()
        ));
        let mut options = fs::OpenOptions::new();
        match options.write(true).create_new(true).open(&probe) {
            Ok(file) => {
                drop(file);
                fs::remove_file(&probe).map_err(|error| {
                    format!("安装目录写入探针无法清理 {}：{error}", probe.display())
                })?;
                return Ok(true);
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) if error.kind() == ErrorKind::PermissionDenied => return Ok(false),
            Err(error) => {
                return Err(format!(
                    "无法确认安装目录是否受保护 {}：{error}",
                    directory.display()
                ))
            }
        }
    }
    Err("无法生成唯一的安装目录写入探针".to_string())
}

#[cfg(windows)]
fn elevated_helper_result_is_complete(exit_code: u32, completed: usize, total: usize) -> bool {
    exit_code == 0 && completed == total
}

#[cfg(windows)]
fn execute_permission_denied_transfers(
    transfers: &[(PathBuf, PathBuf, bool)],
    expected_identities: Option<&BTreeMap<String, DesktopOrganizeIdentity>>,
) -> Result<(), String> {
    if transfers.is_empty() {
        return Ok(());
    }
    let library_root = known_documents_directory()?.join(LIBRARY_ROOT_NAME);
    let library_root = canonical_safe_directory(&library_root, "虫洞派资料库")?;
    let public_desktop =
        public_desktop_path().ok_or_else(|| "无法定位 Windows 公共桌面".to_string())?;
    let public_desktop = canonical_safe_directory(&public_desktop, "公共桌面")?;
    let elevated_transfers = transfers
        .iter()
        .map(|(source, destination, is_dir)| {
            elevated_transfer_from_paths(source.clone(), destination.clone(), *is_dir)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(expected_identities) = expected_identities {
        for transfer in &elevated_transfers {
            let expected = expected_identities
                .get(&path_reservation_key(&transfer.source))
                .ok_or_else(|| {
                    format!(
                        "recovery_required：公共桌面恢复缺少已确认身份：{}",
                        transfer.source.display()
                    )
                })?;
            let actual = DesktopOrganizeIdentity {
                volume_serial: transfer.source_volume_serial,
                file_index: transfer.source_file_index,
                digest: transfer.source_digest.clone(),
            };
            if &actual != expected {
                return Err(format!(
                    "recovery_required：公共桌面待恢复项目已被替换或修改，已拒绝放回：{}",
                    transfer.source.display()
                ));
            }
        }
    }
    for transfer in &elevated_transfers {
        validate_elevated_transfer(transfer, &public_desktop, &library_root)?;
    }
    let helper_root = std::env::temp_dir().join("wormhole-pie-elevated-plans");
    fs::create_dir_all(&helper_root).map_err(|error| format!("无法创建提权计划目录：{error}"))?;
    canonical_safe_directory(&helper_root, "提权计划目录")?;
    let plan_path = helper_root.join(format!("plan-{}-{}.json", now_millis(), std::process::id()));
    let plan = ElevatedFilePlan {
        version: ELEVATED_FILE_PLAN_VERSION,
        created_at_millis: now_millis(),
        library_root,
        transfers: elevated_transfers,
    };
    let serialized =
        serde_json::to_vec(&plan).map_err(|error| format!("无法生成提权文件计划：{error}"))?;
    let plan_digest = elevated_plan_digest(&serialized);
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = options
        .open(&plan_path)
        .map_err(|error| format!("无法保存提权文件计划：{error}"))?;
    use std::io::Write;
    file.write_all(&serialized)
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("无法保存提权文件计划：{error}"))?;
    drop(file);

    let executable = std::env::current_exe()
        .and_then(|path| path.canonicalize())
        .map_err(|error| format!("无法定位虫洞派程序：{error}"))?;
    let executable_metadata = fs::symlink_metadata(&executable)
        .map_err(|error| format!("无法检查虫洞派程序：{error}"))?;
    if !executable_metadata.is_file()
        || executable_metadata.file_type().is_symlink()
        || is_reparse_point(&executable_metadata)
    {
        let _ = fs::remove_file(&plan_path);
        return Err("公共桌面管理员移动要求受保护的正式安装程序。".to_string());
    }
    let protected_roots = protected_program_files_roots();
    if !path_is_under_protected_install_root(&executable, &protected_roots) {
        let _ = fs::remove_file(&plan_path);
        return Err("公共桌面管理员移动只在安装到 Program Files 的正式版本中启用；开发版、便携版和用户可写目录已禁用。".to_string());
    }
    let install_directory = executable
        .parent()
        .ok_or_else(|| "无法识别虫洞派安装目录".to_string())?;
    match directory_is_writable_by_current_process(install_directory) {
        Ok(false) => {}
        Ok(true) => {
            let _ = fs::remove_file(&plan_path);
            return Err(
                "虫洞派安装目录对当前用户可写，已禁用公共桌面管理员移动。请使用受保护的正式安装版。"
                    .to_string(),
            );
        }
        Err(error) => {
            let _ = fs::remove_file(&plan_path);
            return Err(error);
        }
    }
    let verb = wide_null(OsStr::new("runas"));
    let executable_wide = wide_null(executable.as_os_str());
    let parameters = wide_null(OsStr::new(&format!(
        "{} \"{}\" {}",
        ELEVATED_FILE_PLAN_ARG,
        plan_path.display(),
        plan_digest,
    )));
    let mut execute_info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    execute_info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    execute_info.fMask = SEE_MASK_NOCLOSEPROCESS;
    execute_info.lpVerb = verb.as_ptr();
    execute_info.lpFile = executable_wide.as_ptr();
    execute_info.lpParameters = parameters.as_ptr();
    execute_info.nShow = SW_SHOWNORMAL;
    if unsafe { ShellExecuteExW(&mut execute_info) } == 0 || execute_info.hProcess.is_null() {
        let _ = fs::remove_file(&plan_path);
        return Err("需要管理员授权才能收纳公共桌面项目，授权已取消或启动失败。".to_string());
    }
    let process = owned_windows_handle(execute_info.hProcess);
    let wait_ms = ELEVATED_FILE_PLAN_TIMEOUT.as_millis().min(u32::MAX as u128) as u32;
    let wait_result = unsafe { WaitForSingleObject(process.as_raw_handle() as HANDLE, wait_ms) };
    if wait_result == WAIT_TIMEOUT {
        unsafe {
            TerminateProcess(process.as_raw_handle() as HANDLE, 124);
            WaitForSingleObject(process.as_raw_handle() as HANDLE, 5_000);
        }
        let _ = fs::remove_file(&plan_path);
        return Err("管理员文件移动超时并已终止；请检查公共桌面与资料库后重试。".to_string());
    }
    if wait_result != WAIT_OBJECT_0 {
        let _ = fs::remove_file(&plan_path);
        return Err("无法确认管理员文件移动是否结束，请检查公共桌面与资料库。".to_string());
    }
    let mut exit_code = 1u32;
    if unsafe { GetExitCodeProcess(process.as_raw_handle() as HANDLE, &mut exit_code) } == 0 {
        let _ = fs::remove_file(&plan_path);
        return Err("无法读取管理员文件移动结果，请检查公共桌面与资料库。".to_string());
    }
    let _ = fs::remove_file(&plan_path);
    let completed = plan
        .transfers
        .iter()
        .filter(|transfer| verify_elevated_transfer_completed(transfer).is_ok())
        .count();
    if elevated_helper_result_is_complete(exit_code, completed, plan.transfers.len()) {
        return Ok(());
    }
    if completed == plan.transfers.len() {
        return Err(format!(
            "recovery_required：管理员 helper 返回退出码 {exit_code}，虽已找到全部目标，但清理或复核没有完整成功。"
        ));
    }
    if completed > 0 {
        return Err(format!(
            "recovery_required：管理员移动仅完成 {completed}/{} 项，已停止自动回滚；请检查公共桌面与资料库。",
            plan.transfers.len()
        ));
    }
    Err(if exit_code == 0 {
        "管理员文件移动没有通过完成校验，桌面未记录为已整理。".to_string()
    } else {
        format!("管理员文件移动未完成（退出码 {exit_code}）。")
    })
}

#[cfg(not(windows))]
fn execute_permission_denied_transfers(
    _transfers: &[(PathBuf, PathBuf, bool)],
    _expected_identities: Option<&BTreeMap<String, DesktopOrganizeIdentity>>,
) -> Result<(), String> {
    Err("当前系统不支持公共桌面提权移动".to_string())
}

fn move_regular_transfer(
    source: &Path,
    destination: &Path,
    _is_dir: bool,
) -> Result<(), FileTransferError> {
    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::CrossesDevices => {
            #[cfg(windows)]
            {
                let transfer = elevated_transfer_from_paths(
                    source.to_path_buf(),
                    destination.to_path_buf(),
                    _is_dir,
                )
                .map_err(|message| FileTransferError {
                    message,
                    permission_denied: false,
                })?;
                cross_volume_elevated_move(&transfer).map_err(|message| FileTransferError {
                    message,
                    permission_denied: false,
                })
            }
            #[cfg(not(windows))]
            {
                Err(FileTransferError {
                    message: format!("跨卷移动暂不支持：{error}"),
                    permission_denied: false,
                })
            }
        }
        Err(error) => Err(FileTransferError {
            permission_denied: error.kind() == ErrorKind::PermissionDenied,
            message: error.to_string(),
        }),
    }
}

fn rollback_transfers(transfers: &[(PathBuf, PathBuf, bool)]) -> Result<(), String> {
    let mut reserved = HashSet::new();
    let mut failures = Vec::new();
    let mut permission_denied = Vec::new();

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
        if let Err(error) = move_regular_transfer(destination, &rollback_target, *is_dir) {
            if error.permission_denied {
                permission_denied.push((destination.clone(), rollback_target, *is_dir));
            } else {
                failures.push(format!(
                    "无法将 {} 回滚到 {}：{}",
                    destination.display(),
                    rollback_target.display(),
                    error.message,
                ));
            }
        }
    }

    if let Err(error) = execute_permission_denied_transfers(&permission_denied, None) {
        failures.push(error);
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("；"))
    }
}

fn execute_transfers(transfers: &[(PathBuf, PathBuf, bool)]) -> Result<(), String> {
    let mut completed = Vec::new();
    let mut permission_denied = Vec::new();
    for (source, destination, is_dir) in transfers {
        if let Err(error) = move_regular_transfer(source, destination, *is_dir) {
            if error.permission_denied {
                permission_denied.push((source.clone(), destination.clone(), *is_dir));
                continue;
            }
            let rollback = rollback_transfers(&completed);
            return Err(match rollback {
                Ok(()) => format!(
                    "移动 {} 到 {} 失败，已回滚本次整理：{}",
                    source.display(),
                    destination.display(),
                    error.message,
                ),
                Err(rollback_error) => format!(
                    "移动 {} 到 {} 失败：{}；回滚同时失败：{rollback_error}",
                    source.display(),
                    destination.display(),
                    error.message,
                ),
            });
        }
        completed.push((source.clone(), destination.clone(), *is_dir));
    }
    if let Err(error) = execute_permission_denied_transfers(&permission_denied, None) {
        let rollback = rollback_transfers(&completed);
        return Err(match rollback {
            Ok(()) => format!("公共桌面项目移动失败，已回滚本次整理：{error}"),
            Err(rollback_error) => {
                format!("公共桌面项目移动失败：{error}；回滚同时失败：{rollback_error}")
            }
        });
    }
    Ok(())
}

fn transfer_starts_on_public_desktop(source: &Path) -> bool {
    let Some(parent) = source.parent() else {
        return false;
    };
    public_desktop_path().is_some_and(|public_desktop| same_path(parent, &public_desktop))
}

fn transfer_touches_public_desktop(source: &Path, destination: &Path) -> bool {
    transfer_starts_on_public_desktop(source) || transfer_starts_on_public_desktop(destination)
}

fn batch_touches_public_desktop(batch: &DesktopOrganizeBatch) -> bool {
    batch
        .moves
        .iter()
        .any(|item| transfer_starts_on_public_desktop(&item.original))
}

#[cfg(windows)]
fn normalized_confirmation_move_ids(mut move_ids: Vec<String>) -> Vec<String> {
    move_ids.sort();
    move_ids.dedup();
    move_ids
}

#[cfg(windows)]
fn issue_public_desktop_confirmation(
    confirmations: &Mutex<BTreeMap<String, PublicDesktopConfirmation>>,
    action: &str,
    batch_id: Option<String>,
    move_ids: Vec<String>,
    organize_snapshot_digest: Option<String>,
) -> Result<String, String> {
    let now = now_millis();
    let sequence = PUBLIC_DESKTOP_CONFIRMATION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let move_ids = normalized_confirmation_move_ids(move_ids);
    let token = format!(
        "{:x}",
        Sha256::digest(
            format!(
                "{now}:{}:{sequence}:{action}:{}:{}:{}",
                std::process::id(),
                batch_id.as_deref().unwrap_or_default(),
                move_ids.join("\u{0}"),
                organize_snapshot_digest.as_deref().unwrap_or_default()
            )
            .as_bytes()
        )
    );
    let mut confirmations = confirmations
        .lock()
        .map_err(|_| "公共桌面确认状态已锁定".to_string())?;
    confirmations.retain(|_, confirmation| confirmation.expires_at > now);
    if confirmations.len() >= 64 {
        confirmations.clear();
    }
    confirmations.insert(
        token.clone(),
        PublicDesktopConfirmation {
            action: action.to_string(),
            batch_id,
            move_ids,
            organize_snapshot_digest,
            expires_at: now + PUBLIC_DESKTOP_CONFIRMATION_TTL.as_millis() as u64,
        },
    );
    Ok(token)
}

#[cfg(windows)]
fn consume_public_desktop_confirmation(
    confirmations: &Mutex<BTreeMap<String, PublicDesktopConfirmation>>,
    token: Option<&str>,
    action: &str,
    batch_id: Option<&str>,
    move_ids: Vec<String>,
    organize_snapshot_digest: Option<&str>,
) -> Result<(), String> {
    let token = token.ok_or_else(|| "公共桌面操作缺少二次确认令牌".to_string())?;
    let now = now_millis();
    let mut confirmations = confirmations
        .lock()
        .map_err(|_| "公共桌面确认状态已锁定".to_string())?;
    confirmations.retain(|_, confirmation| confirmation.expires_at > now);
    let confirmation = confirmations
        .remove(token)
        .ok_or_else(|| "公共桌面二次确认已失效，请重新确认".to_string())?;
    if confirmation.action != action
        || confirmation.batch_id.as_deref() != batch_id
        || confirmation.move_ids != normalized_confirmation_move_ids(move_ids)
        || confirmation.organize_snapshot_digest.as_deref() != organize_snapshot_digest
        || confirmation.expires_at <= now
    {
        return Err("公共桌面二次确认与当前操作不匹配，请重新确认".to_string());
    }
    Ok(())
}

fn capture_public_move_identities(moves: &mut [DesktopOrganizeMove]) -> Result<(), String> {
    #[cfg(windows)]
    {
        for item in moves
            .iter_mut()
            .filter(|item| transfer_starts_on_public_desktop(&item.original))
        {
            item.public_identity =
                Some(desktop_organize_identity(&item.organized).map_err(|error| {
                    format!(
                        "公共桌面项目移动后无法保存身份，已拒绝记录本批次：{}：{error}",
                        item.organized.display()
                    )
                })?);
        }
    }
    #[cfg(not(windows))]
    {
        let _ = moves;
    }
    Ok(())
}

fn validate_public_move_identity(item: &DesktopOrganizeMove) -> Result<(), String> {
    if !transfer_starts_on_public_desktop(&item.original) {
        return Ok(());
    }
    #[cfg(windows)]
    {
        let expected = item.public_identity.as_ref().ok_or_else(|| {
            format!(
                "recovery_required：公共桌面旧批次缺少文件身份，不能按裸路径放回：{}",
                item.organized.display()
            )
        })?;
        verify_desktop_organize_identity(&item.organized, expected).map_err(|error| {
            format!("recovery_required：公共桌面待恢复项目已被替换或修改，已拒绝放回：{error}")
        })
    }
    #[cfg(not(windows))]
    {
        Err("当前系统不支持公共桌面恢复".to_string())
    }
}

fn execute_transfers_with_public_approval(
    transfers: &[(PathBuf, PathBuf, bool)],
) -> Result<(), String> {
    execute_transfers_with_public_approval_and_identities(transfers, None)
}

fn execute_transfers_with_public_approval_and_identities(
    transfers: &[(PathBuf, PathBuf, bool)],
    expected_identities: Option<&BTreeMap<String, DesktopOrganizeIdentity>>,
) -> Result<(), String> {
    let (public_transfers, regular_transfers): (Vec<_>, Vec<_>) = transfers
        .iter()
        .cloned()
        .partition(|(source, destination, _)| transfer_touches_public_desktop(source, destination));
    execute_transfers(&regular_transfers)?;
    if let Err(error) = execute_permission_denied_transfers(&public_transfers, expected_identities)
    {
        let rollback = rollback_transfers(&regular_transfers);
        return Err(match rollback {
            Ok(()) => format!("公共桌面项目未获管理员批准，其他文件移动已回滚：{error}"),
            Err(rollback_error) => {
                format!("公共桌面项目未完成：{error}；个人桌面整理回滚同时失败：{rollback_error}")
            }
        });
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
        if let Err(error) = execute_transfers_with_public_approval(&[(
            item.original.clone(),
            item.organized.clone(),
            item.is_dir,
        )]) {
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
    if public_desktop_path().is_some_and(|public| same_path(parent, &public)) {
        return canonical_safe_restore_directory(parent);
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

fn canonical_safe_restore_directory(parent: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(parent)
        .map_err(|error| format!("撤销目录不可访问 {}：{error}", parent.display()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err(format!("撤销目录不是安全的普通目录：{}", parent.display()));
    }
    Ok(())
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
    let Some(public_desktop) = public_desktop_path() else {
        return Vec::new();
    };
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

fn include_public_desktop_requested(value: Option<bool>) -> bool {
    value.unwrap_or(false)
}

#[cfg(windows)]
fn public_organize_plan_snapshot(
    plan: &OrganizePlan,
) -> Result<(String, BTreeMap<String, DesktopOrganizeIdentity>), String> {
    #[cfg(windows)]
    {
        let mut entries = plan
            .candidates
            .iter()
            .map(|candidate| {
                let identity = desktop_organize_identity(&candidate.original)?;
                Ok((
                    path_reservation_key(&candidate.original),
                    candidate.is_dir,
                    identity,
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        entries.sort_by(|left, right| left.0.cmp(&right.0));
        let identities = entries
            .iter()
            .map(|(path, _, identity)| (path.clone(), identity.clone()))
            .collect();
        let serialized = serde_json::to_vec(&entries)
            .map_err(|error| format!("无法生成公共桌面确认快照：{error}"))?;
        Ok((elevated_plan_digest(&serialized), identities))
    }
    #[cfg(not(windows))]
    {
        let _ = plan;
        Err("当前系统不支持公共桌面确认快照".to_string())
    }
}

fn append_public_desktop_plan(
    desktop: &Path,
    exclusion_keys: &HashSet<String>,
    plan: &mut OrganizePlan,
) {
    for (display_name, path) in public_desktop_visible_paths() {
        if path
            .parent()
            .is_some_and(|parent| path_reservation_key(parent) == path_reservation_key(desktop))
        {
            continue;
        }
        let name = path
            .file_name()
            .map(OsStr::to_os_string)
            .unwrap_or_else(|| OsString::from(&display_name));
        if is_wormhole_shortcut_name(&name) {
            plan.skipped_items.push(organize_skipped_item(
                display_name,
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
                    display_name,
                    path.to_string_lossy().to_string(),
                    "metadata_error",
                    &format!("无法读取公共桌面项目属性：{error}"),
                ));
                continue;
            }
        };
        let file_type = metadata.file_type();
        if is_hidden_or_system(&name, &metadata) {
            plan.skipped_items.push(organize_skipped_item(
                display_name,
                path.to_string_lossy().to_string(),
                "protected_system_file",
                "公共桌面的隐藏或系统文件已保护，不会移动",
            ));
            continue;
        }
        if file_type.is_symlink() || is_reparse_point(&metadata) {
            plan.skipped_items.push(organize_skipped_item(
                display_name,
                path.to_string_lossy().to_string(),
                "protected_link_or_cloud_placeholder",
                "公共桌面的符号链接、重解析点或云端占位项目已保护；普通 .lnk/.url 快捷方式仍会正常整理",
            ));
            continue;
        }
        if !file_type.is_file() && !file_type.is_dir() {
            plan.skipped_items.push(organize_skipped_item(
                display_name,
                path.to_string_lossy().to_string(),
                "unsupported_type",
                "公共桌面项目不是普通文件或文件夹，无法安全移动",
            ));
            continue;
        }
        let is_dir = file_type.is_dir();
        if exclusion_keys.contains(&normalize_top_level_name(&name)) {
            plan.excluded_count += 1;
            plan.skipped_items.push(organize_skipped_item(
                display_name,
                path.to_string_lossy().to_string(),
                "excluded_by_user",
                "该公共桌面项目在你的整理忽略名单中，会继续留在桌面",
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
    if let Some(public_desktop) = public_desktop_path() {
        roots.push(ProgramSearchRoot {
            path: public_desktop,
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

#[cfg(any(windows, test))]
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

#[cfg(any(windows, test))]
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

fn database_has_column(connection: &Connection, table: &str, column: &str) -> Result<bool, String> {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|error| error.to_string())?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| error.to_string())?;
    for existing in columns {
        if existing.map_err(|error| error.to_string())? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn ensure_database_column(
    connection: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), String> {
    if database_has_column(connection, table, column)? {
        return Ok(());
    }
    connection
        .execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
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
            CREATE TABLE IF NOT EXISTS social_accounts (
                platform TEXT PRIMARY KEY,
                display_name TEXT NOT NULL DEFAULT '',
                followers INTEGER NOT NULL DEFAULT 0,
                unread_messages INTEGER NOT NULL DEFAULT 0,
                unread_notifications INTEGER NOT NULL DEFAULT 0,
                followers_source TEXT NOT NULL DEFAULT 'manual',
                unread_messages_source TEXT NOT NULL DEFAULT 'manual',
                unread_notifications_source TEXT NOT NULL DEFAULT 'manual',
                account_identity TEXT NOT NULL DEFAULT '',
                connected INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL DEFAULT 0
            );
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
    ensure_database_column(
        connection,
        "social_accounts",
        "followers_source",
        "TEXT NOT NULL DEFAULT 'manual'",
    )?;
    ensure_database_column(
        connection,
        "social_accounts",
        "unread_messages_source",
        "TEXT NOT NULL DEFAULT 'manual'",
    )?;
    ensure_database_column(
        connection,
        "social_accounts",
        "unread_notifications_source",
        "TEXT NOT NULL DEFAULT 'manual'",
    )?;
    ensure_database_column(
        connection,
        "social_accounts",
        "account_identity",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    // `connected` means that this app process verified a visible signed-in page.
    // The Edge profile may still retain the login across restarts, but it must be
    // verified again before the UI reports an active connection.
    connection
        .execute("UPDATE social_accounts SET connected = 0", [])
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SocialRoute {
    Home,
    Publish,
}

const SOCIAL_METRIC_MANUAL: &str = "manual";
const SOCIAL_METRIC_VISIBLE_PAGE: &str = "visible-page";
const SOCIAL_METRIC_UNAVAILABLE: &str = "unavailable";

#[derive(Debug)]
struct StoredSocialSnapshotState {
    connected: bool,
    account_identity: String,
    followers: u64,
    followers_source: String,
    unread_messages: u64,
    unread_messages_source: String,
    unread_notifications: u64,
    unread_notifications_source: String,
}

fn normalize_saved_social_metric(
    value: u64,
    source: &str,
    existing_value: u64,
    existing_source: &str,
) -> (u64, String) {
    match source {
        SOCIAL_METRIC_UNAVAILABLE => (0, SOCIAL_METRIC_UNAVAILABLE.to_string()),
        SOCIAL_METRIC_VISIBLE_PAGE
            if existing_source == SOCIAL_METRIC_VISIBLE_PAGE && value == existing_value =>
        {
            (value, SOCIAL_METRIC_VISIBLE_PAGE.to_string())
        }
        _ => (value, SOCIAL_METRIC_MANUAL.to_string()),
    }
}

fn social_session_persistence() -> String {
    "edge-profile".to_string()
}

fn social_route_url(platform: &str, route: SocialRoute) -> Result<&'static str, String> {
    match (platform, route) {
        ("xiaohongshu", SocialRoute::Home) => Ok("https://creator.xiaohongshu.com/"),
        ("xiaohongshu", SocialRoute::Publish) => {
            Ok("https://creator.xiaohongshu.com/publish/publish")
        }
        ("x", SocialRoute::Home) => Ok("https://x.com/home"),
        ("x", SocialRoute::Publish) => Ok("https://x.com/compose/post"),
        ("douyin", SocialRoute::Home) => Ok("https://creator.douyin.com/"),
        ("douyin", SocialRoute::Publish) => {
            Ok("https://creator.douyin.com/creator-micro/content/upload")
        }
        _ => Err("不支持的社交平台或页面".to_string()),
    }
}

fn validate_social_platform(platform: &str) -> Result<(), String> {
    social_route_url(platform, SocialRoute::Home).map(|_| ())
}

#[tauri::command]
fn list_social_accounts(state: State<'_, AppState>) -> Result<Vec<SocialAccountSnapshot>, String> {
    let connection = state
        .index
        .db
        .lock()
        .map_err(|_| "数据库正忙".to_string())?;
    let mut statement = connection
        .prepare(
            "SELECT platform, display_name, followers, unread_messages, unread_notifications,
                followers_source, unread_messages_source, unread_notifications_source,
                account_identity, connected, updated_at
         FROM social_accounts ORDER BY platform",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(SocialAccountSnapshot {
                platform: row.get(0)?,
                display_name: row.get(1)?,
                followers: row.get(2)?,
                unread_messages: row.get(3)?,
                unread_notifications: row.get(4)?,
                followers_source: row.get(5)?,
                unread_messages_source: row.get(6)?,
                unread_notifications_source: row.get(7)?,
                account_identity: row.get(8)?,
                connected: row.get::<_, i64>(9)? != 0,
                updated_at: row.get(10)?,
                session_persistence: social_session_persistence(),
                raw_cookie_accessed: false,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn save_social_snapshot(
    mut snapshot: SocialAccountSnapshot,
    state: State<'_, AppState>,
) -> Result<SocialAccountSnapshot, String> {
    validate_social_platform(&snapshot.platform)?;
    snapshot.display_name = snapshot.display_name.trim().chars().take(80).collect();
    snapshot.updated_at = now_millis() / 1_000;
    snapshot.session_persistence = social_session_persistence();
    snapshot.raw_cookie_accessed = false;
    let connection = state
        .index
        .db
        .lock()
        .map_err(|_| "数据库正忙".to_string())?;
    let existing = connection
        .query_row(
            "SELECT connected, account_identity,
                    followers, followers_source,
                    unread_messages, unread_messages_source,
                    unread_notifications, unread_notifications_source
             FROM social_accounts WHERE platform = ?1",
            [&snapshot.platform],
            |row| {
                Ok(StoredSocialSnapshotState {
                    connected: row.get::<_, i64>(0)? != 0,
                    account_identity: row.get(1)?,
                    followers: row.get(2)?,
                    followers_source: row.get(3)?,
                    unread_messages: row.get(4)?,
                    unread_messages_source: row.get(5)?,
                    unread_notifications: row.get(6)?,
                    unread_notifications_source: row.get(7)?,
                })
            },
        )
        .optional()
        .map_err(|error| error.to_string())?
        .unwrap_or_else(|| StoredSocialSnapshotState {
            connected: false,
            account_identity: String::new(),
            followers: 0,
            followers_source: SOCIAL_METRIC_UNAVAILABLE.to_string(),
            unread_messages: 0,
            unread_messages_source: SOCIAL_METRIC_UNAVAILABLE.to_string(),
            unread_notifications: 0,
            unread_notifications_source: SOCIAL_METRIC_UNAVAILABLE.to_string(),
        });
    snapshot.connected = existing.connected;
    snapshot.account_identity = existing.account_identity;
    (snapshot.followers, snapshot.followers_source) = normalize_saved_social_metric(
        snapshot.followers,
        &snapshot.followers_source,
        existing.followers,
        &existing.followers_source,
    );
    (snapshot.unread_messages, snapshot.unread_messages_source) = normalize_saved_social_metric(
        snapshot.unread_messages,
        &snapshot.unread_messages_source,
        existing.unread_messages,
        &existing.unread_messages_source,
    );
    (
        snapshot.unread_notifications,
        snapshot.unread_notifications_source,
    ) = normalize_saved_social_metric(
        snapshot.unread_notifications,
        &snapshot.unread_notifications_source,
        existing.unread_notifications,
        &existing.unread_notifications_source,
    );
    connection.execute(
        "INSERT INTO social_accounts (
             platform, display_name, followers, unread_messages, unread_notifications,
             followers_source, unread_messages_source, unread_notifications_source,
             account_identity, connected, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(platform) DO UPDATE SET display_name=excluded.display_name, followers=excluded.followers,
         unread_messages=excluded.unread_messages, unread_notifications=excluded.unread_notifications,
         followers_source=excluded.followers_source,
         unread_messages_source=excluded.unread_messages_source,
         unread_notifications_source=excluded.unread_notifications_source,
         account_identity=excluded.account_identity, connected=excluded.connected,
         updated_at=excluded.updated_at",
        params![
            snapshot.platform,
            snapshot.display_name,
            snapshot.followers,
            snapshot.unread_messages,
            snapshot.unread_notifications,
            snapshot.followers_source,
            snapshot.unread_messages_source,
            snapshot.unread_notifications_source,
            snapshot.account_identity,
            i64::from(snapshot.connected),
            snapshot.updated_at
        ],
    ).map_err(|error| error.to_string())?;
    Ok(snapshot)
}

#[cfg(windows)]
fn social_profile_root(app: &tauri::AppHandle, platform: &str) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())
        .map(|root| root.join("browser-sessions").join(platform))
}

#[cfg(windows)]
fn owned_windows_handle(handle: HANDLE) -> OwnedHandle {
    unsafe { OwnedHandle::from_raw_handle(handle as RawHandle) }
}

#[cfg(windows)]
fn create_inheritable_windows_pipe() -> Result<(OwnedHandle, OwnedHandle), String> {
    let mut read_handle: HANDLE = std::ptr::null_mut();
    let mut write_handle: HANDLE = std::ptr::null_mut();
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: 1,
    };
    if unsafe { CreatePipe(&mut read_handle, &mut write_handle, &attributes, 0) } == 0 {
        return Err(format!(
            "无法创建托管 Edge 调试管道：{}",
            std::io::Error::last_os_error()
        ));
    }
    Ok((
        owned_windows_handle(read_handle),
        owned_windows_handle(write_handle),
    ))
}

#[cfg(windows)]
fn set_windows_handle_inheritable(handle: &OwnedHandle, inheritable: bool) -> Result<(), String> {
    let flags = if inheritable { HANDLE_FLAG_INHERIT } else { 0 };
    if unsafe { SetHandleInformation(handle.as_raw_handle() as HANDLE, HANDLE_FLAG_INHERIT, flags) }
        == 0
    {
        return Err(format!(
            "无法限制托管 Edge 管道句柄：{}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn serialize_windows_handle(handle: &OwnedHandle) -> Result<u32, String> {
    u32::try_from(handle.as_raw_handle() as usize)
        .map_err(|_| "托管 Edge 管道句柄超出浏览器支持范围".to_string())
}

#[cfg(windows)]
fn append_windows_command_argument(command_line: &mut Vec<u16>, argument: &OsStr) {
    if !command_line.is_empty() {
        command_line.push(b' ' as u16);
    }
    let units = argument.encode_wide().collect::<Vec<_>>();
    let needs_quotes = units.is_empty()
        || units
            .iter()
            .any(|unit| *unit == b' ' as u16 || *unit == b'\t' as u16 || *unit == b'"' as u16);
    if !needs_quotes {
        command_line.extend(units);
        return;
    }

    command_line.push(b'"' as u16);
    let mut backslashes = 0usize;
    for unit in units {
        if unit == b'\\' as u16 {
            backslashes += 1;
            continue;
        }
        if unit == b'"' as u16 {
            command_line.extend(std::iter::repeat_n(b'\\' as u16, backslashes * 2 + 1));
            command_line.push(unit);
        } else {
            command_line.extend(std::iter::repeat_n(b'\\' as u16, backslashes));
            command_line.push(unit);
        }
        backslashes = 0;
    }
    command_line.extend(std::iter::repeat_n(b'\\' as u16, backslashes * 2));
    command_line.push(b'"' as u16);
}

#[cfg(windows)]
fn build_windows_command_line(executable: &Path, arguments: &[OsString]) -> Vec<u16> {
    let mut command_line = Vec::new();
    append_windows_command_argument(&mut command_line, executable.as_os_str());
    for argument in arguments {
        append_windows_command_argument(&mut command_line, argument);
    }
    command_line.push(0);
    command_line
}

#[cfg(windows)]
fn create_social_process_job() -> Result<OwnedHandle, String> {
    let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if handle.is_null() {
        return Err(format!(
            "无法创建托管 Edge 生命周期容器：{}",
            std::io::Error::last_os_error()
        ));
    }
    let job = owned_windows_handle(handle);
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    let configured = unsafe {
        SetInformationJobObject(
            job.as_raw_handle() as HANDLE,
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    };
    if configured == 0 {
        return Err(format!(
            "无法限制托管 Edge 生命周期：{}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(job)
}

#[cfg(windows)]
fn read_social_cdp_messages(
    output: fs::File,
    sender: std::sync::mpsc::Sender<Result<serde_json::Value, String>>,
) {
    let mut reader = BufReader::new(output);
    loop {
        let mut message = Vec::new();
        let result = (&mut reader)
            .take((SOCIAL_CDP_MAX_MESSAGE_BYTES + 1) as u64)
            .read_until(0, &mut message);
        match result {
            Ok(0) => {
                let _ = sender.send(Err("托管 Edge 调试管道已关闭".to_string()));
                break;
            }
            Ok(_) if message.last() != Some(&0) => {
                let _ = sender.send(Err("托管 Edge 返回的调试消息过大".to_string()));
                break;
            }
            Ok(_) => {
                message.pop();
                if message.is_empty() {
                    continue;
                }
                match serde_json::from_slice(&message) {
                    Ok(value) => {
                        if sender.send(Ok(value)).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        let _ = sender.send(Err(format!("托管 Edge 返回了无效数据：{error}")));
                        break;
                    }
                }
            }
            Err(error) => {
                let _ = sender.send(Err(format!("无法读取托管 Edge：{error}")));
                break;
            }
        }
    }
}

#[cfg(windows)]
impl ManagedSocialSession {
    fn is_running(&self) -> bool {
        (unsafe { WaitForSingleObject(self.process.as_raw_handle() as HANDLE, 0) }) == WAIT_TIMEOUT
    }

    fn write_request(
        &mut self,
        method: &str,
        params: serde_json::Value,
        session_id: Option<&str>,
    ) -> Result<u64, String> {
        let id = self.next_command_id;
        self.next_command_id = self.next_command_id.saturating_add(1);
        let mut request = serde_json::json!({
            "id": id,
            "method": method,
            "params": params,
        });
        if let Some(session_id) = session_id {
            request["sessionId"] = serde_json::Value::String(session_id.to_string());
        }
        let serialized = serde_json::to_vec(&request).map_err(|error| error.to_string())?;
        let input = self
            .command_input
            .as_mut()
            .ok_or_else(|| "托管 Edge 调试管道已经关闭".to_string())?;
        input
            .write_all(&serialized)
            .and_then(|_| input.write_all(&[0]))
            .and_then(|_| input.flush())
            .map_err(|error| format!("无法向托管 Edge 发送命令：{error}"))?;
        Ok(id)
    }

    fn send_command_with_timeout(
        &mut self,
        method: &str,
        params: serde_json::Value,
        session_id: Option<&str>,
        timeout: Duration,
    ) -> Result<serde_json::Value, String> {
        if !self.is_running() {
            return Err(format!("{} 托管 Edge 已关闭", self.platform));
        }
        let id = self.write_request(method, params, session_id)?;
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or_else(|| format!("托管 Edge 命令超时：{method}"))?;
            match self.responses.recv_timeout(remaining) {
                Ok(Ok(response)) => {
                    if response.get("id").and_then(serde_json::Value::as_u64) != Some(id) {
                        continue;
                    }
                    if let Some(message) = response
                        .pointer("/error/message")
                        .and_then(serde_json::Value::as_str)
                    {
                        return Err(format!("托管 Edge 命令失败：{message}"));
                    }
                    return Ok(response);
                }
                Ok(Err(error)) => return Err(error),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    return Err(format!("托管 Edge 命令超时：{method}"));
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("托管 Edge 调试管道已经断开".to_string());
                }
            }
        }
    }

    fn send_command(
        &mut self,
        method: &str,
        params: serde_json::Value,
        session_id: Option<&str>,
    ) -> Result<serde_json::Value, String> {
        self.send_command_with_timeout(method, params, session_id, SOCIAL_CDP_COMMAND_TIMEOUT)
    }

    fn shutdown(&mut self) {
        if self.is_running() {
            let _ = self.write_request("Browser.close", serde_json::json!({}), None);
        }
        self.command_input.take();
        let wait = unsafe {
            WaitForSingleObject(
                self.process.as_raw_handle() as HANDLE,
                SOCIAL_EDGE_SHUTDOWN_TIMEOUT_MS,
            )
        };
        if wait != WAIT_OBJECT_0 {
            unsafe {
                TerminateJobObject(self.job.as_raw_handle() as HANDLE, 1);
                WaitForSingleObject(self.process.as_raw_handle() as HANDLE, 2_000);
            }
        }
    }
}

#[cfg(windows)]
impl Drop for ManagedSocialSession {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(windows)]
fn launch_managed_social_session(
    executable: &Path,
    profile_root: &Path,
    platform: &str,
    initial_url: &str,
) -> Result<ManagedSocialSession, String> {
    fs::create_dir_all(profile_root).map_err(|error| format!("无法创建登录会话目录：{error}"))?;
    for (path, label) in [
        (profile_root.parent(), "托管 Edge Profile 根目录"),
        (Some(profile_root), "托管 Edge Profile 目录"),
    ] {
        let path = path.ok_or_else(|| format!("无法定位{label}"))?;
        let metadata =
            fs::symlink_metadata(path).map_err(|error| format!("无法检查{label}：{error}"))?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            return Err(format!("拒绝使用异常的{label}"));
        }
    }

    let (child_input, parent_input) = create_inheritable_windows_pipe()?;
    let (parent_output, child_output) = create_inheritable_windows_pipe()?;
    set_windows_handle_inheritable(&parent_input, false)?;
    set_windows_handle_inheritable(&parent_output, false)?;
    let child_input_value = serialize_windows_handle(&child_input)?;
    let child_output_value = serialize_windows_handle(&child_output)?;

    let arguments = vec![
        OsString::from(format!("--user-data-dir={}", profile_root.display())),
        OsString::from("--remote-debugging-pipe"),
        OsString::from(format!(
            "--remote-debugging-io-pipes={child_input_value},{child_output_value}"
        )),
        OsString::from("--no-first-run"),
        OsString::from("--no-default-browser-check"),
        OsString::from("--disable-background-mode"),
        OsString::from("--new-window"),
        OsString::from(initial_url),
    ];
    let executable_wide = wide_null(executable.as_os_str());
    let mut command_line = build_windows_command_line(executable, &arguments);
    let job = create_social_process_job()?;
    let startup = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut process = PROCESS_INFORMATION::default();
    let created = unsafe {
        CreateProcessW(
            executable_wide.as_ptr(),
            command_line.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
            CREATE_SUSPENDED,
            std::ptr::null(),
            std::ptr::null(),
            &startup,
            &mut process,
        )
    };
    if created == 0 {
        return Err(format!(
            "无法打开托管登录窗口：{}",
            std::io::Error::last_os_error()
        ));
    }

    let process_handle = owned_windows_handle(process.hProcess);
    let thread_handle = owned_windows_handle(process.hThread);
    if unsafe {
        AssignProcessToJobObject(
            job.as_raw_handle() as HANDLE,
            process_handle.as_raw_handle() as HANDLE,
        )
    } == 0
    {
        unsafe {
            TerminateJobObject(job.as_raw_handle() as HANDLE, 1);
        }
        return Err(format!(
            "无法接管托管 Edge 生命周期：{}",
            std::io::Error::last_os_error()
        ));
    }
    if unsafe { ResumeThread(thread_handle.as_raw_handle() as HANDLE) } == u32::MAX {
        unsafe {
            TerminateJobObject(job.as_raw_handle() as HANDLE, 1);
        }
        return Err(format!(
            "无法启动托管 Edge：{}",
            std::io::Error::last_os_error()
        ));
    }
    drop(thread_handle);
    drop(child_input);
    drop(child_output);

    let input = unsafe { fs::File::from_raw_handle(parent_input.into_raw_handle()) };
    let output = unsafe { fs::File::from_raw_handle(parent_output.into_raw_handle()) };
    let (sender, responses) = std::sync::mpsc::channel();
    thread::spawn(move || read_social_cdp_messages(output, sender));

    Ok(ManagedSocialSession {
        platform: platform.to_string(),
        process: process_handle,
        job,
        command_input: Some(input),
        responses,
        next_command_id: 1,
        active_target_id: None,
    })
}

#[cfg(windows)]
fn social_host_matches(platform: &str, page_url: &str) -> bool {
    let Ok(parsed) = url::Url::parse(page_url) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };
    if parsed.scheme() != "https"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port_or_known_default() != Some(443)
    {
        return false;
    }
    match platform {
        "xiaohongshu" => host == "xiaohongshu.com" || host.ends_with(".xiaohongshu.com"),
        "x" => host == "x.com" || host.ends_with(".x.com"),
        "douyin" => host == "douyin.com" || host.ends_with(".douyin.com"),
        _ => false,
    }
}

#[cfg(windows)]
fn matching_social_targets(platform: &str, targets: &[CdpTargetInfo]) -> Vec<CdpTargetInfo> {
    targets
        .iter()
        .filter(|target| target.target_type == "page" && social_host_matches(platform, &target.url))
        .cloned()
        .collect()
}

#[cfg(windows)]
fn select_unique_social_target(
    platform: &str,
    targets: &[CdpTargetInfo],
) -> Result<CdpTargetInfo, String> {
    let matches = matching_social_targets(platform, targets);
    match matches.as_slice() {
        [target] => Ok(target.clone()),
        [] => Err("没有找到该平台的托管页面，请先打开托管会话并完成登录。".to_string()),
        _ => Err(
            "检测到多个该平台页面，无法确认要读取的账号。请只保留一个托管页面后重试。".to_string(),
        ),
    }
}

#[cfg(windows)]
impl ManagedSocialSession {
    fn target_infos(&mut self) -> Result<Vec<CdpTargetInfo>, String> {
        let response = self.send_command("Target.getTargets", serde_json::json!({}), None)?;
        let targets = response
            .pointer("/result/targetInfos")
            .cloned()
            .ok_or_else(|| "托管 Edge 没有返回页面列表".to_string())?;
        serde_json::from_value(targets).map_err(|error| error.to_string())
    }

    fn create_target(&mut self, url: &str) -> Result<String, String> {
        let response = self.send_command(
            "Target.createTarget",
            serde_json::json!({ "url": url }),
            None,
        )?;
        response
            .pointer("/result/targetId")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| "托管 Edge 没有创建目标页面".to_string())
    }

    fn attach_target(&mut self, target_id: &str) -> Result<String, String> {
        let response = self.send_command(
            "Target.attachToTarget",
            serde_json::json!({ "targetId": target_id, "flatten": true }),
            None,
        )?;
        response
            .pointer("/result/sessionId")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| "无法连接托管 Edge 页面".to_string())
    }

    fn detach_target(&mut self, session_id: &str) {
        let _ = self.send_command(
            "Target.detachFromTarget",
            serde_json::json!({ "sessionId": session_id }),
            None,
        );
    }

    fn navigate_target(&mut self, target_id: &str, url: &str) -> Result<(), String> {
        let session_id = self.attach_target(target_id)?;
        let result = self.send_command(
            "Page.navigate",
            serde_json::json!({ "url": url }),
            Some(&session_id),
        );
        self.detach_target(&session_id);
        let response = result?;
        if let Some(error) = response
            .pointer("/result/errorText")
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.is_empty())
        {
            return Err(format!("托管页面没有打开：{error}"));
        }
        self.active_target_id = Some(target_id.to_string());
        Ok(())
    }

    fn open_route(&mut self, platform: &str, url: &str) -> Result<(), String> {
        let targets = self.target_infos()?;
        let active = self.active_target_id.as_ref().and_then(|active_id| {
            targets
                .iter()
                .find(|target| target.target_type == "page" && target.target_id == *active_id)
                .map(|target| target.target_id.clone())
        });
        let target_id = if let Some(active) = active {
            active
        } else {
            let matches = matching_social_targets(platform, &targets);
            match matches.as_slice() {
                [target] => target.target_id.clone(),
                [] => self.create_target(url)?,
                _ => {
                    return Err(
                        "检测到多个该平台页面，无法确认托管窗口。请只保留一个页面后重试。"
                            .to_string(),
                    )
                }
            }
        };
        self.navigate_target(&target_id, url)
    }

    fn wait_for_initial_target(&mut self, platform: &str) -> Result<(), String> {
        let deadline = Instant::now() + SOCIAL_CDP_COMMAND_TIMEOUT;
        loop {
            match self.target_infos() {
                Ok(targets) => {
                    let matches = matching_social_targets(platform, &targets);
                    match matches.as_slice() {
                        [target] => {
                            self.active_target_id = Some(target.target_id.clone());
                            return Ok(());
                        }
                        [] if Instant::now() < deadline => {}
                        [] => return Err("托管 Edge 已启动，但目标页面没有就绪".to_string()),
                        _ => {
                            return Err("托管 Edge 启动了多个目标页面，无法确认受控页面".to_string())
                        }
                    }
                }
                Err(_) if Instant::now() < deadline => {}
                Err(error) => return Err(error),
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}

#[cfg(windows)]
fn visible_social_metrics_expression(platform: &str) -> Result<String, String> {
    let platform_json = serde_json::to_string(platform).map_err(|error| error.to_string())?;
    let script = r#"(() => {
  const platform = __PLATFORM__;
  const visible = (element) => {
    if (!element) return false;
    const style = getComputedStyle(element);
    const rect = element.getBoundingClientRect();
    return style.display !== "none" && style.visibility !== "hidden" && rect.width > 0 && rect.height > 0;
  };
  const clean = (value) => String(value || "").replace(/\s+/g, " ").trim();
  const firstElement = (selectors, root = document) => {
    for (const selector of selectors) {
      for (const element of root.querySelectorAll(selector)) {
        if (visible(element)) return element;
      }
    }
    return null;
  };
  const elementText = (element) => clean(element && (element.getAttribute("aria-label") || element.textContent));
  const count = (value) => {
    const text = clean(value).replace(/,/g, "");
    const match = text.match(/(\d+(?:\.\d+)?)\s*(亿|万|[KMB])?/i);
    if (!match) return null;
    const multiplier = { "万": 10000, "亿": 100000000, "K": 1000, "M": 1000000, "B": 1000000000 }[String(match[2] || "").toUpperCase()] || 1;
    const result = Math.round(Number(match[1]) * multiplier);
    return Number.isSafeInteger(result) && result >= 0 ? result : null;
  };
  const firstText = (selectors, root = document) => {
    const element = firstElement(selectors, root);
    const value = elementText(element);
    return value && value.length <= 120 ? value : null;
  };
  const countMetric = (selectors) => {
    const element = firstElement(selectors);
    return { confirmed: Boolean(element), value: element ? count(elementText(element)) : null };
  };
  const labeledCountMetric = (labels) => {
    const selectors = "[class*='stat'],[class*='data'],[class*='fans'],[class*='follow'],[class*='count'],[class*='number'],a[aria-label],button[aria-label]";
    for (const element of document.querySelectorAll(selectors)) {
      if (!visible(element)) continue;
      const value = elementText(element);
      if (!value || value.length > 100 || !labels.some((label) => value.toLowerCase().includes(label.toLowerCase()))) continue;
      const parsed = count(value);
      if (parsed !== null) return { confirmed: true, value: parsed };
    }
    return { confirmed: false, value: null };
  };
  const unreadMetric = (surfaceSelectors, badgeSelectors) => {
    const surface = firstElement(surfaceSelectors);
    if (!surface) return { confirmed: false, value: null };
    const badgeText = firstText(badgeSelectors, surface);
    const badgeValue = badgeText === null ? null : count(badgeText);
    if (badgeValue !== null) return { confirmed: true, value: badgeValue };
    const surfaceText = clean(surface.getAttribute("aria-label") || surface.getAttribute("title"));
    if (/unread|未读/i.test(surfaceText)) {
      const surfaceValue = count(surfaceText);
      if (surfaceValue !== null) return { confirmed: true, value: surfaceValue };
    }
    return { confirmed: true, value: 0 };
  };
  const configurations = {
    x: {
      name: ["[data-testid='SideNav_AccountSwitcher_Button'] [dir='ltr'] span", "[data-testid='UserName'] span"],
      account: ["[data-testid='SideNav_AccountSwitcher_Button']"],
      followers: ["a[href$='/followers']", "a[href$='/verified_followers']"],
      messageSurface: ["a[href='/messages']", "a[href^='/messages/']"],
      notificationSurface: ["a[href='/notifications']", "a[href^='/notifications/']"],
      badges: ["[data-testid='badge']", "[aria-label*='unread' i]", "[class*='badge']", "[class*='count']"],
      followerLabels: ["followers", "粉丝"]
    },
    xiaohongshu: {
      name: ["[class*='userName']", "[class*='username']", "[class*='nickname']", "[class*='user-name']"],
      followers: ["[class*='fans'] [class*='count']", "[class*='follower'] [class*='count']", "[class*='fans']"],
      messageSurface: ["a[href*='message']", "button[class*='message']", "[class*='message-entry']", "[class*='messageEntry']", "[class*='im-entry']"],
      notificationSurface: ["a[href*='notice']", "a[href*='notification']", "button[class*='notice']", "[class*='notice-entry']", "[class*='notification-entry']"],
      badges: ["[class*='badge']", "[class*='count']", "[aria-label*='未读']", "[aria-label*='unread' i]"],
      followerLabels: ["粉丝", "followers"]
    },
    douyin: {
      name: ["[class*='userName']", "[class*='username']", "[class*='nickname']", "[class*='account-name']"],
      followers: ["[class*='fans'] [class*='count']", "[class*='follower'] [class*='count']", "[class*='fans']"],
      messageSurface: ["a[href*='message']", "button[class*='message']", "[class*='message-entry']", "[class*='messageEntry']", "[class*='im-entry']"],
      notificationSurface: ["a[href*='notice']", "a[href*='notification']", "button[class*='notice']", "[class*='notice-entry']", "[class*='notification-entry']"],
      badges: ["[class*='badge']", "[class*='count']", "[aria-label*='未读']", "[aria-label*='unread' i]"],
      followerLabels: ["粉丝", "followers"]
    }
  };
  const configuration = configurations[platform];
  if (!configuration) return JSON.stringify({});
  const displayName = firstText(configuration.name);
  const directFollowers = countMetric(configuration.followers);
  const followers = directFollowers.confirmed ? directFollowers : labeledCountMetric(configuration.followerLabels);
  const messages = unreadMetric(configuration.messageSurface, configuration.badges);
  const notifications = unreadMetric(configuration.notificationSurface, configuration.badges);
  let pageConfirmed = Boolean(displayName);
  let accountIdentity = displayName;
  if (platform === "x") {
    const accountElement = firstElement(configuration.account);
    const accountText = clean(accountElement && `${accountElement.getAttribute("aria-label") || ""} ${accountElement.textContent || ""}`);
    const handleMatch = accountText.match(/@([A-Za-z0-9_]+)/);
    const accountHandle = handleMatch ? handleMatch[1].toLowerCase() : null;
    const firstPath = location.pathname.split("/").filter(Boolean)[0] || "home";
    const reserved = new Set(["home", "notifications", "messages", "compose", "explore", "search", "settings", "i"]);
    pageConfirmed = Boolean(accountHandle) && (reserved.has(firstPath.toLowerCase()) || firstPath.toLowerCase() === accountHandle);
    accountIdentity = accountHandle ? `@${accountHandle}` : null;
  }
  const selectorsConfirmed = Boolean(displayName) && (followers.confirmed || messages.confirmed || notifications.confirmed);
  return JSON.stringify({
    pageConfirmed,
    selectorsConfirmed,
    displayName,
    accountIdentity,
    followers: followers.value,
    followersConfirmed: followers.confirmed,
    unreadMessages: messages.value,
    messagesConfirmed: messages.confirmed,
    unreadNotifications: notifications.value,
    notificationsConfirmed: notifications.confirmed
  });
})()"#;
    Ok(script.replace("__PLATFORM__", &platform_json))
}

#[cfg(windows)]
fn evaluate_visible_social_metrics(
    session: &mut ManagedSocialSession,
    target_id: &str,
    platform: &str,
) -> Result<VisibleSocialMetrics, String> {
    let expression = visible_social_metrics_expression(platform)?;
    let page_session_id = session.attach_target(target_id)?;
    let response = session.send_command(
        "Runtime.evaluate",
        serde_json::json!({
            "expression": expression,
            "returnByValue": true,
            "awaitPromise": false
        }),
        Some(&page_session_id),
    );
    session.detach_target(&page_session_id);
    let response = response?;
    if let Some(description) = response
        .pointer("/result/exceptionDetails/text")
        .and_then(serde_json::Value::as_str)
    {
        return Err(format!("页面可见数据读取失败：{description}"));
    }
    let value = response
        .pointer("/result/result/value")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "页面没有返回可见汇总数据".to_string())?;
    serde_json::from_str(value).map_err(|error| error.to_string())
}

#[cfg(windows)]
fn save_social_snapshot_to_index(
    index: &SharedIndex,
    snapshot: &SocialAccountSnapshot,
) -> Result<(), String> {
    let connection = index.db.lock().map_err(|_| "数据库正忙".to_string())?;
    connection.execute(
        "INSERT INTO social_accounts (
             platform, display_name, followers, unread_messages, unread_notifications,
             followers_source, unread_messages_source, unread_notifications_source,
             account_identity, connected, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(platform) DO UPDATE SET display_name=excluded.display_name, followers=excluded.followers,
         unread_messages=excluded.unread_messages, unread_notifications=excluded.unread_notifications,
         followers_source=excluded.followers_source,
         unread_messages_source=excluded.unread_messages_source,
         unread_notifications_source=excluded.unread_notifications_source,
         account_identity=excluded.account_identity, connected=excluded.connected,
         updated_at=excluded.updated_at",
        params![
            snapshot.platform,
            snapshot.display_name,
            snapshot.followers,
            snapshot.unread_messages,
            snapshot.unread_notifications,
            snapshot.followers_source,
            snapshot.unread_messages_source,
            snapshot.unread_notifications_source,
            snapshot.account_identity,
            i64::from(snapshot.connected),
            snapshot.updated_at
        ],
    ).map_err(|error| error.to_string())?;
    Ok(())
}

fn update_social_connection(
    index: &SharedIndex,
    platform: &str,
    connected: bool,
) -> Result<(), String> {
    let connection = index.db.lock().map_err(|_| "数据库正忙".to_string())?;
    connection
        .execute(
            "UPDATE social_accounts SET connected = ?2, updated_at = ?3 WHERE platform = ?1",
            params![platform, i64::from(connected), now_millis() / 1_000],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(windows)]
fn controlled_social_target(
    session: &mut ManagedSocialSession,
    platform: &str,
) -> Result<CdpTargetInfo, String> {
    let targets = session.target_infos()?;
    if let Some(active_target_id) = session.active_target_id.as_deref() {
        let active = targets
            .iter()
            .find(|target| target.target_type == "page" && target.target_id == active_target_id)
            .ok_or_else(|| "受控社交页面已经关闭，请重新打开托管会话。".to_string())?;
        if !social_host_matches(platform, &active.url) {
            return Err("受控社交页面已离开对应平台，拒绝读取其他页面。".to_string());
        }
        return Ok(active.clone());
    }
    let target = select_unique_social_target(platform, &targets)?;
    session.active_target_id = Some(target.target_id.clone());
    Ok(target)
}

#[cfg(windows)]
fn open_managed_social_route(
    app: &tauri::AppHandle,
    platform: &str,
    route: SocialRoute,
    state: &AppState,
) -> Result<bool, String> {
    let url = social_route_url(platform, route)?;
    let edge = find_edge_executable()
        .ok_or_else(|| "未找到 Microsoft Edge，无法创建托管登录会话".to_string())?;
    let profile_root = social_profile_root(app, platform)?;
    let mut sessions = state
        .social_sessions
        .lock()
        .map_err(|_| "托管浏览器状态正忙".to_string())?;
    if sessions
        .get(platform)
        .is_some_and(|session| !session.is_running())
    {
        sessions.remove(platform);
    }
    if let Some(session) = sessions.get_mut(platform) {
        session.open_route(platform, url)?;
        return Ok(false);
    }
    let mut session = launch_managed_social_session(&edge, &profile_root, platform, url)?;
    session.wait_for_initial_target(platform)?;
    sessions.insert(platform.to_string(), session);
    Ok(true)
}

#[tauri::command]
fn sync_social_snapshot(
    platform: String,
    state: State<'_, AppState>,
) -> Result<SocialAccountSnapshot, String> {
    validate_social_platform(&platform)?;
    #[cfg(not(windows))]
    {
        let _ = state;
        Err("当前系统暂不支持托管浏览器可见数据同步".to_string())
    }
    #[cfg(windows)]
    {
        let metrics_result = (|| {
            let mut sessions = state
                .social_sessions
                .lock()
                .map_err(|_| "托管浏览器状态正忙".to_string())?;
            let session = sessions.get_mut(&platform).ok_or_else(|| {
                "没有运行中的托管会话。请先打开该平台，并在 Edge Profile 中完成登录。".to_string()
            })?;
            if !session.is_running() {
                return Err("托管 Edge 已关闭，请重新打开该平台。".to_string());
            }
            let target = controlled_social_target(session, &platform)?;
            evaluate_visible_social_metrics(session, &target.target_id, &platform)
        })();
        let metrics = match metrics_result {
            Ok(metrics) => metrics,
            Err(error) => {
                let _ = update_social_connection(&state.index, &platform, false);
                return Err(error);
            }
        };
        let display_name = metrics
            .display_name
            .filter(|value| !value.trim().is_empty())
            .map(|value| value.trim().chars().take(80).collect::<String>());
        let account_identity = metrics
            .account_identity
            .filter(|value| !value.trim().is_empty())
            .map(|value| value.trim().chars().take(120).collect::<String>());
        if !metrics.page_confirmed
            || !metrics.selectors_confirmed
            || display_name.is_none()
            || account_identity.is_none()
        {
            let _ = update_social_connection(&state.index, &platform, false);
            return Err(
                "当前受控页面没有验证出已登录账号。请在托管 Edge 窗口中完成登录，并打开账号主页或创作者数据页。"
                    .to_string(),
            );
        }
        let metric = |value: Option<u64>, confirmed: bool| {
            if confirmed {
                value
                    .map(|value| (value, SOCIAL_METRIC_VISIBLE_PAGE.to_string()))
                    .unwrap_or_else(|| (0, SOCIAL_METRIC_UNAVAILABLE.to_string()))
            } else {
                (0, SOCIAL_METRIC_UNAVAILABLE.to_string())
            }
        };
        let (followers, followers_source) = metric(metrics.followers, metrics.followers_confirmed);
        let (unread_messages, unread_messages_source) =
            metric(metrics.unread_messages, metrics.messages_confirmed);
        let (unread_notifications, unread_notifications_source) = metric(
            metrics.unread_notifications,
            metrics.notifications_confirmed,
        );
        let snapshot = SocialAccountSnapshot {
            platform,
            display_name: display_name.unwrap_or_default(),
            followers,
            unread_messages,
            unread_notifications,
            followers_source,
            unread_messages_source,
            unread_notifications_source,
            account_identity: account_identity.unwrap_or_default(),
            connected: true,
            updated_at: now_millis() / 1_000,
            session_persistence: social_session_persistence(),
            raw_cookie_accessed: false,
        };
        save_social_snapshot_to_index(&state.index, &snapshot)?;
        append_log(
            &state.index,
            "sync_social_snapshot",
            None,
            &snapshot.platform,
            "visible_summary_only",
        );
        Ok(snapshot)
    }
}

#[cfg(windows)]
fn find_edge_executable() -> Option<PathBuf> {
    [
        std::env::var_os("PROGRAMFILES(X86)").map(PathBuf::from),
        std::env::var_os("PROGRAMFILES").map(PathBuf::from),
    ]
    .into_iter()
    .flatten()
    .map(|root| root.join("Microsoft/Edge/Application/msedge.exe"))
    .find(|path| path.is_file())
}

#[tauri::command]
fn open_social_session(
    _app: tauri::AppHandle,
    platform: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    validate_social_platform(&platform)?;
    #[cfg(windows)]
    {
        if open_managed_social_route(&_app, &platform, SocialRoute::Home, &state)? {
            update_social_connection(&state.index, &platform, false)?;
        }
    }
    #[cfg(not(windows))]
    open::that(social_route_url(&platform, SocialRoute::Home)?)
        .map_err(|error| error.to_string())?;
    append_log(
        &state.index,
        "open_social_session",
        None,
        &platform,
        "success",
    );
    Ok(())
}

#[tauri::command]
fn disconnect_social_session(platform: String, state: State<'_, AppState>) -> Result<(), String> {
    validate_social_platform(&platform)?;
    #[cfg(windows)]
    {
        let session = state
            .social_sessions
            .lock()
            .map_err(|_| "托管浏览器状态正忙".to_string())?
            .remove(&platform);
        drop(session);
    }
    update_social_connection(&state.index, &platform, false)?;
    append_log(
        &state.index,
        "disconnect_social_session",
        None,
        &platform,
        "profile_retained",
    );
    Ok(())
}

#[tauri::command]
fn clear_social_session(
    app: tauri::AppHandle,
    platform: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    validate_social_platform(&platform)?;
    #[cfg(windows)]
    {
        let session = state
            .social_sessions
            .lock()
            .map_err(|_| "托管浏览器状态正忙".to_string())?
            .remove(&platform);
        drop(session);
        let profile_root = social_profile_root(&app, &platform)?;
        match fs::symlink_metadata(&profile_root) {
            Ok(metadata) => {
                if !metadata.is_dir()
                    || metadata.file_type().is_symlink()
                    || is_reparse_point(&metadata)
                {
                    return Err("拒绝清理异常的托管 Edge Profile 目录".to_string());
                }
                fs::remove_dir_all(&profile_root)
                    .map_err(|error| format!("无法清理托管 Edge Profile：{error}"))?;
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(format!("无法检查托管 Edge Profile：{error}")),
        }
    }
    #[cfg(not(windows))]
    let _ = app;
    let connection = state
        .index
        .db
        .lock()
        .map_err(|_| "数据库正忙".to_string())?;
    connection
        .execute(
            "DELETE FROM social_accounts WHERE platform = ?1",
            [&platform],
        )
        .map_err(|error| error.to_string())?;
    drop(connection);
    append_log(
        &state.index,
        "clear_social_session",
        None,
        &platform,
        "edge_profile_deleted",
    );
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
        latest_batch_touches_public_desktop: batches
            .last()
            .is_some_and(batch_touches_public_desktop),
    })
}

#[tauri::command]
fn request_public_desktop_confirmation(
    action: String,
    batch_id: Option<String>,
    move_ids: Option<Vec<String>>,
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<String, String> {
    if window.label() != "main" {
        return Err("公共桌面二次确认只能由主窗口发起".to_string());
    }
    #[cfg(windows)]
    {
        let batches = state
            .organize_batches
            .lock()
            .map_err(|_| "桌面整理历史已锁定".to_string())?;
        let requested_move_ids = normalized_confirmation_move_ids(move_ids.unwrap_or_default());
        let mut organize_snapshot_digest = None;
        let (bound_batch_id, bound_move_ids) = match action.as_str() {
            PUBLIC_DESKTOP_ACTION_ORGANIZE => {
                if batch_id.is_some() || !requested_move_ids.is_empty() {
                    return Err("公共桌面收纳确认参数无效".to_string());
                }
                let desktop = state
                    .index
                    .desktop_path
                    .canonicalize()
                    .map_err(|error| format!("桌面目录不可访问：{error}"))?;
                let exclusion_keys = load_organize_exclusion_keys(&state.index)?;
                let mut plan = OrganizePlan {
                    candidates: Vec::new(),
                    categories: BTreeSet::new(),
                    excluded_count: 0,
                    skipped_items: Vec::new(),
                };
                append_public_desktop_plan(&desktop, &exclusion_keys, &mut plan);
                organize_snapshot_digest = Some(public_organize_plan_snapshot(&plan)?.0);
                (None, Vec::new())
            }
            PUBLIC_DESKTOP_ACTION_UNDO => {
                let batch = batches
                    .last()
                    .ok_or_else(|| "没有可以撤销的桌面整理记录".to_string())?;
                if !batch_touches_public_desktop(batch) {
                    return Err("最近一批整理不需要公共桌面确认".to_string());
                }
                if batch_id
                    .as_deref()
                    .is_some_and(|requested| requested != batch.batch_id)
                {
                    return Err("公共桌面撤销批次已变化，请重新确认".to_string());
                }
                (Some(batch.batch_id.clone()), Vec::new())
            }
            PUBLIC_DESKTOP_ACTION_REVIEW => {
                let batch_id = batch_id
                    .as_deref()
                    .ok_or_else(|| "公共桌面复查缺少批次".to_string())?;
                let batch = batches
                    .iter()
                    .find(|batch| batch.batch_id == batch_id)
                    .ok_or_else(|| "整理批次不存在或已经撤销".to_string())?;
                if requested_move_ids.is_empty() {
                    return Err("公共桌面复查没有选择待放回项目".to_string());
                }
                let known = batch
                    .moves
                    .iter()
                    .map(|item| item.move_id.as_str())
                    .collect::<HashSet<_>>();
                if requested_move_ids
                    .iter()
                    .any(|move_id| !known.contains(move_id.as_str()))
                {
                    return Err("公共桌面复查选择已变化，请重新确认".to_string());
                }
                if !batch.moves.iter().any(|item| {
                    requested_move_ids.contains(&item.move_id)
                        && transfer_starts_on_public_desktop(&item.original)
                }) {
                    return Err("所选复查项目不涉及公共桌面".to_string());
                }
                (Some(batch.batch_id.clone()), requested_move_ids)
            }
            _ => return Err("不支持的公共桌面确认操作".to_string()),
        };
        drop(batches);
        issue_public_desktop_confirmation(
            &state.public_desktop_confirmations,
            &action,
            bound_batch_id,
            bound_move_ids,
            organize_snapshot_digest,
        )
    }
    #[cfg(not(windows))]
    {
        let _ = (action, batch_id, move_ids, state);
        Err("当前系统不支持公共桌面操作".to_string())
    }
}

#[tauri::command]
fn organize_desktop(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    include_public_desktop: Option<bool>,
    _confirmation_token: Option<String>,
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
    let organize_public_desktop = include_public_desktop_requested(include_public_desktop);
    if organize_public_desktop && window.label() != "main" {
        return Err("公共桌面收纳只能由主窗口在明确确认后发起".to_string());
    }
    let mut plan = if organize_public_desktop {
        OrganizePlan {
            candidates: Vec::new(),
            categories: BTreeSet::new(),
            excluded_count: 0,
            skipped_items: Vec::new(),
        }
    } else {
        collect_organize_plan(&desktop, &exclusion_keys)?
    };
    #[cfg(windows)]
    let mut expected_public_identities = None;
    #[cfg(not(windows))]
    let expected_public_identities: Option<BTreeMap<String, DesktopOrganizeIdentity>> = None;
    if organize_public_desktop {
        append_public_desktop_plan(&desktop, &exclusion_keys, &mut plan);
        #[cfg(windows)]
        {
            let (snapshot_digest, identities) = public_organize_plan_snapshot(&plan)?;
            consume_public_desktop_confirmation(
                &state.public_desktop_confirmations,
                _confirmation_token.as_deref(),
                PUBLIC_DESKTOP_ACTION_ORGANIZE,
                None,
                Vec::new(),
                Some(&snapshot_digest),
            )?;
            expected_public_identities = Some(identities);
        }
        #[cfg(not(windows))]
        return Err("当前系统不支持公共桌面操作".to_string());
    } else {
        append_legacy_library_plan(&legacy_root, &mut plan);
    }
    let skipped_count = plan.skipped_items.len();

    if plan.candidates.is_empty() {
        if !organize_public_desktop {
            let legacy_directories = legacy_cleanup_directories(&legacy_root);
            let _ = remove_created_directories(&legacy_directories);
        }
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
            personal_moved_count: 0,
            public_moved_count: 0,
            migrated_count: 0,
            category_count: 0,
            skipped_count,
            root_path: organize_root.to_string_lossy().to_string(),
            batch_id: None,
            items: Vec::new(),
            excluded_count: plan.excluded_count,
            skipped_items: plan.skipped_items,
            indexed_count,
            public_desktop_count: public_desktop_count(),
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
            public_identity: None,
        });
    }

    let transfers = moves
        .iter()
        .map(|item| (item.original.clone(), item.organized.clone(), item.is_dir))
        .collect::<Vec<_>>();
    if let Err(error) = execute_transfers_with_public_approval_and_identities(
        &transfers,
        expected_public_identities.as_ref(),
    ) {
        let cleanup_error = remove_created_directories(&created_directories).err();
        let _ = refresh_desktop_files(&app, &state.index);
        return Err(match cleanup_error {
            Some(cleanup_error) => format!("{error}；清理失败：{cleanup_error}"),
            None => error,
        });
    }

    if let Err(error) = capture_public_move_identities(&mut moves) {
        let rollback_error = rollback_transfers(&transfers).err();
        let cleanup_error = remove_created_directories(&created_directories).err();
        let _ = refresh_desktop_files(&app, &state.index);
        return Err(format!(
            "{error}{}{}",
            rollback_error
                .map(|value| format!("；文件回滚失败：{value}"))
                .unwrap_or_default(),
            cleanup_error
                .map(|value| format!("；新库清理失败：{value}"))
                .unwrap_or_default()
        ));
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

    let legacy_directories = if organize_public_desktop {
        Vec::new()
    } else {
        legacy_cleanup_directories(&legacy_root)
    };
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
    let personal_moved_count = moves
        .iter()
        .filter(|item| {
            item.original
                .parent()
                .is_some_and(|parent| same_path(parent, &desktop))
        })
        .count();
    let public_moved_count = moves
        .iter()
        .filter(|item| transfer_starts_on_public_desktop(&item.original))
        .count();
    let new_moved_count = personal_moved_count + public_moved_count;
    let migrated_count = moved_count.saturating_sub(new_moved_count);
    let category_count = plan.categories.len();
    let items = moves
        .iter()
        .filter(|item| {
            item.original
                .parent()
                .is_some_and(|parent| is_desktop_source_parent(parent, &desktop))
        })
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
        personal_moved_count,
        public_moved_count,
        migrated_count,
        category_count,
        skipped_count,
        root_path: organize_root.to_string_lossy().to_string(),
        batch_id: Some(batch_id),
        items,
        excluded_count: plan.excluded_count,
        skipped_items: plan.skipped_items,
        indexed_count: indexed_files.len(),
        public_desktop_count: public_desktop_count(),
    })
}

#[tauri::command]
fn review_desktop_organize(
    batch_id: String,
    excluded_move_ids: Vec<String>,
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    _confirmation_token: Option<String>,
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
    let selected_touches_public = batch.moves.iter().any(|item| {
        requested_ids.contains(&item.move_id) && transfer_starts_on_public_desktop(&item.original)
    });
    if selected_touches_public {
        if window.label() != "main" {
            return Err("公共桌面复查只能由主窗口在明确确认后发起".to_string());
        }
        #[cfg(windows)]
        consume_public_desktop_confirmation(
            &state.public_desktop_confirmations,
            _confirmation_token.as_deref(),
            PUBLIC_DESKTOP_ACTION_REVIEW,
            Some(&batch.batch_id),
            requested_ids.iter().cloned().collect(),
            None,
        )?;
        #[cfg(not(windows))]
        return Err("当前系统不支持公共桌面复查".to_string());
    }

    let mut expected_public_identities = BTreeMap::new();
    for item in batch.moves.iter().filter(|item| {
        requested_ids.contains(&item.move_id) && transfer_starts_on_public_desktop(&item.original)
    }) {
        let Ok(metadata) = fs::symlink_metadata(&item.organized) else {
            continue;
        };
        if metadata.file_type().is_symlink()
            || is_reparse_point(&metadata)
            || metadata.is_dir() != item.is_dir
            || (!metadata.is_file() && !metadata.is_dir())
        {
            continue;
        }
        validate_public_move_identity(item)?;
        expected_public_identities.insert(
            path_reservation_key(&item.organized),
            item.public_identity
                .clone()
                .ok_or_else(|| "recovery_required：公共桌面批次缺少已确认身份".to_string())?,
        );
    }

    let mut candidate_rules = BTreeMap::<String, (String, String, bool)>::new();
    for item in batch
        .moves
        .iter()
        .filter(|item| requested_ids.contains(&item.move_id))
    {
        if !item
            .original
            .parent()
            .is_some_and(|parent| is_desktop_source_parent(parent, &desktop))
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
        let transfer = [(item.organized.clone(), item.original.clone(), item.is_dir)];
        let expected_identities = transfer_starts_on_public_desktop(&item.original)
            .then_some(&expected_public_identities);
        match execute_transfers_with_public_approval_and_identities(&transfer, expected_identities)
        {
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
    window: tauri::WebviewWindow,
    _confirmation_token: Option<String>,
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
    let undo_touches_public = batch_touches_public_desktop(&batch);
    if undo_touches_public {
        if window.label() != "main" {
            return Err("公共桌面撤销只能由主窗口在明确确认后发起".to_string());
        }
        #[cfg(windows)]
        consume_public_desktop_confirmation(
            &state.public_desktop_confirmations,
            _confirmation_token.as_deref(),
            PUBLIC_DESKTOP_ACTION_UNDO,
            Some(&batch.batch_id),
            Vec::new(),
            None,
        )?;
        #[cfg(not(windows))]
        return Err("当前系统不支持公共桌面撤销".to_string());
    }

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

    let mut expected_public_identities = BTreeMap::new();
    for item in batch
        .moves
        .iter()
        .filter(|item| transfer_starts_on_public_desktop(&item.original))
    {
        let Ok(metadata) = fs::symlink_metadata(&item.organized) else {
            continue;
        };
        if metadata.file_type().is_symlink()
            || is_reparse_point(&metadata)
            || metadata.is_dir() != item.is_dir
            || (!metadata.is_file() && !metadata.is_dir())
        {
            continue;
        }
        validate_public_move_identity(item)?;
        expected_public_identities.insert(
            path_reservation_key(&item.organized),
            item.public_identity
                .clone()
                .ok_or_else(|| "recovery_required：公共桌面批次缺少已确认身份".to_string())?,
        );
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
            || metadata.is_dir() != item.is_dir
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

    if let Err(error) = execute_transfers_with_public_approval_and_identities(
        &restore_transfers,
        undo_touches_public.then_some(&expected_public_identities),
    ) {
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
    let restored_move_ids = restored_items
        .iter()
        .map(|item| item.move_id.as_str())
        .collect::<HashSet<_>>();
    let remove_empty_batch = if let Some(remaining_batch) = next_batches.last_mut() {
        remaining_batch
            .moves
            .retain(|item| !restored_move_ids.contains(item.move_id.as_str()));
        remaining_batch
            .created_directories
            .retain(|directory| directory.exists());
        remaining_batch.moves.is_empty()
    } else {
        false
    };
    if remove_empty_batch {
        next_batches.pop();
    }
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
    let personal_moved_count = restore_transfers
        .iter()
        .filter(|(_, destination, _)| {
            destination
                .parent()
                .is_some_and(|parent| same_path(parent, &state.index.desktop_path))
        })
        .count();
    let public_moved_count = restore_transfers
        .iter()
        .filter(|(_, destination, _)| transfer_starts_on_public_desktop(destination))
        .count();
    let new_moved_count = personal_moved_count + public_moved_count;
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
        new_moved_count,
        personal_moved_count,
        public_moved_count,
        migrated_count: moved_count.saturating_sub(new_moved_count),
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
fn restore_library_items_to_desktop(
    paths: Vec<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<LibraryRestoreResult, String> {
    if paths.is_empty() {
        return Ok(LibraryRestoreResult {
            restored_count: 0,
            conflict_count: 0,
            restored_paths: Vec::new(),
        });
    }
    if paths.len() > 500 {
        return Err("一次最多撤回 500 个项目".to_string());
    }
    let _operation_guard = state
        .organize_lock
        .lock()
        .map_err(|_| "桌面整理操作已锁定".to_string())?;
    let library = state
        .index
        .library_path
        .canonicalize()
        .map_err(|error| format!("无法访问资料库：{error}"))?;
    let desktop = app
        .path()
        .desktop_dir()
        .map_err(|error| format!("无法定位桌面：{error}"))?;
    fs::create_dir_all(&desktop).map_err(|error| format!("无法访问桌面：{error}"))?;
    let desktop = desktop
        .canonicalize()
        .map_err(|error| format!("无法访问桌面：{error}"))?;
    let mut seen = HashSet::new();
    let mut reserved = HashSet::new();
    let mut transfers = Vec::new();
    let mut conflict_count = 0usize;
    let mut source_parents = Vec::new();

    for raw in paths {
        let requested = PathBuf::from(raw);
        if !requested.is_absolute() {
            return Err("只能撤回资料库中的绝对路径".to_string());
        }
        let metadata = fs::symlink_metadata(&requested)
            .map_err(|_| "待撤回项目不存在或已被移动".to_string())?;
        if metadata.file_type().is_symlink()
            || is_reparse_point(&metadata)
            || (!metadata.is_file() && !metadata.is_dir())
        {
            return Err("链接或特殊项目不能撤回到桌面".to_string());
        }
        let source = requested
            .canonicalize()
            .map_err(|error| format!("无法访问待撤回项目：{error}"))?;
        let relative = source
            .strip_prefix(&library)
            .map_err(|_| "只能撤回虫洞派资料库中的项目".to_string())?;
        if relative.components().count() < 2
            || !relative
                .components()
                .all(|component| matches!(component, Component::Normal(_)))
        {
            return Err("不能直接撤回资料库根目录或分类目录".to_string());
        }
        if !seen.insert(agent_candidate_key(&source)) {
            continue;
        }
        let name = source
            .file_name()
            .ok_or_else(|| "待撤回项目缺少文件名".to_string())?;
        if desktop.join(name).exists() {
            conflict_count += 1;
        }
        let destination = unique_destination(&desktop, name, metadata.is_dir(), &mut reserved)?;
        if let Some(parent) = source.parent() {
            source_parents.push(parent.to_path_buf());
        }
        transfers.push((source, destination, metadata.is_dir()));
    }

    execute_transfers(&transfers)?;
    if let Err(error) = repath_index_transfers(&state.index, &transfers) {
        let rollback_error = rollback_transfers(&transfers).err();
        return Err(format!(
            "撤回后的索引更新失败：{error}{}",
            rollback_error
                .map(|value| format!("；文件回滚失败：{value}"))
                .unwrap_or_default()
        ));
    }
    let reverse_index = reversed_transfers(&transfers);
    let source_keys = transfers
        .iter()
        .map(|(source, _, _)| agent_candidate_key(source))
        .collect::<HashSet<_>>();
    let previous_batches = state
        .organize_batches
        .lock()
        .map_err(|_| "桌面整理历史已锁定".to_string())?
        .clone();
    let mut next_batches = previous_batches.clone();
    next_batches.iter_mut().for_each(|batch| {
        batch
            .moves
            .retain(|item| !source_keys.contains(&agent_candidate_key(&item.organized)))
    });
    next_batches.retain(|batch| !batch.moves.is_empty());
    if let Err(error) = persist_organize_batches(&state.index, &next_batches) {
        let rollback_error = rollback_transfers(&transfers).err();
        let index_error = if rollback_error.is_none() {
            repath_index_transfers(&state.index, &reverse_index).err()
        } else {
            None
        };
        return Err(format!(
            "撤回历史保存失败：{error}{}{}",
            rollback_error
                .map(|value| format!("；文件回滚失败：{value}"))
                .unwrap_or_default(),
            index_error
                .map(|value| format!("；索引回滚失败：{value}"))
                .unwrap_or_default()
        ));
    }
    *state
        .organize_batches
        .lock()
        .map_err(|_| "桌面整理历史已锁定".to_string())? = next_batches;

    for parent in source_parents {
        if parent.starts_with(&library) && parent != library {
            let _ = fs::remove_dir(parent);
        }
    }
    if let Err(error) = refresh_desktop_files(&app, &state.index) {
        append_log(
            &state.index,
            "restore_library_items",
            None,
            &desktop.to_string_lossy(),
            &format!("refresh_failed:{error}"),
        );
    }
    let restored_paths = transfers
        .iter()
        .map(|(_, destination, _)| destination.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    append_log(
        &state.index,
        "restore_library_items",
        None,
        &desktop.to_string_lossy(),
        &format!("success:{}", restored_paths.len()),
    );
    Ok(LibraryRestoreResult {
        restored_count: restored_paths.len(),
        conflict_count,
        restored_paths,
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

fn agent_executable_file_names(kind: AgentConnectorKind) -> Vec<String> {
    #[cfg(windows)]
    {
        vec![
            format!("{}.exe", kind.command_name()),
            format!("{}.cmd", kind.command_name()),
            format!("{}.bat", kind.command_name()),
        ]
    }
    #[cfg(not(windows))]
    {
        vec![kind.command_name().to_string()]
    }
}

fn agent_executable_candidates(kind: AgentConnectorKind) -> Vec<PathBuf> {
    let file_names = agent_executable_file_names(kind);
    let mut candidates = Vec::new();
    let mut seen = BTreeSet::new();

    if let Some(path_value) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&path_value) {
            for file_name in &file_names {
                push_agent_candidate(&mut candidates, &mut seen, directory.join(file_name));
            }
        }
    }

    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        let home = PathBuf::from(home);
        for file_name in &file_names {
            push_agent_candidate(
                &mut candidates,
                &mut seen,
                home.join(".wormhole-pie")
                    .join("agents")
                    .join("bin")
                    .join(file_name),
            );
            push_agent_candidate(
                &mut candidates,
                &mut seen,
                home.join(".local").join("bin").join(file_name),
            );
        }
    }

    #[cfg(windows)]
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let local_app_data = PathBuf::from(local_app_data);
        for file_name in &file_names {
            let candidate = match kind {
                AgentConnectorKind::Codex => local_app_data
                    .join("Programs")
                    .join("Codex")
                    .join(file_name),
                AgentConnectorKind::Claude => local_app_data
                    .join("Programs")
                    .join("Claude")
                    .join(file_name),
                AgentConnectorKind::Hermes => local_app_data
                    .join("hermes")
                    .join("hermes-agent")
                    .join("venv")
                    .join("Scripts")
                    .join(file_name),
            };
            push_agent_candidate(&mut candidates, &mut seen, candidate);
        }
    }

    #[cfg(target_os = "macos")]
    for directory in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
        for file_name in &file_names {
            push_agent_candidate(
                &mut candidates,
                &mut seen,
                Path::new(directory).join(file_name),
            );
        }
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
    #[cfg(windows)]
    if executable
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat")
        })
    {
        let command_shell = std::env::var_os("ComSpec")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows\System32\cmd.exe"));
        let mut command = Command::new(command_shell);
        command.args(["/D", "/S", "/C"]).arg(executable);
        command.creation_flags(0x0800_0000);
        return command;
    }
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

fn valid_agent_app_session_id(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.len() <= 160
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

fn valid_provider_session_id(kind: AgentConnectorKind, value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() || value.len() > 128 || value.chars().any(char::is_whitespace) {
        return false;
    }
    match kind {
        AgentConnectorKind::Claude => {
            value.len() == 36
                && value.chars().enumerate().all(|(index, character)| {
                    if matches!(index, 8 | 13 | 18 | 23) {
                        character == '-'
                    } else {
                        character.is_ascii_hexdigit()
                    }
                })
        }
        AgentConnectorKind::Hermes => value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-')),
        AgentConnectorKind::Codex => false,
    }
}

fn new_claude_provider_session_id() -> String {
    let digest = Sha256::digest(
        format!(
            "wormhole-claude:{}:{}:{}",
            now_millis(),
            std::process::id(),
            AGENT_TASK_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        )
        .as_bytes(),
    );
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!(
        "{}-{}-4{}-a{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[13..16],
        &hex[17..20],
        &hex[20..32]
    )
}

fn agent_task_arguments(
    kind: AgentConnectorKind,
    task: &str,
    provider_session_id: Option<&str>,
) -> Vec<OsString> {
    let provider_session_id = provider_session_id
        .map(str::trim)
        .filter(|value| valid_provider_session_id(kind, value));
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
        AgentConnectorKind::Claude => {
            let mut arguments = [
                "--print",
                "--output-format",
                "json",
                "--permission-mode",
                "acceptEdits",
                "--no-chrome",
            ]
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>();
            if let Some(session_id) = provider_session_id {
                arguments.push(OsString::from("--resume"));
                arguments.push(OsString::from(session_id));
            } else {
                arguments.push(OsString::from("--session-id"));
                arguments.push(OsString::from(new_claude_provider_session_id()));
            }
            arguments.push(OsString::from("--"));
            arguments.push(OsString::from(task));
            arguments
        }
        AgentConnectorKind::Hermes => {
            let mut arguments = [
                "chat",
                "-Q",
                "--checkpoints",
                "--max-turns",
                "24",
                "--source",
                "tool",
            ]
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>();
            if let Some(session_id) = provider_session_id {
                arguments.push(OsString::from("--resume"));
                arguments.push(OsString::from(session_id));
            }
            arguments.push(OsString::from("-q"));
            arguments.push(OsString::from(task));
            arguments
        }
    }
}

fn resolved_provider_session_id(
    kind: AgentConnectorKind,
    provider_session_id: Option<&str>,
) -> Option<String> {
    let existing = provider_session_id
        .map(str::trim)
        .filter(|value| valid_provider_session_id(kind, value))
        .map(str::to_string);
    match kind {
        AgentConnectorKind::Claude => existing.or_else(|| Some(new_claude_provider_session_id())),
        AgentConnectorKind::Hermes => existing,
        AgentConnectorKind::Codex => None,
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

fn agent_task_failure_message(kind: AgentConnectorKind, stderr: &str) -> String {
    let normalized = stderr.to_ascii_lowercase();
    if normalized.contains("selected model")
        || normalized.contains("model may not exist")
        || normalized.contains("model_not_found")
        || normalized.contains("unknown model")
    {
        return format!(
            "{} 当前模型不可用或账号无权访问，请在 Agent 管理中检查模型配置。",
            kind.name()
        );
    }
    if normalized.contains("unauthorized")
        || normalized.contains("authentication")
        || normalized.contains("invalid api key")
        || normalized.contains("invalid x-api-key")
        || normalized.contains("status code 401")
    {
        return format!("{} 认证失败，请在 Agent 管理中检查 API 密钥。", kind.name());
    }
    if normalized.contains("connection refused")
        || normalized.contains("econnrefused")
        || normalized.contains("failed to connect")
        || normalized.contains("network error")
        || normalized.contains("dns")
    {
        return format!(
            "{} 无法连接 API 地址，请检查网络和 Agent API 地址。",
            kind.name()
        );
    }
    "本机 Agent 没有完成任务，请检查连接状态或稍后再试。".to_string()
}

fn agent_task_failure_code(
    kind: AgentConnectorKind,
    result: &BoundedProcessOutput,
) -> Option<AgentTaskFailureCode> {
    if result.success || result.cancelled || result.timed_out {
        return None;
    }
    let normalized = format!("{}\n{}", result.stdout, result.stderr).to_ascii_lowercase();
    Some(
        if normalized.contains("session")
            && (normalized.contains("not found") || normalized.contains("unknown"))
        {
            AgentTaskFailureCode::SessionNotFound
        } else if normalized.contains("unauthorized")
            || normalized.contains("authentication")
            || normalized.contains("invalid api key")
            || normalized.contains("status code 401")
        {
            AgentTaskFailureCode::Authentication
        } else if normalized.contains("quota")
            || normalized.contains("rate limit")
            || normalized.contains("status code 429")
        {
            AgentTaskFailureCode::Quota
        } else if normalized.contains("model")
            && (normalized.contains("not found")
                || normalized.contains("unavailable")
                || normalized.contains("no access"))
        {
            AgentTaskFailureCode::ModelUnavailable
        } else if normalized.contains("permission denied")
            || normalized.contains("access is denied")
        {
            AgentTaskFailureCode::Permission
        } else if normalized.contains("connection")
            || normalized.contains("network")
            || normalized.contains("dns")
        {
            AgentTaskFailureCode::Network
        } else {
            let _ = kind;
            AgentTaskFailureCode::ExitFailure
        },
    )
}

fn agent_task_public_output(
    kind: AgentConnectorKind,
    result: &BoundedProcessOutput,
) -> (String, Option<String>) {
    if result.success {
        if result.stdout.is_empty() {
            return ("任务已完成，连接器没有返回文字结果。".to_string(), None);
        }
        if kind == AgentConnectorKind::Claude {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&result.stdout) {
                let output = value
                    .get("result")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(&result.stdout)
                    .to_string();
                let session_id = value
                    .get("session_id")
                    .or_else(|| value.get("sessionId"))
                    .and_then(serde_json::Value::as_str)
                    .filter(|value| valid_provider_session_id(kind, value))
                    .map(str::to_string);
                return (output, session_id);
            }
        }
        (result.stdout.clone(), None)
    } else if result.cancelled {
        ("任务已由用户停止。".to_string(), None)
    } else if result.timed_out {
        ("任务达到长时运行安全上限，已经安全停止。".to_string(), None)
    } else {
        (agent_task_failure_message(kind, &result.stderr), None)
    }
}

fn execute_agent_task(
    app: tauri::AppHandle,
    connector_id: String,
    task: String,
    workspace: String,
    attachment_paths: Option<Vec<String>>,
    app_session_id: String,
    provider_session_id: Option<String>,
) -> Result<AgentTaskResult, String> {
    let kind = AgentConnectorKind::from_id(&connector_id)
        .ok_or_else(|| "仅支持 codex、claude 或 hermes 连接器".to_string())?;
    let task = validate_agent_task(&task)?;
    let app_session_id = app_session_id.trim().to_string();
    if !valid_agent_app_session_id(&app_session_id) {
        return Err("Agent 应用会话标识无效".to_string());
    }
    let provider_session_id = resolved_provider_session_id(kind, provider_session_id.as_deref());
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
    let arguments = agent_task_arguments(kind, &task, provider_session_id.as_deref());
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
                app_session_id,
                success: false,
                timed_out: false,
                cancelled: false,
                output,
                failure_code: Some(if error.permission_denied {
                    AgentTaskFailureCode::Permission
                } else {
                    AgentTaskFailureCode::ProcessLaunch
                }),
                provider_session_id,
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
    let failure_code = agent_task_failure_code(kind, &result);
    let (output, returned_provider_session_id) = agent_task_public_output(kind, &result);
    let provider_session_id = returned_provider_session_id.or(provider_session_id);
    let (output, files) = extract_agent_result_files(&output, &workspace);
    let task_result = AgentTaskResult {
        task_id,
        connector_id: kind.id().to_string(),
        app_session_id,
        success: result.success,
        timed_out: result.timed_out,
        cancelled: result.cancelled,
        output,
        failure_code,
        provider_session_id,
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
fn get_agent_default_install_directory(app: tauri::AppHandle) -> Result<String, String> {
    app.path()
        .document_dir()
        .map(|path| path.join("虫洞派 Agents").to_string_lossy().to_string())
        .map_err(|error| format!("无法定位文档目录：{error}"))
}

#[tauri::command]
fn pick_directory(app: tauri::AppHandle, title: String) -> Result<Option<String>, String> {
    let selected = app.dialog().file().set_title(title).blocking_pick_folder();
    selected
        .map(|directory| {
            directory
                .into_path()
                .map(|path| path.to_string_lossy().to_string())
                .map_err(|_| "只支持选择本机目录".to_string())
        })
        .transpose()
}

fn validate_agent_install_directory(raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.contains('\0') || trimmed.contains('"') {
        return Err("安装目录无效".to_string());
    }
    let requested = PathBuf::from(trimmed);
    if !requested.is_absolute() || requested.parent().is_none() {
        return Err("安装目录必须是非根目录的绝对路径".to_string());
    }
    if let Ok(metadata) = fs::symlink_metadata(&requested) {
        if !metadata.is_dir() || metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            return Err("安装目录必须是普通文件夹".to_string());
        }
    } else {
        fs::create_dir_all(&requested).map_err(|error| format!("无法创建安装目录：{error}"))?;
    }
    let canonical = requested
        .canonicalize()
        .map_err(|error| format!("无法访问安装目录：{error}"))?;
    let metadata = fs::symlink_metadata(&canonical).map_err(|error| error.to_string())?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err("安装目录必须是普通文件夹".to_string());
    }
    Ok(canonical)
}

fn executable_candidates(names: &[&str]) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&path) {
            for name in names {
                candidates.push(directory.join(name));
            }
        }
    }
    #[cfg(windows)]
    {
        if let Some(program_files) = std::env::var_os("ProgramFiles") {
            let root = PathBuf::from(program_files).join("nodejs");
            candidates.extend(names.iter().map(|name| root.join(name)));
        }
        if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
            let root = PathBuf::from(local_app_data)
                .join("Programs")
                .join("Python");
            if let Ok(entries) = fs::read_dir(root) {
                for entry in entries.flatten() {
                    candidates.extend(names.iter().map(|name| entry.path().join(name)));
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    for directory in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
        candidates.extend(names.iter().map(|name| Path::new(directory).join(name)));
    }
    candidates
}

fn find_executable(names: &[&str]) -> Option<PathBuf> {
    executable_candidates(names)
        .into_iter()
        .find_map(|candidate| {
            let metadata = fs::metadata(&candidate).ok()?;
            metadata
                .is_file()
                .then(|| candidate.canonicalize().unwrap_or(candidate))
        })
}

fn install_dependency(kind: AgentConnectorKind) -> Result<bool, String> {
    let needs_node = matches!(kind, AgentConnectorKind::Codex | AgentConnectorKind::Claude);
    if needs_node && find_executable(&["npm.cmd", "npm.exe", "npm"]).is_some() {
        return Ok(false);
    }
    if !needs_node && find_executable(&["python.exe", "python3.exe", "python3", "python"]).is_some()
    {
        return Ok(false);
    }

    #[cfg(windows)]
    {
        let winget = find_executable(&["winget.exe"]).ok_or_else(|| {
            "缺少依赖且未找到 winget，请先安装 Node.js LTS 或 Python 3".to_string()
        })?;
        let package = if needs_node {
            "OpenJS.NodeJS.LTS"
        } else {
            "Python.Python.3.12"
        };
        let arguments = [
            "install",
            "--id",
            package,
            "--exact",
            "--silent",
            "--accept-package-agreements",
            "--accept-source-agreements",
            "--disable-interactivity",
        ]
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
        let result = run_bounded_process(
            &winget,
            &arguments,
            None,
            AGENT_INSTALL_TIMEOUT,
            AGENT_INSTALL_OUTPUT_MAX_BYTES,
        )
        .map_err(|error| error.message)?;
        if !result.success {
            return Err(format!(
                "依赖安装失败：{}",
                if result.stderr.is_empty() {
                    result.stdout
                } else {
                    result.stderr
                }
            ));
        }
        Ok(true)
    }

    #[cfg(target_os = "macos")]
    {
        let brew = find_executable(&["brew"]).ok_or_else(|| {
            if needs_node {
                "缺少 Node.js，请先安装 Homebrew 或 Node.js LTS"
            } else {
                "缺少 Python 3，请先安装 Homebrew 或 Python 3"
            }
            .to_string()
        })?;
        let package = if needs_node { "node" } else { "python" };
        let arguments = ["install", package]
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>();
        let result = run_bounded_process(
            &brew,
            &arguments,
            None,
            AGENT_INSTALL_TIMEOUT,
            AGENT_INSTALL_OUTPUT_MAX_BYTES,
        )
        .map_err(|error| error.message)?;
        if !result.success {
            return Err(format!(
                "依赖安装失败：{}",
                if result.stderr.is_empty() {
                    result.stdout
                } else {
                    result.stderr
                }
            ));
        }
        Ok(true)
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    Err("当前系统暂不支持自动安装依赖".to_string())
}

fn managed_agent_bin_directory() -> Result<PathBuf, String> {
    let home = agent_user_home().ok_or_else(|| "无法定位用户目录".to_string())?;
    let directory = home.join(".wormhole-pie").join("agents").join("bin");
    fs::create_dir_all(&directory).map_err(|error| format!("无法创建 Agent 启动目录：{error}"))?;
    Ok(directory)
}

fn write_agent_launcher(kind: AgentConnectorKind, executable: &Path) -> Result<PathBuf, String> {
    let bin = managed_agent_bin_directory()?;
    #[cfg(windows)]
    {
        let launcher = bin.join(format!("{}.cmd", kind.command_name()));
        let content = format!("@echo off\r\ncall \"{}\" %*\r\n", executable.display());
        fs::write(&launcher, content).map_err(|error| format!("无法创建 Agent 启动器：{error}"))?;
        Ok(launcher)
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        let launcher = bin.join(kind.command_name());
        let escaped = executable.to_string_lossy().replace('"', "\\\"");
        fs::write(&launcher, format!("#!/bin/sh\nexec \"{escaped}\" \"$@\"\n"))
            .map_err(|error| format!("无法创建 Agent 启动器：{error}"))?;
        let mut permissions = fs::metadata(&launcher)
            .map_err(|error| error.to_string())?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&launcher, permissions).map_err(|error| error.to_string())?;
        Ok(launcher)
    }
}

fn npm_agent_package(kind: AgentConnectorKind) -> Option<&'static str> {
    match kind {
        AgentConnectorKind::Codex => Some("@openai/codex@latest"),
        AgentConnectorKind::Claude => Some("@anthropic-ai/claude-code@latest"),
        AgentConnectorKind::Hermes => None,
    }
}

fn install_agent_blocking(request: AgentInstallRequest) -> Result<AgentInstallResult, String> {
    let kind = AgentConnectorKind::from_id(request.connector_id.trim())
        .ok_or_else(|| "仅支持 codex、claude 或 hermes".to_string())?;
    let install_root = validate_agent_install_directory(&request.install_directory)?;
    let dependency_installed = install_dependency(kind)?;
    let use_china_mirror = request.locale.eq_ignore_ascii_case("zh-CN");
    let target = install_root.join(kind.id());
    fs::create_dir_all(&target).map_err(|error| format!("无法创建 Agent 安装目录：{error}"))?;

    let executable = if let Some(package) = npm_agent_package(kind) {
        let npm = find_executable(&["npm.cmd", "npm.exe", "npm"]).ok_or_else(|| {
            "Node.js 已安装，但当前进程仍找不到 npm；请重启虫洞派后再试".to_string()
        })?;
        let registry = if use_china_mirror {
            "https://registry.npmmirror.com"
        } else {
            "https://registry.npmjs.org"
        };
        let arguments = [
            "install".into(),
            "--prefix".into(),
            target.as_os_str().to_owned(),
            package.into(),
            "--no-audit".into(),
            "--no-fund".into(),
            "--registry".into(),
            registry.into(),
        ];
        let result = run_bounded_process(
            &npm,
            &arguments,
            None,
            AGENT_INSTALL_TIMEOUT,
            AGENT_INSTALL_OUTPUT_MAX_BYTES,
        )
        .map_err(|error| error.message)?;
        if !result.success {
            return Err(format!(
                "{} 安装失败：{}",
                kind.name(),
                if result.stderr.is_empty() {
                    result.stdout
                } else {
                    result.stderr
                }
            ));
        }
        #[cfg(windows)]
        let candidate = target
            .join("node_modules")
            .join(".bin")
            .join(format!("{}.cmd", kind.command_name()));
        #[cfg(not(windows))]
        let candidate = target
            .join("node_modules")
            .join(".bin")
            .join(kind.command_name());
        candidate
    } else {
        let python = find_executable(&["python.exe", "python3.exe", "python3", "python"])
            .ok_or_else(|| {
                "Python 3 已安装，但当前进程仍找不到它；请重启虫洞派后再试".to_string()
            })?;
        let venv = target.join("venv");
        let create_arguments = ["-m".into(), "venv".into(), venv.as_os_str().to_owned()];
        let created = run_bounded_process(
            &python,
            &create_arguments,
            None,
            AGENT_INSTALL_TIMEOUT,
            AGENT_INSTALL_OUTPUT_MAX_BYTES,
        )
        .map_err(|error| error.message)?;
        if !created.success {
            return Err(format!(
                "Hermes Python 环境创建失败：{}",
                if created.stderr.is_empty() {
                    created.stdout
                } else {
                    created.stderr
                }
            ));
        }
        #[cfg(windows)]
        let venv_python = venv.join("Scripts").join("python.exe");
        #[cfg(not(windows))]
        let venv_python = venv.join("bin").join("python");
        let index = if use_china_mirror {
            "https://pypi.tuna.tsinghua.edu.cn/simple"
        } else {
            "https://pypi.org/simple"
        };
        let pip_arguments = [
            "-m".into(),
            "pip".into(),
            "install".into(),
            "--upgrade".into(),
            "hermes-agent".into(),
            "--index-url".into(),
            index.into(),
        ];
        let installed = run_bounded_process(
            &venv_python,
            &pip_arguments,
            None,
            AGENT_INSTALL_TIMEOUT,
            AGENT_INSTALL_OUTPUT_MAX_BYTES,
        )
        .map_err(|error| error.message)?;
        if !installed.success {
            return Err(format!(
                "Hermes Agent 安装失败：{}",
                if installed.stderr.is_empty() {
                    installed.stdout
                } else {
                    installed.stderr
                }
            ));
        }
        #[cfg(windows)]
        let candidate = venv.join("Scripts").join("hermes.exe");
        #[cfg(not(windows))]
        let candidate = venv.join("bin").join("hermes");
        candidate
    };

    if !executable.is_file() {
        return Err(format!("安装已结束，但没有找到 {} 启动文件", kind.name()));
    }
    let launcher = write_agent_launcher(kind, &executable)?;
    invalidate_agent_connector_cache(kind);
    Ok(AgentInstallResult {
        connector_id: kind.id().to_string(),
        success: true,
        dependency_installed,
        executable: Some(launcher.to_string_lossy().to_string()),
        detail: format!(
            "{} 已安装到 {}，使用{}",
            kind.name(),
            target.display(),
            if use_china_mirror {
                "中国镜像源"
            } else {
                "国际官方源"
            }
        ),
    })
}

#[tauri::command]
async fn install_agent(request: AgentInstallRequest) -> Result<AgentInstallResult, String> {
    tauri::async_runtime::spawn_blocking(move || install_agent_blocking(request))
        .await
        .map_err(|error| format!("Agent 安装任务异常：{error}"))?
}

fn validate_agent_api_config(
    config: &AgentApiConfig,
) -> Result<(AgentConnectorKind, url::Url), String> {
    let kind = AgentConnectorKind::from_id(config.connector_id.trim())
        .ok_or_else(|| "不支持的 Agent 类型".to_string())?;
    let url =
        url::Url::parse(config.base_url.trim()).map_err(|_| "接口地址格式无效".to_string())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("接口地址必须是有效的 http 或 https 地址".to_string());
    }
    if config.model.len() > 200 || config.api_key.len() > 8_192 {
        return Err("API 配置内容过长".to_string());
    }
    if config
        .api_key
        .chars()
        .chain(config.model.chars())
        .any(|character| matches!(character, '\0' | '\r' | '\n'))
    {
        return Err("API 配置不能包含换行或控制字符".to_string());
    }
    Ok((kind, url))
}

#[tauri::command]
async fn test_agent_api(config: AgentApiConfig) -> Result<AgentApiTestResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (_, url) = validate_agent_api_config(&config)?;
        let host = url
            .host_str()
            .ok_or_else(|| "接口地址缺少主机".to_string())?;
        let port = url
            .port_or_known_default()
            .ok_or_else(|| "接口地址端口无效".to_string())?;
        let addresses = (host, port)
            .to_socket_addrs()
            .map_err(|error| format!("无法解析接口地址：{error}"))?;
        for address in addresses {
            if TcpStream::connect_timeout(&address, Duration::from_secs(5)).is_ok() {
                return Ok(AgentApiTestResult {
                    reachable: true,
                    detail: format!("连接成功：{host}:{port}"),
                });
            }
        }
        Ok(AgentApiTestResult {
            reachable: false,
            detail: format!("无法连接：{host}:{port}"),
        })
    })
    .await
    .map_err(|error| format!("接口测试任务异常：{error}"))?
}

fn json_object(path: &Path) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    if !path.exists() {
        return Ok(serde_json::Map::new());
    }
    let content = fs::read_to_string(path).map_err(|error| format!("无法读取配置：{error}"))?;
    serde_json::from_str::<serde_json::Value>(&content)
        .map_err(|error| format!("现有配置不是有效 JSON：{error}"))?
        .as_object()
        .cloned()
        .ok_or_else(|| "现有配置必须是 JSON 对象".to_string())
}

fn write_pretty_json(
    path: &Path,
    object: serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("无法创建配置目录：{error}"))?;
    }
    let bytes = serde_json::to_vec_pretty(&serde_json::Value::Object(object))
        .map_err(|error| error.to_string())?;
    fs::write(path, bytes).map_err(|error| format!("无法写入配置：{error}"))
}

fn toml_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\r', '\n'], " ")
}

fn replace_managed_codex_block(existing: &str, block: &str) -> String {
    const START: &str = "# BEGIN WORMHOLE PIE MANAGED CONFIG";
    const END: &str = "# END WORMHOLE PIE MANAGED CONFIG";
    if let Some(start) = existing.find(START) {
        if let Some(relative_end) = existing[start..].find(END) {
            let end = start + relative_end + END.len();
            let mut rest = existing.to_string();
            rest.replace_range(start..end, "");
            return format!("{block}\n{}", rest.trim_start());
        }
    }
    format!("{block}\n{existing}")
}

fn update_env_file(path: &Path, updates: &[(&str, String)]) -> Result<(), String> {
    let existing = fs::read_to_string(path).unwrap_or_default();
    let active_updates = updates
        .iter()
        .filter(|(_, value)| !value.is_empty())
        .collect::<Vec<_>>();
    let keys = active_updates
        .iter()
        .map(|(key, _)| *key)
        .collect::<HashSet<_>>();
    let mut lines = existing
        .lines()
        .filter(|line| {
            line.split_once('=')
                .is_none_or(|(key, _)| !keys.contains(key.trim()))
        })
        .map(str::to_string)
        .collect::<Vec<_>>();
    lines.extend(
        active_updates
            .into_iter()
            .map(|(key, value)| format!("{key}={value}")),
    );
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(path, format!("{}\n", lines.join("\n")))
        .map_err(|error| format!("无法写入配置：{error}"))
}

fn save_agent_api_config_blocking(config: AgentApiConfig) -> Result<(), String> {
    let (kind, _) = validate_agent_api_config(&config)?;
    let home = agent_user_home().ok_or_else(|| "无法定位用户目录".to_string())?;
    match kind {
        AgentConnectorKind::Codex => {
            let root = home.join(".codex");
            fs::create_dir_all(&root).map_err(|error| error.to_string())?;
            let config_path = root.join("config.toml");
            let existing = fs::read_to_string(&config_path).unwrap_or_default();
            let block = format!(
                "# BEGIN WORMHOLE PIE MANAGED CONFIG\nmodel = \"{}\"\nmodel_provider = \"wormhole-pie\"\n\n[model_providers.\"wormhole-pie\"]\nname = \"Wormhole Pie\"\nbase_url = \"{}\"\nenv_key = \"OPENAI_API_KEY\"\nwire_api = \"responses\"\n# END WORMHOLE PIE MANAGED CONFIG",
                toml_string(&config.model), toml_string(&config.base_url)
            );
            fs::write(&config_path, replace_managed_codex_block(&existing, &block))
                .map_err(|error| format!("无法写入 Codex 配置：{error}"))?;
            if !config.api_key.is_empty() {
                let auth_path = root.join("auth.json");
                let mut auth = json_object(&auth_path)?;
                auth.insert("OPENAI_API_KEY".to_string(), config.api_key.into());
                write_pretty_json(&auth_path, auth)?;
            }
        }
        AgentConnectorKind::Claude => {
            let path = home.join(".claude").join("settings.json");
            let mut settings = json_object(&path)?;
            let env = settings
                .entry("env".to_string())
                .or_insert_with(|| serde_json::json!({}));
            let env = env
                .as_object_mut()
                .ok_or_else(|| "Claude settings.json 中的 env 必须是对象".to_string())?;
            if !config.api_key.is_empty() {
                env.insert("ANTHROPIC_API_KEY".to_string(), config.api_key.into());
            }
            env.insert("ANTHROPIC_BASE_URL".to_string(), config.base_url.into());
            if !config.model.is_empty() {
                env.insert("ANTHROPIC_MODEL".to_string(), config.model.into());
            }
            write_pretty_json(&path, settings)?;
        }
        AgentConnectorKind::Hermes => {
            update_env_file(
                &home.join(".hermes").join(".env"),
                &[
                    ("OPENAI_API_KEY", config.api_key),
                    ("OPENAI_BASE_URL", config.base_url),
                    ("LLM_MODEL", config.model),
                ],
            )?;
        }
    }
    invalidate_agent_connector_cache(kind);
    Ok(())
}

#[tauri::command]
async fn save_agent_api_config(config: AgentApiConfig) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || save_agent_api_config_blocking(config))
        .await
        .map_err(|error| format!("保存 API 配置任务异常：{error}"))?
}

fn cc_switch_database_path() -> Option<PathBuf> {
    agent_user_home().map(|home| home.join(".cc-switch").join("cc-switch.db"))
}

fn cc_switch_app_type(value: &str) -> Result<&str, String> {
    match value.trim() {
        "claude" | "codex" | "hermes" => Ok(value.trim()),
        _ => Err("不支持的 CC Switch Agent 类型".to_string()),
    }
}

fn json_string_at<'a>(value: &'a serde_json::Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn unquote_toml_value(value: &str) -> Option<String> {
    let value = value.trim();
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        return serde_json::from_str::<String>(value).ok();
    }
    if value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'') {
        return Some(value[1..value.len() - 1].to_string());
    }
    None
}

fn toml_assignment(line: &str, key: &str) -> Option<String> {
    let clean = line.split('#').next()?.trim();
    let (candidate, value) = clean.split_once('=')?;
    (candidate.trim() == key)
        .then(|| unquote_toml_value(value))
        .flatten()
}

fn codex_live_fields(config: &str) -> (Option<String>, Option<String>) {
    let mut model = None;
    let mut provider = None;
    let mut section = String::new();
    let mut provider_urls = BTreeMap::<String, String>::new();
    for line in config.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            section = trimmed[1..trimmed.len() - 1].trim().to_string();
            continue;
        }
        if section.is_empty() {
            model = model.or_else(|| toml_assignment(trimmed, "model"));
            provider = provider.or_else(|| toml_assignment(trimmed, "model_provider"));
        } else if let Some(name) = section.strip_prefix("model_providers.") {
            if let Some(url) = toml_assignment(trimmed, "base_url") {
                provider_urls.insert(name.trim_matches(['"', '\'']).to_string(), url);
            }
        }
    }
    let endpoint = provider.and_then(|name| provider_urls.remove(&name));
    (model, endpoint)
}

fn cc_switch_config(app_type: &str, settings: &str) -> Result<AgentApiConfig, String> {
    let app_type = cc_switch_app_type(app_type)?;
    let value: serde_json::Value = serde_json::from_str(settings)
        .map_err(|_| "CC Switch Provider 配置格式无效".to_string())?;
    let (api_key, base_url, model) = match app_type {
        "claude" => {
            let key = json_string_at(&value, &["env", "ANTHROPIC_AUTH_TOKEN"])
                .or_else(|| json_string_at(&value, &["env", "ANTHROPIC_API_KEY"]));
            let url = json_string_at(&value, &["env", "ANTHROPIC_BASE_URL"]);
            let model = json_string_at(&value, &["env", "ANTHROPIC_MODEL"])
                .or_else(|| json_string_at(&value, &["env", "ANTHROPIC_DEFAULT_SONNET_MODEL"]))
                .or_else(|| json_string_at(&value, &["env", "ANTHROPIC_DEFAULT_OPUS_MODEL"]));
            (key, url, model)
        }
        "codex" => {
            let key = json_string_at(&value, &["auth", "OPENAI_API_KEY"]);
            let live = json_string_at(&value, &["config"]).unwrap_or_default();
            let (model, endpoint) = codex_live_fields(live);
            return Ok(AgentApiConfig {
                connector_id: app_type.to_string(),
                api_key: key.unwrap_or_default().to_string(),
                base_url: endpoint
                    .ok_or_else(|| "CC Switch Codex 配置缺少 base_url".to_string())?,
                model: model.unwrap_or_default(),
            });
        }
        "hermes" => {
            let model = json_string_at(&value, &["model"]).or_else(|| {
                value
                    .get("models")?
                    .as_array()?
                    .first()
                    .and_then(|item| {
                        item.as_str()
                            .or_else(|| item.get("id").and_then(|v| v.as_str()))
                            .or_else(|| item.get("name").and_then(|v| v.as_str()))
                    })
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
            });
            (
                json_string_at(&value, &["api_key"]),
                json_string_at(&value, &["base_url"]),
                model,
            )
        }
        _ => unreachable!(),
    };
    Ok(AgentApiConfig {
        connector_id: app_type.to_string(),
        api_key: api_key.unwrap_or_default().to_string(),
        base_url: base_url
            .ok_or_else(|| "CC Switch Provider 配置缺少接口地址".to_string())?
            .to_string(),
        model: model.unwrap_or_default().to_string(),
    })
}

fn safe_endpoint(value: &str) -> Option<String> {
    let url = url::Url::parse(value).ok()?;
    let host = url.host_str()?;
    let port = url
        .port()
        .map(|value| format!(":{value}"))
        .unwrap_or_default();
    Some(format!("{}://{host}{port}", url.scheme()))
}

fn cc_switch_provider_summary(
    id: String,
    app_type: String,
    name: String,
    settings: &str,
    is_current: bool,
) -> CcSwitchProviderSummary {
    let config = cc_switch_config(&app_type, settings).ok();
    CcSwitchProviderSummary {
        id,
        app_type,
        name,
        is_current,
        model: config
            .as_ref()
            .map(|value| value.model.clone())
            .filter(|value| !value.is_empty()),
        endpoint: config
            .as_ref()
            .and_then(|value| safe_endpoint(&value.base_url)),
    }
}

fn open_cc_switch_database(path: &Path) -> Result<Connection, String> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| "无法只读打开 CC Switch 数据库".to_string())
}

fn get_cc_switch_status_blocking() -> Result<CcSwitchStatus, String> {
    let Some(path) = cc_switch_database_path() else {
        return Ok(CcSwitchStatus {
            detected: false,
            database_path: None,
            providers: Vec::new(),
        });
    };
    if !path.is_file() {
        return Ok(CcSwitchStatus {
            detected: false,
            database_path: Some(path.to_string_lossy().to_string()),
            providers: Vec::new(),
        });
    }
    let connection = open_cc_switch_database(&path)?;
    let mut statement = connection.prepare(
        "SELECT id, app_type, name, settings_config, is_current FROM providers WHERE app_type IN ('claude','codex','hermes') ORDER BY app_type, is_current DESC, sort_index, name",
    ).map_err(|_| "无法读取 CC Switch Provider 列表".to_string())?;
    let providers = statement
        .query_map([], |row| {
            Ok(cc_switch_provider_summary(
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                &row.get::<_, String>(3)?,
                row.get(4)?,
            ))
        })
        .map_err(|_| "无法查询 CC Switch Provider".to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "无法解析 CC Switch Provider".to_string())?;
    Ok(CcSwitchStatus {
        detected: true,
        database_path: Some(path.to_string_lossy().to_string()),
        providers,
    })
}

#[tauri::command]
async fn get_cc_switch_status() -> Result<CcSwitchStatus, String> {
    tauri::async_runtime::spawn_blocking(get_cc_switch_status_blocking)
        .await
        .map_err(|error| format!("CC Switch 检测任务异常：{error}"))?
}

fn apply_cc_switch_provider_blocking(request: CcSwitchApplyRequest) -> Result<(), String> {
    let app_type = cc_switch_app_type(&request.app_type)?.to_string();
    let provider_id = request.provider_id.trim();
    if provider_id.is_empty()
        || provider_id.len() > 256
        || provider_id.chars().any(char::is_control)
    {
        return Err("CC Switch Provider ID 无效".to_string());
    }
    let path = cc_switch_database_path().ok_or_else(|| "无法定位用户目录".to_string())?;
    let connection = open_cc_switch_database(&path)?;
    let settings = connection
        .query_row(
            "SELECT settings_config FROM providers WHERE id = ?1 AND app_type = ?2",
            params![provider_id, app_type],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|_| "无法查询 CC Switch Provider".to_string())?
        .ok_or_else(|| "没有找到指定的 CC Switch Provider".to_string())?;
    save_agent_api_config_blocking(cc_switch_config(&app_type, &settings)?)
}

#[tauri::command]
async fn apply_cc_switch_provider(request: CcSwitchApplyRequest) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || apply_cc_switch_provider_blocking(request))
        .await
        .map_err(|error| format!("CC Switch 配置应用任务异常：{error}"))?
}

#[tauri::command]
fn pick_dialogue_files(
    app: tauri::AppHandle,
    locale: Option<String>,
) -> Result<Vec<String>, String> {
    let english = locale
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("en-US"));
    let title = if english {
        "Choose files for the Agent"
    } else {
        "选择要交给 Agent 的文件"
    };
    let files = app
        .dialog()
        .file()
        .set_title(title)
        .blocking_pick_files()
        .unwrap_or_default();
    files
        .into_iter()
        .map(|file| {
            file.into_path()
                .map(|path| path.to_string_lossy().to_string())
                .map_err(|_| {
                    if english {
                        "Only local files can be selected".to_string()
                    } else {
                        "只支持选择本机文件".to_string()
                    }
                })
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
    app_session_id: String,
    provider_session_id: Option<String>,
) -> Result<AgentTaskResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        execute_agent_task(
            app,
            connector_id,
            task,
            workspace,
            attachment_paths,
            app_session_id,
            provider_session_id,
        )
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
fn open_social(
    _app: tauri::AppHandle,
    platform: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    validate_social_platform(&platform)?;
    #[cfg(windows)]
    {
        if open_managed_social_route(&_app, &platform, SocialRoute::Publish, &state)? {
            update_social_connection(&state.index, &platform, false)?;
        }
    }
    #[cfg(not(windows))]
    open::that(social_route_url(&platform, SocialRoute::Publish)?)
        .map_err(|error| error.to_string())?;
    append_log(
        &state.index,
        "open_social",
        None,
        &platform,
        "managed_profile",
    );
    Ok(())
}

fn sensitive_url_query_name(name: &str) -> bool {
    let normalized = name
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    matches!(
        normalized.as_str(),
        "token"
            | "accesstoken"
            | "refreshtoken"
            | "idtoken"
            | "authorization"
            | "auth"
            | "apikey"
            | "password"
            | "passwd"
            | "secret"
            | "clientsecret"
            | "credential"
            | "credentials"
            | "cookie"
            | "session"
            | "sessionid"
            | "jwt"
            | "signature"
            | "sig"
            | "code"
            | "state"
    )
}

fn sanitized_external_url(value: &str) -> Result<(url::Url, String), String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 2048 || value.chars().any(char::is_control) {
        return Err("链接为空、过长或包含控制字符".to_string());
    }
    let mut parsed = url::Url::parse(value).map_err(|_| "链接格式无效".to_string())?;
    if parsed.scheme() != "https" || parsed.host_str().is_none() {
        return Err("只支持有效的 HTTPS 链接".to_string());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("链接不能包含用户名或密码".to_string());
    }
    let retained_query = parsed
        .query_pairs()
        .filter(|(name, _)| !sensitive_url_query_name(name))
        .map(|(name, value)| (name.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    parsed.set_query(None);
    if !retained_query.is_empty() {
        let mut query = parsed.query_pairs_mut();
        for (name, value) in retained_query {
            query.append_pair(&name, &value);
        }
    }
    parsed.set_fragment(None);
    let safe_log_target = format!(
        "{}{}",
        parsed.origin().ascii_serialization(),
        if parsed.path().is_empty() {
            "/"
        } else {
            parsed.path()
        }
    );
    Ok((parsed, safe_log_target))
}

#[tauri::command]
fn open_external_url(url: String, state: State<'_, AppState>) -> Result<(), String> {
    let (sanitized, safe_log_target) = sanitized_external_url(&url)?;
    open::that(sanitized.as_str()).map_err(|error| error.to_string())?;
    append_log(
        &state.index,
        "open_external_url",
        None,
        &safe_log_target,
        "success",
    );
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

fn shutdown_social_sessions(app: &tauri::AppHandle) {
    #[cfg(windows)]
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut sessions) = state.social_sessions.lock() {
            sessions.clear();
        }
    }
    #[cfg(not(windows))]
    let _ = app;
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
        TRAY_QUIT_ID => {
            shutdown_social_sessions(app);
            app.exit(0);
        }
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
    shutdown_social_sessions(&app);
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
    let english = APP_LOCALE
        .lock()
        .map(|locale| locale.eq_ignore_ascii_case("en-US"))
        .unwrap_or(false);
    let open_main = MenuItem::with_id(
        &app,
        PET_MENU_OPEN_ID,
        if english {
            "Open Wormhole Pie"
        } else {
            "打开虫洞派"
        },
        true,
        None::<&str>,
    )
    .map_err(|error| error.to_string())?;
    let dialogue = MenuItem::with_id(
        &app,
        PET_MENU_DIALOGUE_ID,
        if english {
            "Start dialogue"
        } else {
            "开始对话"
        },
        true,
        None::<&str>,
    )
    .map_err(|error| error.to_string())?;
    let settings = MenuItem::with_id(
        &app,
        PET_MENU_SETTINGS_ID,
        if english {
            "Pet settings"
        } else {
            "宠物设置"
        },
        true,
        None::<&str>,
    )
    .map_err(|error| error.to_string())?;
    let normal = MenuItem::with_id(
        &app,
        PET_MENU_LAYER_NORMAL_ID,
        if english {
            "Normal layer"
        } else {
            "普通层级"
        },
        true,
        None::<&str>,
    )
    .map_err(|error| error.to_string())?;
    let top = MenuItem::with_id(
        &app,
        PET_MENU_LAYER_TOP_ID,
        if english { "Always on top" } else { "置顶" },
        true,
        None::<&str>,
    )
    .map_err(|error| error.to_string())?;
    let bottom = MenuItem::with_id(
        &app,
        PET_MENU_LAYER_BOTTOM_ID,
        if english {
            "Desktop layer"
        } else {
            "桌面底层"
        },
        true,
        None::<&str>,
    )
    .map_err(|error| error.to_string())?;
    let hide = MenuItem::with_id(
        &app,
        PET_MENU_HIDE_ID,
        if english { "Hide pet" } else { "隐藏宠物" },
        true,
        None::<&str>,
    )
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

fn build_tray_menu(app: &tauri::AppHandle, english: bool) -> Result<Menu<tauri::Wry>, String> {
    let tray_show = MenuItem::with_id(
        app,
        TRAY_SHOW_ID,
        if english {
            "Show Wormhole Pie"
        } else {
            "显示虫洞派"
        },
        true,
        None::<&str>,
    )
    .map_err(|error| error.to_string())?;
    let tray_exit_rest = MenuItem::with_id(
        app,
        TRAY_EXIT_REST_ID,
        if english {
            "End break and show app"
        } else {
            "结束休息并显示虫洞派"
        },
        true,
        None::<&str>,
    )
    .map_err(|error| error.to_string())?;
    let tray_restore_pet = MenuItem::with_id(
        app,
        TRAY_RESTORE_PET_ID,
        if english {
            "Find pet (normal layer)"
        } else {
            "找回宠物（普通层级）"
        },
        true,
        None::<&str>,
    )
    .map_err(|error| error.to_string())?;
    let separator = PredefinedMenuItem::separator(app).map_err(|error| error.to_string())?;
    let tray_quit = MenuItem::with_id(
        app,
        TRAY_QUIT_ID,
        if english {
            "Quit Wormhole Pie"
        } else {
            "退出虫洞派"
        },
        true,
        None::<&str>,
    )
    .map_err(|error| error.to_string())?;
    Menu::with_items(
        app,
        &[
            &tray_show,
            &tray_exit_rest,
            &tray_restore_pet,
            &separator,
            &tray_quit,
        ],
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_app_locale(app: tauri::AppHandle, locale: String) -> Result<(), String> {
    let english = locale.eq_ignore_ascii_case("en-US");
    *APP_LOCALE
        .lock()
        .map_err(|_| "语言状态不可用".to_string())? =
        if english { "en-US" } else { "zh-CN" }.to_string();
    if let Some(tray) = app.tray_by_id("wormhole-pie-tray") {
        tray.set_menu(Some(build_tray_menu(&app, english)?))
            .map_err(|error| error.to_string())?;
        tray.set_tooltip(Some(if english { "Wormhole Pie" } else { "虫洞派" }))
            .map_err(|error| error.to_string())?;
    }
    app.emit("locale://changed", if english { "en-US" } else { "zh-CN" })
        .map_err(|error| error.to_string())
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
fn trash_paths_match(left: &Path, right: &Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        fn comparable(path: &Path) -> String {
            let mut value = path.to_string_lossy().replace('/', "\\");
            if let Some(rest) = value.strip_prefix(r"\\?\UNC\") {
                value = format!(r"\\{rest}");
            } else if let Some(rest) = value.strip_prefix(r"\\?\") {
                value = rest.to_string();
            }
            value.trim_end_matches('\\').to_lowercase()
        }
        comparable(left) == comparable(right)
    }
    #[cfg(not(target_os = "windows"))]
    {
        left == right
    }
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
    const LIST_RETRY_ATTEMPTS: usize = 20;
    const LIST_RETRY_DELAY: Duration = Duration::from_millis(100);
    for attempt in 0..LIST_RETRY_ATTEMPTS {
        let trash_items = trash::os_limited::list().map_err(|error| error.to_string())?;
        let mut selected = Vec::new();
        for path in paths {
            if let Some(item) = trash_items
                .iter()
                .filter(|item| trash_paths_match(&item.original_path(), path))
                .max_by_key(|item| item.time_deleted)
            {
                selected.push(item.clone());
            }
        }
        if selected.len() == paths.len() {
            return trash::os_limited::restore_all(selected).map_err(|error| error.to_string());
        }
        if attempt + 1 < LIST_RETRY_ATTEMPTS {
            thread::sleep(LIST_RETRY_DELAY);
        }
    }
    Err("???????????????".to_string())
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
    let _operation_guard = state
        .organize_lock
        .lock()
        .map_err(|_| "桌面整理操作已锁定".to_string())?;
    let (payload, deleted) =
        feed_paths_to_trash(&state.index, paths.into_iter().map(PathBuf::from).collect())?;
    if let Ok(mut last_feed) = state.last_feed.lock() {
        *last_feed = deleted;
    }
    let _ = app.emit("pet://fed", payload.clone());
    Ok(payload)
}

fn feed_paths_to_trash(
    index: &SharedIndex,
    paths: Vec<PathBuf>,
) -> Result<(FeedEvent, Vec<PathBuf>), String> {
    if paths.is_empty() {
        return Err("没有收到文件".to_string());
    }
    if paths.len() > 20 {
        return Err("一次最多喂给宠物 20 个项目".to_string());
    }

    let mut approved = Vec::new();
    for raw_path in paths {
        approved.push(validate_feed_path(index, &raw_path)?);
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
                    let _ = scan_index(index, true);
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
    let _ = scan_index(index, true);
    let payload = FeedEvent {
        count: names.len(),
        failed_count: requested_count.saturating_sub(names.len()),
        names,
        warning: partial_error.clone(),
    };
    Ok((payload, deleted))
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
    #[cfg(windows)]
    if let Some(exit_code) = maybe_run_elevated_file_plan() {
        std::process::exit(exit_code);
    }

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
                #[cfg(windows)]
                public_desktop_confirmations: Mutex::new(BTreeMap::new()),
                #[cfg(windows)]
                social_sessions: Mutex::new(BTreeMap::new()),
            });

            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let main = app
                .get_webview_window("main")
                .ok_or_else(|| std::io::Error::other("主窗口不存在"))?;
            #[cfg(not(target_os = "macos"))]
            main.set_skip_taskbar(true)?;
            position_main_top_right(&main).map_err(std::io::Error::other)?;

            #[allow(unused_variables)]
            let pet = app
                .get_webview_window("pet")
                .ok_or_else(|| std::io::Error::other("宠物窗口不存在"))?;
            #[cfg(not(target_os = "macos"))]
            pet.set_skip_taskbar(true)?;

            #[allow(unused_variables)]
            let pet_dialogue = app
                .get_webview_window("pet-dialogue")
                .ok_or_else(|| std::io::Error::other("宠物对话窗口不存在"))?;
            #[cfg(not(target_os = "macos"))]
            pet_dialogue.set_skip_taskbar(true)?;

            let tray_menu = build_tray_menu(app.handle(), false).map_err(std::io::Error::other)?;
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
            request_public_desktop_confirmation,
            organize_desktop,
            review_desktop_organize,
            list_organize_exclusions,
            remove_organize_exclusion,
            undo_desktop_organize,
            restore_library_items_to_desktop,
            update_file_category,
            open_file,
            list_programs,
            launch_program,
            list_agent_connectors,
            get_agent_default_workspace,
            get_agent_default_install_directory,
            pick_directory,
            install_agent,
            test_agent_api,
            save_agent_api_config,
            get_cc_switch_status,
            apply_cc_switch_provider,
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
            open_social_session,
            disconnect_social_session,
            clear_social_session,
            list_social_accounts,
            save_social_snapshot,
            sync_social_snapshot,
            open_external_url,
            start_watching,
            window_action,
            hide_main_to_tray,
            show_main_from_tray,
            quit_app,
            show_pet_dialogue,
            hide_pet_dialogue,
            show_pet_context_menu,
            set_app_locale,
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
    fn public_desktop_organization_requires_explicit_opt_in() {
        assert!(!include_public_desktop_requested(None));
        assert!(!include_public_desktop_requested(Some(false)));
        assert!(include_public_desktop_requested(Some(true)));
    }

    #[cfg(windows)]
    #[test]
    fn elevated_plan_digest_and_expiry_are_checked_before_any_move() {
        let workspace = TempWorkspace::new("elevated-plan-digest");
        let helper_root = std::env::temp_dir().join("wormhole-pie-elevated-plans");
        fs::create_dir_all(&helper_root).unwrap();
        let plan_path = helper_root.join(format!(
            "test-plan-{}-{}.json",
            std::process::id(),
            now_millis()
        ));
        let transfer = ElevatedFileTransfer {
            source: workspace.0.join("source.txt"),
            destination: workspace.0.join("destination.txt"),
            is_dir: false,
            source_digest: "00".repeat(32),
            source_volume_serial: 0,
            source_file_index: 0,
            source_size: 0,
            source_last_write_time: 0,
        };
        let plan = ElevatedFilePlan {
            version: ELEVATED_FILE_PLAN_VERSION,
            created_at_millis: now_millis(),
            library_root: workspace.0.clone(),
            transfers: vec![transfer.clone()],
        };
        let serialized = serde_json::to_vec(&plan).unwrap();
        fs::write(&plan_path, &serialized).unwrap();
        let wrong_digest = "0".repeat(64);
        assert!(execute_elevated_file_plan(&plan_path, &wrong_digest)
            .unwrap_err()
            .contains("摘要不匹配"));

        let original_digest = elevated_plan_digest(&serialized);
        let mut tampered = serialized.clone();
        tampered.push(b' ');
        fs::write(&plan_path, tampered).unwrap();
        assert!(execute_elevated_file_plan(&plan_path, &original_digest)
            .unwrap_err()
            .contains("摘要不匹配"));

        let expired = ElevatedFilePlan {
            version: ELEVATED_FILE_PLAN_VERSION,
            created_at_millis: now_millis().saturating_sub(6 * 60 * 1_000),
            library_root: workspace.0.clone(),
            transfers: vec![transfer],
        };
        let serialized = serde_json::to_vec(&expired).unwrap();
        fs::write(&plan_path, &serialized).unwrap();
        assert!(
            execute_elevated_file_plan(&plan_path, &elevated_plan_digest(&serialized))
                .unwrap_err()
                .contains("版本无效")
        );
        let _ = fs::remove_file(plan_path);
    }

    #[cfg(windows)]
    #[test]
    fn cross_volume_copy_helper_preserves_file_and_directory_digests() {
        let workspace = TempWorkspace::new("cross-volume-copy");
        let source_file = workspace.0.join("source.txt");
        let destination_file = workspace.0.join("copied.txt");
        fs::write(&source_file, b"wormhole-pie-cross-volume").unwrap();
        copy_elevated_path(&source_file, &destination_file).unwrap();
        assert_eq!(
            elevated_path_digest(&source_file).unwrap(),
            elevated_path_digest(&destination_file).unwrap()
        );

        let source_directory = workspace.0.join("source-directory");
        let nested = source_directory.join("nested");
        fs::create_dir_all(&nested).unwrap();
        fs::write(source_directory.join("one.txt"), b"one").unwrap();
        fs::write(nested.join("two.bin"), [0u8, 1, 2, 3, 4]).unwrap();
        let destination_directory = workspace.0.join("copied-directory");
        copy_elevated_path(&source_directory, &destination_directory).unwrap();
        assert_eq!(
            elevated_path_digest(&source_directory).unwrap(),
            elevated_path_digest(&destination_directory).unwrap()
        );
    }

    #[test]
    fn elevated_file_moves_only_allow_public_desktop_library_pairs() {
        let workspace = TempWorkspace::new("elevated-boundaries");
        let public_desktop = workspace.0.join("Public").join("Desktop");
        let library = workspace.0.join("Documents").join(LIBRARY_ROOT_NAME);
        let category = library.join("文档");
        let user_desktop = workspace.0.join("User").join("Desktop");
        let outside = workspace.0.join("Outside");

        assert!(elevated_parent_pair_allowed(
            &public_desktop,
            &category,
            &public_desktop,
            &library
        ));
        assert!(elevated_parent_pair_allowed(
            &category,
            &public_desktop,
            &public_desktop,
            &library
        ));
        assert!(!elevated_parent_pair_allowed(
            &user_desktop,
            &category,
            &public_desktop,
            &library
        ));
        assert!(!elevated_parent_pair_allowed(
            &public_desktop,
            &outside,
            &public_desktop,
            &library
        ));
        assert!(!elevated_parent_pair_allowed(
            &public_desktop,
            &library,
            &public_desktop,
            &library
        ));
        assert!(!elevated_parent_pair_allowed(
            &public_desktop,
            &category.join("nested"),
            &public_desktop,
            &library
        ));
    }

    #[cfg(windows)]
    #[test]
    fn social_page_matching_rejects_lookalike_and_non_web_hosts() {
        assert!(social_host_matches(
            "xiaohongshu",
            "https://www.xiaohongshu.com/user/profile"
        ));
        assert!(social_host_matches("x", "https://x.com/notifications"));
        assert!(social_host_matches(
            "douyin",
            "https://creator.douyin.com/creator-micro/home"
        ));
        assert!(!social_host_matches(
            "x",
            "https://x.com.attacker.example/messages"
        ));
        assert!(!social_host_matches(
            "douyin",
            "file:///C:/Users/Public/Desktop/test.html"
        ));
        assert!(!social_host_matches("unknown", "https://x.com/home"));
    }

    #[test]
    fn saved_social_metrics_keep_unavailable_and_verified_source_semantics() {
        assert_eq!(
            normalize_saved_social_metric(42, SOCIAL_METRIC_UNAVAILABLE, 99, SOCIAL_METRIC_MANUAL),
            (0, SOCIAL_METRIC_UNAVAILABLE.to_string())
        );
        assert_eq!(
            normalize_saved_social_metric(
                1_250,
                SOCIAL_METRIC_VISIBLE_PAGE,
                1_250,
                SOCIAL_METRIC_VISIBLE_PAGE,
            ),
            (1_250, SOCIAL_METRIC_VISIBLE_PAGE.to_string())
        );
        assert_eq!(
            normalize_saved_social_metric(
                1_251,
                SOCIAL_METRIC_VISIBLE_PAGE,
                1_250,
                SOCIAL_METRIC_VISIBLE_PAGE,
            ),
            (1_251, SOCIAL_METRIC_MANUAL.to_string())
        );
        assert_eq!(
            normalize_saved_social_metric(0, SOCIAL_METRIC_MANUAL, 9, SOCIAL_METRIC_VISIBLE_PAGE),
            (0, SOCIAL_METRIC_MANUAL.to_string())
        );
        assert_eq!(
            normalize_saved_social_metric(7, "unexpected", 0, SOCIAL_METRIC_UNAVAILABLE),
            (7, SOCIAL_METRIC_MANUAL.to_string())
        );
    }

    #[cfg(windows)]
    #[test]
    fn social_metrics_script_only_returns_visible_summaries() {
        for platform in ["xiaohongshu", "x", "douyin"] {
            let script = visible_social_metrics_expression(platform).unwrap();
            let normalized = script.to_ascii_lowercase();
            assert!(!normalized.contains("document.cookie"));
            assert!(!normalized.contains("cookiestore"));
            assert!(!normalized.contains("localstorage"));
            assert!(!normalized.contains("sessionstorage"));
            assert!(!normalized.contains("indexeddb"));
            assert!(!normalized.contains("innerhtml"));
            assert!(!normalized.contains("outerhtml"));
            assert!(!script.contains("elementText(surface)"));
            assert!(script.contains("displayName"));
            assert!(script.contains("accountIdentity"));
            assert!(script.contains("followers"));
            assert!(script.contains("unreadMessages"));
            assert!(script.contains("unreadNotifications"));
        }
    }

    #[cfg(windows)]
    #[test]
    fn social_target_selection_never_guesses_between_multiple_pages() {
        let page = |target_id: &str, url: &str| CdpTargetInfo {
            target_id: target_id.to_string(),
            target_type: "page".to_string(),
            url: url.to_string(),
        };
        let one = vec![
            page("x-main", "https://x.com/home"),
            page("unrelated", "https://example.com/"),
        ];
        assert_eq!(
            select_unique_social_target("x", &one).unwrap().target_id,
            "x-main"
        );

        let ambiguous = vec![
            page("x-main", "https://x.com/home"),
            page("x-notifications", "https://x.com/notifications"),
        ];
        assert!(select_unique_social_target("x", &ambiguous).is_err());
    }

    #[test]
    fn external_urls_require_https_and_remove_secrets_from_open_and_log_values() {
        assert!(sanitized_external_url("http://example.com/").is_err());
        assert!(sanitized_external_url("https://user:password@example.com/").is_err());

        let (url, log_target) = sanitized_external_url(
            "https://example.com/path?q=public&access_token=secret&state=private#fragment",
        )
        .unwrap();
        assert_eq!(url.as_str(), "https://example.com/path?q=public");
        assert_eq!(log_target, "https://example.com/path");
        assert!(!url.as_str().contains("secret"));
        assert!(!url.as_str().contains("private"));
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
                        public_identity: None,
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

    #[cfg(target_os = "windows")]
    #[test]
    #[ignore = "requires an interactive Windows Recycle Bin"]
    fn windows_pet_feed_round_trip_uses_the_system_recycle_bin() {
        let workspace = TempWorkspace::new("feed-recycle-round-trip");
        let index = test_index(&workspace);
        let file = index.desktop_path.join("wormhole-pie-feed-round-trip.txt");
        fs::write(&file, b"wormhole pie recycle-bin verification").unwrap();
        let canonical = file.canonicalize().unwrap();

        let (event, deleted) = feed_paths_to_trash(&index, vec![canonical.clone()]).unwrap();
        assert_eq!(event.count, 1);
        assert_eq!(event.failed_count, 0);
        assert_eq!(deleted, vec![canonical.clone()]);
        assert!(
            !canonical.exists(),
            "file should be inside the Windows recycle bin"
        );

        restore_paths_from_trash(&deleted).unwrap();
        assert!(
            canonical.exists(),
            "undo should restore the file to its original path"
        );
        assert_eq!(
            fs::read(&canonical).unwrap(),
            b"wormhole pie recycle-bin verification"
        );
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
            public_identity: None,
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
    fn agent_install_and_api_inputs_are_bounded_and_platform_safe() {
        let workspace = TempWorkspace::new("agent-install-root");
        let install = validate_agent_install_directory(&workspace.0.to_string_lossy()).unwrap();
        assert_eq!(install, workspace.0.canonicalize().unwrap());
        assert!(validate_agent_install_directory("relative/agents").is_err());
        assert!(validate_agent_install_directory(
            install
                .ancestors()
                .last()
                .unwrap()
                .to_string_lossy()
                .as_ref()
        )
        .is_err());

        let valid = AgentApiConfig {
            connector_id: "codex".to_string(),
            api_key: "secret-not-logged".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            model: "gpt-test".to_string(),
        };
        assert!(validate_agent_api_config(&valid).is_ok());
        assert!(validate_agent_api_config(&AgentApiConfig {
            base_url: "file:///tmp/key".to_string(),
            ..valid.clone()
        })
        .is_err());
        assert!(validate_agent_api_config(&AgentApiConfig {
            connector_id: "unknown".to_string(),
            ..valid
        })
        .is_err());
    }

    #[test]
    fn codex_managed_config_replacement_is_idempotent() {
        let first = "# BEGIN WORMHOLE PIE MANAGED CONFIG\nmodel = \"old\"\n# END WORMHOLE PIE MANAGED CONFIG";
        let replacement = "# BEGIN WORMHOLE PIE MANAGED CONFIG\nmodel = \"new\"\n# END WORMHOLE PIE MANAGED CONFIG";
        let updated = replace_managed_codex_block(
            &format!("{first}\n[history]\npersistence = \"save-all\"\n"),
            replacement,
        );
        assert_eq!(updated.matches("BEGIN WORMHOLE PIE").count(), 1);
        assert!(updated.contains("model = \"new\""));
        assert!(updated.contains("[history]"));
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
            let arguments = agent_task_arguments(kind, task, None);
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

        let codex = agent_task_arguments(AgentConnectorKind::Codex, task, None);
        assert!(codex.iter().any(|argument| argument == "workspace-write"));
        let hermes = agent_task_arguments(AgentConnectorKind::Hermes, task, None);
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
        let public = agent_task_public_output(AgentConnectorKind::Claude, &failed);
        assert!(!public.0.contains("workflow"));
        assert!(!public.0.contains("settings.json"));
        assert!(!public.0.contains("secret"));

        let succeeded = BoundedProcessOutput {
            success: true,
            stdout: "最终结果".to_string(),
            ..failed
        };
        assert_eq!(
            agent_task_public_output(AgentConnectorKind::Claude, &succeeded),
            ("最终结果".to_string(), None)
        );

        let succeeded_json = BoundedProcessOutput {
            stdout: r#"{"result":"最终结果","session_id":"12345678-1234-4abc-a123-123456789abc"}"#
                .to_string(),
            ..succeeded
        };
        assert_eq!(
            agent_task_public_output(AgentConnectorKind::Claude, &succeeded_json),
            (
                "最终结果".to_string(),
                Some("12345678-1234-4abc-a123-123456789abc".to_string())
            )
        );
    }

    #[test]
    fn failed_agent_output_exposes_only_safe_actionable_categories() {
        let failed = BoundedProcessOutput {
            success: false,
            timed_out: false,
            cancelled: false,
            stdout: String::new(),
            stderr: "There's an issue with the selected model (private-model). It may not exist or you may not have access to it. token=secret".to_string(),
            exit_code: Some(1),
            duration_ms: 10,
            truncated: false,
        };
        let public = agent_task_public_output(AgentConnectorKind::Claude, &failed);
        assert_eq!(
            public.0,
            "Claude Code 当前模型不可用或账号无权访问，请在 Agent 管理中检查模型配置。"
        );
        assert!(!public.0.contains("private-model"));
        assert!(!public.0.contains("secret"));
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

    #[test]
    fn cc_switch_extracts_supported_provider_formats() {
        let claude = cc_switch_config(
            "claude",
            r#"{"env":{"ANTHROPIC_AUTH_TOKEN":"secret-claude","ANTHROPIC_BASE_URL":"https://claude.example/v1","ANTHROPIC_MODEL":"claude-test"}}"#,
        ).unwrap();
        assert_eq!(claude.connector_id, "claude");
        assert_eq!(claude.base_url, "https://claude.example/v1");
        assert_eq!(claude.model, "claude-test");

        let codex = cc_switch_config(
            "codex",
            r#"{"auth":{"OPENAI_API_KEY":"secret-codex"},"config":"model_provider = \"selected\"\nmodel = \"gpt-test\"\n[model_providers.other]\nbase_url = \"https://wrong.example/v1\"\n[model_providers.selected]\nbase_url = \"https://codex.example/v1\""}"#,
        ).unwrap();
        assert_eq!(codex.base_url, "https://codex.example/v1");
        assert_eq!(codex.model, "gpt-test");

        let hermes = cc_switch_config(
            "hermes",
            r#"{"api_key":"secret-hermes","base_url":"https://hermes.example/v1","models":[{"id":"hermes-test"}]}"#,
        ).unwrap();
        assert_eq!(hermes.base_url, "https://hermes.example/v1");
        assert_eq!(hermes.model, "hermes-test");
    }

    #[test]
    fn cc_switch_summary_never_serializes_credentials() {
        let summary = cc_switch_provider_summary(
            "provider-1".to_string(),
            "claude".to_string(),
            "Work profile".to_string(),
            r#"{"env":{"ANTHROPIC_API_KEY":"do-not-leak","ANTHROPIC_BASE_URL":"https://user:password@example.com/private/path?token=hidden","ANTHROPIC_MODEL":"claude-test"}}"#,
            true,
        );
        let serialized = serde_json::to_string(&summary).unwrap();
        assert_eq!(summary.endpoint.as_deref(), Some("https://example.com"));
        assert!(!serialized.contains("do-not-leak"));
        assert!(!serialized.contains("password"));
        assert!(!serialized.contains("private/path"));
        assert!(!serialized.contains("token"));
    }

    #[test]
    fn installed_cc_switch_database_is_readable_without_serializing_settings() {
        let Some(path) = cc_switch_database_path() else {
            return;
        };
        if !path.is_file() {
            return;
        }
        let status = get_cc_switch_status_blocking().unwrap();
        assert!(status.detected);
        assert!(!status.providers.is_empty());
        let serialized = serde_json::to_string(&status).unwrap();
        assert!(!serialized.contains("settings_config"));
        assert!(!serialized.contains("OPENAI_API_KEY"));
        assert!(!serialized.contains("ANTHROPIC_AUTH_TOKEN"));

        let connection = open_cc_switch_database(&path).unwrap();
        let mut statement = connection.prepare(
            "SELECT app_type, settings_config FROM providers WHERE app_type IN ('claude','codex','hermes')",
        ).unwrap();
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap();
        for row in rows {
            let (app_type, settings) = row.unwrap();
            if let Ok(config) = cc_switch_config(&app_type, &settings) {
                if !config.api_key.is_empty() {
                    assert!(!serialized.contains(&config.api_key));
                }
            }
        }
    }

    #[test]
    fn cc_switch_rejects_unknown_app_types() {
        assert!(cc_switch_config("unknown", "{}").is_err());
        assert!(cc_switch_app_type("codex\n").is_ok());
        assert!(cc_switch_app_type("codex-other").is_err());
    }
}
