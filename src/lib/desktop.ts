import type { DesktopFile, FileKind, OrganizedCategory, OrganizeExclusion, ProgramEntry } from "../types";
import type { PetMarkerEvent, PetRuntimeSnapshot, PetSignalEvent } from "../pet/petBehaviorTypes";
import { mockFiles } from "../data/mockData";
import { LogicalPosition } from "@tauri-apps/api/dpi";
import { getCurrentWindow } from "@tauri-apps/api/window";

type TauriWindow = Window & { __TAURI_INTERNALS__?: unknown };

const browserPrograms: ProgramEntry[] = [
  { name: "记事本", path: "C:\\Windows\\System32\\notepad.exe", source: "system" },
  { name: "画图", path: "C:\\Windows\\System32\\mspaint.exe", source: "system" },
  { name: "计算器", path: "calculator:", source: "system" },
];

export type AgentConnectorId = "codex" | "claude" | "hermes";
export type DialogueConnectorId = "local" | AgentConnectorId;
export type DialogueSubmission = {
  text: string;
  attachmentPaths: string[];
};
export type DialogueCommand = DialogueSubmission & {
  connectorId: DialogueConnectorId;
};
export type AgentConnectorConfigurationState = "ready" | "not_configured" | "permission_blocked" | "probe_failed" | "not_installed";

export type AgentConnectorStatus = {
  id: AgentConnectorId;
  name: string;
  detected: boolean;
  available: boolean;
  configured: boolean;
  configurationState: AgentConnectorConfigurationState;
  configLocationLabel?: string;
  executable?: string;
  version?: string;
  detail: string;
};

export type AgentTaskResult = {
  taskId: string;
  connectorId: AgentConnectorId;
  success: boolean;
  timedOut: boolean;
  cancelled: boolean;
  output: string;
  exitCode: number | null;
  durationMs: number;
  truncated: boolean;
  files: AgentResultFile[];
};

export type AgentResultFile = {
  name: string;
  path: string;
  relativePath: string;
};

export type AgentTaskState = "starting" | "running" | "cancelling" | "succeeded" | "failed" | "cancelled" | "timed_out";

export type AgentTaskStatus = {
  taskId: string;
  connectorId: AgentConnectorId;
  state: AgentTaskState;
  startedAt: number;
  updatedAt: number;
  elapsedMs: number;
  cancelRequested: boolean;
  detail: string;
};

const browserAgentConnectors: AgentConnectorStatus[] = [
  { id: "codex", name: "Codex", detected: false, available: false, configured: false, configurationState: "not_installed", detail: "浏览器预览无法检测本机 CLI 与配置" },
  { id: "claude", name: "Claude Code", detected: false, available: false, configured: false, configurationState: "not_installed", detail: "浏览器预览无法检测本机 CLI 与配置" },
  { id: "hermes", name: "Hermes Agent", detected: false, available: false, configured: false, configurationState: "not_installed", detail: "浏览器预览无法检测本机 CLI 与配置" },
];

let browserExclusions: OrganizeExclusion[] = [];
let browserDesktopIconsHidden = false;
let browserDesktopFiles: DesktopFile[] = mockFiles.map((file) => ({ ...file }));
let browserOrganizedFiles: DesktopFile[] = [];
let browserBatchSequence = 0;
let browserBatches: Array<{
  batchId: string;
  items: DesktopOrganizeItem[];
  originals: Map<string, DesktopFile>;
}> = [];

export const isTauri = () => Boolean((window as TauriWindow).__TAURI_INTERNALS__);

const STARTUP_STATE_RETRY_TIMEOUT_MS = 10_000;

function isStartupStateRace(error: unknown) {
  const message = error instanceof Error ? error.message : String(error);
  return /state not managed for field .*\.manage\(\)/i.test(message);
}

async function invokeWhenStateReady<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  const deadline = Date.now() + STARTUP_STATE_RETRY_TIMEOUT_MS;
  while (true) {
    try {
      return await invoke<T>(command, args);
    } catch (error) {
      if (!isStartupStateRace(error) || Date.now() >= deadline) throw error;
      await new Promise<void>((resolve) => window.setTimeout(resolve, 75));
    }
  }
}

export async function listDesktopFiles(): Promise<DesktopFile[]> {
  if (!isTauri()) return [...browserDesktopFiles, ...browserOrganizedFiles].sort((left, right) => right.modifiedAt - left.modifiedAt);
  return invokeWhenStateReady<DesktopFile[]>("list_files");
}

export async function scanDesktopFiles(): Promise<DesktopFile[]> {
  if (!isTauri()) return listDesktopFiles();
  return invokeWhenStateReady<DesktopFile[]>("scan_desktop");
}

export type DesktopOrganizeResult = {
  movedCount: number;
  newMovedCount?: number;
  migratedCount?: number;
  categoryCount: number;
  skippedCount: number;
  rootPath: string;
  batchId?: string;
  items?: DesktopOrganizeItem[];
  excludedCount?: number;
  skippedItems?: Array<{
    name: string;
    path: string;
    reasonCode: string;
    reason: string;
  }>;
  indexedCount?: number;
  publicDesktopCount?: number;
};

export type DesktopIconState = {
  supported: boolean;
  hidden: boolean;
  publicDesktopCount: number;
};

export type OrganizeState = {
  canUndo: boolean;
  batchCount: number;
};

export type DesktopOrganizeItem = {
  moveId: string;
  name: string;
  originalPath: string;
  organizedPath: string;
  category: string;
  kind: FileKind;
  launchable: boolean;
};

export type DesktopOrganizeReviewResult = {
  restoredCount: number;
  rememberedCount: number;
  remainingUndoCount: number;
  conflictCount: number;
  restoredMoveIds: string[];
};

function mockOrganizeCategory(file: DesktopFile): OrganizedCategory {
  const extension = file.extension.toLocaleLowerCase();
  if (file.kind === "folder") return extension === "app" ? "程序" : "文件夹";
  if (["doc", "docx", "pdf", "ppt", "pptx", "xls", "xlsx", "txt", "md", "rtf", "csv", "odt", "ods", "odp", "pages", "numbers", "key", "epub"].includes(extension)) return "文档";
  if (["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "heic", "heif", "tif", "tiff", "ico"].includes(extension)) return "图片";
  if (["mp4", "mov", "avi", "mkv", "webm", "m4v", "wmv", "flv", "mpeg", "mpg"].includes(extension)) return "视频";
  if (["mp3", "wav", "m4a", "flac", "aac", "ogg", "wma", "aiff", "mid", "midi"].includes(extension)) return "音频";
  if (["zip", "rar", "7z", "tar", "gz", "bz2", "xz", "tgz", "zst"].includes(extension)) return "压缩包";
  if (["ts", "tsx", "js", "jsx", "py", "rs", "go", "java", "kt", "swift", "c", "cpp", "h", "hpp", "cs", "rb", "php", "html", "css", "scss", "json", "yaml", "yml", "toml", "sh", "sql"].includes(extension)) return "代码";
  if (["lnk", "url", "webloc", "desktop", "alias"].includes(extension)) return "快捷方式";
  if (["exe", "msi", "msix", "msixbundle", "bat", "cmd", "ps1", "vbs", "command", "app", "appinstaller", "dmg", "pkg"].includes(extension)) return "程序";
  return "其他";
}

function joinWindowsPath(...parts: string[]) {
  return parts.map((part, index) => index ? part.replace(/^[\\/]+|[\\/]+$/g, "") : part.replace(/[\\/]+$/g, "")).join("\\");
}

export async function organizeDesktop(): Promise<DesktopOrganizeResult> {
  if (!isTauri()) {
    const rootPath = "C:\\Users\\你\\Documents\\虫洞派资料库";
    const exclusionKeys = new Set(browserExclusions.map((item) => item.nameKey));
    const candidates = browserDesktopFiles.filter((file) => !exclusionKeys.has(file.name.trim().toLocaleLowerCase()));
    const excludedCount = browserDesktopFiles.length - candidates.length;
    const batchId = `browser-preview-${++browserBatchSequence}`;
    const originals = new Map<string, DesktopFile>();
    const items = candidates.map((file, index) => {
      const category = mockOrganizeCategory(file);
      const moveId = `${batchId}-${index}`;
      originals.set(moveId, file);
      return {
        moveId,
        name: file.name,
        originalPath: file.path,
        organizedPath: joinWindowsPath(rootPath, category, file.name),
        category,
        kind: file.kind,
        launchable: category === "快捷方式" || category === "程序",
      };
    });
    const movedIds = new Set(candidates.map((file) => file.id));
    browserDesktopFiles = browserDesktopFiles.filter((file) => !movedIds.has(file.id));
    browserOrganizedFiles = [
      ...browserOrganizedFiles,
      ...items.map((item) => ({
        ...originals.get(item.moveId)!,
        path: item.organizedPath,
        organizedCategory: item.category,
        isNew: true,
      })),
    ];
    if (items.length) browserBatches.push({ batchId, items, originals });
    return {
      movedCount: items.length,
      newMovedCount: items.length,
      migratedCount: 0,
      categoryCount: new Set(items.map((item) => item.category)).size,
      skippedCount: 0,
      rootPath,
      batchId: items.length ? batchId : undefined,
      excludedCount,
      publicDesktopCount: 2,
      items,
    };
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<DesktopOrganizeResult>("organize_desktop");
}

export async function reviewDesktopOrganize(batchId: string, excludedMoveIds: string[]): Promise<DesktopOrganizeReviewResult> {
  if (!isTauri()) {
    const batchIndex = browserBatches.findIndex((batch) => batch.batchId === batchId);
    if (batchIndex < 0) throw new Error("浏览器预览中的整理批次已失效");
    const batch = browserBatches[batchIndex];
    const selected = new Set(excludedMoveIds);
    const now = Date.now();
    const known = new Map(browserExclusions.map((item) => [item.nameKey, item]));
    const restored: DesktopFile[] = [];
    batch.items.forEach((item) => {
      if (!selected.has(item.moveId)) return;
      const file = batch.originals.get(item.moveId);
      if (!file) return;
      const nameKey = file.name.trim().toLocaleLowerCase();
      known.set(nameKey, {
        nameKey,
        displayName: file.name,
        isDirectory: file.kind === "folder",
        createdAt: now,
      });
      restored.push({ ...file, isNew: false });
    });
    browserExclusions = [...known.values()];
    const restoredPaths = new Set(batch.items.filter((item) => selected.has(item.moveId)).map((item) => item.organizedPath.toLocaleLowerCase()));
    browserOrganizedFiles = browserOrganizedFiles.filter((file) => !restoredPaths.has(file.path.toLocaleLowerCase()));
    browserDesktopFiles = [...browserDesktopFiles, ...restored];
    batch.items = batch.items.filter((item) => !selected.has(item.moveId));
    if (!batch.items.length) browserBatches.splice(batchIndex, 1);
    return {
      restoredCount: restored.length,
      rememberedCount: restored.length,
      remainingUndoCount: batch.items.length,
      conflictCount: 0,
      restoredMoveIds: restored.map((file) => {
        const matched = [...batch.originals.entries()].find(([, original]) => original.id === file.id);
        return matched?.[0] ?? "";
      }).filter(Boolean),
    };
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<DesktopOrganizeReviewResult>("review_desktop_organize", { batchId, excludedMoveIds });
}

export async function undoDesktopOrganize(): Promise<DesktopOrganizeResult> {
  if (!isTauri()) {
    const batch = browserBatches.pop();
    if (!batch) return { movedCount: 0, categoryCount: 0, skippedCount: 0, rootPath: "Documents\\虫洞派资料库", items: [] };
    const restored = batch.items.flatMap((item) => {
      const file = batch.originals.get(item.moveId);
      return file ? [{ ...file, isNew: false }] : [];
    });
    const organizedPaths = new Set(batch.items.map((item) => item.organizedPath.toLocaleLowerCase()));
    browserOrganizedFiles = browserOrganizedFiles.filter((file) => !organizedPaths.has(file.path.toLocaleLowerCase()));
    browserDesktopFiles = [...browserDesktopFiles, ...restored];
    return {
      movedCount: restored.length,
      categoryCount: new Set(batch.items.map((item) => item.category)).size,
      skippedCount: 0,
      rootPath: "Documents\\虫洞派资料库",
      batchId: batch.batchId,
      items: batch.items,
    };
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<DesktopOrganizeResult>("undo_desktop_organize");
}

export async function getOrganizeState(): Promise<OrganizeState> {
  if (!isTauri()) {
    return { canUndo: browserBatches.length > 0, batchCount: browserBatches.length };
  }
  return invokeWhenStateReady<OrganizeState>("get_organize_state");
}

export async function getDesktopIconState(): Promise<DesktopIconState> {
  if (!isTauri()) return { supported: true, hidden: browserDesktopIconsHidden, publicDesktopCount: 2 };
  return invokeWhenStateReady<DesktopIconState>("get_desktop_icon_state");
}

export async function setDesktopIconsHidden(hidden: boolean): Promise<DesktopIconState> {
  if (!isTauri()) {
    browserDesktopIconsHidden = hidden;
    return { supported: true, hidden, publicDesktopCount: 2 };
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<DesktopIconState>("set_desktop_icons_hidden", { hidden });
}

export async function openDesktopFile(fileId: DesktopFile["id"]): Promise<void> {
  if (!isTauri()) return;
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("open_file", { fileId });
}

export async function updateDesktopFileCategory(fileId: DesktopFile["id"], category: string): Promise<void> {
  if (!isTauri()) return;
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("update_file_category", { fileId, category });
}

export async function openSocialPage(platform: "xiaohongshu" | "x" | "douyin"): Promise<void> {
  if (!isTauri()) {
    const urls = {
      xiaohongshu: "https://creator.xiaohongshu.com/publish/publish",
      x: "https://x.com/compose/post",
      douyin: "https://creator.douyin.com/creator-micro/content/upload",
    } as const;
    window.open(urls[platform], "_blank", "noopener,noreferrer");
    return;
  }
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("open_social", { platform });
}

export async function openExternalUrl(url: string): Promise<void> {
  const parsed = new URL(url);
  if (parsed.protocol !== "https:" && parsed.protocol !== "http:") throw new Error("只支持 http 或 https 链接");
  if (!isTauri()) {
    window.open(parsed.toString(), "_blank", "noopener,noreferrer");
    return;
  }
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("open_external_url", { url: parsed.toString() });
}

export async function startDesktopWatcher(): Promise<void> {
  if (!isTauri()) return;
  await invokeWhenStateReady<void>("start_watching");
}

export async function subscribeToFileChanges(onChange: () => void) {
  if (!isTauri()) return () => undefined;
  const { listen } = await import("@tauri-apps/api/event");
  return listen("files://changed", onChange);
}

export async function windowAction(action: "minimize" | "close") {
  if (!isTauri()) return;
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("window_action", { action });
}

export async function hideMainToTray(): Promise<void> {
  if (!isTauri()) return;
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("hide_main_to_tray");
}

export async function listPrograms(): Promise<ProgramEntry[]> {
  if (!isTauri()) return browserPrograms;
  return invokeWhenStateReady<ProgramEntry[]>("list_programs");
}

export async function quitApp(): Promise<void> {
  if (!isTauri()) return;
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("quit_app");
}

export async function launchProgram(path: string): Promise<void> {
  if (!isTauri()) return;
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("launch_program", { path });
}

export async function listAgentConnectors(force = false): Promise<AgentConnectorStatus[]> {
  if (!isTauri()) return browserAgentConnectors.map((connector) => ({ ...connector }));
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<AgentConnectorStatus[]>("list_agent_connectors", { force });
}

export async function getAgentDefaultWorkspace(): Promise<string> {
  if (!isTauri()) return "C:\\Users\\你\\Documents";
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<string>("get_agent_default_workspace");
}

export async function runAgentTask(
  connectorId: AgentConnectorId,
  task: string,
  workspace: string,
  attachmentPaths: string[] = [],
): Promise<AgentTaskResult> {
  if (!isTauri()) throw new Error("浏览器预览无法运行本机 Agent 任务，请在虫洞派桌面应用中使用。");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<AgentTaskResult>("run_agent_task", { connectorId, task, workspace, attachmentPaths });
}

export async function pickDialogueFiles(): Promise<string[]> {
  if (!isTauri()) return [];
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<string[]>("pick_dialogue_files");
}

export async function openAgentResultFile(path: string, workspace: string): Promise<void> {
  if (!isTauri()) return;
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("open_agent_result_file", { path, workspace });
}

export async function getAgentTaskStatus(): Promise<AgentTaskStatus | null> {
  if (!isTauri()) return null;
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<AgentTaskStatus | null>("get_agent_task_status");
}

export async function getAgentTaskResult(taskId?: string): Promise<AgentTaskResult | null> {
  if (!isTauri()) return null;
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<AgentTaskResult | null>("get_agent_task_result", { taskId });
}

export async function stopAgentTask(taskId?: string): Promise<boolean> {
  if (!isTauri()) return false;
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<boolean>("stop_agent_task", { taskId });
}

export async function subscribeToAgentTaskStatus(onStatus: (status: AgentTaskStatus) => void) {
  if (!isTauri()) return () => undefined;
  const { listen } = await import("@tauri-apps/api/event");
  return listen<AgentTaskStatus>("agent://task-status", (event) => onStatus(event.payload));
}

export async function subscribeToAgentTaskResult(onResult: (result: AgentTaskResult) => void) {
  if (!isTauri()) return () => undefined;
  const { listen } = await import("@tauri-apps/api/event");
  return listen<AgentTaskResult>("agent://task-result", (event) => onResult(event.payload));
}

export async function recognizeLocalSpeech(): Promise<string> {
  if (!isTauri()) throw new Error("浏览器预览无法访问本机麦克风模型，请先使用文字输入。");
  const { invoke } = await import("@tauri-apps/api/core");
  const result = await invoke<string | { text?: string; transcript?: string }>("recognize_speech_local");
  const transcript = typeof result === "string" ? result : result.text ?? result.transcript ?? "";
  if (!transcript.trim()) throw new Error("没有听清，再说一次吧。");
  return transcript.trim();
}

export async function listOrganizeExclusions(): Promise<OrganizeExclusion[]> {
  if (!isTauri()) return browserExclusions;
  return invokeWhenStateReady<OrganizeExclusion[]>("list_organize_exclusions");
}

export async function removeOrganizeExclusion(nameKey: string): Promise<void> {
  if (!isTauri()) {
    browserExclusions = browserExclusions.filter((item) => item.nameKey !== nameKey);
    return;
  }
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("remove_organize_exclusion", { nameKey });
}

export async function showPetContextMenu(): Promise<void> {
  if (!isTauri()) return;
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("show_pet_context_menu");
}

export type DialogueState = {
  userMessage: string;
  reply: string;
  listening: boolean;
  busy?: boolean;
  connectorId?: DialogueConnectorId;
  resultFiles?: AgentResultFile[];
};

export type PetRuntimeAnimationEndEvent = {
  actionInstanceId: string;
  atEpochMs: number;
};

export type PetRuntimeLoopBoundaryEvent = PetRuntimeAnimationEndEvent & {
  loopCount: number;
};

export async function showPetDialogue(): Promise<void> {
  if (!isTauri()) return;
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("show_pet_dialogue");
}

export async function hidePetDialogue(): Promise<void> {
  if (!isTauri()) return;
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("hide_pet_dialogue");
}

export async function sendDialogueCommand(command: DialogueCommand): Promise<void> {
  const payload = {
    text: command.text,
    connectorId: command.connectorId,
    attachmentPaths: command.attachmentPaths ?? [],
  };
  if (!isTauri()) {
    window.dispatchEvent(new CustomEvent("wormhole:dialogue-command", { detail: payload }));
    return;
  }
  const { emit } = await import("@tauri-apps/api/event");
  await emit("dialogue://command", payload);
}

export async function publishDialogueState(state: DialogueState): Promise<void> {
  if (!isTauri()) return;
  const { emit } = await import("@tauri-apps/api/event");
  await emit("dialogue://state", state);
}

export async function subscribeToDialogueCommand(onCommand: (command: DialogueCommand) => void) {
  if (!isTauri()) {
    const listener = (event: Event) => {
      const detail = (event as CustomEvent<Partial<DialogueCommand> & { text: string }>).detail;
      onCommand({
        text: detail.text,
        connectorId: detail.connectorId ?? "local",
        attachmentPaths: Array.isArray(detail.attachmentPaths) ? detail.attachmentPaths : [],
      });
    };
    window.addEventListener("wormhole:dialogue-command", listener);
    return () => window.removeEventListener("wormhole:dialogue-command", listener);
  }
  const { listen } = await import("@tauri-apps/api/event");
  return listen<Partial<DialogueCommand> & { text: string }>("dialogue://command", (event) => onCommand({
    text: event.payload.text,
    connectorId: event.payload.connectorId ?? "local",
    attachmentPaths: Array.isArray(event.payload.attachmentPaths) ? event.payload.attachmentPaths : [],
  }));
}

export async function subscribeToDialogueState(onState: (state: DialogueState) => void) {
  if (!isTauri()) return () => undefined;
  const { listen } = await import("@tauri-apps/api/event");
  return listen<DialogueState>("dialogue://state", (event) => onState(event.payload));
}

export async function publishPetRuntimeSignal(signal: PetSignalEvent): Promise<void> {
  if (!isTauri()) {
    window.dispatchEvent(new CustomEvent("wormhole:pet-runtime-signal", { detail: signal }));
    return;
  }
  const { emit } = await import("@tauri-apps/api/event");
  await emit("pet://runtime/signal", signal);
}

export async function subscribeToPetRuntimeSignal(onSignal: (signal: PetSignalEvent) => void) {
  if (!isTauri()) {
    const listener = (event: Event) => onSignal((event as CustomEvent<PetSignalEvent>).detail);
    window.addEventListener("wormhole:pet-runtime-signal", listener);
    return () => window.removeEventListener("wormhole:pet-runtime-signal", listener);
  }
  const { listen } = await import("@tauri-apps/api/event");
  return listen<PetSignalEvent>("pet://runtime/signal", (event) => onSignal(event.payload));
}

export async function publishPetRuntimeState(state: PetRuntimeSnapshot): Promise<void> {
  if (!isTauri()) {
    window.dispatchEvent(new CustomEvent("wormhole:pet-runtime-state", { detail: state }));
    return;
  }
  const { emit } = await import("@tauri-apps/api/event");
  await emit("pet://runtime/state", state);
}

export async function subscribeToPetRuntimeState(onState: (state: PetRuntimeSnapshot) => void) {
  if (!isTauri()) {
    const listener = (event: Event) => onState((event as CustomEvent<PetRuntimeSnapshot>).detail);
    window.addEventListener("wormhole:pet-runtime-state", listener);
    return () => window.removeEventListener("wormhole:pet-runtime-state", listener);
  }
  const { listen } = await import("@tauri-apps/api/event");
  return listen<PetRuntimeSnapshot>("pet://runtime/state", (event) => onState(event.payload));
}

export async function publishPetRuntimeMarker(marker: PetMarkerEvent): Promise<void> {
  if (!isTauri()) {
    window.dispatchEvent(new CustomEvent("wormhole:pet-runtime-marker", { detail: marker }));
    return;
  }
  const { emit } = await import("@tauri-apps/api/event");
  await emit("pet://runtime/marker", marker);
}

export async function subscribeToPetRuntimeMarker(onMarker: (marker: PetMarkerEvent) => void) {
  if (!isTauri()) {
    const listener = (event: Event) => onMarker((event as CustomEvent<PetMarkerEvent>).detail);
    window.addEventListener("wormhole:pet-runtime-marker", listener);
    return () => window.removeEventListener("wormhole:pet-runtime-marker", listener);
  }
  const { listen } = await import("@tauri-apps/api/event");
  return listen<PetMarkerEvent>("pet://runtime/marker", (event) => onMarker(event.payload));
}

export async function publishPetRuntimeAnimationEnd(event: PetRuntimeAnimationEndEvent): Promise<void> {
  if (!isTauri()) {
    window.dispatchEvent(new CustomEvent("wormhole:pet-runtime-animation-end", { detail: event }));
    return;
  }
  const { emit } = await import("@tauri-apps/api/event");
  await emit("pet://runtime/animation-end", event);
}

export async function subscribeToPetRuntimeAnimationEnd(onEnd: (event: PetRuntimeAnimationEndEvent) => void) {
  if (!isTauri()) {
    const listener = (event: Event) => onEnd((event as CustomEvent<PetRuntimeAnimationEndEvent>).detail);
    window.addEventListener("wormhole:pet-runtime-animation-end", listener);
    return () => window.removeEventListener("wormhole:pet-runtime-animation-end", listener);
  }
  const { listen } = await import("@tauri-apps/api/event");
  return listen<PetRuntimeAnimationEndEvent>("pet://runtime/animation-end", (event) => onEnd(event.payload));
}

export async function publishPetRuntimeLoopBoundary(event: PetRuntimeLoopBoundaryEvent): Promise<void> {
  if (!isTauri()) {
    window.dispatchEvent(new CustomEvent("wormhole:pet-runtime-loop-boundary", { detail: event }));
    return;
  }
  const { emit } = await import("@tauri-apps/api/event");
  await emit("pet://runtime/loop-boundary", event);
}

export async function subscribeToPetRuntimeLoopBoundary(onBoundary: (event: PetRuntimeLoopBoundaryEvent) => void) {
  if (!isTauri()) {
    const listener = (event: Event) => onBoundary((event as CustomEvent<PetRuntimeLoopBoundaryEvent>).detail);
    window.addEventListener("wormhole:pet-runtime-loop-boundary", listener);
    return () => window.removeEventListener("wormhole:pet-runtime-loop-boundary", listener);
  }
  const { listen } = await import("@tauri-apps/api/event");
  return listen<PetRuntimeLoopBoundaryEvent>("pet://runtime/loop-boundary", (event) => onBoundary(event.payload));
}

export async function requestPetRuntimeState(): Promise<void> {
  if (!isTauri()) {
    window.dispatchEvent(new CustomEvent("wormhole:pet-runtime-request-state"));
    return;
  }
  const { emit } = await import("@tauri-apps/api/event");
  await emit("pet://runtime/request-state");
}

export async function subscribeToPetRuntimeStateRequests(onRequest: () => void) {
  if (!isTauri()) {
    window.addEventListener("wormhole:pet-runtime-request-state", onRequest);
    return () => window.removeEventListener("wormhole:pet-runtime-request-state", onRequest);
  }
  const { listen } = await import("@tauri-apps/api/event");
  return listen("pet://runtime/request-state", onRequest);
}

async function subscribeToUiEvent(eventName: string, onEvent: () => void) {
  if (!isTauri()) return () => undefined;
  const { listen } = await import("@tauri-apps/api/event");
  return listen(eventName, onEvent);
}

export function subscribeToOpenDialogue(onOpen: () => void) {
  return subscribeToUiEvent("ui://open-dialogue", onOpen);
}

export function subscribeToOpenPetSettings(onOpen: () => void) {
  return subscribeToUiEvent("ui://open-pet-settings", onOpen);
}

export function subscribeToMainShown(onShown: () => void) {
  return subscribeToUiEvent("main://shown", onShown);
}

export function subscribeToExitRestRequest(onExitRest: () => void) {
  return subscribeToUiEvent("ui://exit-rest", onExitRest);
}

export type WindowLogicalPosition = { x: number; y: number };

export async function getWindowLogicalPosition(): Promise<WindowLogicalPosition | null> {
  if (!isTauri()) return null;
  const currentWindow = getCurrentWindow();
  const [position, scaleFactor] = await Promise.all([
    currentWindow.outerPosition(),
    currentWindow.scaleFactor(),
  ]);
  return { x: position.x / scaleFactor, y: position.y / scaleFactor };
}

export async function setWindowLogicalPosition(position: WindowLogicalPosition): Promise<void> {
  if (!isTauri()) return;
  await getCurrentWindow().setPosition(new LogicalPosition(position.x, position.y));
}

export async function getCursorPosition(): Promise<{ x: number; y: number }> {
  if (!isTauri()) return { x: 0, y: 0 };
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<{ x: number; y: number }>("cursor_position");
}

export async function enterRestMode(): Promise<void> {
  if (!isTauri()) return;
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("window_action", { action: "enter_rest" });
}

export async function exitRestMode(): Promise<void> {
  if (!isTauri()) return;
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("window_action", { action: "exit_rest" });
}

export type PetLayer = "normal" | "bottom" | "top";

export async function setPetVisibility(visible: boolean): Promise<void> {
  if (!isTauri()) return;
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("pet_visibility", { visible });
}

export async function subscribeToPetVisibilityChanged(onVisible: (visible: boolean) => void) {
  if (!isTauri()) return () => undefined;
  const { listen } = await import("@tauri-apps/api/event");
  return listen<boolean | { visible: boolean }>("pet://visibility-changed", (event) => {
    onVisible(typeof event.payload === "boolean" ? event.payload : event.payload.visible);
  });
}

export async function setPetLayer(layer: PetLayer): Promise<void> {
  if (!isTauri()) return;
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("pet_layer", { layer });
}

export async function subscribeToPetLayerChanged(onLayer: (layer: PetLayer) => void) {
  if (!isTauri()) return () => undefined;
  const { listen } = await import("@tauri-apps/api/event");
  return listen<PetLayer | { layer: PetLayer }>("pet://layer-changed", (event) => {
    const layer = typeof event.payload === "string" ? event.payload : event.payload.layer;
    if (layer === "normal" || layer === "bottom" || layer === "top") onLayer(layer);
  });
}

export type PetFeedResult = { names: string[]; count: number; warning?: string | null; failedCount?: number };
export type PetDropEvent =
  | { type: "enter"; paths: string[] }
  | { type: "over" }
  | { type: "drop"; paths: string[] }
  | { type: "leave" };

export async function subscribeToPetFileDrop(onEvent: (event: PetDropEvent) => void) {
  if (!isTauri()) return () => undefined;
  const { getCurrentWebview } = await import("@tauri-apps/api/webview");
  return getCurrentWebview().onDragDropEvent((event) => {
    const payload = event.payload;
    if (payload.type === "enter" || payload.type === "drop") onEvent({ type: payload.type, paths: payload.paths });
    else onEvent({ type: payload.type });
  });
}

export const subscribeToWindowFileDrop = subscribeToPetFileDrop;

export async function feedFilesToPet(paths: string[]): Promise<PetFeedResult> {
  if (!isTauri()) return { names: paths.map((path) => path.split(/[\\/]/).pop() ?? path), count: paths.length };
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<PetFeedResult>("feed_files", { paths });
}

export async function undoLastPetFeed(): Promise<PetFeedResult> {
  if (!isTauri()) return { names: [], count: 0 };
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<PetFeedResult>("undo_last_feed");
}

export async function subscribeToPetFeed(onFeed: (result: PetFeedResult) => void) {
  if (!isTauri()) return () => undefined;
  const { listen } = await import("@tauri-apps/api/event");
  return listen<PetFeedResult>("pet://fed", (event) => onFeed(event.payload));
}

export async function subscribeToPetRestore(onRestore: (result: PetFeedResult) => void) {
  if (!isTauri()) return () => undefined;
  const { listen } = await import("@tauri-apps/api/event");
  return listen<PetFeedResult>("pet://restored", (event) => onRestore(event.payload));
}
