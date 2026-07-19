import {
  AppWindow,
  Archive,
  ArrowDownAZ,
  ArrowUp,
  Bell,
  Bot,
  Check,
  CheckSquare2,
  Clock3,
  Code2,
  Coffee,
  ChevronLeft,
  ChevronRight,
  ChevronUp,
  Circle,
  ExternalLink,
  Eye,
  EyeOff,
  File,
  FileImage,
  FileText,
  Folder,
  FolderOutput,
  Home,
  Lightbulb,
  Layers3,
  Languages,
  Link2,
  LogOut,
  MessageCircle,
  Minimize2,
  MousePointer2,
  Music2,
  MoreHorizontal,
  PawPrint,
  Plus,
  PlugZap,
  RefreshCw,
  Search,
  Sparkles,
  Star,
  Sprout,
  Tag,
  Trash2,
  Undo2,
  Unplug,
  Volume2,
  Video,
  X,
} from "lucide-react";
import {
  AnimationEvent as ReactAnimationEvent,
  CSSProperties,
  DragEvent as ReactDragEvent,
  FormEvent,
  MouseEvent as ReactMouseEvent,
  PointerEvent as ReactPointerEvent,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { MinimalDialogue } from "./components/MinimalDialogue";
import { AgentManagerPanel } from "./components/AgentManagerPanel";
import { HomeSectionFrame, HomeSectionRail } from "./components/HomeSectionRail";
import type { HomeSectionId, HomeSectionMotion } from "./components/HomeSectionRail";
import { WormholeTransition } from "./components/WormholeTransition";
import type { WindowMotion } from "./components/WormholeTransition";
import { mockFiles, seedIdeas, seedNotices, seedTodos } from "./data/mockData";
import { usePersistentState } from "./hooks/usePersistentState";
import { useDocumentLanguage, type AppLocale } from "./i18n";
import {
  appendDialogueAssistantMessage,
  appendDialogueUserMessage,
  buildProviderTaskWithHistory,
  createDialogueSessions,
  dialogueSessionStorageKey,
  mergeDialogueSessions,
  normalizeDialogueSessions,
  setDialogueSessionPreview,
  updateDialogueSessionFields,
  type DialogueSession,
  type DialogueSessions,
} from "./dialogueSessions";
import { getBlenderPetPreviewSrc, PetSprite } from "./pet/PetSprite";
import type { PetAnimationLoopBoundaryEvent } from "./pet/PetSprite";
import { getPetAssetId, getPetFrameSrc, loadPetManifests } from "./pet/petManifest";
import type { PetSpriteAction } from "./pet/petManifest";
import { PetBehaviorRuntime } from "./pet/petBehaviorRuntime";
import type {
  PetActionsManifest as BehaviorActionsManifest,
  PetBehaviorTransition,
  PetMarkerEvent,
  PetRuntimeSnapshot,
  PetSignalEvent,
  PetStateMachineManifest as BehaviorStateMachineManifest,
} from "./pet/petBehaviorTypes";
import {
  isTauri,
  enterRestMode,
  exitRestMode,
  clearSocialSession,
  disconnectSocialSession,
  feedFilesToPet,
  getAgentDefaultWorkspace,
  getAgentTaskResult,
  getAgentTaskStatus,
  getCursorPosition,
  getDesktopIconState,
  getOrganizeState,
  getWindowLogicalPosition,
  hideMainToTray,
  hidePetDialogue,
  launchProgram,
  listAgentConnectors,
  listDesktopFiles,
  listOrganizeExclusions,
  listPrograms,
  listSocialAccounts,
  organizeDesktop,
  openAgentResultFile,
  openDesktopFile,
  openExternalUrl,
  openSocialPage,
  openSocialSession,
  quitApp,
  recognizeLocalSpeech,
  removeOrganizeExclusion,
  requestPublicDesktopConfirmation,
  reviewDesktopOrganize,
  restoreLibraryItemsToDesktop,
  runAgentTask,
  sanitizeExternalUrl,
  saveSocialSnapshot,
  scanDesktopFiles,
  acknowledgeDialogueCommand,
  sendDialogueCommand,
  setPetLayer,
  setPetVisibility,
  setDesktopIconsHidden,
  setAppLocale,
  syncSocialSnapshot,
  setWindowLogicalPosition,
  showMainFromTray,
  showPetContextMenu,
  showPetDialogue,
  startDesktopWatcher,
  stopAgentTask,
  subscribeToAgentTaskResult,
  subscribeToAgentTaskStatus,
  subscribeToFileChanges,
  subscribeToAppLocale,
  subscribeToDialogueCommand,
  subscribeToDialogueState,
  subscribeToExitRestRequest,
  subscribeToOpenDialogue,
  subscribeToOpenPetSettings,
  subscribeToMainShown,
  subscribeToPetFeed,
  subscribeToPetFileDrop,
  subscribeToPetLayerChanged,
  subscribeToPetRestore,
  subscribeToPetVisibilityChanged,
  updateDesktopFileCategory,
  undoLastPetFeed,
  undoDesktopOrganize,
  publishDialogueState,
  publishPetRuntimeAnimationEnd,
  publishPetRuntimeLoopBoundary,
  publishPetRuntimeMarker,
  publishPetRuntimeSignal,
  publishPetRuntimeState,
  requestPetRuntimeState,
  subscribeToPetRuntimeAnimationEnd,
  subscribeToPetRuntimeLoopBoundary,
  subscribeToPetRuntimeMarker,
  subscribeToPetRuntimeSignal,
  subscribeToPetRuntimeState,
  subscribeToPetRuntimeStateRequests,
} from "./lib/desktop";
import type { AgentConnectorId, AgentConnectorStatus, AgentResultFile, AgentTaskResult, AgentTaskStatus, DesktopIconState, DesktopOrganizeResult, DesktopOrganizeReviewResult, DialogueCommand, DialogueConnectorId, DialogueState, DialogueSubmission, PetFeedResult, PetLayer, SocialAccountSnapshot, SocialPlatform } from "./lib/desktop";
import { relativeTime } from "./lib/format";
import {
  evolutionBadges,
  evolutionPathOptions,
  evolutionStageOptions,
  calculatePetEvolution,
  defaultPetEvolutionMetrics,
  mergePetEvolutionMetrics,
  petClassName,
  petMotionOptions,
  petSpeciesOptions,
  petStorageKeys,
  petThemeOptions,
} from "./petProfile";
import type { EvolutionPath, EvolutionStage, PetEvolutionMetrics, PetMotionMode, PetSpecies, PetTheme } from "./petProfile";
import type { DesktopFile, FavoriteProgram, Idea, Notice, OrganizedCategory, OrganizeExclusion, ProgramEntry, Todo } from "./types";

type WidgetTab = "home" | "files" | "todos" | "ideas";
type FileFilter = OrganizedCategory | "all" | "unorganized";
type Platform = SocialPlatform;
type LegacyPetAction = "idle" | "blink" | "wave" | "stretch" | "peek" | "hungry" | "eat" | "happy" | "error";
type PetAction = LegacyPetAction | PetSpriteAction;
type CursorPoint = { x: number; y: number };
type OrganizePhase = "idle" | "organizing" | "done" | "undoing" | "undone" | "error";
type HomeLibrary = "desktop" | "programs";
type LibraryArrangeMode = "type" | "name" | "created" | "initial";
type CustomSocialShortcut = { id: string; name: string; url: string };
type RestStage = "preparing" | "peek" | "approach" | "settle" | "resting" | "exiting" | "recovery";

const libraryNameCollator = new Intl.Collator("zh-CN", { numeric: true, sensitivity: "base" });

function isAgentConnectorReady(connector: AgentConnectorStatus) {
  return connector.detected && connector.available;
}

function agentConnectorStatusText(connector: AgentConnectorStatus) {
  const version = connector.version?.trim();
  const suffix = version ? ` · ${version}` : "";
  const state = connector.configurationState as string;
  if (state === "ready") return `命令可用 · 配置痕迹已找到${suffix}`;
  if (state === "permission_blocked") return `命令已找到 · 权限受限${suffix}`;
  if (state === "unavailable" || state === "probe_failed") return `命令已找到 · 当前不可用${suffix}`;
  if (state === "not_configured") return `命令可用 · 配置待运行确认${suffix}`;
  return "未安装";
}

function isAgentTaskActive(status: AgentTaskStatus | null) {
  return status?.state === "starting" || status?.state === "running" || status?.state === "cancelling";
}

function useAgentTaskTiming(busy: boolean, status: AgentTaskStatus | null) {
  const fallbackStartedAt = useRef<number | null>(null);
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (!busy) {
      fallbackStartedAt.current = null;
      return;
    }
    if (fallbackStartedAt.current === null) fallbackStartedAt.current = Date.now();
    setNow(Date.now());
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [busy]);

  if (!busy) return { elapsedMs: 0, heartbeatKnown: false, heartbeatFresh: false };
  const activeStatus = isAgentTaskActive(status) ? status : null;
  const startedAt = activeStatus?.startedAt ?? fallbackStartedAt.current ?? now;
  const heartbeatKnown = activeStatus?.state === "running" || activeStatus?.state === "cancelling";
  return {
    elapsedMs: Math.max(activeStatus?.elapsedMs ?? 0, now - startedAt),
    heartbeatKnown,
    heartbeatFresh: activeStatus !== null && heartbeatKnown && now - activeStatus.updatedAt <= 6_500,
  };
}

function libraryNameBucket(name: string) {
  const first = name.trim().charAt(0);
  if (!first) return "其他";
  if (/\d/.test(first)) return "0–9";
  if (/[a-z]/i.test(first)) return first.toLocaleUpperCase();
  if (/\p{Script=Han}/u.test(first)) return first;
  return "其他";
}

function gentleLookVector(cursor: CursorPoint, centerX: number, centerY: number) {
  const deltaX = cursor.x - centerX;
  const deltaY = cursor.y - centerY;
  const distance = Math.hypot(deltaX, deltaY);
  const deadZone = 38;
  if (distance <= deadZone) return { x: 0, y: 0 };
  const strength = Math.min(1, (distance - deadZone) / 220);
  return {
    x: (deltaX / distance) * strength,
    y: (deltaY / distance) * strength,
  };
}

function toPetSpriteAction(action: PetAction): PetSpriteAction {
  switch (action) {
    case "idle": return "idle_breathe";
    case "blink": return "idle_breathe";
    case "wave": return "affection";
    case "stretch": return "walk";
    case "peek": return "hide_peek";
    case "hungry": return "think";
    case "eat": return "file_eat";
    case "happy": return "task_success";
    case "error": return "think";
    default: return action;
  }
}

type WindowDragState = {
  pointerId: number;
  startScreenX: number;
  startScreenY: number;
  currentScreenX: number;
  currentScreenY: number;
  startWindowX: number;
  startWindowY: number;
  dragStarted: boolean;
};

function useDraggableWindow(ignoreInteractiveTargets = false, onTap?: () => void, onPointerBegin?: () => void, onPointerEnd?: (dragged: boolean) => void) {
  const lastPosition = useRef<{ x: number; y: number } | null>(null);
  const dragState = useRef<WindowDragState | null>(null);
  const frame = useRef(0);

  const refreshPosition = useCallback(async () => {
    try {
      lastPosition.current = await getWindowLogicalPosition();
    } catch (error) {
      console.error(error);
    }
  }, []);

  useEffect(() => {
    void refreshPosition();
    return () => {
      if (frame.current) window.cancelAnimationFrame(frame.current);
    };
  }, [refreshPosition]);

  const moveWindow = useCallback((next: WindowDragState) => {
    const position = {
      x: next.startWindowX + next.currentScreenX - next.startScreenX,
      y: next.startWindowY + next.currentScreenY - next.startScreenY,
    };
    lastPosition.current = position;
    void setWindowLogicalPosition(position).catch(console.error);
  }, []);

  const onPointerDown = useCallback((event: ReactPointerEvent<HTMLElement>) => {
    if (event.button !== 0 || !isTauri()) return;
    if (ignoreInteractiveTargets && (event.target as HTMLElement).closest("button, input, select, a")) return;
    const position = lastPosition.current;
    if (!position) {
      void refreshPosition();
      return;
    }
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    dragState.current = {
      pointerId: event.pointerId,
      startScreenX: event.screenX,
      startScreenY: event.screenY,
      currentScreenX: event.screenX,
      currentScreenY: event.screenY,
      startWindowX: position.x,
      startWindowY: position.y,
      dragStarted: false,
    };
  }, [ignoreInteractiveTargets, refreshPosition]);

  const onPointerMove = useCallback((event: ReactPointerEvent<HTMLElement>) => {
    const active = dragState.current;
    if (!active || active.pointerId !== event.pointerId) return;
    active.currentScreenX = event.screenX;
    active.currentScreenY = event.screenY;
    if (!active.dragStarted && Math.hypot(active.currentScreenX - active.startScreenX, active.currentScreenY - active.startScreenY) >= 5) {
      active.dragStarted = true;
      onPointerBegin?.();
    }
    if (frame.current) return;
    frame.current = window.requestAnimationFrame(() => {
      frame.current = 0;
      if (dragState.current) moveWindow(dragState.current);
    });
  }, [moveWindow, onPointerBegin]);

  const finishDrag = useCallback((event: ReactPointerEvent<HTMLElement>) => {
    const active = dragState.current;
    if (!active || active.pointerId !== event.pointerId) return;
    active.currentScreenX = event.screenX;
    active.currentScreenY = event.screenY;
    const distance = Math.hypot(active.currentScreenX - active.startScreenX, active.currentScreenY - active.startScreenY);
    if (frame.current) {
      window.cancelAnimationFrame(frame.current);
      frame.current = 0;
    }
    moveWindow(active);
    dragState.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    const dragged = distance >= 5;
    if (!dragged) onTap?.();
    onPointerEnd?.(dragged);
  }, [moveWindow, onPointerEnd, onTap]);

  return {
    onPointerDown,
    onPointerMove,
    onPointerUp: finishDrag,
    onPointerCancel: finishDrag,
  };
}

const makeId = (prefix: string) => `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;

function createPetSignal(
  signal: string,
  sourceWindow: PetSignalEvent["sourceWindow"],
  petId: string,
  options: {
    payload?: Record<string, unknown>;
    transactionId?: string;
    source?: PetSignalEvent["source"];
    essentialInQuietMode?: boolean;
  } = {},
): PetSignalEvent {
  return {
    eventId: typeof crypto !== "undefined" && "randomUUID" in crypto ? crypto.randomUUID() : makeId("pet-event"),
    signal,
    sourceWindow,
    atEpochMs: Date.now(),
    petId,
    ...options,
  };
}

function localDateKey(date = new Date()) {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

const organizedCategories: OrganizedCategory[] = ["文件夹", "文档", "图片", "视频", "音频", "压缩包", "代码", "快捷方式", "程序", "其他"];
const userTagOptions = ["待整理", "工作项目", "小红书素材", "个人资料"];
const codeExtensions = new Set(["ts", "tsx", "js", "jsx", "py", "rs", "go", "java", "kt", "swift", "c", "cpp", "h", "hpp", "cs", "rb", "php", "html", "css", "scss", "json", "yaml", "yml", "toml", "sh", "sql"]);
const shortcutExtensions = new Set(["lnk", "url", "webloc", "desktop", "alias"]);
const programExtensions = new Set(["exe", "msi", "msix", "msixbundle", "bat", "cmd", "ps1", "vbs", "command", "app", "appinstaller", "dmg", "pkg"]);
const directlyLaunchableExtensions = new Set(["lnk", "url", "exe", "app"]);

function normalizePathKey(path: string) {
  return path.replace(/\//g, "\\").toLocaleLowerCase();
}

function isDirectlyLaunchableFile(file: DesktopFile) {
  return directlyLaunchableExtensions.has(file.extension.toLocaleLowerCase());
}

function isWormholeShortcut(file: DesktopFile) {
  return file.name === "虫洞派.lnk" || file.name.toLocaleLowerCase() === "wormhole pie.lnk";
}

function inferTodoAction(title: string): Partial<Todo> {
  const normalized = title.trim();
  if (/(小红书|红薯|xhs)/i.test(normalized) && /(发布|发帖|笔记|创作|上传)/.test(normalized)) {
    return { actionType: "social_publish", actionTarget: "xiaohongshu" };
  }
  if (/抖音/.test(normalized) && /(发布|发视频|创作|上传)/.test(normalized)) {
    return { actionType: "social_publish", actionTarget: "douyin" };
  }
  if (/(?:^|[^a-z])x(?:$|[^a-z])|twitter|推特/i.test(normalized) && /(发布|发帖|创作)/.test(normalized)) {
    return { actionType: "social_publish", actionTarget: "x" };
  }
  if (/(整理|查看|检查).*(桌面|文件|资料)/.test(normalized)) {
    return { actionType: "open_category", actionTarget: "unorganized" };
  }
  return {};
}

function withInferredTodoAction(todo: Todo): Todo {
  return todo.actionType ? todo : { ...todo, ...inferTodoAction(todo.title) };
}

function inferOrganizedCategory(file: DesktopFile): OrganizedCategory {
  const supplied = file.organizedCategory === "应用" ? "程序" : file.organizedCategory;
  if (organizedCategories.includes(supplied as OrganizedCategory)) return supplied as OrganizedCategory;
  const extension = file.extension.toLocaleLowerCase();
  if (codeExtensions.has(extension)) return "代码";
  if (shortcutExtensions.has(extension)) return "快捷方式";
  if (programExtensions.has(extension)) return "程序";
  if (file.kind === "folder") return "文件夹";
  if (file.kind === "document") return "文档";
  if (file.kind === "image") return "图片";
  if (file.kind === "video") return "视频";
  if (file.kind === "audio") return "音频";
  if (file.kind === "archive") return "压缩包";
  if (file.kind === "application") return shortcutExtensions.has(extension) ? "快捷方式" : "程序";
  return "其他";
}

function isOrganizedFile(file: DesktopFile) {
  return Boolean(file.organizedCategory?.trim()) || /[\\/](?:虫洞派整理|虫洞派资料库)[\\/]/i.test(file.path);
}

function categoryClassName(category: OrganizedCategory) {
  const names: Record<OrganizedCategory, string> = {
    待整理: "pending",
    文件夹: "folder",
    文档: "document",
    图片: "image",
    视频: "video",
    音频: "audio",
    压缩包: "archive",
    代码: "code",
    快捷方式: "shortcut",
    程序: "program",
    其他: "other",
  };
  return names[category];
}

const platformNames: Record<Platform, string> = {
  xiaohongshu: "小红书",
  x: "X",
  douyin: "抖音",
};

const organizedCategoryIconMap = {
  待整理: File,
  文件夹: Folder,
  文档: FileText,
  图片: FileImage,
  视频: Video,
  音频: Music2,
  压缩包: Archive,
  代码: Code2,
  快捷方式: Link2,
  程序: AppWindow,
  其他: File,
} satisfies Record<OrganizedCategory, typeof File>;

function compactDateLabel() {
  const date = new Date();
  const monthDay = new Intl.DateTimeFormat("zh-CN", { month: "long", day: "numeric" }).format(date);
  const weekday = new Intl.DateTimeFormat("zh-CN", { weekday: "long" }).format(date);
  return `${monthDay} · ${weekday}`;
}

function levenshteinSimilarity(left: string, right: string) {
  if (left === right) return 1;
  if (!left.length || !right.length) return 0;
  const previous = Array.from({ length: right.length + 1 }, (_, index) => index);
  const current = new Array<number>(right.length + 1);
  for (let i = 1; i <= left.length; i += 1) {
    current[0] = i;
    for (let j = 1; j <= right.length; j += 1) {
      current[j] = Math.min(
        current[j - 1] + 1,
        previous[j] + 1,
        previous[j - 1] + (left[i - 1] === right[j - 1] ? 0 : 1),
      );
    }
    for (let j = 0; j <= right.length; j += 1) previous[j] = current[j];
  }
  return 1 - previous[right.length] / Math.max(left.length, right.length);
}

function rankFiles(files: DesktopFile[], keyword: string) {
  const normalizedKeyword = keyword.toLowerCase().replace(/\s+/g, "");
  if (!normalizedKeyword) return [];
  return files
    .map((file) => {
      const normalizedName = file.name.toLowerCase().replace(/\s+/g, "");
      const baseName = normalizedName.replace(/\.[^.]+$/, "");
      let score = 0;
      if (normalizedName === normalizedKeyword || baseName === normalizedKeyword) score = 100;
      else if (normalizedName.includes(normalizedKeyword) || normalizedKeyword.includes(baseName)) score = 82;
      else score = Math.round(levenshteinSimilarity(baseName, normalizedKeyword) * 68);
      return { file, score };
    })
    .filter((candidate) => candidate.score >= 40)
    .sort((left, right) => right.score - left.score);
}

function rankPrograms(programs: ProgramEntry[], keyword: string) {
  const normalizedKeyword = keyword.toLowerCase().replace(/\s+/g, "");
  if (!normalizedKeyword) return [];
  return programs
    .map((program) => {
      const normalizedName = program.name.toLowerCase().replace(/\s+/g, "");
      let score = 0;
      if (normalizedName === normalizedKeyword) score = 100;
      else if (normalizedName.includes(normalizedKeyword) || normalizedKeyword.includes(normalizedName)) score = 86;
      else score = Math.round(levenshteinSimilarity(normalizedName, normalizedKeyword) * 70);
      return { program, score };
    })
    .filter((candidate) => candidate.score >= 40)
    .sort((left, right) => right.score - left.score);
}

function FileTypeIcon({ file, size = 16 }: { file: DesktopFile; size?: number }) {
  const category = inferOrganizedCategory(file);
  const Icon = organizedCategoryIconMap[category];
  return (
    <span className={`widget-file-icon physical-${categoryClassName(category)}`}>
      <Icon size={size} strokeWidth={1.9} />
    </span>
  );
}

type HeaderProps = {
  unread: number;
  menuOpen: boolean;
  locale: AppLocale;
  onLocale: (locale: AppLocale) => void;
  onNotifications: () => void;
  onMenu: () => void;
  onSettings: () => void;
  onPetSettings: () => void;
  onExclusions: () => void;
  onMinimize: () => void;
  onQuit: () => void;
};

function WidgetHeader({ unread, menuOpen, locale, onLocale, onNotifications, onMenu, onSettings, onPetSettings, onExclusions, onMinimize, onQuit }: HeaderProps) {
  const dragHandlers = useDraggableWindow(true);
  return (
    <header
      className="widget-header"
      data-tauri-drag-region
      {...dragHandlers}
    >
      <div className="widget-brand" data-tauri-drag-region>
        <strong>虫洞派</strong>
        <span>{compactDateLabel()}</span>
      </div>
      <div className="widget-header-actions">
        <span className="offline-state"><i />本地</span>
        <button
          className="header-language"
          type="button"
          onClick={() => onLocale(locale === "zh-CN" ? "en-US" : "zh-CN")}
          aria-label={locale === "zh-CN" ? "切换为英文" : "Switch to Chinese"}
          title={locale === "zh-CN" ? "English" : "中文"}
        >
          <Languages size={13} /><span>{locale === "zh-CN" ? "EN" : "中"}</span>
        </button>
        <button className="header-icon collapse-button" onClick={onMinimize} aria-label="向上收起到系统托盘" title="收起到系统托盘">
          <ChevronUp size={18} />
        </button>
        <button className="header-icon" onClick={onNotifications} aria-label="查看通知">
          <Bell size={17} />
          {unread ? <em>{Math.min(9, unread)}</em> : null}
        </button>
        <button className={`header-icon ${menuOpen ? "is-active" : ""}`} onClick={onMenu} aria-label="组件菜单">
          <MoreHorizontal size={18} />
        </button>
      </div>
      {menuOpen ? (
        <div className="widget-menu">
          <button onClick={onPetSettings}><PawPrint size={15} />宠物设置</button>
          <button onClick={onSettings}><Clock3 size={15} />休息设置</button>
          <button onClick={onExclusions}><EyeOff size={15} />已忽略整理</button>
          <button onClick={onMinimize}><Minimize2 size={15} />暂时收起</button>
          <button onClick={onQuit}><LogOut size={15} />退出组件</button>
        </div>
      ) : null}
    </header>
  );
}

type PetProps = {
  name: string;
  species: PetSpecies;
  theme: PetTheme;
  evolution: EvolutionPath;
  evolutionStage: EvolutionStage;
  motionMode: PetMotionMode;
  message: string;
  pendingCount: number;
  isListening: boolean;
  dialogueEnabled: boolean;
  cursor: CursorPoint;
  action: PetAction;
  runtime: PetRuntimeSnapshot | null;
  onChat: () => void;
  onPetSignal: () => void;
};

function PetSummary({ name, species, theme, evolution, evolutionStage, motionMode, message, pendingCount, isListening, dialogueEnabled, cursor, action, runtime, onChat, onPetSignal }: PetProps) {
  const buttonRef = useRef<HTMLButtonElement>(null);
  const bounds = buttonRef.current?.getBoundingClientRect();
  const centerX = bounds ? bounds.left + bounds.width / 2 : 0;
  const centerY = bounds ? bounds.top + bounds.height / 2 : 0;
  const look = gentleLookVector(cursor, centerX, centerY);
  const effectiveAction = runtime?.actionId as PetSpriteAction | undefined ?? (isListening ? "listen" : toPetSpriteAction(action));

  const handleClick = () => {
    onPetSignal();
    onChat();
  };

  return (
    <section className="pet-summary">
      <button
        ref={buttonRef}
        className={`mini-pet ${petClassName(species, theme, evolution)} action-${action} ${isListening ? "is-listening" : ""}`}
        onClick={handleClick}
        aria-label={dialogueEnabled ? `打开与${name}的对话` : "打开宠物设置"}
      >
        <PetSprite
          species={species}
          action={effectiveAction}
          evolutionStage={evolutionStage}
          evolutionPath={evolution}
          motionMode={runtime?.behaviorMode ?? motionMode}
          className="pet-sprite--summary"
          lookX={look.x}
          lookY={look.y}
          gaze
          actionInstanceId={runtime?.actionInstanceId}
          startedAtEpochMs={runtime?.startedAtEpochMs}
          velocity={runtime?.velocity}
          reducedMotion={runtime?.reducedMotion}
          transactionId={runtime?.transactionId}
        />
      </button>
      <div className="pet-bubble">
        <strong>{message || `今天还有 ${pendingCount} 件事`}</strong>
        <span>{dialogueEnabled ? <>点一下，和<span data-no-i18n>{name}</span>说句话</> : "对话已关闭，点我设置"}</span>
      </div>
    </section>
  );
}

type TodayCardProps = {
  todos: Todo[];
  onToggle: (id: string) => void;
  onOpenAll: () => void;
  onAction: (todo: Todo) => void;
};

function TodayCard({ todos, onToggle, onOpenAll, onAction }: TodayCardProps) {
  const visible = todos.filter((todo) => todo.status !== "done").slice(0, 2);
  return (
    <section className="home-card today-card">
      <div className="home-card-heading">
        <h2>今天</h2>
        <button onClick={onOpenAll}>查看全部<ChevronRight size={14} /></button>
      </div>
      <div className="compact-todo-list">
        {visible.length ? visible.map((todo) => {
          const actionableTodo = withInferredTodoAction(todo);
          return (
            <article className="compact-todo" key={todo.id}>
              <button className="compact-check" onClick={() => onToggle(todo.id)} aria-label={`完成 ${todo.title}`}>
                <Circle size={17} />
              </button>
              <button className="compact-todo-title" data-no-i18n onClick={() => actionableTodo.actionType && onAction(actionableTodo)}>
                {todo.title}
              </button>
              <time>{todo.time}</time>
            </article>
          );
        }) : (
          <div className="compact-empty"><Check size={17} />今天的事情都完成了</div>
        )}
      </div>
    </section>
  );
}

type FilesCardProps = {
  files: DesktopFile[];
  pendingCount: number;
  libraryCount: number;
  programs: ProgramEntry[];
  library: HomeLibrary;
  scanning: boolean;
  programsLoading: boolean;
  organizing: boolean;
  canUndoOrganize: boolean;
  onLibrary: (library: HomeLibrary) => void;
  onRefresh: () => void;
  onRefreshPrograms: () => void;
  onOrganize: () => void;
  onUndoOrganize: () => void;
  onOpen: (file: DesktopFile) => void;
  onLaunchProgram: (program: ProgramEntry) => void;
  onRemoveProgram: (program: ProgramEntry) => void;
  onOpenAll: () => void;
};

function RecentFilesCard({
  files,
  pendingCount,
  libraryCount,
  programs,
  library,
  scanning,
  programsLoading,
  organizing,
  canUndoOrganize,
  onLibrary,
  onRefresh,
  onRefreshPrograms,
  onOrganize,
  onUndoOrganize,
  onOpen,
  onLaunchProgram,
  onRemoveProgram,
  onOpenAll,
}: FilesCardProps) {
  const organizeLabel = organizing ? "整理中" : pendingCount ? `整理新增 ${pendingCount}` : "检查新增";
  const isDesktop = library === "desktop";
  return (
    <section className="home-card recent-files-card">
      <div className="home-card-heading">
        <div className="library-switch" role="tablist" aria-label="桌面内容">
          <button role="tab" aria-selected={isDesktop} className={isDesktop ? "is-active" : ""} onClick={() => onLibrary("desktop")}>待整理 {pendingCount} · 资料库 {libraryCount}</button>
          <button role="tab" aria-selected={!isDesktop} className={!isDesktop ? "is-active" : ""} onClick={() => onLibrary("programs")}>常用程序 <span>{programs.length}</span></button>
        </div>
        <div className="file-card-tools">
          {isDesktop ? (
            <button
              className={`organize-mini ${organizing ? "is-working" : ""}`}
              disabled={organizing || scanning}
              onClick={onOrganize}
              aria-label="整理桌面上的新增项目"
            >
              {organizing ? <RefreshCw size={12} /> : <Sparkles size={12} />}
              {organizeLabel}
            </button>
          ) : null}
          {isDesktop && canUndoOrganize ? (
            <button className="undo-mini" disabled={organizing} onClick={onUndoOrganize} aria-label="撤销最近一次桌面整理" title="撤销最近整理"><Undo2 size={14} /></button>
          ) : null}
          <button className={`refresh-mini ${(isDesktop ? scanning : programsLoading) ? "is-spinning" : ""}`} onClick={isDesktop ? onRefresh : onRefreshPrograms} aria-label={isDesktop ? "刷新文件索引" : "刷新常用程序"}>
            <RefreshCw size={15} />
          </button>
        </div>
      </div>
      <div className="recent-file-list">
        {isDesktop ? (
          <>
            {files.slice(0, 3).map((file) => (
              <button className="recent-file" key={file.id} onClick={() => onOpen(file)}>
                <FileTypeIcon file={file} />
                <span className="recent-file-name" data-no-i18n>{file.name}</span>
                {file.isNew ? <i className="new-file-dot" /> : null}
                <time>{relativeTime(file.modifiedAt)}</time>
                <ExternalLink size={13} />
              </button>
            ))}
            {!files.length ? <button className="compact-empty compact-empty-action" onClick={onOpenAll}><Folder size={17} />还没有个人文件</button> : null}
          </>
        ) : (
          <>
            {programs.slice(0, 3).map((program) => (
              <article className="recent-file recent-program" key={program.path}>
                <button className="recent-program-open" onClick={() => onLaunchProgram(program)}>
                  <span className="widget-file-icon kind-application"><AppWindow size={16} strokeWidth={1.9} /></span>
                  <span className="recent-file-name" data-no-i18n>{program.name}</span>
                  <time>{program.source === "favorite" ? "自定义" : "系统"}</time>
                  <ExternalLink size={13} />
                </button>
                {program.source === "favorite" ? (
                  <button className="recent-program-remove" onClick={() => onRemoveProgram(program)} aria-label={`移除常用程序 ${program.name}`} title="移除"><X size={12} /></button>
                ) : null}
              </article>
            ))}
            {!programs.length ? (
              <div className="compact-empty"><Star size={16} />{programsLoading ? "正在查找常用程序…" : "整理复查时可加入常用"}</div>
            ) : null}
          </>
        )}
      </div>
    </section>
  );
}

function SocialRow({ onOpen, onManage, customCount }: { onOpen: (platform: Platform) => void; onManage: () => void; customCount: number }) {
  return (
    <section className="social-shortcuts" aria-label="社交媒体快捷入口">
      <button onClick={() => onOpen("xiaohongshu")}><i className="social-icon xhs-icon">红</i><span>小红书</span></button>
      <button onClick={() => onOpen("x")}><i className="social-icon x-icon">X</i><span>X</span></button>
      <button onClick={() => onOpen("douyin")}><i className="social-icon douyin-icon">抖</i><span>抖音</span></button>
      <button className="social-add-shortcut" onClick={onManage} aria-label="添加或管理社交媒体">
        <i className="social-icon social-add-icon"><Plus size={12} /></i><span>添加</span>
        {customCount ? <em>{customCount}</em> : null}
      </button>
    </section>
  );
}

function IdeaShortcut({ count, onClick }: { count: number; onClick: () => void }) {
  return (
    <button className="idea-shortcut" onClick={onClick}>
      <span><Lightbulb size={16} />意见整理 <em>{count}</em></span>
    </button>
  );
}

type CommandBarProps = {
  petName: string;
  onOpen: () => void;
};

function CommandBar({ petName, onOpen }: CommandBarProps) {
  return (
    <button className="command-bar" onClick={onOpen} aria-label={`与${petName}对话`}>
      <MessageCircle size={15} />
      <span>告诉{petName}要做什么…</span>
      <ArrowUp size={15} />
    </button>
  );
}

function WidgetTabs({ active, onChange }: { active: WidgetTab; onChange: (tab: WidgetTab) => void }) {
  const items = [
    { id: "home" as const, label: "首页", icon: Home },
    { id: "files" as const, label: "文件", icon: Folder },
    { id: "todos" as const, label: "待办", icon: CheckSquare2 },
    { id: "ideas" as const, label: "意见", icon: Lightbulb },
  ];
  return (
    <nav className="widget-tabs" aria-label="组件页面">
      {items.map(({ id, label, icon: Icon }) => (
        <button key={id} className={active === id ? "is-active" : ""} onClick={() => onChange(id)}>
          <Icon size={16} />
          <span>{label}</span>
        </button>
      ))}
    </nav>
  );
}

function ViewHeader({ title, detail, onBack }: { title: string; detail: string; onBack: () => void }) {
  return (
    <div className="view-heading">
      <button onClick={onBack} aria-label="返回首页"><ChevronLeft size={17} /></button>
      <div><h1>{title}</h1><span>{detail}</span></div>
    </div>
  );
}

type FilesViewProps = {
  files: DesktopFile[];
  programs: ProgramEntry[];
  programsLoading: boolean;
  arrangeMode: LibraryArrangeMode;
  query: string;
  filter: FileFilter;
  onQuery: (value: string) => void;
  onFilter: (value: FileFilter) => void;
  onArrangeMode: (value: LibraryArrangeMode) => void;
  onBack: () => void;
  onOpen: (file: DesktopFile) => void;
  onLaunchProgram: (program: ProgramEntry) => void;
  onCategory: (id: DesktopFile["id"], category: string) => void;
  onRestore: (files: DesktopFile[]) => void;
  restoring: boolean;
};

type LibraryQuickEntry =
  | { id: string; name: string; kind: "file"; file: DesktopFile }
  | { id: string; name: string; kind: "program"; program: ProgramEntry };

type LibraryQuickSidebarProps = {
  files: DesktopFile[];
  programs: ProgramEntry[];
  programsLoading: boolean;
  filter: FileFilter;
  onFilter: (value: FileFilter) => void;
  onOpen: (file: DesktopFile) => void;
  onLaunchProgram: (program: ProgramEntry) => void;
};

function LibraryQuickSidebar({ files, programs, programsLoading, filter, onFilter, onOpen, onLaunchProgram }: LibraryQuickSidebarProps) {
  const sections = useMemo(() => {
    const indexedPaths = new Set(files.map((file) => normalizePathKey(file.path)));
    const sortEntries = (entries: LibraryQuickEntry[]) => [...entries].sort((left, right) => libraryNameCollator.compare(left.name, right.name));
    const programEntries: LibraryQuickEntry[] = [
      ...files.filter((file) => inferOrganizedCategory(file) === "程序").map((file) => ({ id: `file-${file.id}`, name: file.name, kind: "file" as const, file })),
      ...programs.filter((program) => !indexedPaths.has(normalizePathKey(program.path))).map((program) => ({ id: `program-${normalizePathKey(program.path)}`, name: program.name, kind: "program" as const, program })),
    ];
    const folderEntries: LibraryQuickEntry[] = files
      .filter((file) => inferOrganizedCategory(file) === "文件夹")
      .map((file) => ({ id: `folder-${file.id}`, name: file.name, kind: "file" as const, file }));
    const shortcutEntries: LibraryQuickEntry[] = files
      .filter((file) => inferOrganizedCategory(file) === "快捷方式")
      .map((file) => ({ id: `shortcut-${file.id}`, name: file.name, kind: "file" as const, file }));

    return [
      { id: "programs", label: "程序", filter: "程序" as FileFilter, icon: AppWindow, entries: sortEntries(programEntries), loading: programsLoading },
      { id: "folders", label: "文件夹", filter: "文件夹" as FileFilter, icon: Folder, entries: sortEntries(folderEntries), loading: false },
      { id: "shortcuts", label: "快捷", filter: "快捷方式" as FileFilter, icon: Link2, entries: sortEntries(shortcutEntries), loading: false },
    ];
  }, [files, programs, programsLoading]);

  return (
    <aside className="library-quick-sidebar" data-testid="library-quick-sidebar" aria-label="程序、文件夹和快捷方式">
      <span className="library-sidebar-wormhole" aria-hidden="true"><i /><i /></span>
      {sections.map((section) => {
        const Icon = section.icon;
        return (
          <section className="library-quick-section" key={section.id}>
            <button className={filter === section.filter ? "library-quick-heading is-active" : "library-quick-heading"} onClick={() => onFilter(section.filter)} aria-pressed={filter === section.filter}>
              <Icon size={12} /><span>{section.label}</span><em>{section.entries.length}</em>
            </button>
            {section.entries.slice(0, 2).map((entry) => (
              <button
                className="library-quick-entry"
                key={entry.id}
                onClick={() => entry.kind === "file" ? onOpen(entry.file) : onLaunchProgram(entry.program)}
                title={entry.name}
                aria-label={`打开${entry.name}`}
              >
                <Icon size={12} /><span>{entry.name}</span>
              </button>
            ))}
            {!section.entries.length ? <span className="library-quick-empty">{section.loading ? "读取中" : "暂无"}</span> : null}
          </section>
        );
      })}
    </aside>
  );
}

function FilesView({ files, programs, programsLoading, arrangeMode, query, filter, onQuery, onFilter, onArrangeMode, onBack, onOpen, onLaunchProgram, onCategory, onRestore, restoring }: FilesViewProps) {
  const [contextMenu, setContextMenu] = useState<{ file: DesktopFile; x: number; y: number } | null>(null);
  const [confirmRestoreAll, setConfirmRestoreAll] = useState(false);
  const filtered = useMemo(() => files.filter((file) => {
    const physicalCategory = inferOrganizedCategory(file);
    const matchesText = !query || `${file.name} ${file.category} ${physicalCategory}`.toLowerCase().includes(query.toLowerCase());
    const matchesType = filter === "all" || (filter === "unorganized" ? !isOrganizedFile(file) : physicalCategory === filter);
    return matchesText && matchesType;
  }), [files, filter, query]);
  const visiblePrograms = useMemo(() => {
    if (filter !== "all" && filter !== "程序") return [];
    const normalizedQuery = query.trim().toLocaleLowerCase();
    const indexedPaths = new Set(files.map((file) => normalizePathKey(file.path)));
    return programs.filter((program) => {
      if (indexedPaths.has(normalizePathKey(program.path))) return false;
      return !normalizedQuery || `${program.name} ${program.source}`.toLocaleLowerCase().includes(normalizedQuery);
    });
  }, [files, filter, programs, query]);
  const itemCount = filtered.length + visiblePrograms.length;
  const organizedFiles = useMemo(() => files.filter(isOrganizedFile), [files]);

  useEffect(() => {
    if (!contextMenu) return;
    const close = () => setContextMenu(null);
    window.addEventListener("blur", close);
    return () => {
      window.removeEventListener("blur", close);
    };
  }, [contextMenu]);

  useEffect(() => {
    if (!confirmRestoreAll) return;
    const timer = window.setTimeout(() => setConfirmRestoreAll(false), 4_000);
    return () => window.clearTimeout(timer);
  }, [confirmRestoreAll]);

  const openFileMenu = (event: ReactMouseEvent, file: DesktopFile) => {
    event.preventDefault();
    event.stopPropagation();
    setContextMenu({ file, x: Math.min(event.clientX, window.innerWidth - 176), y: Math.min(event.clientY, window.innerHeight - 112) });
  };
  const groupedItems = useMemo(() => {
    const items = [
      ...filtered.map((file) => ({ kind: "file" as const, name: file.name, category: inferOrganizedCategory(file), createdAt: file.createdAt ?? 0, file })),
      ...visiblePrograms.map((program) => ({ kind: "program" as const, name: program.name, category: "程序" as OrganizedCategory, createdAt: 0, program })),
    ];
    const byName = [...items].sort((left, right) => libraryNameCollator.compare(left.name, right.name));
    if (arrangeMode === "name") return [{ label: "全部项目", items: byName }];
    if (arrangeMode === "created") {
      const timed = items
        .filter((item) => item.createdAt > 0)
        .sort((left, right) => right.createdAt - left.createdAt || libraryNameCollator.compare(left.name, right.name));
      const unknown = byName.filter((item) => item.createdAt <= 0);
      return [
        ...(timed.length ? [{ label: "最近创建", items: timed }] : []),
        ...(unknown.length ? [{ label: "时间未知", items: unknown }] : []),
      ];
    }
    const groups = new Map<string, typeof items>();
    byName.forEach((item) => {
      const key = arrangeMode === "type" ? item.category : libraryNameBucket(item.name);
      const group = groups.get(key) ?? [];
      group.push(item);
      groups.set(key, group);
    });
    const categoryOrder = ["待整理", ...organizedCategories];
    return [...groups.entries()]
      .map(([label, groupItems]) => ({ label, items: groupItems }))
      .sort((left, right) => arrangeMode === "type"
        ? categoryOrder.indexOf(left.label) - categoryOrder.indexOf(right.label)
        : libraryNameCollator.compare(left.label, right.label));
  }, [arrangeMode, filtered, visiblePrograms]);

  return (
    <section className="widget-view">
      <ViewHeader title="文件资料库" detail={`${itemCount} 个项目`} onBack={onBack} />
      <div className="file-library-surface">
        <div className="file-library-main">
          <div className="file-view-controls">
            <label className="widget-search"><Search size={15} /><input value={query} onChange={(event) => onQuery(event.target.value)} placeholder="搜索文件…" /></label>
            <label className="physical-filter">
              <Layers3 size={14} />
              <select data-testid="library-category-filter" value={filter} onChange={(event) => onFilter(event.target.value as FileFilter)} aria-label="物理分类筛选">
                <option value="all">全部分类</option>
                <option value="unorganized">待整理</option>
                {organizedCategories.map((category) => <option value={category} key={category}>{category}</option>)}
              </select>
              <ChevronRight size={13} />
            </label>
            <label className="arrange-filter">
              {arrangeMode === "created" ? <Clock3 size={14} /> : <ArrowDownAZ size={14} />}
              <select data-testid="library-arrange-mode" value={arrangeMode} onChange={(event) => onArrangeMode(event.target.value as LibraryArrangeMode)} aria-label="资料库分类和排序方式">
                <option value="type">按类型</option>
                <option value="name">按名称</option>
                <option value="created">按创建时间</option>
                <option value="initial">按首字母</option>
              </select>
              <ChevronRight size={13} />
            </label>
            <button
              type="button"
              className={`restore-all-library ${confirmRestoreAll ? "is-confirming" : ""}`}
              disabled={restoring || !organizedFiles.length}
              onClick={() => {
                if (!confirmRestoreAll) {
                  setConfirmRestoreAll(true);
                  return;
                }
                setConfirmRestoreAll(false);
                onRestore(organizedFiles);
              }}
            >
              <FolderOutput size={13} />
              {confirmRestoreAll ? `确认撤回 ${organizedFiles.length} 项` : "一键撤回到桌面"}
            </button>
          </div>
          <div className="full-file-list">
          {groupedItems.map((group) => (
            <section className="library-auto-group" key={group.label}>
              <header><strong>{group.label}</strong><span>{group.items.length}</span></header>
              {group.items.map((item) => {
                if (item.kind === "program") {
                  return (
                    <article className="full-file-row program-file-row" key={`program-${item.program.path}`}>
                      <button className="full-file-open" onClick={() => onLaunchProgram(item.program)}>
                        <span className="widget-file-icon physical-program"><AppWindow size={17} strokeWidth={1.9} /></span>
                        <span><strong>{item.program.name}</strong><small><em>程序</em>{item.program.source}</small></span>
                      </button>
                      <span className="program-auto-tag"><Sparkles size={11} />自动</span>
                    </article>
                  );
                }
                const file = item.file;
                const currentTag = file.category || "待整理";
                const tagOptions = userTagOptions.includes(currentTag) ? userTagOptions : [currentTag, ...userTagOptions];
                const timeLabel = arrangeMode === "created"
                  ? file.createdAt ? `创建 ${relativeTime(file.createdAt)}` : "创建时间未知"
                  : relativeTime(file.modifiedAt);
                return (
                  <article className="full-file-row" key={file.id} onContextMenu={(event) => openFileMenu(event, file)}>
                    <button className="full-file-open" onClick={() => onOpen(file)}>
                      <FileTypeIcon file={file} size={17} />
                      <span><strong data-no-i18n>{file.name}</strong><small><em>{item.category}</em>{isOrganizedFile(file) ? "已整理" : "桌面"} · {timeLabel}</small></span>
                    </button>
                    <label className="compact-category">
                      <select value={currentTag} onChange={(event) => onCategory(file.id, event.target.value)} aria-label={`${file.name} 分类`}>
                        {tagOptions.map((tag) => <option key={tag}>{tag}</option>)}
                      </select>
                    </label>
                    <button className="file-row-more" type="button" onClick={(event) => openFileMenu(event, file)} aria-label={`${file.name} 更多操作`}><MoreHorizontal size={15} /></button>
                  </article>
                );
              })}
            </section>
          ))}
          {!itemCount ? (
            <div className="full-file-empty"><Folder size={22} />{programsLoading && filter === "程序" ? "正在查找本机程序…" : "这个分类还没有项目"}</div>
          ) : null}
          </div>
        </div>
        <LibraryQuickSidebar files={files} programs={programs} programsLoading={programsLoading} filter={filter} onFilter={onFilter} onOpen={onOpen} onLaunchProgram={onLaunchProgram} />
      </div>
      {contextMenu ? (
        <div className="file-context-menu" style={{ left: contextMenu.x, top: contextMenu.y }} role="menu" onClick={(event) => event.stopPropagation()}>
          <button type="button" role="menuitem" onClick={() => { onOpen(contextMenu.file); setContextMenu(null); }}><ExternalLink size={14} />打开</button>
          {isOrganizedFile(contextMenu.file) ? (
            <button type="button" role="menuitem" disabled={restoring} onClick={() => { onRestore([contextMenu.file]); setContextMenu(null); }}><FolderOutput size={14} />撤回到桌面</button>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}

type TodosViewProps = {
  todos: Todo[];
  onBack: () => void;
  onAdd: (title: string) => void;
  onToggle: (id: string) => void;
  onAction: (todo: Todo) => void;
};

function TodosView({ todos, onBack, onAdd, onToggle, onAction }: TodosViewProps) {
  const [draft, setDraft] = useState("");
  const today = localDateKey();
  const todayPending = todos.filter((todo) => (todo.date ?? today) === today && todo.status !== "done").length;
  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!draft.trim()) return;
    onAdd(draft.trim());
    setDraft("");
  };
  return (
    <section className="widget-view">
      <ViewHeader title="今天待办" detail={`${todayPending} 项今日未完成`} onBack={onBack} />
      <form className="view-add" onSubmit={submit}><Plus size={15} /><input value={draft} onChange={(event) => setDraft(event.target.value)} placeholder="新增待办…" /><button>添加</button></form>
      <div className="full-todo-list">
        {todos.map((todo) => {
          const actionableTodo = withInferredTodoAction(todo);
          return (
            <article className={`full-todo-row ${todo.status === "done" ? "is-complete" : ""}`} key={todo.id}>
              <button className="full-todo-check" onClick={() => onToggle(todo.id)}>{todo.status === "done" ? <Check size={14} /> : <Circle size={17} />}</button>
              <button className="full-todo-copy" onClick={() => actionableTodo.actionType && onAction(actionableTodo)}><strong data-no-i18n>{todo.title}</strong><span>{(todo.date ?? today) === today ? todo.time : todo.date} · {todo.status === "doing" ? "进行中" : todo.status === "done" ? "已完成" : "待开始"}</span></button>
              {actionableTodo.actionType ? <button className="row-link" onClick={() => onAction(actionableTodo)}><ExternalLink size={14} /></button> : null}
            </article>
          );
        })}
      </div>
    </section>
  );
}

type IdeasViewProps = {
  ideas: Idea[];
  onBack: () => void;
  onAdd: (title: string) => void;
  onAccept: (id: string) => void;
  onConvert: (idea: Idea) => void;
};

function IdeasView({ ideas, onBack, onAdd, onAccept, onConvert }: IdeasViewProps) {
  const [draft, setDraft] = useState("");
  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!draft.trim()) return;
    onAdd(draft.trim());
    setDraft("");
  };
  return (
    <section className="widget-view">
      <ViewHeader title="意见整理" detail="想法、反馈与灵感" onBack={onBack} />
      <form className="view-add" onSubmit={submit}><Lightbulb size={15} /><input value={draft} onChange={(event) => setDraft(event.target.value)} placeholder="记录一个想法…" /><button>记录</button></form>
      <div className="full-idea-list">
        {ideas.map((idea) => (
          <article className="full-idea-row" key={idea.id}>
            <div><span className={`idea-state state-${idea.status}`}>{idea.status === "accepted" ? "已采纳" : idea.status === "converted" ? "已转待办" : "待整理"}</span><strong>{idea.title}</strong><small>{idea.tags.join(" · ")}</small></div>
            {idea.status === "pending" ? <div className="idea-row-actions"><button onClick={() => onAccept(idea.id)}>采纳</button><button onClick={() => onConvert(idea)}>转待办</button></div> : null}
          </article>
        ))}
      </div>
    </section>
  );
}

function NotificationSheet({ notices, onClose, onReadAll }: { notices: Notice[]; onClose: () => void; onReadAll: () => void }) {
  return (
    <div className="sheet-scrim" onMouseDown={onClose}>
      <section className="notification-sheet" onMouseDown={(event) => event.stopPropagation()}>
        <div className="sheet-heading"><div><Bell size={17} /><h2>通知</h2></div><button onClick={onClose} aria-label="关闭通知"><X size={17} /></button></div>
        <button className="read-all" onClick={onReadAll}>全部已读</button>
        <div className="sheet-list">
          {notices.map((notice) => <article className={notice.read ? "is-read" : ""} key={notice.id}><i /><div><strong>{notice.title}</strong><span>{notice.message}</span><small>{relativeTime(notice.createdAt)}</small></div></article>)}
        </div>
      </section>
    </div>
  );
}

function normalizeCustomSocialUrl(value: string) {
  const candidate = /^https?:\/\//i.test(value.trim()) ? value.trim() : `https://${value.trim()}`;
  return sanitizeExternalUrl(candidate).toString();
}

type SocialSettingsProps = {
  shortcuts: CustomSocialShortcut[];
  accounts: SocialAccountSnapshot[];
  loading: boolean;
  onAdd: (name: string, url: string) => void;
  onRemove: (id: string) => void;
  onOpen: (shortcut: CustomSocialShortcut) => void;
  onConnect: (platform: SocialPlatform) => Promise<void>;
  onSyncSnapshot: (platform: SocialPlatform) => Promise<SocialAccountSnapshot>;
  onSaveSnapshot: (snapshot: SocialAccountSnapshot) => Promise<void>;
  onDisconnect: (platform: SocialPlatform) => Promise<void>;
  onClear: (platform: SocialPlatform) => Promise<void>;
  onClose: () => void;
};

const socialPlatforms: Array<{ id: SocialPlatform; label: string; mark: string }> = [
  { id: "xiaohongshu", label: "小红书", mark: "红" },
  { id: "x", label: "X", mark: "X" },
  { id: "douyin", label: "抖音", mark: "抖" },
];

const emptySocialSnapshot = (platform: SocialPlatform): SocialAccountSnapshot => ({
  platform,
  displayName: "",
  followers: 0,
  unreadMessages: 0,
  unreadNotifications: 0,
  followersSource: "unavailable",
  unreadMessagesSource: "unavailable",
  unreadNotificationsSource: "unavailable",
  accountIdentity: "",
  connected: false,
  updatedAt: 0,
  sessionPersistence: "edge-profile",
  rawCookieAccessed: false,
});

function normalizeSocialMetricInput(value: string) {
  if (!value.trim()) return { value: 0, source: "unavailable" as const };
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < 0) return null;
  return { value: parsed, source: "manual" as const };
}

function SocialSettingsSheet({ shortcuts, accounts, loading, onAdd, onRemove, onOpen, onConnect, onSyncSnapshot, onSaveSnapshot, onDisconnect, onClear, onClose }: SocialSettingsProps) {
  const [name, setName] = useState("");
  const [url, setUrl] = useState("");
  const [error, setError] = useState("");
  const [drafts, setDrafts] = useState<Record<SocialPlatform, SocialAccountSnapshot>>(() => Object.fromEntries(
    socialPlatforms.map(({ id }) => [id, accounts.find((account) => account.platform === id) ?? emptySocialSnapshot(id)]),
  ) as Record<SocialPlatform, SocialAccountSnapshot>);
  const [busyPlatform, setBusyPlatform] = useState<SocialPlatform | null>(null);
  const [confirmClearPlatform, setConfirmClearPlatform] = useState<SocialPlatform | null>(null);

  useEffect(() => {
    setDrafts(Object.fromEntries(socialPlatforms.map(({ id }) => [
      id,
      accounts.find((account) => account.platform === id) ?? emptySocialSnapshot(id),
    ])) as Record<SocialPlatform, SocialAccountSnapshot>);
  }, [accounts]);

  useEffect(() => {
    if (!confirmClearPlatform) return;
    const timer = window.setTimeout(() => setConfirmClearPlatform(null), 4_000);
    return () => window.clearTimeout(timer);
  }, [confirmClearPlatform]);

  const updateDraft = (platform: SocialPlatform, patch: Partial<SocialAccountSnapshot>) => {
    setDrafts((current) => ({ ...current, [platform]: { ...current[platform], ...patch } }));
  };

  const connect = async (platform: SocialPlatform) => {
    setBusyPlatform(platform);
    setError("");
    try {
      await onConnect(platform);
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : "托管浏览器会话没有打开");
    } finally {
      setBusyPlatform(null);
    }
  };

  const saveSnapshot = async (platform: SocialPlatform) => {
    setBusyPlatform(platform);
    setError("");
    try {
      await onSaveSnapshot(drafts[platform]);
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : "汇总数据没有保存");
    } finally {
      setBusyPlatform(null);
    }
  };

  const syncSnapshot = async (platform: SocialPlatform) => {
    setBusyPlatform(platform);
    setError("");
    try {
      const snapshot = await onSyncSnapshot(platform);
      updateDraft(platform, snapshot);
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : "没有读取到当前页面的可见汇总数据");
    } finally {
      setBusyPlatform(null);
    }
  };

  const disconnect = async (platform: SocialPlatform) => {
    setBusyPlatform(platform);
    setError("");
    try {
      await onDisconnect(platform);
      updateDraft(platform, { connected: false });
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : "托管浏览器会话没有断开");
    } finally {
      setBusyPlatform(null);
    }
  };

  const clear = async (platform: SocialPlatform) => {
    if (confirmClearPlatform !== platform) {
      setConfirmClearPlatform(platform);
      return;
    }
    setBusyPlatform(platform);
    setError("");
    try {
      await onClear(platform);
      updateDraft(platform, emptySocialSnapshot(platform));
      setConfirmClearPlatform(null);
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : "登录资料没有清除");
    } finally {
      setBusyPlatform(null);
    }
  };

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const trimmedName = name.trim();
    if (!trimmedName || !url.trim()) {
      setError("请填写名称和主页链接");
      return;
    }
    try {
      const normalizedUrl = normalizeCustomSocialUrl(url);
      if (shortcuts.some((shortcut) => shortcut.url === normalizedUrl)) {
        setError("这个社交媒体已经添加过了");
        return;
      }
      onAdd(trimmedName.slice(0, 12), normalizedUrl);
      setName("");
      setUrl("");
      setError("");
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : "链接格式不正确");
    }
  };

  return (
    <div className="sheet-scrim" onMouseDown={onClose}>
      <section className="social-settings-sheet" onMouseDown={(event) => event.stopPropagation()} aria-label="社交媒体快捷入口设置">
        <div className="sheet-heading"><div><Link2 size={18} /><h2>社交账号中心</h2></div><button onClick={onClose} aria-label="关闭社交媒体设置"><X size={17} /></button></div>
        <p>每个平台使用独立的 Edge Profile 保持登录；登录态由 Edge 自己保存，应用不会提取、导出或复制原始 Cookie。</p>
        <p className="social-security-note">连接后请先在 Edge 完成登录，再点“同步并验证”。仅读取当前账号页面可见的名称和汇总数字，不读取私信正文。</p>
        <div className="social-account-grid" aria-busy={loading}>
          {socialPlatforms.map((platform) => {
            const snapshot = drafts[platform.id];
            const busy = loading || busyPlatform === platform.id;
            return (
              <article className="social-account-card" key={platform.id}>
                <div className="social-account-heading"><i>{platform.mark}</i><span><strong>{platform.label}</strong><small>{snapshot.connected ? `账号已验证${snapshot.displayName ? ` · ${snapshot.displayName}` : ""}` : "登录状态尚未验证"}</small></span></div>
                <label><span>账号名称</span><input value={snapshot.displayName} maxLength={64} placeholder="可选" onChange={(event) => updateDraft(platform.id, { displayName: event.target.value })} /></label>
                <div className="social-metric-row">
                  <label><span>粉丝</span><input type="number" min="0" step="1" value={snapshot.followersSource === "unavailable" ? "" : snapshot.followers} placeholder="未读取到" onChange={(event) => { const metric = normalizeSocialMetricInput(event.target.value); if (metric) updateDraft(platform.id, { followers: metric.value, followersSource: metric.source }); }} /><small>{snapshot.followersSource === "visible-page" ? "页面可见" : snapshot.followersSource === "manual" ? "手动值" : "未读取到"}</small></label>
                  <label><span>未读私信</span><input type="number" min="0" step="1" value={snapshot.unreadMessagesSource === "unavailable" ? "" : snapshot.unreadMessages} placeholder="未读取到" onChange={(event) => { const metric = normalizeSocialMetricInput(event.target.value); if (metric) updateDraft(platform.id, { unreadMessages: metric.value, unreadMessagesSource: metric.source }); }} /><small>{snapshot.unreadMessagesSource === "visible-page" ? "页面可见" : snapshot.unreadMessagesSource === "manual" ? "手动值" : "未读取到"}</small></label>
                  <label><span>通知</span><input type="number" min="0" step="1" value={snapshot.unreadNotificationsSource === "unavailable" ? "" : snapshot.unreadNotifications} placeholder="未读取到" onChange={(event) => { const metric = normalizeSocialMetricInput(event.target.value); if (metric) updateDraft(platform.id, { unreadNotifications: metric.value, unreadNotificationsSource: metric.source }); }} /><small>{snapshot.unreadNotificationsSource === "visible-page" ? "页面可见" : snapshot.unreadNotificationsSource === "manual" ? "手动值" : "未读取到"}</small></label>
                </div>
                <div className="social-account-actions">
                  <button type="button" onClick={() => void connect(platform.id)} disabled={busy}><ExternalLink size={12} />打开登录窗口</button>
                  <button type="button" onClick={() => void syncSnapshot(platform.id)} disabled={busy}><RefreshCw size={12} className={busyPlatform === platform.id ? "is-spinning" : ""} />{busyPlatform === platform.id ? "同步中…" : "同步并验证"}</button>
                  <button type="button" onClick={() => void saveSnapshot(platform.id)} disabled={busy}><Check size={12} />保存汇总</button>
                  <button type="button" onClick={() => void disconnect(platform.id)} disabled={busy}><Unplug size={12} />断开窗口</button>
                  <button type="button" className={confirmClearPlatform === platform.id ? "is-danger" : ""} onClick={() => void clear(platform.id)} disabled={busy}><Trash2 size={12} />{confirmClearPlatform === platform.id ? "确认清除登录资料" : "清除登录资料"}</button>
                </div>
              </article>
            );
          })}
        </div>
        <p className="social-security-note">没有官方 API 授权时，可同步托管页面上可见的汇总数字，也可手动修正后保存。</p>
        <h3 className="social-custom-heading">自定义快捷入口</h3>
        <form className="social-add-form" onSubmit={submit}>
          <label><span>名称</span><input value={name} onChange={(event) => setName(event.target.value)} maxLength={12} placeholder="例如：微信公众平台" /></label>
          <label><span>主页链接</span><input value={url} onChange={(event) => setUrl(event.target.value)} maxLength={2048} placeholder="mp.weixin.qq.com" /></label>
          {error ? <small role="alert">{error}</small> : null}
          <button type="submit"><Plus size={13} />添加快捷入口</button>
        </form>
        <div className="social-custom-list">
          {shortcuts.map((shortcut) => (
            <article key={shortcut.id}>
              <button className="social-custom-open" onClick={() => onOpen(shortcut)} title={shortcut.url}>
                <i>{shortcut.name.trim().charAt(0) || "社"}</i><span><strong>{shortcut.name}</strong><small>{shortcut.url}</small></span><ExternalLink size={13} />
              </button>
              <button className="social-custom-remove" onClick={() => onRemove(shortcut.id)} aria-label={`移除${shortcut.name}`} title="移除"><Trash2 size={13} /></button>
            </article>
          ))}
          {!shortcuts.length ? <div className="social-custom-empty">还没有自定义入口</div> : null}
        </div>
      </section>
    </div>
  );
}

type RestSettingsProps = {
  enabled: boolean;
  workMinutes: number;
  restMinutes: number;
  nextRestAt: number;
  onEnabled: (value: boolean) => void;
  onWorkMinutes: (value: number) => void;
  onRestMinutes: (value: number) => void;
  onPreview: () => void;
  onClose: () => void;
};

function RestSettingsSheet({
  enabled,
  workMinutes,
  restMinutes,
  nextRestAt,
  onEnabled,
  onWorkMinutes,
  onRestMinutes,
  onPreview,
  onClose,
}: RestSettingsProps) {
  const nextLabel = enabled
    ? new Intl.DateTimeFormat("zh-CN", { hour: "2-digit", minute: "2-digit" }).format(new Date(nextRestAt))
    : "未开启";
  return (
    <div className="sheet-scrim" onMouseDown={onClose}>
      <section className="rest-settings-sheet" onMouseDown={(event) => event.stopPropagation()}>
        <div className="sheet-heading"><div><Coffee size={18} /><h2>强制休息</h2></div><button onClick={onClose} aria-label="关闭休息设置"><X size={17} /></button></div>
        <div className="rest-setting-main">
          <div><strong>到点显示全屏休息提醒</strong><span>真实鼠标和键盘仍可使用，随时可以安全退出。</span></div>
          <button className={`widget-switch ${enabled ? "is-on" : ""}`} onClick={() => onEnabled(!enabled)} aria-label="切换强制休息" aria-pressed={enabled}><i /></button>
        </div>
        <div className="rest-setting-grid">
          <label><span>连续工作</span><select value={workMinutes} onChange={(event) => onWorkMinutes(Number(event.target.value))}><option value="15">15 分钟</option><option value="25">25 分钟</option><option value="45">45 分钟</option><option value="60">60 分钟</option></select></label>
          <label><span>休息时间</span><select value={restMinutes} onChange={(event) => onRestMinutes(Number(event.target.value))}><option value="1">1 分钟</option><option value="5">5 分钟</option><option value="10">10 分钟</option><option value="15">15 分钟</option></select></label>
        </div>
        <div className="next-rest-row"><Clock3 size={15} /><span>下次休息</span><strong>{nextLabel}</strong></div>
        <button className="preview-rest" onClick={onPreview}>立即预览休息动画</button>
        <p>不会移动或隐藏系统鼠标；点击退出按钮，或按 Esc 立即返回。</p>
      </section>
    </div>
  );
}

type PetSettingsProps = {
  locale: AppLocale;
  name: string;
  species: PetSpecies;
  theme: PetTheme;
  evolution: EvolutionPath;
  autoEvolution: boolean;
  manualEvolutionStage: EvolutionStage;
  evolutionStage: EvolutionStage;
  evolutionPoints: number;
  evolutionStagePoints: number;
  evolutionStageSpan: number;
  motionMode: PetMotionMode;
  visible: boolean;
  layer: PetLayer;
  focusMode: boolean;
  dialogueEnabled: boolean;
  voiceEnabled: boolean;
  connectorId: DialogueConnectorId;
  connectors: AgentConnectorStatus[];
  connectorsLoading: boolean;
  connectorScanMessage: string;
  agentWorkspace: string;
  lastFeed: PetFeedResult | null;
  onName: (value: string) => void;
  onSpecies: (value: PetSpecies) => void;
  onTheme: (value: PetTheme) => void;
  onEvolution: (value: EvolutionPath) => void;
  onEvolutionStage: (value: EvolutionStage) => void;
  onAutoEvolution: (value: boolean) => void;
  onMotionMode: (value: PetMotionMode) => void;
  onVisible: (value: boolean) => void;
  onLayer: (value: PetLayer) => void;
  onFocusMode: (value: boolean) => void;
  onDialogueEnabled: (value: boolean) => void;
  onVoiceEnabled: (value: boolean) => void;
  onConnector: (value: DialogueConnectorId) => void;
  onAgentWorkspace: (value: string) => void;
  onRefreshConnectors: () => void;
  onUndoFeed: () => void;
  onClose: () => void;
};

function PetSettingsSheet({
  locale,
  name,
  species,
  theme,
  evolution,
  autoEvolution,
  manualEvolutionStage,
  evolutionStage,
  evolutionPoints,
  evolutionStagePoints,
  evolutionStageSpan,
  motionMode,
  visible,
  layer,
  focusMode,
  dialogueEnabled,
  voiceEnabled,
  connectorId,
  connectors,
  connectorsLoading,
  connectorScanMessage,
  agentWorkspace,
  lastFeed,
  onName,
  onSpecies,
  onTheme,
  onEvolution,
  onEvolutionStage,
  onAutoEvolution,
  onMotionMode,
  onVisible,
  onLayer,
  onFocusMode,
  onDialogueEnabled,
  onVoiceEnabled,
  onConnector,
  onAgentWorkspace,
  onRefreshConnectors,
  onUndoFeed,
  onClose,
}: PetSettingsProps) {
  return (
    <div className="sheet-scrim" onMouseDown={onClose}>
      <section className="pet-settings-sheet" onMouseDown={(event) => event.stopPropagation()}>
        <div className="sheet-heading"><div><PawPrint size={18} /><h2>我的宠物</h2></div><button onClick={onClose} aria-label="关闭宠物设置"><X size={17} /></button></div>

        <div className="pet-identity-block">
          <span className={`pet-profile-avatar ${petClassName(species, theme, evolution)}`} aria-hidden="true">
            <img src={getPetFrameSrc(species, "idle_breathe", 1)} alt="" draggable={false} />
            <em>{evolutionBadges[evolution]}</em>
          </span>
          <label><span>名字</span><input value={name} maxLength={8} onChange={(event) => onName(event.target.value.replace(/\s+/g, "").slice(0, 8))} placeholder="给它起个名字" /></label>
        </div>

        <div className="pet-custom-block">
          <label><PawPrint size={15} />选择角色</label>
          <div className="pet-species-options">
            {petSpeciesOptions.map((option) => (
              <button key={option.value} aria-pressed={species === option.value} className={`${species === option.value ? "is-active" : ""} pet-species-${option.value}`} onClick={() => onSpecies(option.value)}>
                <img className="species-art" src={getPetFrameSrc(option.value, "idle_breathe", 1)} alt="" draggable={false} />
                <strong>{option.label}</strong><small>{option.detail}</small>
              </button>
            ))}
          </div>
        </div>

        <div className="pet-custom-block compact-custom-block">
          <label><Sparkles size={15} />动作速率</label>
          <div className="pet-motion-options">
            {petMotionOptions.map((option) => (
              <button key={option.value} aria-pressed={motionMode === option.value} className={motionMode === option.value ? "is-active" : ""} onClick={() => onMotionMode(option.value)}>
                <span><strong>{option.label}</strong><small>{option.detail}</small></span>
              </button>
            ))}
          </div>
        </div>

        <div className="pet-custom-block compact-custom-block">
          <label><Sparkles size={15} />外观主题</label>
          <div className="pet-theme-options">
            {petThemeOptions.map((option) => (
              <button key={option.value} aria-pressed={theme === option.value} className={theme === option.value ? "is-active" : ""} onClick={() => onTheme(option.value)}>
                <i className={`theme-dot pet-theme-${option.value}`} />{option.label}
              </button>
            ))}
          </div>
        </div>

        <div className="pet-custom-block">
          <div className="evolution-heading">
            <label><Sprout size={15} />进化路线</label>
            <button className={`widget-switch ${autoEvolution ? "is-on" : ""}`} onClick={() => onAutoEvolution(!autoEvolution)} aria-label="切换自动进化" aria-pressed={autoEvolution}><i /></button>
          </div>
          <div className="evolution-progress">
            <img src={getBlenderPetPreviewSrc(species, evolutionStage, evolution)} alt="Blender 3D 渲染的进化外观" />
            <span>
              <strong>{autoEvolution ? "指标自动进化" : "手动选择路线与阶段"}</strong>
              <small>
                {evolutionStage === "seedling" ? "幼苗阶段" : evolutionStage === "growing" ? "成长阶段" : "进化完成"}
                {autoEvolution ? ` · 总计 ${evolutionPoints} 点` : " · 手动外观"}
              </small>
            </span>
            <progress max={autoEvolution ? evolutionStageSpan : 1} value={autoEvolution ? evolutionStagePoints : 1} />
          </div>
          <div className="evolution-options">
            {evolutionPathOptions.map((option) => (
              <button key={option.value} disabled={autoEvolution} aria-pressed={evolution === option.value} className={evolution === option.value ? "is-active" : ""} onClick={() => onEvolution(option.value)}>
                <i>{evolutionBadges[option.value]}</i><span><strong>{option.label}</strong><small>{option.detail}</small></span>
              </button>
            ))}
          </div>
          <div className="evolution-stage-options" aria-label="进化阶段">
            {evolutionStageOptions.map((option) => (
              <button
                key={option.value}
                disabled={autoEvolution}
                aria-pressed={manualEvolutionStage === option.value}
                className={manualEvolutionStage === option.value ? "is-active" : ""}
                onClick={() => onEvolutionStage(option.value)}
              >
                <strong>{option.label}</strong><small>{option.detail}</small>
              </button>
            ))}
          </div>
        </div>

        <div className="pet-custom-block agent-connector-block">
          <div className="agent-connector-heading">
            <label><PlugZap size={15} />任务 Agent</label>
            <button className="agent-detect-button" type="button" onClick={onRefreshConnectors} disabled={connectorsLoading} aria-label="一键识别 Claude、Hermes 和 Codex 配置">
              <RefreshCw size={13} className={connectorsLoading ? "is-spinning" : ""} />
              <span>{connectorsLoading ? "正在识别…" : "一键识别配置"}</span>
            </button>
          </div>
          <div className="agent-connector-options">
            <button type="button" className={connectorId === "local" ? "is-active" : ""} aria-pressed={connectorId === "local"} onClick={() => onConnector("local")}>
              <Bot size={15} /><span><strong>本地助手</strong><small>文件、待办与社交入口</small></span><i className="is-ready" />
            </button>
            {connectors.map((connector) => (
              <button
                type="button"
                key={connector.id}
                disabled={!isAgentConnectorReady(connector)}
                className={`${connectorId === connector.id ? "is-active" : ""} is-${connector.configurationState}`}
                aria-pressed={connectorId === connector.id}
                onClick={() => onConnector(connector.id)}
                title={[connector.detail, connector.configLocationLabel].filter(Boolean).join(" · ")}
              >
                <Code2 size={15} />
                <span>
                  <strong>{connector.name}</strong>
                  <small>{agentConnectorStatusText(connector)}</small>
                  {connector.configLocationLabel ? <em>{connector.configLocationLabel}</em> : null}
                </span>
                <i className={`is-${connector.configurationState}`} />
              </button>
            ))}
          </div>
          {connectorsLoading || connectorScanMessage ? (
            <p className={`agent-scan-feedback ${connectorsLoading ? "is-loading" : ""}`} role="status" aria-live="polite">
              {connectorsLoading ? "正在识别 Claude、Hermes 和 Codex…" : connectorScanMessage}
            </p>
          ) : null}
          <AgentManagerPanel
            locale={locale}
            connectors={connectors}
            workspace={agentWorkspace}
            onWorkspace={onAgentWorkspace}
            onInstalled={onRefreshConnectors}
          />
          <p className="agent-privacy-note">凭据只在点击保存时写入所选 Agent 的默认配置；测试通断只检查接口地址，不发送密钥。任务只交给你选择的本机 CLI；对话里不会显示终端、思考或工作流。</p>
        </div>

        <div className="pet-setting-row">
          <div><MessageCircle size={17} /><span><strong>对话窗口</strong><small>只显示你的话、必要确认和最终结果</small></span></div>
          <button aria-pressed={dialogueEnabled} className={`widget-switch ${dialogueEnabled ? "is-on" : ""}`} onClick={() => onDialogueEnabled(!dialogueEnabled)} aria-label="切换对话窗口"><i /></button>
        </div>
        <div className={`pet-setting-row ${!dialogueEnabled ? "is-disabled" : ""}`}>
          <div><Volume2 size={17} /><span><strong>语音</strong><small>默认关闭，开启后才显示麦克风</small></span></div>
          <button disabled={!dialogueEnabled} aria-pressed={voiceEnabled} className={`widget-switch ${voiceEnabled ? "is-on" : ""}`} onClick={() => onVoiceEnabled(!voiceEnabled)} aria-label="切换语音"><i /></button>
        </div>
        <div className="pet-setting-row">
          <div>{visible ? <Eye size={17} /> : <EyeOff size={17} />}<span><strong>显示宠物</strong><small>独立透明窗口，可直接拖到任意位置</small></span></div>
          <button aria-pressed={visible} className={`widget-switch ${visible ? "is-on" : ""}`} onClick={() => onVisible(!visible)} aria-label="切换宠物显示"><i /></button>
        </div>
        <div className="pet-layer-block">
          <label><Layers3 size={16} />显示层级</label>
          <div className="layer-options">
            <button aria-pressed={layer === "normal"} className={layer === "normal" ? "is-active" : ""} onClick={() => onLayer("normal")}><strong>普通</strong><span>软件可以盖住</span></button>
            <button aria-pressed={layer === "bottom"} className={layer === "bottom" ? "is-active" : ""} onClick={() => onLayer("bottom")}><strong>桌面底层</strong><span>软件盖住，桌面可点</span></button>
            <button aria-pressed={layer === "top"} className={layer === "top" ? "is-active" : ""} onClick={() => onLayer("top")}><strong>置顶</strong><span>一直陪着你</span></button>
          </div>
        </div>
        <div className="pet-setting-row">
          <div><EyeOff size={17} /><span><strong>专注模式</strong><small>工作时立即隐藏宠物，结束后再恢复</small></span></div>
          <button aria-pressed={focusMode} className={`widget-switch ${focusMode ? "is-on" : ""}`} onClick={() => onFocusMode(!focusMode)} aria-label="切换专注模式"><i /></button>
        </div>
        {lastFeed ? (
          <div className="last-feed-row">
            <span><Archive size={16} /><strong>{`刚吃掉 ${lastFeed.count} 个项目`}</strong><small>{lastFeed.warning || lastFeed.names.slice(0, 2).join("、")}</small></span>
            <button onClick={onUndoFeed}>撤销</button>
          </div>
        ) : null}
        <p>桌面底层会待在其他软件下方，但回到桌面后仍可拖动、右键；也能从托盘一键找回。</p>
      </section>
    </div>
  );
}

type OrganizeSheetProps = {
  phase: OrganizePhase;
  result: DesktopOrganizeResult | null;
  error: string;
  canUndo: boolean;
  organizedTotal: number;
  desktopIconState: DesktopIconState;
  onClose: () => void;
  onUndo: (confirmationToken?: string) => void;
  onReview: (excludedMoveIds: string[], favoriteMoveIds: string[], confirmationToken?: string) => Promise<DesktopOrganizeReviewResult>;
  onRequestPublicConfirmation: (action: "organize" | "review" | "undo", batchId?: string, moveIds?: string[]) => Promise<string>;
  onOpenExclusions: () => void;
  onToggleDesktopIcons: () => Promise<void>;
  onOrganizePublicDesktop: (confirmationToken: string) => Promise<void>;
};

function OrganizeSheet({ phase, result, error, canUndo, organizedTotal, desktopIconState, onClose, onUndo, onReview, onRequestPublicConfirmation, onOpenExclusions, onToggleDesktopIcons, onOrganizePublicDesktop }: OrganizeSheetProps) {
  const [excludedMoveIds, setExcludedMoveIds] = useState<Set<string>>(() => new Set());
  const [favoriteMoveIds, setFavoriteMoveIds] = useState<Set<string>>(() => new Set());
  const [reviewing, setReviewing] = useState(false);
  const [reviewResult, setReviewResult] = useState<DesktopOrganizeReviewResult | null>(null);
  const [reviewError, setReviewError] = useState("");
  const [iconsUpdating, setIconsUpdating] = useState(false);
  const [publicOrganizeToken, setPublicOrganizeToken] = useState<string | null>(null);
  const [publicUndoToken, setPublicUndoToken] = useState<string | null>(null);
  const [publicReviewToken, setPublicReviewToken] = useState<string | null>(null);
  const [publicConfirmationLoading, setPublicConfirmationLoading] = useState(false);
  const confirmPublicOrganize = Boolean(publicOrganizeToken);
  const confirmPublicUndo = Boolean(publicUndoToken);
  const confirmPublicReview = Boolean(publicReviewToken);

  useEffect(() => {
    setExcludedMoveIds(new Set());
    setFavoriteMoveIds(new Set());
    setReviewing(false);
    setReviewResult(null);
    setReviewError("");
    setIconsUpdating(false);
    setPublicOrganizeToken(null);
    setPublicUndoToken(null);
    setPublicReviewToken(null);
    setPublicConfirmationLoading(false);
  }, [phase, result?.batchId]);

  useEffect(() => {
    if (!confirmPublicOrganize && !confirmPublicUndo && !confirmPublicReview) return;
    const timer = window.setTimeout(() => {
      setPublicOrganizeToken(null);
      setPublicUndoToken(null);
      setPublicReviewToken(null);
    }, 5_000);
    return () => window.clearTimeout(timer);
  }, [confirmPublicOrganize, confirmPublicReview, confirmPublicUndo]);

  if (phase === "idle") return null;
  const busy = phase === "organizing" || phase === "undoing" || reviewing || publicConfirmationLoading;
  const items = result?.items ?? [];
  const skippedTotal = result?.skippedCount ?? 0;
  const publicDesktopCount = result?.publicDesktopCount ?? desktopIconState.publicDesktopCount;
  const hasVisibleDesktopRemainders = !desktopIconState.hidden && (publicDesktopCount > 0 || skippedTotal > 0);
  const title = phase === "organizing"
    ? "正在整理桌面…"
    : phase === "undoing"
      ? "正在放回桌面…"
      : phase === "done"
        ? result?.movedCount
          ? (result.publicMovedCount ?? 0) > 0
            ? "公共桌面图标已收纳"
            : "个人桌面已整理"
          : hasVisibleDesktopRemainders
            ? "没有可移动项目"
            : "桌面已经很干净"
        : phase === "undone"
          ? "已撤销整理"
          : "这次没整理好";
  const detail = phase === "organizing"
    ? "文件会按类型放进 Documents「虫洞派资料库」，不会删除任何内容。"
    : phase === "undoing"
      ? "正在把最近一次整理的项目放回桌面。"
      : phase === "done"
        ? result?.movedCount
          ? `${result.movedCount} 个项目已归入 ${result.categoryCount} 个分类。`
          : "没有需要移动的项目。"
        : phase === "undone"
          ? `${result?.movedCount ?? 0} 个项目已经回到桌面。`
          : error;

  const toggleSelection = (setter: React.Dispatch<React.SetStateAction<Set<string>>>, moveId: string) => {
    setPublicReviewToken(null);
    setter((current) => {
      const next = new Set(current);
      if (next.has(moveId)) next.delete(moveId);
      else next.add(moveId);
      return next;
    });
  };

  const submitReview = async () => {
    if (!result?.batchId || reviewing) return;
    setReviewError("");
    const excludedIds = [...excludedMoveIds];
    if ((result.publicMovedCount ?? 0) > 0 && excludedIds.length > 0 && !publicReviewToken) {
      setPublicConfirmationLoading(true);
      try {
        setPublicReviewToken(await onRequestPublicConfirmation("review", result.batchId, excludedIds));
      } catch (confirmationError) {
        setReviewError(confirmationError instanceof Error ? confirmationError.message : String(confirmationError));
      } finally {
        setPublicConfirmationLoading(false);
      }
      return;
    }
    setReviewing(true);
    try {
      setReviewResult(await onReview(excludedIds, [...favoriteMoveIds], publicReviewToken ?? undefined));
      setPublicReviewToken(null);
    } catch (reviewFailure) {
      console.error(reviewFailure);
      setReviewError(reviewFailure instanceof Error ? reviewFailure.message : String(reviewFailure));
    } finally {
      setReviewing(false);
    }
  };

  return (
    <div className="sheet-scrim organize-scrim" onMouseDown={busy ? undefined : onClose}>
      <section className="organize-sheet" onMouseDown={(event) => event.stopPropagation()} aria-label="桌面整理结果">
        <div className="sheet-heading">
          <div><Sparkles size={18} /><h2>一键整理</h2></div>
          {!busy ? <button onClick={onClose} aria-label="关闭整理结果"><X size={17} /></button> : null}
        </div>
        <div className={`organize-result-state phase-${phase}`}>
          <span className="organize-result-icon">
            {busy ? <RefreshCw size={23} /> : phase === "error" ? <X size={23} /> : <Check size={23} />}
          </span>
          <div><h3>{title}</h3><p>{detail}</p></div>
        </div>
        {phase === "done" ? (
          <div className="organize-metrics" aria-label="整理统计">
            <span><strong>{result?.newMovedCount ?? result?.movedCount ?? 0}</strong><small>本次新增</small></span>
            <span><strong>{organizedTotal}</strong><small>资料库已整理</small></span>
            <span><strong>{skippedTotal}</strong><small>跳过项</small></span>
          </div>
        ) : null}
        {phase === "done" && Boolean(result?.migratedCount) ? (
          <p className="organize-migration-note"><Archive size={13} />{`旧整理库已迁入资料库 ${result?.migratedCount} 个项目。`}</p>
        ) : null}
        {phase === "done" && desktopIconState.supported ? (
          <div className="public-desktop-note">
            <span>
              <Eye size={15} />
              <strong>
                {desktopIconState.hidden
                  ? "桌面图标已隐藏，文件和快捷方式都还在"
                  : publicDesktopCount > 0
                    ? `检测到 ${publicDesktopCount} 个公共桌面图标，系统图标也可一并隐藏`
                    : skippedTotal > 0
                      ? `${skippedTotal} 个项目保持原位，系统图标也可一并隐藏`
                      : "回收站、此电脑等系统图标仍可能显示，可一并隐藏"}
              </strong>
            </span>
            <button
              disabled={iconsUpdating}
              onClick={() => {
                setIconsUpdating(true);
                void onToggleDesktopIcons().finally(() => setIconsUpdating(false));
              }}
            >
              {iconsUpdating ? "处理中…" : desktopIconState.hidden ? "恢复显示" : "隐藏桌面图标"}
            </button>
          </div>
        ) : null}
        {phase === "done" && publicDesktopCount > 0 ? (
          <div className={`public-desktop-admin-note ${confirmPublicOrganize ? "is-confirming" : ""}`}>
            <span>
              <Layers3 size={16} />
              <strong>{`${publicDesktopCount} 个公共桌面项目仍在原位`}</strong>
              <small>它们由所有 Windows 用户共享。收纳会影响其他账号，并会单独请求管理员批准。</small>
            </span>
            <button
              type="button"
              disabled={publicConfirmationLoading}
              onClick={() => {
                if (!confirmPublicOrganize) {
                  setPublicConfirmationLoading(true);
                  void onRequestPublicConfirmation("organize")
                    .then(setPublicOrganizeToken)
                    .catch((confirmationError) => setReviewError(confirmationError instanceof Error ? confirmationError.message : String(confirmationError)))
                    .finally(() => setPublicConfirmationLoading(false));
                  return;
                }
                const token = publicOrganizeToken;
                setPublicOrganizeToken(null);
                if (token) void onOrganizePublicDesktop(token);
              }}
            >
              {confirmPublicOrganize ? "确认并请求管理员批准" : "收纳公共桌面"}
            </button>
          </div>
        ) : null}
        {(phase === "done" || phase === "undone") && result?.skippedCount ? (
          result.skippedItems?.length ? (
            <details className="organize-skip-details">
              <summary>{`${result.skippedCount} 个项目保持原位，查看原因`}</summary>
              <div>
                {result.skippedItems.slice(0, 8).map((item) => (
                  <span key={`${item.reasonCode}-${item.path}`}><strong data-no-i18n>{item.name}</strong><small>{item.reason}</small></span>
                ))}
              </div>
            </details>
          ) : <p className="organize-skip-note">{`${result.skippedCount} 个项目保持原位。`}</p>
        ) : null}
        {phase === "done" && items.length ? (
          <div className={`organize-review ${reviewResult ? "is-reviewed" : ""}`}>
            <div className="organize-review-heading">
              <div><strong>快速复查</strong><span>哪些不该收进去？</span></div>
              <button onClick={onOpenExclusions}>已忽略整理</button>
            </div>
            <div className="organize-review-list">
              {items.map((item) => {
                const physicalCategory = organizedCategories.includes(item.category as OrganizedCategory)
                  ? item.category as OrganizedCategory
                  : item.category === "应用" ? "程序" : "其他";
                const Icon = organizedCategoryIconMap[physicalCategory];
                const excluded = excludedMoveIds.has(item.moveId);
                const favorite = favoriteMoveIds.has(item.moveId);
                return (
                  <article className={`organize-review-row ${excluded ? "is-excluded" : ""}`} key={item.moveId}>
                    <label>
                      <input
                        type="checkbox"
                        checked={excluded}
                        disabled={Boolean(reviewResult) || reviewing}
                        onChange={() => toggleSelection(setExcludedMoveIds, item.moveId)}
                      />
                      <span className={`review-file-icon physical-${categoryClassName(physicalCategory)}`}><Icon size={14} strokeWidth={1.9} /></span>
                      <span className="review-file-copy"><strong data-no-i18n>{item.name}</strong><small>{item.category}</small></span>
                    </label>
                    {item.launchable ? (
                      <button
                        className={favorite ? "is-favorite" : ""}
                        aria-pressed={favorite}
                        disabled={Boolean(reviewResult) || reviewing}
                        onClick={() => toggleSelection(setFavoriteMoveIds, item.moveId)}
                      >
                        <Star size={12} fill={favorite ? "currentColor" : "none"} />
                        {favorite ? "已加入" : "加入常用"}
                      </button>
                    ) : null}
                  </article>
                );
              })}
            </div>
            {reviewResult ? (
              <p className="organize-review-feedback">
                {`已放回并记住 ${reviewResult.rememberedCount} 个，仍可撤销 ${reviewResult.remainingUndoCount} 个。`}
                {reviewResult.conflictCount ? ` 另有 ${reviewResult.conflictCount} 个因重名或位置变化未能放回。` : ""}
              </p>
            ) : (
              <p className="organize-review-help">勾选后放回桌面，并记住以后不再整理同名项目。</p>
            )}
            {reviewError ? <p className="organize-review-error">{`保存失败：${reviewError}`}</p> : null}
          </div>
        ) : null}
        {busy ? <div className="organize-progress"><i /><i /><i /></div> : null}
        {!busy ? (
          <div className="organize-sheet-actions">
            {phase === "done" && canUndo ? (
              <button
                className={`organize-undo ${confirmPublicUndo ? "is-confirming" : ""}`}
                disabled={publicConfirmationLoading}
                onClick={() => {
                  if ((result?.publicMovedCount ?? 0) > 0 && !confirmPublicUndo) {
                    setPublicConfirmationLoading(true);
                    void onRequestPublicConfirmation("undo", result?.batchId)
                      .then(setPublicUndoToken)
                      .catch((confirmationError) => setReviewError(confirmationError instanceof Error ? confirmationError.message : String(confirmationError)))
                      .finally(() => setPublicConfirmationLoading(false));
                    return;
                  }
                  const token = publicUndoToken ?? undefined;
                  setPublicUndoToken(null);
                  onUndo(token);
                }}
              >
                {confirmPublicUndo ? "确认撤销公共桌面收纳" : "撤销整理"}
              </button>
            ) : null}
            {phase === "done" && items.length && !reviewResult ? (
              <button className="organize-finish" onClick={() => void submitReview()}>{confirmPublicReview ? "确认放回公共桌面" : "保存复查"}</button>
            ) : (
              <button className="organize-finish" onClick={onClose}>完成</button>
            )}
          </div>
        ) : null}
      </section>
    </div>
  );
}

type ExclusionsSheetProps = {
  open: boolean;
  exclusions: OrganizeExclusion[];
  loading: boolean;
  onClose: () => void;
  onRemove: (nameKey: string) => void;
};

function ExclusionsSheet({ open, exclusions, loading, onClose, onRemove }: ExclusionsSheetProps) {
  if (!open) return null;
  return (
    <div className="sheet-scrim exclusions-scrim" onMouseDown={onClose}>
      <section className="exclusions-sheet" onMouseDown={(event) => event.stopPropagation()} aria-label="已忽略整理">
        <div className="sheet-heading">
          <div><EyeOff size={18} /><h2>已忽略整理</h2></div>
          <button onClick={onClose} aria-label="关闭已忽略整理"><X size={17} /></button>
        </div>
        <p className="exclusions-intro">这些同名文件以后会留在桌面，不再被一键整理。</p>
        <div className="exclusions-list">
          {exclusions.map((item) => (
            <article className="exclusion-row" key={item.nameKey}>
              <span className="exclusion-icon">{item.isDirectory ? <Folder size={15} /> : <File size={15} />}</span>
              <span><strong data-no-i18n>{item.displayName}</strong><small>{relativeTime(item.createdAt)}</small></span>
              <button onClick={() => onRemove(item.nameKey)} aria-label={`不再忽略 ${item.displayName}`} title="移出忽略名单"><Trash2 size={14} /></button>
            </article>
          ))}
          {!exclusions.length ? (
            <div className="exclusions-empty"><Check size={17} />{loading ? "正在读取…" : "还没有忽略项"}</div>
          ) : null}
        </div>
      </section>
    </div>
  );
}

function formatCountdown(totalSeconds: number) {
  const minutes = Math.floor(totalSeconds / 60).toString().padStart(2, "0");
  const seconds = Math.max(0, totalSeconds % 60).toString().padStart(2, "0");
  return `${minutes}:${seconds}`;
}

function waitForStableViewport() {
  return new Promise<void>((resolve) => {
    window.requestAnimationFrame(() => window.requestAnimationFrame(() => resolve()));
  });
}

const restStageAnimationNames: Partial<Record<RestStage, string>> = {
  peek: "rest-pet-peek",
  approach: "rest-pet-approach",
  settle: "rest-pet-settle",
  exiting: "rest-pet-exit",
};

function RestOverlay({
  seconds,
  cursor,
  petName,
  species,
  theme,
  evolution,
  evolutionStage,
  stage,
  onRequestExit,
  onStageComplete,
}: {
  seconds: number;
  cursor: CursorPoint;
  petName: string;
  species: PetSpecies;
  theme: PetTheme;
  evolution: EvolutionPath;
  evolutionStage: EvolutionStage;
  stage: RestStage;
  onRequestExit: () => void;
  onStageComplete: (stage: RestStage) => void;
}) {
  const exitButtonRef = useRef<HTMLButtonElement>(null);
  const contactRef = useRef<HTMLSpanElement>(null);
  const latestCursorRef = useRef(cursor);
  const [cursorOrigin, setCursorOrigin] = useState(cursor);
  const [cursorContact, setCursorContact] = useState<CursorPoint>(() => ({
    x: window.innerWidth * 0.7,
    y: window.innerHeight * 0.54,
  }));
  latestCursorRef.current = cursor;

  useLayoutEffect(() => {
    if (stage !== "settle") return;
    setCursorOrigin(latestCursorRef.current);
    const bounds = contactRef.current?.getBoundingClientRect();
    if (!bounds) return;
    setCursorContact({
      x: bounds.left + bounds.width / 2,
      y: bounds.top + bounds.height / 2,
    });
  }, [stage]);

  useEffect(() => {
    if (stage === "preparing" || stage === "exiting") return;
    const frame = window.requestAnimationFrame(() => exitButtonRef.current?.focus({ preventScroll: true }));
    return () => window.cancelAnimationFrame(frame);
  }, [stage]);

  useEffect(() => {
    const handleEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      onRequestExit();
    };
    window.addEventListener("keydown", handleEscape, true);
    return () => window.removeEventListener("keydown", handleEscape, true);
  }, [onRequestExit]);

  useEffect(() => {
    const fallbackMs: Partial<Record<RestStage, number>> = {
      peek: 900,
      approach: 1_120,
      settle: 980,
      exiting: 820,
    };
    const delay = fallbackMs[stage];
    if (!delay) return;
    const timer = window.setTimeout(() => onStageComplete(stage), delay);
    return () => window.clearTimeout(timer);
  }, [onStageComplete, stage]);

  const handlePetStageEnd = (event: ReactAnimationEvent<HTMLDivElement>) => {
    if (event.target !== event.currentTarget) return;
    const expectedName = restStageAnimationNames[stage];
    if (expectedName && event.animationName === expectedName) onStageComplete(stage);
  };

  const petAction: PetSpriteAction = stage === "peek"
    ? "hide_peek"
    : stage === "approach"
      ? "run"
      : stage === "resting" || stage === "recovery"
        ? "idle_breathe"
        : "rest_reminder";
  const isRecovery = stage === "recovery";
  const isExiting = stage === "exiting";
  const statusText = isRecovery
    ? "窗口还没有完全恢复，可以再次尝试退出。"
    : isExiting
      ? "正在安全返回桌面。"
      : stage === "resting"
        ? "休息计时已经开始，真实鼠标和键盘仍然可用。"
        : "宠物正在轻轻来到屏幕边缘。";
  const cursorStyle = {
    "--rest-cursor-start-x": `${cursorOrigin.x}px`,
    "--rest-cursor-start-y": `${cursorOrigin.y}px`,
    "--rest-cursor-contact-x": `${cursorContact.x}px`,
    "--rest-cursor-contact-y": `${cursorContact.y}px`,
  } as CSSProperties;

  return (
    <main
      className={`rest-overlay rest-stage-${stage}`}
      data-stage={stage}
      role="dialog"
      aria-modal="true"
      aria-labelledby="rest-title"
      aria-describedby="rest-description rest-safety-note"
      aria-busy={stage === "preparing" || isExiting}
    >
      <p className="rest-status-live" aria-live="polite" aria-atomic="true">{statusText}</p>
      <div aria-hidden="true" className="rest-ambient rest-ambient-one" />
      <div aria-hidden="true" className="rest-ambient rest-ambient-two" />
      <div aria-hidden="true" className="rest-cursor-prop" style={cursorStyle}>
        <MousePointer2 size={18} />
        <span>动画</span>
      </div>
      <section className="rest-copy">
        <span><Coffee size={17} />{isRecovery ? "安全恢复" : "强制休息中"}</span>
        <h1 id="rest-title">{isRecovery ? "再试一次，就能回到桌面" : "把眼睛从屏幕移开一下"}</h1>
        <p id="rest-description">{isRecovery ? "窗口恢复刚才没有完成，宠物和真实鼠标都仍然可用。" : <>站起来、喝口水，看看远处。<span data-no-i18n>{petName}</span>会安静地替你守着桌面。</>}</p>
        <time aria-label={`剩余休息时间 ${formatCountdown(seconds)}`}>{formatCountdown(seconds)}</time>
        <p className="rest-safety-note" id="rest-safety-note"><MousePointer2 size={14} />这只是可爱动画，真实鼠标始终可见、可用</p>
      </section>
      <div
        className={`rest-pet-entry ${petClassName(species, theme, evolution)}`}
        data-stage={stage}
        onAnimationEnd={handlePetStageEnd}
      >
        <PetSprite
          species={species}
          action={petAction}
          evolutionStage={evolutionStage}
          evolutionPath={evolution}
          motionMode="quiet"
          className="pet-sprite--rest"
          lookX={0}
          lookY={0}
          gaze={stage === "resting" || stage === "recovery"}
        />
        <span className="rest-pet-contact" ref={contactRef} aria-hidden="true" />
      </div>
      <button
        ref={exitButtonRef}
        type="button"
        className="emergency-exit"
        aria-label={isRecovery ? "再次尝试退出休息模式" : "立即结束休息模式，或按 Escape 退出"}
        onClick={onRequestExit}
        disabled={stage === "preparing" || isExiting}
      >
        <MousePointer2 size={21} />
        <span>
          <strong>{isRecovery ? "再次尝试返回桌面" : isExiting ? "正在安全返回…" : "结束休息"}</strong>
          <small>点击即可退出，也可以按 Esc</small>
        </span>
      </button>
    </main>
  );
}

function DesktopPetWindow() {
  const [petName] = usePersistentState<string>(petStorageKeys.name, "派派");
  const [species] = usePersistentState<PetSpecies>(petStorageKeys.species, "star-cat");
  const [theme] = usePersistentState<PetTheme>(petStorageKeys.theme, "lilac");
  const [evolution] = usePersistentState<EvolutionPath>(petStorageKeys.evolution, "companion");
  const [autoEvolution] = usePersistentState<boolean>(petStorageKeys.autoEvolution, true);
  const [evolutionMetrics] = usePersistentState<PetEvolutionMetrics>(petStorageKeys.evolutionMetrics, defaultPetEvolutionMetrics, mergePetEvolutionMetrics);
  const evolutionState = useMemo(() => calculatePetEvolution(evolutionMetrics), [evolutionMetrics]);
  const [manualEvolutionStage] = usePersistentState<EvolutionStage>(petStorageKeys.manualEvolutionStage, evolutionState.stage);
  const effectiveEvolution = autoEvolution ? evolutionState.path : evolution;
  const effectiveEvolutionStage = autoEvolution ? evolutionState.stage : manualEvolutionStage;
  const [motionMode] = usePersistentState<PetMotionMode>(petStorageKeys.motionMode, "gentle");
  const [dialogueEnabled] = usePersistentState<boolean>(petStorageKeys.dialogueEnabled, true);
  const [cursor, setCursor] = useState<CursorPoint>({ x: 75, y: 40 });
  const [runtimeSnapshot, setRuntimeSnapshot] = useState<PetRuntimeSnapshot | null>(null);
  const runtimeSnapshotRef = useRef<PetRuntimeSnapshot | null>(null);
  const [feeding, setFeeding] = useState(false);
  const [dropHover, setDropHover] = useState(false);
  const [feedMessage, setFeedMessage] = useState("拖动我");
  const [snackName, setSnackName] = useState("");
  const lastDropRef = useRef<{ key: string; at: number } | null>(null);
  const petRef = useRef<HTMLDivElement>(null);
  const sendRuntimeSignal = useCallback((signal: string, payload?: Record<string, unknown>, transactionId?: string) => {
    const petId = runtimeSnapshot?.petId ?? getPetAssetId(species);
    return publishPetRuntimeSignal(createPetSignal(signal, "pet", petId, {
      payload,
      transactionId,
      source: signal.startsWith("file") || signal.includes("recycle") ? "file" : "pointer",
      essentialInQuietMode: signal === "confirmed_file_drop",
    }));
  }, [runtimeSnapshot?.petId, species]);
  const dragHandlers = useDraggableWindow(
    false,
    () => {
      void sendRuntimeSignal("single_click").catch(console.error);
      if (dialogueEnabled) void showPetDialogue().catch(console.error);
    },
    () => {
      void sendRuntimeSignal("pointer_drag").catch(console.error);
      void hidePetDialogue().catch(console.error);
    },
    (dragged) => {
      if (dragged) void sendRuntimeSignal("pointer_release_slow").catch(console.error);
    },
  );

  const confirmFileDrop = useCallback((paths: string[]) => {
    const cleanPaths = [...new Set(paths.filter((path) => path.trim()))];
    if (!cleanPaths.length) {
      setFeedMessage("请从桌面或虫洞派资料库拖入文件");
      return;
    }
    const key = cleanPaths.join("\n").toLowerCase();
    const now = Date.now();
    if (lastDropRef.current?.key === key && now - lastDropRef.current.at < 800) return;
    lastDropRef.current = { key, at: now };
    const name = cleanPaths[0]?.split(/[\\/]/).pop() ?? "文件";
    const transactionId = typeof crypto !== "undefined" && "randomUUID" in crypto ? crypto.randomUUID() : makeId("feed");
    setSnackName(name);
    setDropHover(false);
    setFeeding(true);
    setFeedMessage("啊呜——");
    void sendRuntimeSignal("confirmed_file_drop", { paths: cleanPaths, name }, transactionId)
      .catch((error) => {
        console.error(error);
        setFeeding(false);
        setFeedMessage("这个文件我不能吃");
      });
  }, [sendRuntimeSignal]);

  useEffect(() => {
    let cleanup: undefined | (() => void);
    let cancelled = false;
    const acceptSnapshot = (snapshot: PetRuntimeSnapshot) => {
      if (cancelled) return;
      runtimeSnapshotRef.current = snapshot;
      setRuntimeSnapshot((current) => !current || snapshot.revision > current.revision || snapshot.petId !== current.petId ? snapshot : current);
    };
    void (async () => {
      const unlisten = await subscribeToPetRuntimeState(acceptSnapshot);
      if (cancelled) {
        unlisten();
        return;
      }
      cleanup = unlisten;
      for (let attempt = 0; attempt < 5 && !runtimeSnapshotRef.current && !cancelled; attempt += 1) {
        await requestPetRuntimeState();
        if (!runtimeSnapshotRef.current) await new Promise((resolve) => window.setTimeout(resolve, 400));
      }
    })().catch(console.error);
    return () => {
      cancelled = true;
      cleanup?.();
    };
  }, []);

  useEffect(() => {
    if (isTauri()) {
      const interval = window.setInterval(() => {
        void getCursorPosition().then(setCursor).catch(() => undefined);
      }, 120);
      return () => window.clearInterval(interval);
    }
    const onPointerMove = (event: PointerEvent) => setCursor({ x: event.clientX, y: event.clientY });
    window.addEventListener("pointermove", onPointerMove, { passive: true });
    return () => window.removeEventListener("pointermove", onPointerMove);
  }, []);

  useEffect(() => {
    let unlisten: undefined | (() => void);
    let cancelled = false;
    subscribeToPetFileDrop((event) => {
      if (event.type === "enter") {
        const name = event.paths[0]?.split(/[\\/]/).pop() ?? "文件";
        setSnackName(name);
        setFeedMessage("放开喂给我");
        setDropHover(true);
        void sendRuntimeSignal("file_entered", { paths: event.paths, name }).catch(console.error);
      } else if (event.type === "over") {
        setDropHover(true);
      } else if (event.type === "leave") {
        setDropHover(false);
        setFeedMessage("拖动我");
      } else if (event.type === "drop") {
        confirmFileDrop(event.paths);
      }
    }).then((cleanup) => {
      if (cancelled) cleanup();
      else unlisten = cleanup;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [confirmFileDrop]);

  const handleDomDragOver = (event: ReactDragEvent<HTMLElement>) => {
    event.preventDefault();
    event.stopPropagation();
    setDropHover(true);
    setFeedMessage("放开喂给我");
  };

  const handleDomDragLeave = (event: ReactDragEvent<HTMLElement>) => {
    event.preventDefault();
    event.stopPropagation();
    const nextTarget = event.relatedTarget;
    if (!(nextTarget instanceof Node) || !event.currentTarget.contains(nextTarget)) {
      setDropHover(false);
      setFeedMessage("拖动我");
    }
  };

  const handleDomDrop = (event: ReactDragEvent<HTMLElement>) => {
    event.preventDefault();
    event.stopPropagation();
    const paths = Array.from(event.dataTransfer.files)
      .map((file) => (file as File & { path?: string }).path ?? "")
      .filter(Boolean);
    confirmFileDrop(paths);
  };

  useEffect(() => {
    let feedCleanup: undefined | (() => void);
    let restoreCleanup: undefined | (() => void);
    let resetTimer = 0;
    let cancelled = false;
    const settle = (message: string) => {
      setFeedMessage(message);
      window.clearTimeout(resetTimer);
      resetTimer = window.setTimeout(() => {
        if (cancelled) return;
        setFeeding(false);
        setSnackName("");
        setFeedMessage("拖动我");
      }, 1900);
    };
    subscribeToPetFeed((result) => settle(result.warning ? "部分失败，请检查回收站" : `${result.count} 个项目已进回收站`))
      .then((unlisten) => {
        if (cancelled) unlisten();
        else feedCleanup = unlisten;
      }).catch(console.error);
    subscribeToPetRestore((result) => settle(`${result.count} 个项目已恢复`))
      .then((unlisten) => {
        if (cancelled) unlisten();
        else restoreCleanup = unlisten;
      }).catch(console.error);
    return () => {
      cancelled = true;
      window.clearTimeout(resetTimer);
      feedCleanup?.();
      restoreCleanup?.();
    };
  }, []);

  const bounds = petRef.current?.getBoundingClientRect();
  const centerX = bounds ? bounds.left + bounds.width / 2 : 75;
  const centerY = bounds ? bounds.top + bounds.height / 2 : 85;
  const look = gentleLookVector(cursor, centerX, centerY);
  const spriteAction = (runtimeSnapshot?.actionId as PetSpriteAction | undefined) ?? "idle_breathe";

  return (
    <main
      className={`desktop-pet-window ${petClassName(species, theme, effectiveEvolution)} action-${spriteAction} ${dropHover ? "is-drop-target" : ""}`}
      data-tauri-drag-region
      {...dragHandlers}
      onDragOver={handleDomDragOver}
      onDragEnter={handleDomDragOver}
      onDragLeave={handleDomDragLeave}
      onDrop={handleDomDrop}
      onContextMenu={(event) => {
        event.preventDefault();
        void showPetContextMenu().catch(console.error);
      }}
    >
      <span className="pet-window-tip">{feedMessage}</span>
      {dropHover || feeding ? <span className="file-snack"><File size={13} />{snackName}</span> : null}
      <div className="floating-pet" ref={petRef} data-tauri-drag-region>
        <PetSprite
          species={species}
          action={spriteAction}
          evolutionStage={effectiveEvolutionStage}
          evolutionPath={effectiveEvolution}
          motionMode={runtimeSnapshot?.behaviorMode ?? motionMode}
          className="pet-sprite--desktop"
          lookX={look.x}
          lookY={look.y}
          gaze
          actionInstanceId={runtimeSnapshot?.actionInstanceId}
          startedAtEpochMs={runtimeSnapshot?.startedAtEpochMs}
          velocity={runtimeSnapshot?.velocity}
          reducedMotion={runtimeSnapshot?.reducedMotion}
          transactionId={runtimeSnapshot?.transactionId}
          onMarker={(event) => void publishPetRuntimeMarker(event).catch(console.error)}
          onLoopBoundary={(event) => void publishPetRuntimeLoopBoundary({
            actionInstanceId: event.actionInstanceId,
            loopCount: event.loopCount,
            atEpochMs: event.atEpochMs,
          }).catch(console.error)}
          onAnimationEnd={() => {
            if (!runtimeSnapshot) return;
            void publishPetRuntimeAnimationEnd({ actionInstanceId: runtimeSnapshot.actionInstanceId, atEpochMs: Date.now() }).catch(console.error);
          }}
        />
        <span className="floating-badge pet-sprite-badge">{evolutionBadges[effectiveEvolution]}</span>
      </div>
    </main>
  );
}

function PetDialogueWindow() {
  const [petName] = usePersistentState<string>(petStorageKeys.name, "派派");
  const [dialogueEnabled] = usePersistentState<boolean>(petStorageKeys.dialogueEnabled, true);
  const [voiceEnabled] = usePersistentState<boolean>(petStorageKeys.voiceEnabled, false);
  const [connectorId, setConnectorId] = usePersistentState<DialogueConnectorId>("wormhole-pie.agent.connector.v1", "local");
  const [legacyAgentWorkspace, setLegacyAgentWorkspace] = usePersistentState<string>("wormhole-pie.agent.workspace.v1", "");
  const [storedDialogueSessions, setStoredDialogueSessions] = usePersistentState<DialogueSessions>(
    dialogueSessionStorageKey,
    createDialogueSessions(),
    mergeDialogueSessions,
  );
  const dialogueSessions = normalizeDialogueSessions(storedDialogueSessions);
  const selectedSession = dialogueSessions[connectorId];
  const updatePetDialogueSession = useCallback((targetConnector: DialogueConnectorId, patch: Partial<DialogueSession> | ((current: DialogueSession) => Partial<DialogueSession>)) => {
    setStoredDialogueSessions((currentValue) => {
      const current = normalizeDialogueSessions(currentValue);
      const session = current[targetConnector];
      const resolved = typeof patch === "function" ? patch(session) : patch;
      return {
        ...current,
        [targetConnector]: updateDialogueSessionFields(targetConnector, session, resolved),
      };
    });
  }, [setStoredDialogueSessions]);
  const [connectors, setConnectors] = useState<AgentConnectorStatus[]>([]);
  const [state, setState] = useState<DialogueState>({ userMessage: "", reply: "想让我做什么？", listening: false, busy: false, connectorId: "local" });
  const [agentTaskStatus, setAgentTaskStatus] = useState<AgentTaskStatus | null>(null);
  const taskBusy = (isAgentTaskActive(agentTaskStatus) && agentTaskStatus?.connectorId === connectorId)
    || selectedSession.busy
    || (state.busy === true && state.connectorId === connectorId);
  const taskTiming = useAgentTaskTiming(taskBusy, agentTaskStatus);

  useEffect(() => {
    setStoredDialogueSessions((current) => normalizeDialogueSessions(current));
  }, [setStoredDialogueSessions]);

  useEffect(() => {
    const legacyWorkspace = legacyAgentWorkspace.trim();
    if (!legacyWorkspace) return;
    updatePetDialogueSession(connectorId, (current) => current.workspace.trim() ? {} : { workspace: legacyWorkspace });
    setLegacyAgentWorkspace("");
  }, [connectorId, legacyAgentWorkspace, setLegacyAgentWorkspace, updatePetDialogueSession]);

  useEffect(() => {
    let cancelled = false;
    void listAgentConnectors()
      .then((items) => {
        if (!cancelled) setConnectors(items);
      })
      .catch(console.error);
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    let cleanup: undefined | (() => void);
    let cancelled = false;
    subscribeToDialogueState((next) => {
      setState(next);
      if (next.sessions) {
        setStoredDialogueSessions((current) => mergeDialogueSessions(current, next.sessions));
      }
      void listAgentConnectors().then(setConnectors).catch(console.error);
    }).then((unlisten) => {
      if (cancelled) unlisten();
      else cleanup = unlisten;
    }).catch(console.error);
    return () => {
      cancelled = true;
      cleanup?.();
    };
  }, [setStoredDialogueSessions]);

  useEffect(() => {
    if (!taskBusy) return;
    let cancelled = false;
    const poll = () => {
      void getAgentTaskStatus().then((next) => {
        if (cancelled || !next) return;
        setAgentTaskStatus((current) => !current || next.updatedAt >= current.updatedAt ? next : current);
        const active = isAgentTaskActive(next);
        updatePetDialogueSession(next.connectorId, { busy: active, activeTaskId: active ? next.taskId : null });
      }).catch(console.error);
    };
    const timer = window.setInterval(poll, 4_000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [taskBusy, updatePetDialogueSession]);

  useEffect(() => {
    let cleanup: undefined | (() => void);
    let cancelled = false;
    const acceptStatus = (next: AgentTaskStatus) => {
      if (cancelled) return;
      setAgentTaskStatus((current) => !current || next.updatedAt >= current.updatedAt ? next : current);
      const active = isAgentTaskActive(next);
      updatePetDialogueSession(next.connectorId, { busy: active, activeTaskId: active ? next.taskId : null });
    };
    void getAgentTaskStatus().then((next) => {
      if (next) acceptStatus(next);
      else (["codex", "claude", "hermes"] as const).forEach((targetConnector) => {
        updatePetDialogueSession(targetConnector, { busy: false, activeTaskId: null });
      });
    }).catch((error) => {
      console.error(error);
      if (cancelled) return;
      setAgentTaskStatus(null);
      (["codex", "claude", "hermes"] as const).forEach((targetConnector) => {
        updatePetDialogueSession(targetConnector, { busy: false, activeTaskId: null });
      });
    });
    void subscribeToAgentTaskStatus(acceptStatus).then((unlisten) => {
      if (cancelled) unlisten();
      else cleanup = unlisten;
    }).catch(console.error);
    return () => {
      cancelled = true;
      cleanup?.();
    };
  }, [updatePetDialogueSession]);

  useEffect(() => {
    if (!dialogueEnabled) void hidePetDialogue().catch(console.error);
  }, [dialogueEnabled]);

  const recognize = useCallback(async () => {
    if (!voiceEnabled) {
      updatePetDialogueSession(connectorId, (current) => appendDialogueAssistantMessage(current, "本地语音还没开启，可以先打字，或在宠物设置里打开。"));
      return null;
    }
    setState((current) => ({ ...current, listening: true }));
    updatePetDialogueSession(connectorId, (current) => setDialogueSessionPreview(current, "我在听。"));
    try {
      return await recognizeLocalSpeech();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      updatePetDialogueSession(connectorId, (current) => appendDialogueAssistantMessage(current, message));
      return null;
    } finally {
      setState((current) => ({ ...current, listening: false }));
    }
  }, [connectorId, updatePetDialogueSession, voiceEnabled]);

  const sendCommand = useCallback(async (submission: DialogueSubmission) => {
    updatePetDialogueSession(connectorId, (current) => ({
      ...appendDialogueUserMessage(current, submission.text),
      busy: connectorId !== "local",
      activeTaskId: connectorId === "local" ? null : current.activeTaskId ?? "pending",
    }));
    setState((current) => ({ ...current, userMessage: submission.text, busy: connectorId !== "local", connectorId, resultFiles: [] }));
    try {
      await sendDialogueCommand({ ...submission, connectorId });
    } catch (error) {
      console.error(error);
      setState((current) => ({ ...current, busy: false }));
      updatePetDialogueSession(connectorId, (current) => appendDialogueAssistantMessage({ ...current, busy: false, activeTaskId: null }, "这句话没有送出去，再试一次吧。"));
      throw error;
    }
  }, [connectorId, updatePetDialogueSession]);

  const stopCurrentTask = useCallback(() => {
    if (!taskBusy) return;
    const taskConnector = agentTaskStatus?.connectorId ?? (connectorId === "local" ? undefined : connectorId);
    if (!taskConnector) return;
    setState((current) => ({ ...current, busy: true }));
    updatePetDialogueSession(taskConnector, (current) => setDialogueSessionPreview(current, "正在温柔停下来…"));
    const activeTaskId = agentTaskStatus && isAgentTaskActive(agentTaskStatus) ? agentTaskStatus.taskId : undefined;
    void stopAgentTask(activeTaskId).catch((error) => {
      console.error(error);
      updatePetDialogueSession(taskConnector, (current) => appendDialogueAssistantMessage(current, "暂时没能停下来，再试一次吧。"));
    });
  }, [agentTaskStatus, connectorId, taskBusy, updatePetDialogueSession]);

  const openResultFile = useCallback(async (file: AgentResultFile) => {
    try {
      const sessionWorkspace = selectedSession.workspace.trim();
      const workspace = sessionWorkspace || await getAgentDefaultWorkspace();
      if (!sessionWorkspace) updatePetDialogueSession(connectorId, { workspace });
      await openAgentResultFile(file.path, workspace);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      updatePetDialogueSession(connectorId, (current) => appendDialogueAssistantMessage(current, message || "这个文件暂时打不开。"));
    }
  }, [connectorId, selectedSession.workspace, updatePetDialogueSession]);

  if (!dialogueEnabled) return null;

  return (
    <MinimalDialogue
      open
      standalone
      petName={petName || "派派"}
      userMessage={selectedSession.userMessage}
      reply={selectedSession.reply}
      messages={selectedSession.messages}
      draft={selectedSession.draft}
      attachmentPaths={selectedSession.attachmentPaths}
      voiceEnabled={voiceEnabled}
      isListening={state.listening}
      busy={taskBusy}
      busyElapsedMs={taskTiming.elapsedMs}
      busyHeartbeatKnown={taskTiming.heartbeatKnown}
      busyHeartbeatFresh={taskTiming.heartbeatFresh}
      busyCancelling={agentTaskStatus?.connectorId === connectorId && (agentTaskStatus.state === "cancelling" || agentTaskStatus.cancelRequested === true)}
      connectorId={connectorId}
      connectors={connectors}
      resultFiles={selectedSession.resultFiles}
      onClose={() => void hidePetDialogue().catch(console.error)}
      onSubmit={sendCommand}
      onMic={recognize}
      onConnector={setConnectorId}
      onDraftChange={(draft) => updatePetDialogueSession(connectorId, { draft })}
      onAttachmentPathsChange={(next) => updatePetDialogueSession(connectorId, (current) => ({
        attachmentPaths: typeof next === "function" ? next(current.attachmentPaths) : next,
      }))}
      onOpenResultFile={openResultFile}
      onStop={taskBusy ? stopCurrentTask : undefined}
    />
  );
}

function WidgetApplication({ locale, onLocale }: { locale: AppLocale; onLocale: (locale: AppLocale) => void }) {
  const [activeTab, setActiveTab] = useState<WidgetTab>("home");
  const [homeLibrary, setHomeLibrary] = useState<HomeLibrary>("desktop");
  const [files, setFiles] = useState<DesktopFile[]>(isTauri() ? [] : mockFiles);
  const filesRef = useRef(files);
  const [programs, setPrograms] = useState<ProgramEntry[]>([]);
  const [programsLoaded, setProgramsLoaded] = useState(false);
  const [programsLoading, setProgramsLoading] = useState(false);
  const [favoritePrograms, setFavoritePrograms] = usePersistentState<FavoriteProgram[]>("wormhole-pie.widget.favoritePrograms.v1", []);
  const [todos, setTodos] = usePersistentState<Todo[]>("wormhole-pie.widget.todos.v1", seedTodos);
  const [todayKey, setTodayKey] = useState(() => localDateKey());
  const [ideas, setIdeas] = usePersistentState<Idea[]>("wormhole-pie.widget.ideas.v1", seedIdeas);
  const [notices, setNotices] = usePersistentState<Notice[]>("wormhole-pie.widget.notices.v1", seedNotices);
  const [fileQuery, setFileQuery] = useState("");
  const [fileFilter, setFileFilter] = useState<FileFilter>("all");
  const [libraryArrangeMode, setLibraryArrangeMode] = usePersistentState<LibraryArrangeMode>("wormhole-pie.library.arrangeMode.v1", "type");
  const [customSocialShortcuts, setCustomSocialShortcuts] = usePersistentState<CustomSocialShortcut[]>("wormhole-pie.social.shortcuts.v1", []);
  const [socialAccounts, setSocialAccounts] = useState<SocialAccountSnapshot[]>([]);
  const [socialAccountsLoading, setSocialAccountsLoading] = useState(false);
  const [dialogueOpen, setDialogueOpen] = useState(false);
  const [agentConnector, setAgentConnector] = usePersistentState<DialogueConnectorId>("wormhole-pie.agent.connector.v1", "local");
  const agentConnectorRef = useRef(agentConnector);
  agentConnectorRef.current = agentConnector;
  const [storedDialogueSessions, setStoredDialogueSessions] = usePersistentState<DialogueSessions>(
    dialogueSessionStorageKey,
    createDialogueSessions(),
    mergeDialogueSessions,
  );
  const dialogueSessions = normalizeDialogueSessions(storedDialogueSessions);
  const dialogueSessionsRef = useRef(dialogueSessions);
  dialogueSessionsRef.current = dialogueSessions;
  const activeDialogueSession = dialogueSessions[agentConnector];
  const assistantMessage = activeDialogueSession.reply;
  const lastUserMessage = activeDialogueSession.userMessage;
  const agentResultFiles = activeDialogueSession.resultFiles;
  const updateDialogueSession = useCallback((connectorId: DialogueConnectorId, patch: Partial<DialogueSession> | ((current: DialogueSession) => Partial<DialogueSession>)) => {
    setStoredDialogueSessions((currentValue) => {
      const current = normalizeDialogueSessions(currentValue);
      const session = current[connectorId];
      const resolved = typeof patch === "function" ? patch(session) : patch;
      return {
        ...current,
        [connectorId]: updateDialogueSessionFields(connectorId, session, resolved),
      };
    });
  }, [setStoredDialogueSessions]);
  useEffect(() => {
    setStoredDialogueSessions((current) => normalizeDialogueSessions(current));
  }, [setStoredDialogueSessions]);
  const recordSessionUserMessage = useCallback((connectorId: DialogueConnectorId, message: string) => {
    updateDialogueSession(connectorId, (current) => appendDialogueUserMessage(current, message));
  }, [updateDialogueSession]);
  const recordSessionAssistantMessage = useCallback((connectorId: DialogueConnectorId, message: string, files: AgentResultFile[] = [], taskId?: string) => {
    updateDialogueSession(connectorId, (current) => appendDialogueAssistantMessage(current, message, files, taskId));
  }, [updateDialogueSession]);
  const previewSessionAssistantMessage = useCallback((connectorId: DialogueConnectorId, message: string, files?: AgentResultFile[]) => {
    updateDialogueSession(connectorId, (current) => setDialogueSessionPreview(current, message, files));
  }, [updateDialogueSession]);
  const setAssistantMessage = useCallback((next: string | ((current: string) => string)) => {
    updateDialogueSession("local", (current) => appendDialogueAssistantMessage(current, typeof next === "function" ? next(current.reply) : next));
  }, [updateDialogueSession]);
  const setLastUserMessage = useCallback((next: string | ((current: string) => string)) => {
    agentConnectorRef.current = "local";
    setAgentConnector("local");
    updateDialogueSession("local", (current) => appendDialogueUserMessage(current, typeof next === "function" ? next(current.userMessage) : next));
  }, [setAgentConnector, updateDialogueSession]);
  const [agentConnectors, setAgentConnectors] = useState<AgentConnectorStatus[]>([]);
  const [agentConnectorsLoading, setAgentConnectorsLoading] = useState(false);
  const [agentConnectorScanMessage, setAgentConnectorScanMessage] = useState("");
  const [legacyAgentWorkspace, setLegacyAgentWorkspace] = usePersistentState<string>("wormhole-pie.agent.workspace.v1", "");
  const [agentBusy, setAgentBusy] = useState(false);
  const [agentBusyConnector, setAgentBusyConnector] = useState<AgentConnectorId | null>(null);
  const [agentTaskStatus, setAgentTaskStatus] = useState<AgentTaskStatus | null>(null);
  const [lastDeliveredAgentTaskId, setLastDeliveredAgentTaskId] = usePersistentState<string>("wormhole-pie.agent.lastDeliveredTask.v1", "");
  const lastDeliveredAgentTaskIdRef = useRef(lastDeliveredAgentTaskId);
  const agentInvokeOwnedRef = useRef(false);
  const dialogueStateRef = useRef<DialogueState>({ userMessage: "", reply: "想让我做什么？", listening: false, busy: false, connectorId: agentConnector, resultFiles: [] });
  const [isScanning, setIsScanning] = useState(false);
  const [restoringLibrary, setRestoringLibrary] = useState(false);
  const [isListening, setIsListening] = useState(false);
  const [notificationsOpen, setNotificationsOpen] = useState(false);
  const [socialSettingsOpen, setSocialSettingsOpen] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [petSettingsOpen, setPetSettingsOpen] = useState(false);
  const [exclusionsOpen, setExclusionsOpen] = useState(false);
  const [organizeExclusions, setOrganizeExclusions] = useState<OrganizeExclusion[]>([]);
  const [exclusionsLoading, setExclusionsLoading] = useState(false);
  const [isHidingToTray, setIsHidingToTray] = useState(false);
  const [windowMotion, setWindowMotion] = useState<WindowMotion>("opening");
  const [collapsedHomeSections, setCollapsedHomeSections] = usePersistentState<HomeSectionId[]>("wormhole-pie.home.collapsedSections.v1", []);
  const [homeSectionMotion, setHomeSectionMotion] = useState<HomeSectionMotion>(null);
  const [cursor, setCursor] = useState<CursorPoint>({ x: 200, y: 180 });
  const [petAction, setPetAction] = useState<PetAction>("idle");
  const [petRuntimeSnapshot, setPetRuntimeSnapshot] = useState<PetRuntimeSnapshot | null>(null);
  const [reducedMotion, setReducedMotion] = useState(() => window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false);
  const petRuntimeRef = useRef<PetBehaviorRuntime | null>(null);
  const petRuntimeRevisionRef = useRef(-1);
  const petRuntimeTransactionsRef = useRef(new Map<string, string[]>());
  const petRuntimeCommitRef = useRef<(transition: PetBehaviorTransition) => void>(() => undefined);
  const [restEnabled, setRestEnabled] = usePersistentState<boolean>("wormhole-pie.widget.restEnabled.v1", false);
  const [workMinutes, setWorkMinutes] = usePersistentState<number>("wormhole-pie.widget.workMinutes.v1", 25);
  const [restMinutes, setRestMinutes] = usePersistentState<number>("wormhole-pie.widget.restMinutes.v1", 5);
  const [nextRestAt, setNextRestAt] = usePersistentState<number>("wormhole-pie.widget.nextRestAt.v1", Date.now() + workMinutes * 60_000);
  const [restActive, setRestActive] = useState(false);
  const [restStage, setRestStage] = useState<RestStage>("preparing");
  const [restSeconds, setRestSeconds] = useState(restMinutes * 60);
  const restTransitionRef = useRef(false);
  const restExitInFlightRef = useRef(false);
  const restExitRequestedRef = useRef(false);
  const [petVisible, setPetVisible] = usePersistentState<boolean>("wormhole-pie.widget.petVisible.v1", true);
  const [petLayer, setPetLayerState] = usePersistentState<PetLayer>("wormhole-pie.widget.petLayer.v1", "normal");
  const [focusMode, setFocusMode] = usePersistentState<boolean>("wormhole-pie.widget.focusMode.v1", false);
  const [petName, setPetName] = usePersistentState<string>(petStorageKeys.name, "派派");
  const [petSpecies, setPetSpecies] = usePersistentState<PetSpecies>(petStorageKeys.species, "star-cat");
  const [petTheme, setPetTheme] = usePersistentState<PetTheme>(petStorageKeys.theme, "lilac");
  const [evolution, setEvolution] = usePersistentState<EvolutionPath>(petStorageKeys.evolution, "companion");
  const [autoEvolution, setAutoEvolution] = usePersistentState<boolean>(petStorageKeys.autoEvolution, true);
  const [evolutionMetrics, setEvolutionMetrics] = usePersistentState<PetEvolutionMetrics>(petStorageKeys.evolutionMetrics, defaultPetEvolutionMetrics, mergePetEvolutionMetrics);
  const evolutionState = useMemo(() => calculatePetEvolution(evolutionMetrics), [evolutionMetrics]);
  const [manualEvolutionStage, setManualEvolutionStage] = usePersistentState<EvolutionStage>(petStorageKeys.manualEvolutionStage, evolutionState.stage);
  const effectiveEvolution = autoEvolution ? evolutionState.path : evolution;
  const effectiveEvolutionStage = autoEvolution ? evolutionState.stage : manualEvolutionStage;
  const [petMotionMode, setPetMotionMode] = usePersistentState<PetMotionMode>(petStorageKeys.motionMode, "gentle");
  const [dialogueEnabled, setDialogueEnabled] = usePersistentState<boolean>(petStorageKeys.dialogueEnabled, true);
  const [voiceEnabled, setVoiceEnabled] = usePersistentState<boolean>(petStorageKeys.voiceEnabled, false);
  const [lastFeed, setLastFeed] = useState<PetFeedResult | null>(null);
  const [organizePhase, setOrganizePhase] = useState<OrganizePhase>("idle");
  const [organizeResult, setOrganizeResult] = useState<DesktopOrganizeResult | null>(null);
  const [organizeError, setOrganizeError] = useState("");
  const [canUndoOrganize, setCanUndoOrganize] = useState(false);
  const [latestUndoTouchesPublicDesktop, setLatestUndoTouchesPublicDesktop] = useState(false);
  const [desktopIconState, setDesktopIconState] = useState<DesktopIconState>({ supported: true, hidden: false, publicDesktopCount: 0 });
  const suppressFileNoticeUntil = useRef(0);
  const expectedPetVisibilityEvents = useRef<boolean[]>([]);
  const agentTaskRunning = agentBusy || isAgentTaskActive(agentTaskStatus);
  const runningAgentConnector = isAgentTaskActive(agentTaskStatus) ? agentTaskStatus?.connectorId ?? null : agentBusyConnector;
  const activeConnectorBusy = agentTaskRunning && runningAgentConnector === agentConnector;
  const agentTaskTiming = useAgentTaskTiming(activeConnectorBusy, agentTaskStatus);
  const activeAgentWorkspace = activeDialogueSession.workspace.trim();
  const updateActiveAgentWorkspace = useCallback((workspace: string) => {
    updateDialogueSession(agentConnectorRef.current, { workspace });
  }, [updateDialogueSession]);

  useEffect(() => {
    const today = localDateKey();
    setEvolutionMetrics((current) => current.activeDays.includes(today) ? current : {
      ...current,
      activeDays: [...current.activeDays, today].slice(-365),
    });
  }, [setEvolutionMetrics]);

  useEffect(() => {
    setSocialAccountsLoading(true);
    void listSocialAccounts()
      .then(setSocialAccounts)
      .catch(console.error)
      .finally(() => setSocialAccountsLoading(false));
  }, []);

  const todayTodos = useMemo(() => todos.filter((todo) => (todo.date ?? todayKey) === todayKey), [todayKey, todos]);
  const pendingTodos = useMemo(() => todayTodos.filter((todo) => todo.status !== "done").length, [todayTodos]);
  const pendingIdeas = useMemo(() => ideas.filter((idea) => idea.status === "pending").length, [ideas]);
  const unread = useMemo(() => notices.filter((notice) => !notice.read).length, [notices]);
  const excludedNameKeys = useMemo(() => new Set(organizeExclusions.map((item) => item.nameKey)), [organizeExclusions]);
  const pendingDesktopCount = useMemo(() => files.filter((file) => !isOrganizedFile(file) && !isWormholeShortcut(file) && !excludedNameKeys.has(file.name.trim().toLocaleLowerCase())).length, [excludedNameKeys, files]);
  const libraryFileCount = useMemo(() => files.filter(isOrganizedFile).length, [files]);
  const commonPrograms = useMemo(() => {
    const byPath = new Map<string, ProgramEntry>();
    favoritePrograms.forEach((program) => {
      const key = normalizePathKey(program.path);
      byPath.set(key, { name: program.name, path: program.path, source: "favorite" });
    });
    programs.forEach((program) => {
      const key = normalizePathKey(program.path);
      if (!byPath.has(key)) byPath.set(key, program);
    });
    return [...byPath.values()];
  }, [favoritePrograms, programs]);
  const homeMotionTimer = useRef(0);

  const refreshAgentConnectors = useCallback(async () => {
    setAgentConnectorsLoading(true);
    setAgentConnectorScanMessage("");
    try {
      const items = await listAgentConnectors(true);
      setAgentConnectors(items);
      const readyConnectors = items.filter(isAgentConnectorReady);
      const currentReady = agentConnector !== "local" ? readyConnectors.find((item) => item.id === agentConnector) : undefined;
      const selectedConnector = currentReady ?? readyConnectors[0];
      setAgentConnector(selectedConnector?.id ?? "local");
      if (selectedConnector) {
        setAgentConnectorScanMessage(`识别完成：${readyConnectors.length} 个本机命令可运行，已选择 ${selectedConnector.name}。`);
      } else if (items.some((item) => item.detected)) {
        setAgentConnectorScanMessage("识别完成：发现本机 Agent，但命令当前不可运行。");
      } else {
        setAgentConnectorScanMessage("识别完成：暂未发现 Claude、Hermes 或 Codex CLI。");
      }
      if (isTauri()) void publishDialogueState(dialogueStateRef.current).catch(console.error);
    } catch (error) {
      console.error(error);
      setAgentConnectors([]);
      setAgentConnectorScanMessage("识别没有完成，请稍后再试。");
    } finally {
      setAgentConnectorsLoading(false);
    }
  }, [agentConnector, setAgentConnector]);

  useEffect(() => {
    let cancelled = false;
    void Promise.all([listAgentConnectors(), getAgentDefaultWorkspace()])
      .then(([items, defaultWorkspace]) => {
        if (cancelled) return;
        setAgentConnectors(items);
        const currentConnector = agentConnectorRef.current;
        const nextConnector = currentConnector !== "local" && !items.some((item) => item.id === currentConnector && isAgentConnectorReady(item)) ? "local" : currentConnector;
        if (nextConnector !== currentConnector) {
          agentConnectorRef.current = nextConnector;
          setAgentConnector(nextConnector);
        }
        const sessionWorkspace = dialogueSessionsRef.current[nextConnector].workspace.trim();
        if (!sessionWorkspace) updateDialogueSession(nextConnector, { workspace: legacyAgentWorkspace.trim() || defaultWorkspace });
        if (legacyAgentWorkspace.trim()) setLegacyAgentWorkspace("");
      })
      .catch(console.error);
    return () => { cancelled = true; };
  }, [legacyAgentWorkspace, setAgentConnector, setLegacyAgentWorkspace, updateDialogueSession]);

  useEffect(() => {
    let cleanup: undefined | (() => void);
    let cancelled = false;
    const acceptStatus = (next: AgentTaskStatus) => {
      if (cancelled) return;
      const active = isAgentTaskActive(next);
      setAgentTaskStatus((current) => !current || next.updatedAt >= current.updatedAt ? next : current);
      setAgentBusy(active);
      setAgentBusyConnector(active ? next.connectorId : null);
      updateDialogueSession(next.connectorId, { busy: active, activeTaskId: active ? next.taskId : null });
    };
    void getAgentTaskStatus().then((next) => {
      if (next) acceptStatus(next);
      else {
        setAgentBusy(false);
        setAgentBusyConnector(null);
        (["codex", "claude", "hermes"] as const).forEach((connectorId) => updateDialogueSession(connectorId, { busy: false, activeTaskId: null }));
      }
    }).catch((error) => {
      console.error(error);
      if (cancelled) return;
      setAgentTaskStatus(null);
      setAgentBusy(false);
      setAgentBusyConnector(null);
      (["codex", "claude", "hermes"] as const).forEach((connectorId) => updateDialogueSession(connectorId, { busy: false, activeTaskId: null }));
    });
    void subscribeToAgentTaskStatus(acceptStatus).then((unlisten) => {
      if (cancelled) unlisten();
      else cleanup = unlisten;
    }).catch(console.error);
    return () => {
      cancelled = true;
      cleanup?.();
    };
  }, [updateDialogueSession]);

  useEffect(() => {
    if (!agentTaskRunning) return;
    let cancelled = false;
    const poll = () => {
      void getAgentTaskStatus().then((next) => {
        if (cancelled || !next) return;
        const active = isAgentTaskActive(next);
        setAgentTaskStatus((current) => !current || next.updatedAt >= current.updatedAt ? next : current);
        setAgentBusy(active);
        setAgentBusyConnector(active ? next.connectorId : null);
        updateDialogueSession(next.connectorId, { busy: active, activeTaskId: active ? next.taskId : null });
      }).catch(console.error);
    };
    const timer = window.setInterval(poll, 4_000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [agentTaskRunning, updateDialogueSession]);

  useEffect(() => {
    lastDeliveredAgentTaskIdRef.current = lastDeliveredAgentTaskId;
  }, [lastDeliveredAgentTaskId]);

  useEffect(() => () => window.clearTimeout(homeMotionTimer.current), []);

  useEffect(() => {
    const timer = window.setTimeout(() => setWindowMotion("idle"), 760);
    return () => window.clearTimeout(timer);
  }, []);

  useEffect(() => {
    let cleanup: undefined | (() => void);
    let cancelled = false;
    let idleTimer = 0;
    subscribeToMainShown(() => {
      if (cancelled) return;
      window.clearTimeout(idleTimer);
      setWindowMotion("opening");
      idleTimer = window.setTimeout(() => setWindowMotion("idle"), 760);
    }).then((unlisten) => {
      if (cancelled) unlisten();
      else cleanup = unlisten;
    }).catch(console.error);
    return () => {
      cancelled = true;
      window.clearTimeout(idleTimer);
      cleanup?.();
    };
  }, []);

  const collapseHomeSection = useCallback((id: HomeSectionId) => {
    if (homeSectionMotion || collapsedHomeSections.includes(id)) return;
    window.clearTimeout(homeMotionTimer.current);
    setHomeSectionMotion({ id, direction: "absorb" });
    homeMotionTimer.current = window.setTimeout(() => {
      setCollapsedHomeSections((current) => current.includes(id) ? current : [...current, id]);
      setHomeSectionMotion(null);
    }, 440);
  }, [collapsedHomeSections, homeSectionMotion, setCollapsedHomeSections]);

  const restoreHomeSection = useCallback((id: HomeSectionId) => {
    if (homeSectionMotion || !collapsedHomeSections.includes(id)) return;
    window.clearTimeout(homeMotionTimer.current);
    setCollapsedHomeSections((current) => current.filter((item) => item !== id));
    setHomeSectionMotion({ id, direction: "eject" });
    homeMotionTimer.current = window.setTimeout(() => setHomeSectionMotion(null), 520);
  }, [collapsedHomeSections, homeSectionMotion, setCollapsedHomeSections]);

  const addNotice = useCallback((title: string, message: string, type: Notice["type"] = "action") => {
    setNotices((current) => [{ id: makeId("notice"), title, message, type, read: false, createdAt: Date.now() }, ...current].slice(0, 100));
  }, [setNotices]);

  const loadFiles = useCallback(async (forceScan = false) => {
    setIsScanning(true);
    try {
      const loaded = forceScan ? await scanDesktopFiles() : await listDesktopFiles();
      filesRef.current = loaded;
      setFiles(loaded);
      return loaded;
    } catch (error) {
      console.error(error);
      setAssistantMessage("桌面没同步上，稍后再试一下吧。");
      return filesRef.current;
    } finally {
      setIsScanning(false);
    }
  }, []);

  const loadCommonPrograms = useCallback(async () => {
    setProgramsLoading(true);
    try {
      setPrograms(await listPrograms());
      setProgramsLoaded(true);
    } catch (error) {
      console.error(error);
      setAssistantMessage("常用程序没读取到，再刷新一次试试。");
    } finally {
      setProgramsLoading(false);
    }
  }, []);

  const selectHomeLibrary = useCallback((library: HomeLibrary) => {
    setHomeLibrary(library);
    if (library === "programs" && !programsLoaded && !programsLoading) void loadCommonPrograms();
  }, [loadCommonPrograms, programsLoaded, programsLoading]);

  const selectFileFilter = useCallback((filter: FileFilter) => {
    setFileFilter(filter);
    if (filter === "程序" && !programsLoaded && !programsLoading) void loadCommonPrograms();
  }, [loadCommonPrograms, programsLoaded, programsLoading]);

  const loadExclusions = useCallback(async () => {
    setExclusionsLoading(true);
    try {
      setOrganizeExclusions(await listOrganizeExclusions());
    } catch (error) {
      console.error(error);
      setAssistantMessage("忽略名单暂时没读取到。");
    } finally {
      setExclusionsLoading(false);
    }
  }, []);

  const syncPetVisibility = useCallback(async (visible: boolean) => {
    expectedPetVisibilityEvents.current.push(visible);
    try {
      await setPetVisibility(visible);
    } catch (error) {
      const index = expectedPetVisibilityEvents.current.lastIndexOf(visible);
      if (index >= 0) expectedPetVisibilityEvents.current.splice(index, 1);
      throw error;
    }
  }, []);

  const dispatchPetRuntimeSignal = useCallback((event: PetSignalEvent) => {
    const runtime = petRuntimeRef.current;
    if (!runtime) return;
    if (event.signal === "confirmed_file_drop" && event.transactionId) {
      const paths = Array.isArray(event.payload?.paths)
        ? event.payload.paths.filter((path): path is string => typeof path === "string")
        : [];
      if (paths.length) petRuntimeTransactionsRef.current.set(event.transactionId, paths);
    }
    petRuntimeCommitRef.current(runtime.receiveSignal(event));
  }, []);

  const commitPetRuntimeTransition = useCallback((transition: PetBehaviorTransition) => {
    const snapshot = transition.snapshot;
    if (snapshot.revision > petRuntimeRevisionRef.current || snapshot.petId !== petRuntimeSnapshot?.petId) {
      petRuntimeRevisionRef.current = snapshot.revision;
      setPetRuntimeSnapshot(snapshot);
      setPetAction(snapshot.actionId as PetSpriteAction);
      void publishPetRuntimeState(snapshot).catch(console.error);
    }

    transition.effects.forEach((effect) => {
      if (effect.type !== "marker" || effect.marker.marker !== "commitToRecycleBin") return;
      const transactionId = effect.marker.transactionId;
      const paths = transactionId ? petRuntimeTransactionsRef.current.get(transactionId) : undefined;
      if (transactionId) petRuntimeTransactionsRef.current.delete(transactionId);
      if (!transactionId || !paths?.length) {
        dispatchPetRuntimeSignal(createPetSignal("recycle_failed", "main", snapshot.petId, {
          transactionId,
          source: "file",
          essentialInQuietMode: true,
          payload: { reason: "missing_confirmed_paths" },
        }));
        return;
      }
      void feedFilesToPet(paths)
        .then((result) => {
          setAssistantMessage(result.warning ? "有些文件没能吃掉，请检查回收站。" : `${result.count} 个项目已经送进回收站。`);
          dispatchPetRuntimeSignal(createPetSignal(result.warning ? "recycle_failed" : "recycle_completed", "main", snapshot.petId, {
            transactionId,
            source: "file",
            essentialInQuietMode: true,
            payload: { count: result.count, failedCount: result.failedCount, warning: result.warning },
          }));
        })
        .catch((error) => {
          console.error(error);
          setAssistantMessage("这个文件没有被删除，换一个再试吧。");
          dispatchPetRuntimeSignal(createPetSignal("recycle_failed", "main", snapshot.petId, {
            transactionId,
            source: "file",
            essentialInQuietMode: true,
            payload: { reason: error instanceof Error ? error.message : String(error) },
          }));
        });
    });
  }, [dispatchPetRuntimeSignal, petRuntimeSnapshot?.petId]);

  useEffect(() => {
    petRuntimeCommitRef.current = commitPetRuntimeTransition;
  }, [commitPetRuntimeTransition]);

  useEffect(() => {
    let cancelled = false;
    let runtime: PetBehaviorRuntime | null = null;
    void loadPetManifests().then((index) => {
      if (cancelled) return;
      const petId = getPetAssetId(petSpecies, index);
      const pet = index.petsById.get(petId);
      runtime = new PetBehaviorRuntime({
        petId,
        initialAtEpochMs: Date.now(),
        behaviorMode: petMotionMode,
        reducedMotion,
        layer: petLayer,
        actionWeightModifiers: pet?.actionWeightModifiers,
        actions: {
          version: index.actionsManifest.version,
          defaults: {
            frames: index.actionsManifest.defaultFrames,
            sequence: index.actionsManifest.defaultSequence,
          },
          actions: index.actionsManifest.actions,
        } as BehaviorActionsManifest,
        stateMachine: {
          version: index.stateMachine.version,
          default: index.stateMachine.defaultAction,
          signals: index.stateMachine.signals,
          autonomy: index.stateMachine.autonomy,
          needs: index.stateMachine.needs,
        } as BehaviorStateMachineManifest,
      });
      petRuntimeRef.current = runtime;
      petRuntimeRevisionRef.current = -1;
      const snapshot = runtime.getSnapshot();
      petRuntimeRevisionRef.current = snapshot.revision;
      setPetRuntimeSnapshot(snapshot);
      setPetAction(snapshot.actionId as PetSpriteAction);
      void publishPetRuntimeState(snapshot).catch(console.error);
    }).catch(console.error);
    return () => {
      cancelled = true;
      if (runtime && petRuntimeRef.current === runtime) petRuntimeRef.current = null;
    };
  }, [petSpecies]);

  useEffect(() => {
    const media = window.matchMedia?.("(prefers-reduced-motion: reduce)");
    if (!media) return;
    const update = () => setReducedMotion(media.matches);
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, []);

  useEffect(() => {
    const runtime = petRuntimeRef.current;
    if (!runtime) return;
    petRuntimeCommitRef.current(runtime.dispatch({
      type: "context",
      atEpochMs: Date.now(),
      behaviorMode: petMotionMode,
      reducedMotion,
      layer: petLayer,
      constraints: {
        doNotDisturb: focusMode,
        voiceCapturing: isListening,
      },
    }));
  }, [focusMode, isListening, petLayer, petMotionMode, reducedMotion]);

  useEffect(() => {
    const interval = window.setInterval(() => {
      const runtime = petRuntimeRef.current;
      if (runtime) petRuntimeCommitRef.current(runtime.tick(Date.now(), new Date().getHours()));
    }, 1000);
    return () => window.clearInterval(interval);
  }, []);

  useEffect(() => {
    let cancelled = false;
    let cleanups: Array<() => void> = [];
    Promise.all([
      subscribeToPetRuntimeSignal(dispatchPetRuntimeSignal),
      subscribeToPetRuntimeMarker((marker) => {
        const runtime = petRuntimeRef.current;
        if (runtime) petRuntimeCommitRef.current(runtime.onMarker(marker.actionInstanceId, marker.marker, marker.frame, marker.atEpochMs));
      }),
      subscribeToPetRuntimeAnimationEnd((event) => {
        const runtime = petRuntimeRef.current;
        if (runtime) petRuntimeCommitRef.current(runtime.onAnimationEnd(event.actionInstanceId, event.atEpochMs));
      }),
      subscribeToPetRuntimeLoopBoundary((event) => {
        const runtime = petRuntimeRef.current;
        if (runtime) petRuntimeCommitRef.current(runtime.onLoopBoundary(event.actionInstanceId, event.atEpochMs, event.loopCount));
      }),
      subscribeToPetRuntimeStateRequests(() => {
        const snapshot = petRuntimeRef.current?.getSnapshot();
        if (snapshot) void publishPetRuntimeState(snapshot).catch(console.error);
      }),
    ]).then((listeners) => {
      if (cancelled) listeners.forEach((cleanup) => cleanup());
      else cleanups = listeners;
    }).catch(console.error);
    return () => {
      cancelled = true;
      cleanups.forEach((cleanup) => cleanup());
    };
  }, [dispatchPetRuntimeSignal]);

  const sendMainPetSignal = useCallback((signal: string, options: Parameters<typeof createPetSignal>[3] = {}) => {
    const petId = petRuntimeRef.current?.getSnapshot().petId ?? getPetAssetId(petSpecies);
    dispatchPetRuntimeSignal(createPetSignal(signal, "main", petId, options));
  }, [dispatchPetRuntimeSignal, petSpecies]);

  const handleMainPetMarker = useCallback((marker: PetMarkerEvent) => {
    const runtime = petRuntimeRef.current;
    if (runtime) petRuntimeCommitRef.current(runtime.onMarker(marker.actionInstanceId, marker.marker, marker.frame, marker.atEpochMs));
  }, []);

  const handleMainPetLoopBoundary = useCallback((event: PetAnimationLoopBoundaryEvent) => {
    const runtime = petRuntimeRef.current;
    if (runtime) petRuntimeCommitRef.current(runtime.onLoopBoundary(event.actionInstanceId, event.atEpochMs, event.loopCount));
  }, []);

  const handleMainPetAnimationEnd = useCallback(() => {
    const runtime = petRuntimeRef.current;
    const snapshot = runtime?.getSnapshot();
    if (runtime && snapshot) petRuntimeCommitRef.current(runtime.onAnimationEnd(snapshot.actionInstanceId, Date.now()));
  }, []);

  const deliverRecoveredAgentResult = useCallback((result: AgentTaskResult) => {
    if (agentInvokeOwnedRef.current || !result.taskId || result.taskId === lastDeliveredAgentTaskIdRef.current) return;
    const targetSession = dialogueSessionsRef.current[result.connectorId];
    if (!targetSession || result.appSessionId !== targetSession.appSessionId) return;
    lastDeliveredAgentTaskIdRef.current = result.taskId;
    setLastDeliveredAgentTaskId(result.taskId);
    setAgentBusy(false);
    setAgentBusyConnector(null);
    const connectorName = agentConnectors.find((connector) => connector.id === result.connectorId)?.name ?? result.connectorId;
    const output = result.output.trim();
    if (result.success) {
      updateDialogueSession(result.connectorId, (current) => ({
        ...appendDialogueAssistantMessage(current, output || "任务完成啦。", result.files ?? [], result.taskId),
        providerSessionId: result.failureCode === "session_not_found" ? null : result.providerSessionId ?? current.providerSessionId,
        busy: false,
        activeTaskId: null,
      }));
      sendMainPetSignal("task_completed", { source: "agent", payload: { connectorId: result.connectorId, durationMs: result.durationMs, recovered: true } });
      addNotice(`${connectorName} 已完成任务`, "结果已经重新送回宠物对话。", "action");
    } else if (result.cancelled) {
      updateDialogueSession(result.connectorId, (current) => ({
        ...appendDialogueAssistantMessage(current, "任务已经停下来了。", result.files ?? [], result.taskId),
        providerSessionId: result.failureCode === "session_not_found" ? null : result.providerSessionId ?? current.providerSessionId,
        busy: false,
        activeTaskId: null,
      }));
      sendMainPetSignal("task_failed", { source: "agent", payload: { connectorId: result.connectorId, cancelled: true, recovered: true } });
    } else {
      updateDialogueSession(result.connectorId, (current) => ({
        ...appendDialogueAssistantMessage(current, output || (result.timedOut ? "任务达到长时安全上限，已经停止。" : "任务没有完成，请稍后再试。"), result.files ?? [], result.taskId),
        providerSessionId: result.failureCode === "session_not_found" ? null : result.providerSessionId ?? current.providerSessionId,
        busy: false,
        activeTaskId: null,
      }));
      sendMainPetSignal("task_failed", { source: "agent", payload: { connectorId: result.connectorId, timedOut: result.timedOut, recovered: true } });
    }
  }, [addNotice, agentConnectors, sendMainPetSignal, setLastDeliveredAgentTaskId, updateDialogueSession]);

  useEffect(() => {
    let cleanup: undefined | (() => void);
    let cancelled = false;
    const acceptResult = (result: AgentTaskResult) => {
      if (!cancelled) deliverRecoveredAgentResult(result);
    };
    void getAgentTaskResult().then((result) => {
      if (result) acceptResult(result);
    }).catch(console.error);
    void subscribeToAgentTaskResult(acceptResult).then((unlisten) => {
      if (cancelled) unlisten();
      else cleanup = unlisten;
    }).catch(console.error);
    return () => {
      cancelled = true;
      cleanup?.();
    };
  }, [deliverRecoveredAgentResult]);

  useEffect(() => {
    setTodos((current) => current.some((todo) => !todo.date)
      ? current.map((todo) => todo.date ? todo : { ...todo, date: todayKey })
      : current);
    const timer = window.setInterval(() => setTodayKey(localDateKey()), 60_000);
    return () => window.clearInterval(timer);
  }, [setTodos, todayKey]);

  const openExclusions = useCallback(() => {
    setMenuOpen(false);
    setExclusionsOpen(true);
    void loadExclusions();
  }, [loadExclusions]);

  const handleLaunchProgram = useCallback(async (program: ProgramEntry) => {
    try {
      await launchProgram(program.path);
      setAssistantMessage(`已经打开「${program.name}」。`);
    } catch (error) {
      console.error(error);
      const removalHint = program.source === "favorite" ? " 可能已被移动，可以点右侧 × 移除。" : " 请刷新程序列表再试一次。";
      setAssistantMessage(`没能打开「${program.name}」。${removalHint}`);
      addNotice("程序没有打开", `${program.name} 的位置可能已经变化。`, "action");
    }
  }, [addNotice]);

  const handleRemoveFavoriteProgram = useCallback((program: ProgramEntry) => {
    const key = normalizePathKey(program.path);
    setFavoritePrograms((current) => current.filter((item) => normalizePathKey(item.path) !== key));
    setAssistantMessage(`已从常用程序移除「${program.name}」。`);
  }, [setFavoritePrograms]);

  const remapFavoriteProgramPaths = useCallback((
    items: NonNullable<DesktopOrganizeResult["items"]>,
    destination: "original" | "organized",
  ) => {
    if (!items.length) return;
    const mappings = new Map(items.map((item) => [
      normalizePathKey(destination === "original" ? item.organizedPath : item.originalPath),
      destination === "original" ? item.originalPath : item.organizedPath,
    ]));
    setFavoritePrograms((current) => current.map((program) => {
      const nextPath = mappings.get(normalizePathKey(program.path));
      return nextPath ? { ...program, path: nextPath } : program;
    }));
  }, [setFavoritePrograms]);

  const handleRemoveExclusion = useCallback(async (nameKey: string) => {
    try {
      await removeOrganizeExclusion(nameKey);
      setOrganizeExclusions((current) => current.filter((item) => item.nameKey !== nameKey));
    } catch (error) {
      console.error(error);
      setAssistantMessage("这条忽略规则没有移除成功。");
    }
  }, []);

  const handleToggleDesktopIcons = useCallback(async () => {
    try {
      const next = await setDesktopIconsHidden(!desktopIconState.hidden);
      setDesktopIconState(next);
      setAssistantMessage(next.hidden ? "桌面图标已隐藏，需要时可以随时恢复。" : "桌面图标已经恢复显示。");
    } catch (error) {
      console.error(error);
      setAssistantMessage("桌面图标状态没有改动成功。");
    }
  }, [desktopIconState.hidden]);

  const handleOrganizeDesktop = useCallback(async (includePublicDesktop = false, confirmationToken?: string) => {
    suppressFileNoticeUntil.current = Date.now() + 2500;
    setLastUserMessage(includePublicDesktop ? "收纳公共桌面" : "一键整理个人桌面");
    setDialogueOpen(false);
    setOrganizeError("");
    setOrganizePhase("organizing");
    try {
      const result = await organizeDesktop(includePublicDesktop, confirmationToken);
      setOrganizeResult(result);
      remapFavoriteProgramPaths(result.items ?? [], "organized");
      const [, , organizeState] = await Promise.all([loadFiles(), loadCommonPrograms(), getOrganizeState()]);
      setCanUndoOrganize(organizeState.canUndo);
      setLatestUndoTouchesPublicDesktop(organizeState.latestBatchTouchesPublicDesktop);
      if (typeof result.publicDesktopCount === "number") {
        setDesktopIconState((current) => ({ ...current, publicDesktopCount: result.publicDesktopCount ?? current.publicDesktopCount }));
      }
      const skipped = result.skippedCount;
      const newMoved = result.newMovedCount ?? result.movedCount;
      if (newMoved > 0) {
        setEvolutionMetrics((current) => ({ ...current, organizedFiles: current.organizedFiles + newMoved }));
      }
      const migrated = result.migratedCount ?? 0;
      setAssistantMessage(newMoved
        ? `${includePublicDesktop ? "公共桌面" : "个人桌面"}新增整理了 ${newMoved} 个项目，跳过 ${skipped} 个。`
        : migrated
          ? `旧整理库的 ${migrated} 个项目已迁入资料库。`
          : `没有新增项目，跳过 ${skipped} 个。`);
      addNotice(
        includePublicDesktop ? "公共桌面收纳完成" : "个人桌面整理完成",
        result.movedCount ? `${result.movedCount} 个项目已移入“虫洞派资料库”。` : "桌面没有需要移动的项目。",
        "file",
      );
      setOrganizePhase("done");
    } catch (error) {
      console.error(error);
      const message = error instanceof Error ? error.message : String(error);
      setOrganizeError(message);
      setAssistantMessage(includePublicDesktop ? "公共桌面没有改动；管理员批准可能被取消或移动未通过安全检查。" : "这次没整理好，桌面没有被半途改乱。" );
      setOrganizePhase("error");
    }
  }, [addNotice, loadCommonPrograms, loadFiles, remapFavoriteProgramPaths, setEvolutionMetrics]);

  const handleUndoDesktopOrganize = useCallback(async (confirmationToken?: string) => {
    suppressFileNoticeUntil.current = Date.now() + 2500;
    setDialogueOpen(false);
    setOrganizeError("");
    setOrganizePhase("undoing");
    try {
      const result = await undoDesktopOrganize(confirmationToken);
      setOrganizeResult(result);
      remapFavoriteProgramPaths(result.items ?? [], "original");
      const restoredDesktopItems = result.newMovedCount ?? result.movedCount;
      if (restoredDesktopItems > 0) {
        setEvolutionMetrics((current) => ({
          ...current,
          organizedFiles: Math.max(0, current.organizedFiles - restoredDesktopItems),
        }));
      }
      const [, , organizeState] = await Promise.all([loadFiles(), loadCommonPrograms(), getOrganizeState()]);
      setCanUndoOrganize(organizeState.canUndo);
      setLatestUndoTouchesPublicDesktop(organizeState.latestBatchTouchesPublicDesktop);
      setAssistantMessage(result.skippedCount
        ? `已放回 ${result.movedCount} 个项目，另有 ${result.skippedCount} 个因位置变化没有恢复。`
        : `放回来啦，${result.movedCount} 个项目已经回到桌面。`);
      addNotice(
        result.skippedCount ? "撤销整理未完全恢复" : "已撤销桌面整理",
        result.skippedCount
          ? `${result.skippedCount} 个项目未能恢复，请查看结果详情。`
          : `${result.movedCount} 个项目已恢复。`,
        "file",
      );
      setOrganizePhase("undone");
    } catch (error) {
      console.error(error);
      const message = error instanceof Error ? error.message : String(error);
      setOrganizeError(message);
      setAssistantMessage("这次没有放回来，整理后的文件仍然安全保留。" );
      setOrganizePhase("error");
    }
  }, [addNotice, loadCommonPrograms, loadFiles, remapFavoriteProgramPaths, setEvolutionMetrics]);

  const handleUndoFromShortcut = useCallback(async () => {
    if (!latestUndoTouchesPublicDesktop) {
      await handleUndoDesktopOrganize();
      return;
    }
    const confirmed = window.confirm(locale === "zh-CN"
      ? "最近一次整理包含公共桌面项目。放回后会影响所有 Windows 用户，并请求管理员批准。是否继续？"
      : "The latest batch contains shared desktop items. Restoring them affects every Windows user and requires administrator approval. Continue?");
    if (!confirmed) return;
    try {
      const token = await requestPublicDesktopConfirmation("undo");
      await handleUndoDesktopOrganize(token);
    } catch (error) {
      console.error(error);
      setAssistantMessage("公共桌面撤销确认没有完成，请重新确认。");
    }
  }, [handleUndoDesktopOrganize, latestUndoTouchesPublicDesktop, locale]);

  const handleOrganizeReview = useCallback(async (excludedMoveIds: string[], favoriteMoveIds: string[], confirmationToken?: string) => {
    suppressFileNoticeUntil.current = Date.now() + 2500;
    if (!organizeResult?.batchId) throw new Error("这批整理记录无法复查，请重新整理一次。");
    const review = await reviewDesktopOrganize(organizeResult.batchId, excludedMoveIds, confirmationToken);
    if (review.restoredCount > 0) {
      setEvolutionMetrics((current) => ({
        ...current,
        organizedFiles: Math.max(0, current.organizedFiles - review.restoredCount),
      }));
    }
    const excluded = new Set(excludedMoveIds);
    const favorites = new Set(favoriteMoveIds);
    const items = organizeResult.items ?? [];
    const favoriteItems = items.filter((item) => favorites.has(item.moveId));
    if (favoriteItems.length) {
      const restoredMoveIds = new Set(review.restoredMoveIds);
      setFavoritePrograms((current) => {
        const next = new Map(current.map((item) => [normalizePathKey(item.path), item]));
        favoriteItems.forEach((item) => {
          const path = excluded.has(item.moveId) && restoredMoveIds.has(item.moveId) ? item.originalPath : item.organizedPath;
          next.set(normalizePathKey(path), { name: item.name, path, addedAt: Date.now() });
        });
        return [...next.values()];
      });
    }
    const [, , , organizeState] = await Promise.all([loadFiles(), loadCommonPrograms(), loadExclusions(), getOrganizeState()]);
    setCanUndoOrganize(organizeState.canUndo);
    setLatestUndoTouchesPublicDesktop(organizeState.latestBatchTouchesPublicDesktop);
    setAssistantMessage(review.conflictCount
      ? `已保存复查，但有 ${review.conflictCount} 个项目因重名或位置变化没有放回。`
      : review.rememberedCount
        ? `已放回 ${review.restoredCount} 个项目，以后会留在桌面。`
        : "复查已保存，当前分类保持不变。");
    addNotice(
      review.conflictCount ? "整理复查有未处理项目" : "整理复查已保存",
      review.conflictCount
        ? `${review.conflictCount} 个项目未能放回，请检查桌面和资料库。`
        : `${review.rememberedCount} 个项目已加入忽略名单。`,
      "file",
    );
    return review;
  }, [addNotice, loadCommonPrograms, loadExclusions, loadFiles, organizeResult, setEvolutionMetrics, setFavoritePrograms]);

  useEffect(() => {
    let cleanup: undefined | (() => void);
    let cancelled = false;
    let refreshTimer = 0;
    void loadCommonPrograms();
    Promise.all([
      loadFiles(),
      startDesktopWatcher(),
      loadExclusions(),
      getDesktopIconState().then(setDesktopIconState),
      getOrganizeState().then((state) => {
        setCanUndoOrganize(state.canUndo);
        setLatestUndoTouchesPublicDesktop(state.latestBatchTouchesPublicDesktop);
      }),
    ]).catch(console.error);
    subscribeToFileChanges(() => {
      if (cancelled) return;
      window.clearTimeout(refreshTimer);
      refreshTimer = window.setTimeout(() => {
        if (cancelled) return;
        const previousFiles = filesRef.current;
        const previousPaths = new Set(previousFiles.map((file) => normalizePathKey(file.path)));
        void loadFiles().then((loaded) => {
          if (cancelled) return;
          const nextPaths = new Set(loaded.map((file) => normalizePathKey(file.path)));
          const addedCount = [...nextPaths].filter((path) => !previousPaths.has(path)).length;
          const removedCount = [...previousPaths].filter((path) => !nextPaths.has(path)).length;
          const launchableChanged = loaded.some((file) => !previousPaths.has(normalizePathKey(file.path)) && isDirectlyLaunchableFile(file))
            || previousFiles.some((file) => !nextPaths.has(normalizePathKey(file.path)) && isDirectlyLaunchableFile(file));
          if (launchableChanged) void loadCommonPrograms();
          if (Date.now() < suppressFileNoticeUntil.current) return;
          if (addedCount && !removedCount) addNotice("发现新文件", `${addedCount} 个新项目已经放进组件。`, "file");
          else if (removedCount && !addedCount) addNotice("桌面项目已移除", `${removedCount} 个项目已不在桌面或资料库。`, "file");
          else if (addedCount || removedCount) addNotice("桌面内容已更新", "项目名称或位置发生了变化。", "file");
        });
      }, 280);
    }).then((unlisten) => {
      if (cancelled) unlisten();
      else cleanup = unlisten;
    });
    return () => {
      cancelled = true;
      window.clearTimeout(refreshTimer);
      cleanup?.();
    };
  }, [addNotice, loadCommonPrograms, loadExclusions, loadFiles]);

  useEffect(() => {
    let cancelled = false;
    let cleanups: Array<() => void> = [];
    Promise.all([
      subscribeToOpenDialogue(() => {
        setMenuOpen(false);
        if (!dialogueEnabled) {
          return;
        }
        if (isTauri()) {
          void showPetDialogue()
            .then(() => publishDialogueState(dialogueStateRef.current))
            .catch(console.error);
        } else {
          setDialogueOpen(true);
        }
      }),
      subscribeToOpenPetSettings(() => {
        setPetSettingsOpen(true);
        setMenuOpen(false);
      }),
      subscribeToPetLayerChanged((layer) => setPetLayerState(layer)),
      subscribeToPetVisibilityChanged((visible) => {
        if (expectedPetVisibilityEvents.current[0] === visible) {
          expectedPetVisibilityEvents.current.shift();
          return;
        }
        setPetVisible(visible);
      }),
    ]).then((listeners) => {
      if (cancelled) listeners.forEach((cleanup) => cleanup());
      else cleanups = listeners;
    }).catch(console.error);
    return () => {
      cancelled = true;
      cleanups.forEach((cleanup) => cleanup());
    };
  }, [dialogueEnabled, setPetLayerState, setPetVisible]);

  useEffect(() => {
    const state = {
      userMessage: lastUserMessage,
      reply: assistantMessage || "想让我做什么？",
      listening: isListening,
      busy: activeConnectorBusy,
      connectorId: agentConnector,
      resultFiles: agentResultFiles,
      sessions: dialogueSessions,
    };
    dialogueStateRef.current = state;
    if (isTauri()) void publishDialogueState(state).catch(console.error);
  }, [activeConnectorBusy, agentConnector, agentResultFiles, assistantMessage, dialogueSessions, isListening, lastUserMessage]);

  useEffect(() => {
    if (isTauri()) {
      const interval = window.setInterval(() => {
        void getCursorPosition().then(setCursor).catch(() => undefined);
      }, 120);
      return () => window.clearInterval(interval);
    }
    const onPointerMove = (event: PointerEvent) => setCursor({ x: event.clientX, y: event.clientY });
    window.addEventListener("pointermove", onPointerMove, { passive: true });
    return () => window.removeEventListener("pointermove", onPointerMove);
  }, []);

  useEffect(() => {
    void syncPetVisibility(petVisible && !focusMode).catch(console.error);
  }, [focusMode, petVisible, syncPetVisibility]);

  useEffect(() => {
    void setPetLayer(petLayer).catch(console.error);
  }, [petLayer]);

  useEffect(() => {
    let unlistenFeed: undefined | (() => void);
    let unlistenRestore: undefined | (() => void);
    let cancelled = false;
    subscribeToPetFeed((result) => {
      suppressFileNoticeUntil.current = Date.now() + 2500;
      setLastFeed(result);
      if (result.count > 0) {
        setEvolutionMetrics((current) => ({ ...current, feedCount: current.feedCount + result.count }));
      }
      addNotice(result.warning ? "喂食未完全成功" : "宠物吃掉了文件", result.warning || `${result.names.slice(0, 2).join("、")} 已送入回收站。`, "file");
      void loadFiles();
    }).then((cleanup) => {
      if (cancelled) cleanup();
      else unlistenFeed = cleanup;
    });
    subscribeToPetRestore((result) => {
      suppressFileNoticeUntil.current = Date.now() + 2500;
      setLastFeed(null);
      if (result.count > 0) {
        setEvolutionMetrics((current) => ({
          ...current,
          feedCount: Math.max(0, current.feedCount - result.count),
        }));
      }
      addNotice("已撤销喂食", `${result.names.slice(0, 2).join("、")} 已恢复到原位置。`, "file");
      void loadFiles();
    }).then((cleanup) => {
      if (cancelled) cleanup();
      else unlistenRestore = cleanup;
    });
    return () => {
      cancelled = true;
      unlistenFeed?.();
      unlistenRestore?.();
    };
  }, [addNotice, loadFiles]);

  const openFile = useCallback(async (file: DesktopFile) => {
    try {
      await openDesktopFile(file.id);
      setAssistantMessage(`打开啦，是「${file.name}」。`);
    } catch (error) {
      console.error(error);
      const message = error instanceof Error ? error.message : String(error);
      setAssistantMessage(message.includes("二次确认")
        ? "安装包不会被自动运行，请在文件资源管理器中确认后手动打开。"
        : "这个文件好像被移走了。");
    }
  }, []);

  const openIndexedItem = useCallback(async (file: DesktopFile) => {
    if (isDirectlyLaunchableFile(file)) {
      await handleLaunchProgram({
        name: file.name,
        path: file.path,
        source: isOrganizedFile(file) ? "虫洞派资料库" : "桌面",
      });
      return;
    }
    await openFile(file);
  }, [handleLaunchProgram, openFile]);

  const openSocial = useCallback(async (platform: Platform) => {
    try {
      await openSocialPage(platform);
      setAssistantMessage(`${platformNames[platform]}页面打开啦，主人去确认发布就好。`);
      addNotice(`已打开${platformNames[platform]}`, "请在浏览器中确认发布内容。", "action");
    } catch (error) {
      console.error(error);
      setAssistantMessage("浏览器没有打开，主人再试一次吧。");
    }
  }, [addNotice]);

  const openCustomSocial = useCallback(async (shortcut: CustomSocialShortcut) => {
    try {
      await openExternalUrl(shortcut.url);
      setAssistantMessage(`${shortcut.name}已经打开啦。`);
      addNotice(`已打开${shortcut.name}`, "登录状态由系统浏览器继续保留。", "action");
    } catch (error) {
      console.error(error);
      setAssistantMessage("这个社交媒体链接没有打开，再检查一下地址吧。");
    }
  }, [addNotice]);

  const addTodo = useCallback((title: string) => {
    setTodos((current) => [...current, {
      id: makeId("todo"),
      title,
      time: "今天",
      date: localDateKey(),
      priority: "medium",
      status: "pending",
      ...inferTodoAction(title),
    }]);
    setAssistantMessage("记好啦，已经放进今天待办。");
  }, [setTodos]);

  const addIdea = useCallback((title: string, source: Idea["source"] = "manual") => {
    setIdeas((current) => [{ id: makeId("idea"), title, status: "pending", tags: ["待整理"], source }, ...current]);
    setAssistantMessage("收好啦，已经放进意见整理。");
  }, [setIdeas]);

  const setRestRuntimeContext = useCallback((active: boolean) => {
    const runtime = petRuntimeRef.current;
    if (!runtime) return;
    petRuntimeCommitRef.current(runtime.dispatch({
      type: "context",
      atEpochMs: Date.now(),
      constraints: {
        doNotDisturb: active || focusMode,
        fullscreen: active,
      },
    }));
  }, [focusMode]);

  const releaseRestPetAction = useCallback(() => {
    const runtime = petRuntimeRef.current;
    const snapshot = runtime?.getSnapshot();
    if (!runtime || !snapshot || snapshot.actionId !== "rest_reminder") return;
    petRuntimeCommitRef.current(runtime.dispatch({
      type: "terminate",
      actionInstanceId: snapshot.actionInstanceId,
      atEpochMs: Date.now(),
      force: true,
    }));
  }, []);

  const restoreRestPetVisibility = useCallback(async () => {
    const shouldShow = petVisible && !focusMode;
    let lastError: unknown;
    for (let attempt = 0; attempt < 2; attempt += 1) {
      try {
        await syncPetVisibility(shouldShow);
        return true;
      } catch (error) {
        lastError = error;
        if (attempt === 0) await new Promise((resolve) => window.setTimeout(resolve, 120));
      }
    }
    console.error(lastError);
    return false;
  }, [focusMode, petVisible, syncPetVisibility]);

  const completeRestExit = useCallback(async () => {
    if (restExitInFlightRef.current) return;
    restExitInFlightRef.current = true;
    releaseRestPetAction();
    setRestRuntimeContext(false);

    let nativeExited = false;
    let lastExitError: unknown;
    for (let attempt = 0; attempt < 2; attempt += 1) {
      try {
        await exitRestMode();
        nativeExited = true;
        break;
      } catch (error) {
        lastExitError = error;
        if (attempt === 0) await waitForStableViewport();
      }
    }
    const petRestored = await restoreRestPetVisibility();

    if (nativeExited && petRestored) {
      setRestActive(false);
      setRestStage("preparing");
      setRestSeconds(restMinutes * 60);
      setNextRestAt(Date.now() + workMinutes * 60_000);
      restExitRequestedRef.current = false;
    } else {
      if (lastExitError) console.error(lastExitError);
      setRestStage("recovery");
      setAssistantMessage(nativeExited ? "宠物还没恢复显示，再点一次返回桌面。" : "窗口还没退出全屏，再点一次我会继续恢复。");
    }
    restExitInFlightRef.current = false;
  }, [releaseRestPetAction, restMinutes, restoreRestPetVisibility, setRestRuntimeContext, workMinutes]);

  const finishRest = useCallback((nativeAlreadyExited = false) => {
    if (!restActive) return;
    restExitRequestedRef.current = true;
    if (nativeAlreadyExited) {
      void completeRestExit();
      return;
    }
    if (restStage === "preparing") return;
    setRestStage((current) => current === "exiting" ? current : "exiting");
  }, [completeRestExit, restActive, restStage]);

  const handleRestStageComplete = useCallback((completedStage: RestStage) => {
    if (completedStage === "exiting") {
      void completeRestExit();
      return;
    }
    setRestStage((current) => {
      if (current !== completedStage) return current;
      if (completedStage === "peek") return "approach";
      if (completedStage === "approach") return "settle";
      if (completedStage === "settle") return "resting";
      return current;
    });
  }, [completeRestExit]);

  const triggerRest = useCallback(async () => {
    if (restActive || restTransitionRef.current) return;
    restTransitionRef.current = true;
    restExitRequestedRef.current = false;
    setMenuOpen(false);
    setSettingsOpen(false);
    setRestSeconds(restMinutes * 60);
    setRestStage("preparing");
    setRestActive(true);
    setRestRuntimeContext(true);

    try {
      await syncPetVisibility(false);
      await enterRestMode();
      await waitForStableViewport();
      setRestStage(restExitRequestedRef.current ? "exiting" : "peek");
    } catch (error) {
      console.error(error);
      releaseRestPetAction();
      setRestRuntimeContext(false);
      let nativeRecovered = false;
      try {
        await exitRestMode();
        nativeRecovered = true;
      } catch (rollbackError) {
        console.error(rollbackError);
      }
      const petRestored = await restoreRestPetVisibility();
      if (nativeRecovered && petRestored) {
        setRestActive(false);
        setRestStage("preparing");
        setNextRestAt(Date.now() + workMinutes * 60_000);
        setAssistantMessage("休息模式刚才没有进入，窗口和宠物已经安全恢复。");
      } else {
        setRestStage("recovery");
        setAssistantMessage("休息模式没有完全恢复，请点退出按钮再试一次。");
      }
    } finally {
      restTransitionRef.current = false;
    }
  }, [releaseRestPetAction, restActive, restMinutes, restoreRestPetVisibility, setRestRuntimeContext, syncPetVisibility, workMinutes]);

  useEffect(() => {
    let cleanup: undefined | (() => void);
    let cancelled = false;
    subscribeToExitRestRequest(() => finishRest(true)).then((unlisten) => {
      if (cancelled) unlisten();
      else cleanup = unlisten;
    }).catch(console.error);
    return () => {
      cancelled = true;
      cleanup?.();
    };
  }, [finishRest]);

  useEffect(() => {
    if (!restEnabled || restActive) return;
    const interval = window.setInterval(() => {
      if (Date.now() >= nextRestAt) void triggerRest();
    }, 1000);
    return () => window.clearInterval(interval);
  }, [nextRestAt, restActive, restEnabled, triggerRest]);

  useEffect(() => {
    if (!restActive) return;
    const interval = window.setInterval(() => setRestSeconds((seconds) => Math.max(0, seconds - 1)), 1000);
    return () => window.clearInterval(interval);
  }, [restActive]);

  useEffect(() => {
    if (restActive && restSeconds === 0 && restStage !== "exiting" && restStage !== "recovery") finishRest();
  }, [finishRest, restActive, restSeconds, restStage]);

  const toggleTodo = (id: string) => {
    const target = todos.find((todo) => todo.id === id);
    if (!target) return;
    const completing = target.status !== "done";
    setTodos((current) => current.map((todo) => todo.id === id
      ? { ...todo, status: completing ? "done" : "pending" }
      : todo));
    setEvolutionMetrics((metrics) => ({
      ...metrics,
      completedTodos: Math.max(0, metrics.completedTodos + (completing ? 1 : -1)),
    }));
  };

  const handleTodoAction = (todo: Todo) => {
    if (todo.actionType === "social_publish" && todo.actionTarget && todo.actionTarget !== "unorganized") {
      setTodos((current) => current.map((item) => item.id === todo.id ? { ...item, status: "doing" } : item));
      void openSocial(todo.actionTarget);
    } else if (todo.actionType === "open_category") {
      setFileFilter("unorganized");
      setActiveTab("files");
    }
  };

  const openDialogue = () => {
    if (!dialogueEnabled) {
      setPetSettingsOpen(true);
      return;
    }
    if (isTauri()) {
      void showPetDialogue()
        .then(() => publishDialogueState(dialogueStateRef.current))
        .catch((error) => {
          console.error(error);
          setAssistantMessage("对话气泡没有弹出来，再点一次试试。");
        });
    } else {
      setDialogueOpen(true);
    }
  };

  const handleStopAgentTask = useCallback(async () => {
    if (!agentTaskRunning) return;
    const taskConnector = agentTaskStatus && isAgentTaskActive(agentTaskStatus)
      ? agentTaskStatus.connectorId
      : agentBusyConnector;
    if (!taskConnector) return;
    previewSessionAssistantMessage(taskConnector, "正在温柔停下来…");
    setAgentTaskStatus((current) => current ? { ...current, state: "cancelling", cancelRequested: true, updatedAt: Date.now() } : current);
    try {
      const activeTaskId = agentTaskStatus && isAgentTaskActive(agentTaskStatus) ? agentTaskStatus.taskId : undefined;
      const requested = await stopAgentTask(activeTaskId);
      if (!requested) recordSessionAssistantMessage(taskConnector, "当前没有可停止的 Agent 任务。");
    } catch (error) {
      console.error(error);
      recordSessionAssistantMessage(taskConnector, "暂时没能停下来，再试一次吧。");
    }
  }, [agentBusyConnector, agentTaskRunning, agentTaskStatus, previewSessionAssistantMessage, recordSessionAssistantMessage]);

  const handleOpenAgentResultFile = useCallback(async (file: AgentResultFile, resultConnector: DialogueConnectorId = agentConnectorRef.current) => {
    try {
      const sessionWorkspace = dialogueSessionsRef.current[resultConnector].workspace.trim();
      const workspace = sessionWorkspace || await getAgentDefaultWorkspace();
      if (!sessionWorkspace) updateDialogueSession(resultConnector, { workspace });
      await openAgentResultFile(file.path, workspace);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      recordSessionAssistantMessage(resultConnector, message || "这个交付文件暂时打不开。");
    }
  }, [recordSessionAssistantMessage, updateDialogueSession]);

  const handleCommand = useCallback(async ({ text: rawText, connectorId: connectorOverride, attachmentPaths }: DialogueCommand) => {
    const request = rawText.trim();
    if (!request) return;
    const selectedConnector = connectorOverride ?? agentConnector;
    const attachments = Array.isArray(attachmentPaths) ? attachmentPaths : [];
    const sessionBeforeRequest = dialogueSessionsRef.current[selectedConnector];
    if (selectedConnector !== agentConnector) {
      agentConnectorRef.current = selectedConnector;
      setAgentConnector(selectedConnector);
    }
    updateDialogueSession(selectedConnector, (current) => ({
      ...appendDialogueUserMessage(current, request),
      draft: "",
      attachmentPaths: [],
    }));
    setEvolutionMetrics((current) => ({ ...current, interactions: current.interactions + 1 }));
    if (dialogueEnabled && !isTauri()) setDialogueOpen(true);
    const text = request.replace(/[，。！？,.!?]/g, " ").replace(/\s+/g, " ");
    if (/^(?:不要|别|取消|停止|cancel\b|stop\b|never\s*mind\b|do\s+not\b|don't\b)/i.test(text)) {
      if (agentTaskRunning) await handleStopAgentTask();
      else recordSessionAssistantMessage(selectedConnector, "好，不做。");
      return;
    }
    if (selectedConnector === "local" && attachments.length) {
      recordSessionAssistantMessage(selectedConnector, "这些文件需要交给本机 Agent，请先选择 Claude、Hermes 或 Codex。");
      return;
    }
    if (selectedConnector !== "local") {
      if (agentTaskRunning) {
        recordSessionAssistantMessage(selectedConnector, "上一个任务还在处理，请稍等一下。");
        return;
      }
      let connector = agentConnectors.find((item) => item.id === selectedConnector);
      if (!connector || !isAgentConnectorReady(connector)) {
        try {
          const refreshed = await listAgentConnectors();
          setAgentConnectors(refreshed);
          connector = refreshed.find((item) => item.id === selectedConnector);
        } catch (error) {
          console.error(error);
        }
      }
      if (!connector || !isAgentConnectorReady(connector)) {
        recordSessionAssistantMessage(selectedConnector, connector?.detail || "这个本机 Agent 暂时不可用，请在设置里重新检测。");
        return;
      }
      setAgentTaskStatus(null);
      setAgentBusy(true);
      setAgentBusyConnector(selectedConnector);
      updateDialogueSession(selectedConnector, (current) => ({
        ...setDialogueSessionPreview(current, attachments.length ? `正在整理 ${attachments.length} 个附件并启动本机 Agent…` : "正在启动本机 Agent…"),
        busy: true,
        activeTaskId: "pending",
      }));
      sendMainPetSignal("request_submitted", { source: "agent", payload: { connectorId: selectedConnector, attachmentCount: attachments.length } });
      try {
        const workspace = sessionBeforeRequest.workspace.trim() || await getAgentDefaultWorkspace();
        updateDialogueSession(selectedConnector, { workspace });
        agentInvokeOwnedRef.current = true;
        const contextualTask = buildProviderTaskWithHistory(sessionBeforeRequest, request);
        const result = await runAgentTask(
          selectedConnector,
          contextualTask,
          workspace,
          attachments,
          sessionBeforeRequest.appSessionId,
          sessionBeforeRequest.providerSessionId,
        );
        lastDeliveredAgentTaskIdRef.current = result.taskId;
        setLastDeliveredAgentTaskId(result.taskId);
        const finalOutput = result.output.trim();
        if (result.success) {
          updateDialogueSession(selectedConnector, (current) => ({
            ...appendDialogueAssistantMessage(current, finalOutput || "任务完成啦。", result.files ?? [], result.taskId),
            providerSessionId: result.failureCode === "session_not_found" ? null : result.providerSessionId ?? current.providerSessionId,
            busy: false,
            activeTaskId: null,
          }));
          setEvolutionMetrics((current) => ({ ...current, agentSuccesses: current.agentSuccesses + 1 }));
          sendMainPetSignal("task_completed", { source: "agent", payload: { connectorId: selectedConnector, durationMs: result.durationMs } });
          addNotice(`${connector.name} 已完成任务`, result.truncated ? "结果较长，已显示可用部分。" : "结果已经送回宠物对话。", "action");
        } else if (result.cancelled) {
          updateDialogueSession(selectedConnector, (current) => ({
            ...appendDialogueAssistantMessage(current, "任务已经停下来了。", result.files ?? [], result.taskId),
            providerSessionId: result.failureCode === "session_not_found" ? null : result.providerSessionId ?? current.providerSessionId,
            busy: false,
            activeTaskId: null,
          }));
          sendMainPetSignal("task_failed", { source: "agent", payload: { connectorId: selectedConnector, cancelled: true } });
          addNotice(`${connector.name} 已停止`, "任务已按你的要求安全停止。", "action");
        } else {
          updateDialogueSession(selectedConnector, (current) => ({
            ...appendDialogueAssistantMessage(current, finalOutput || (result.timedOut ? "任务运行时间过长，已经安全停止。" : "任务没有完成，请换一种说法再试。"), result.files ?? [], result.taskId),
            providerSessionId: result.failureCode === "session_not_found" ? null : result.providerSessionId ?? current.providerSessionId,
            busy: false,
            activeTaskId: null,
          }));
          sendMainPetSignal("task_failed", { source: "agent", payload: { connectorId: selectedConnector, timedOut: result.timedOut } });
          addNotice(`${connector.name} 未完成任务`, result.timedOut ? "任务运行时间过长，已安全停止。" : "请打开宠物对话查看结果。", "action");
        }
      } catch (error) {
        console.error(error);
        const message = error instanceof Error ? error.message : String(error);
        updateDialogueSession(selectedConnector, (current) => ({
          ...appendDialogueAssistantMessage(current, message || "任务没有完成，请稍后再试。"),
          busy: false,
          activeTaskId: null,
        }));
        sendMainPetSignal("task_failed", { source: "agent", payload: { connectorId: selectedConnector, error: message } });
      } finally {
        agentInvokeOwnedRef.current = false;
        setAgentBusy(false);
        setAgentBusyConnector(null);
      }
      return;
    }
    sendMainPetSignal("request_submitted", { source: "system", payload: { local: true } });
    if (/(撤销|恢复|放回).*(整理|桌面)/.test(text)
      || /\b(?:undo|restore|put\s+back)\b.*\b(?:desktop|organization|organizing|organize)\b/i.test(text)) {
      await handleUndoFromShortcut();
      sendMainPetSignal("task_completed", { source: "system" });
      return;
    }
    if (/(一键整理|整理桌面|收拾桌面|清理桌面)/.test(text)
      || /\b(?:organize|tidy|sort|clean\s+up)\b\s+(?:(?:my|the)\s+)?desktop\b/i.test(text)) {
      await handleOrganizeDesktop();
      sendMainPetSignal("task_completed", { source: "system" });
      return;
    }
    const ideaMatch = text.match(/(?:记录|记下|记一下)(?:一个)?(?:意见|想法|灵感)?\s*(.+)/)
      ?? text.match(/^(?:record|save|capture|add)\s+(?:an?\s+)?(?:idea|note)\s*[:：-]?\s*(.+)$/i);
    if (ideaMatch?.[1]) {
      addIdea(ideaMatch[1], "voice");
      sendMainPetSignal("task_completed", { source: "system" });
      return;
    }
    const todoMatch = text.match(/(?:新增|添加|创建)(?:一个)?待办\s*(.+)/)
      ?? text.match(/^(?:add|create|record)\s+(?:an?\s+)?(?:todo|to-do|task)\s*[:：-]?\s*(.+)$/i);
    if (todoMatch?.[1]) {
      addTodo(todoMatch[1]);
      sendMainPetSignal("task_completed", { source: "system" });
      return;
    }
    const explicitWeb = /(发布页|创作中心|网页|网站|发布图文|发布视频)/.test(text);
    if (/小红书|红薯|xhs|xiaohongshu|rednote/i.test(text)
      && (explicitWeb || /^发布/.test(text) || /\b(?:open|publish|post|creator|studio)\b/i.test(text))) {
      await openSocial("xiaohongshu");
      sendMainPetSignal("task_completed", { source: "system" });
      return;
    }
    if (/(发布到|打开)\s*x\b/i.test(text)
      || /\b(?:open|publish|post)\b.*\b(?:x|twitter)\b/i.test(text)
      || /\b(?:x|twitter)\b.*\b(?:open|publish|post)\b/i.test(text)) {
      await openSocial("x");
      sendMainPetSignal("task_completed", { source: "system" });
      return;
    }
    if (/抖音|douyin/i.test(text) && /(发布|打开|创作|\bopen\b|\bpublish\b|\bpost\b|\bcreator\b)/i.test(text)) {
      await openSocial("douyin");
      sendMainPetSignal("task_completed", { source: "system" });
      return;
    }
    if (/(打开|找|查找|搜索|查看)/.test(text)
      || /\b(?:open|find|locate|view|show|search)\b/i.test(text)) {
      const keyword = text
        .replace(/^(?:帮我|请|麻烦|please|could\s+you|can\s+you|would\s+you)\s*/i, "")
        .replace(/(?:打开|找一下|找|查找|搜索|查看|\bsearch(?:\s+for)?\b|\bopen\b|\bfind\b|\blocate\b|\bview\b|\bshow\b)/gi, " ")
        .replace(/(?:桌面上的?|桌面的?|文件|程序|软件|应用|一下|那个|\b(?:on\s+)?(?:(?:my|the)\s+)?desktop\b|\b(?:file|program|software|app|application)\b|\bplease\b)/gi, " ")
        .replace(/\s+/g, " ")
        .trim();
      const fileCandidates = rankFiles(files, keyword).map((candidate) => ({ ...candidate, kind: "file" as const }));
      const programCandidates = rankPrograms(commonPrograms, keyword).map((candidate) => ({ ...candidate, kind: "program" as const }));
      const candidatesByPath = new Map<string, (typeof fileCandidates)[number] | (typeof programCandidates)[number]>();
      programCandidates.forEach((candidate) => candidatesByPath.set(normalizePathKey(candidate.program.path), candidate));
      fileCandidates.forEach((candidate) => {
        const key = normalizePathKey(candidate.file.path);
        if (!candidatesByPath.has(key)) candidatesByPath.set(key, candidate);
      });
      const candidates = [...candidatesByPath.values()].sort((left, right) => right.score - left.score);
      if (candidates.length && candidates[0].score >= 72 && (!candidates[1] || candidates[0].score - candidates[1].score >= 10)) {
        const best = candidates[0];
        if (best.kind === "program") await handleLaunchProgram(best.program);
        else await openIndexedItem(best.file);
        sendMainPetSignal("task_completed", { source: "system" });
        return;
      }
      setFileQuery(keyword);
      setFileFilter("all");
      setActiveTab("files");
      await showMainFromTray().catch(console.error);
      setAssistantMessage(candidates.length ? `找到 ${candidates.length} 个相似项，选一个吧。` : `没有找到「${keyword}」。`);
      sendMainPetSignal(candidates.length ? "task_completed" : "task_failed", { source: "system" });
      return;
    }
    setAssistantMessage("这件事我还不会，换句话试试吧。");
    sendMainPetSignal("task_failed", { source: "system" });
  }, [addIdea, addNotice, addTodo, agentConnector, agentConnectors, agentTaskRunning, commonPrograms, dialogueEnabled, files, handleLaunchProgram, handleOrganizeDesktop, handleStopAgentTask, handleUndoFromShortcut, openIndexedItem, openSocial, sendMainPetSignal, setAgentConnector, setLastDeliveredAgentTaskId]);

  useEffect(() => {
    let cleanup: undefined | (() => void);
    let cancelled = false;
    subscribeToDialogueCommand((command) => {
      if (command.commandId) {
        void acknowledgeDialogueCommand({ commandId: command.commandId, accepted: true }).catch(console.error);
      }
      void handleCommand(command);
    }).then((unlisten) => {
      if (cancelled) unlisten();
      else cleanup = unlisten;
    }).catch(console.error);
    return () => {
      cancelled = true;
      cleanup?.();
    };
  }, [handleCommand]);

  const handleMic = async () => {
    if (!dialogueEnabled) return null;
    if (!voiceEnabled) {
      setAssistantMessage("本地语音还没开启，可以先打字，或在宠物设置里打开。");
      return null;
    }
    if (isListening) return null;
    setIsListening(true);
    setAssistantMessage("我在听。");
    sendMainPetSignal("voice_started", { source: "voice", essentialInQuietMode: true });
    try {
      return await recognizeLocalSpeech();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setAssistantMessage(message);
      sendMainPetSignal("voice_cancelled", { source: "voice", essentialInQuietMode: true });
      return null;
    } finally {
      setIsListening(false);
    }
  };

  const handleRestoreLibraryItems = useCallback(async (selectedFiles: DesktopFile[]) => {
    const organized = selectedFiles.filter(isOrganizedFile);
    if (!organized.length || restoringLibrary) return;
    setRestoringLibrary(true);
    try {
      const result = await restoreLibraryItemsToDesktop(organized.map((file) => file.path));
      if (!result.restoredCount) throw new Error("没有找到可撤回的资料库项目，请刷新文件列表后重试。");
      await loadFiles(true);
      const conflictText = result.conflictCount ? `；${result.conflictCount} 个重名项目已自动改名` : "";
      setAssistantMessage(`已撤回 ${result.restoredCount} 个项目到桌面${conflictText}。`);
      addNotice("文件已撤回桌面", `${result.restoredCount} 个项目已安全放回桌面${conflictText}。`, "action");
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setAssistantMessage(message || "文件暂时没能撤回桌面。");
    } finally {
      setRestoringLibrary(false);
    }
  }, [addNotice, loadFiles, restoringLibrary]);

  const categoryChange = (fileId: DesktopFile["id"], category: string) => {
    setFiles((current) => {
      const next = current.map((file) => file.id === fileId ? { ...file, category } : file);
      filesRef.current = next;
      return next;
    });
    void updateDesktopFileCategory(fileId, category).catch((error) => {
      console.error(error);
      setAssistantMessage("分类没有保存，已经重新同步文件列表。");
      void loadFiles();
    });
  };

  const updateRestEnabled = (enabled: boolean) => {
    setRestEnabled(enabled);
    if (enabled) setNextRestAt(Date.now() + workMinutes * 60_000);
  };

  const updateWorkMinutes = (minutes: number) => {
    setWorkMinutes(minutes);
    setNextRestAt(Date.now() + minutes * 60_000);
  };

  const handleUndoFeed = async () => {
    try {
      const result = await undoLastPetFeed();
      setLastFeed(null);
      setAssistantMessage(`吐回来啦，${result.count} 个项目已经恢复。`);
    } catch (error) {
      console.error(error);
      setAssistantMessage("这次没能恢复，主人再试一次吧。");
    }
  };

  const handleHideToTray = useCallback(async () => {
    if (isHidingToTray) return;
    setMenuOpen(false);
    setIsHidingToTray(true);
    setWindowMotion("closing");
    await new Promise((resolve) => window.setTimeout(resolve, 640));
    try {
      if (isTauri()) await hideMainToTray();
      else {
        setAssistantMessage("浏览器预览：组件已模拟收进小虫洞。");
        setWindowMotion("opening");
        window.setTimeout(() => setWindowMotion("idle"), 760);
      }
    } catch (error) {
      console.error(error);
      setAssistantMessage("没有收起来，再试一次吧。");
      setWindowMotion("opening");
      window.setTimeout(() => setWindowMotion("idle"), 760);
    } finally {
      setIsHidingToTray(false);
      if (isTauri()) setWindowMotion("idle");
    }
  }, [isHidingToTray]);

  const handleQuit = useCallback(async () => {
    try {
      if (isTauri()) await quitApp();
      else setAssistantMessage("浏览器预览不会退出真实进程。");
    } catch (error) {
      console.error(error);
      setAssistantMessage("这次没有退出成功。");
    }
  }, []);

  if (restActive) {
    return (
      <RestOverlay
        seconds={restSeconds}
        cursor={cursor}
        petName={petName || "派派"}
        species={petSpecies}
        theme={petTheme}
        evolution={effectiveEvolution}
        evolutionStage={effectiveEvolutionStage}
        stage={restStage}
        onRequestExit={finishRest}
        onStageComplete={handleRestStageComplete}
      />
    );
  }

  return (
    <div className={`widget-scene ${isTauri() ? "is-tauri" : ""}`}>
      <WormholeTransition motion={windowMotion} />
      <main className={`widget-shell window-motion-${windowMotion} ${isHidingToTray ? "is-hiding-to-tray" : ""}`}>
        {petRuntimeSnapshot ? (
          <span className="pet-runtime-driver" aria-hidden="true">
            <PetSprite
              species={petSpecies}
              action={petRuntimeSnapshot.actionId as PetSpriteAction}
              evolutionStage={effectiveEvolutionStage}
              evolutionPath={effectiveEvolution}
              motionMode={petRuntimeSnapshot.behaviorMode}
              actionInstanceId={petRuntimeSnapshot.actionInstanceId}
              startedAtEpochMs={petRuntimeSnapshot.startedAtEpochMs}
              velocity={petRuntimeSnapshot.velocity}
              reducedMotion={petRuntimeSnapshot.reducedMotion}
              transactionId={petRuntimeSnapshot.transactionId}
              onMarker={handleMainPetMarker}
              onLoopBoundary={handleMainPetLoopBoundary}
              onAnimationEnd={handleMainPetAnimationEnd}
            />
          </span>
        ) : null}
        <WidgetHeader
          unread={unread}
          menuOpen={menuOpen}
          locale={locale}
          onLocale={onLocale}
          onNotifications={() => { setNotificationsOpen(true); setMenuOpen(false); }}
          onMenu={() => setMenuOpen((value) => !value)}
          onSettings={() => { setSettingsOpen(true); setMenuOpen(false); }}
          onPetSettings={() => { setPetSettingsOpen(true); setMenuOpen(false); }}
          onExclusions={openExclusions}
          onMinimize={() => void handleHideToTray()}
          onQuit={() => void handleQuit()}
        />

        <div className="widget-content">
          {activeTab === "home" ? (
            <div className="home-surface">
              <div className="home-main-column">
                <div className="home-view">
                {!collapsedHomeSections.includes("pet") ? (
                  <HomeSectionFrame id="pet" motion={homeSectionMotion} onCollapse={collapseHomeSection}>
                    <PetSummary
                      name={petName || "派派"}
                      species={petSpecies}
                      theme={petTheme}
                      evolution={effectiveEvolution}
                      evolutionStage={effectiveEvolutionStage}
                      motionMode={petMotionMode}
                      message={assistantMessage}
                      pendingCount={pendingTodos}
                      isListening={isListening}
                      dialogueEnabled={dialogueEnabled}
                      cursor={cursor}
                      action={petAction}
                      runtime={petRuntimeSnapshot}
                      onChat={openDialogue}
                      onPetSignal={() => sendMainPetSignal("single_click", { source: "pointer" })}
                    />
                  </HomeSectionFrame>
                ) : null}
                {!collapsedHomeSections.includes("today") ? (
                  <HomeSectionFrame id="today" motion={homeSectionMotion} onCollapse={collapseHomeSection}>
                    <TodayCard todos={todayTodos} onToggle={toggleTodo} onOpenAll={() => setActiveTab("todos")} onAction={handleTodoAction} />
                  </HomeSectionFrame>
                ) : null}
                {!collapsedHomeSections.includes("library") ? (
                  <HomeSectionFrame id="library" motion={homeSectionMotion} onCollapse={collapseHomeSection}>
                    <RecentFilesCard
                      files={files}
                      pendingCount={pendingDesktopCount}
                      libraryCount={libraryFileCount}
                      programs={commonPrograms}
                      library={homeLibrary}
                      scanning={isScanning}
                      programsLoading={programsLoading}
                      organizing={organizePhase === "organizing" || organizePhase === "undoing"}
                      canUndoOrganize={canUndoOrganize}
                      onLibrary={selectHomeLibrary}
                      onRefresh={() => void loadFiles(true)}
                      onRefreshPrograms={() => void loadCommonPrograms()}
                      onOrganize={() => void handleOrganizeDesktop()}
                      onUndoOrganize={() => void handleUndoFromShortcut()}
                      onOpen={(file) => { setLastUserMessage(`打开「${file.name}」`); void openIndexedItem(file); }}
                      onLaunchProgram={(program) => void handleLaunchProgram(program)}
                      onRemoveProgram={handleRemoveFavoriteProgram}
                      onOpenAll={() => setActiveTab("files")}
                    />
                  </HomeSectionFrame>
                ) : null}
                {!collapsedHomeSections.includes("social") ? (
                  <HomeSectionFrame id="social" motion={homeSectionMotion} onCollapse={collapseHomeSection}>
                    <SocialRow
                      customCount={customSocialShortcuts.length}
                      onManage={() => setSocialSettingsOpen(true)}
                      onOpen={(platform) => { setLastUserMessage(`打开${platformNames[platform]}发布页`); void openSocial(platform); }}
                    />
                  </HomeSectionFrame>
                ) : null}
                {!collapsedHomeSections.includes("ideas") ? (
                  <HomeSectionFrame id="ideas" motion={homeSectionMotion} onCollapse={collapseHomeSection}>
                    <IdeaShortcut count={pendingIdeas} onClick={() => setActiveTab("ideas")} />
                  </HomeSectionFrame>
                ) : null}
                </div>
                <div className="home-command-dock"><CommandBar petName={petName || "派派"} onOpen={openDialogue} /></div>
              </div>
              <HomeSectionRail collapsed={collapsedHomeSections} motion={homeSectionMotion} onRestore={restoreHomeSection} />
            </div>
          ) : null}

          {activeTab === "files" ? (
            <FilesView
              files={files}
              programs={commonPrograms}
              programsLoading={programsLoading}
              arrangeMode={libraryArrangeMode}
              query={fileQuery}
              filter={fileFilter}
              onQuery={setFileQuery}
              onFilter={selectFileFilter}
              onArrangeMode={setLibraryArrangeMode}
              onBack={() => setActiveTab("home")}
              onOpen={(file) => { setLastUserMessage(`打开「${file.name}」`); void openIndexedItem(file); }}
              onLaunchProgram={(program) => void handleLaunchProgram(program)}
              onCategory={categoryChange}
              onRestore={(selectedFiles) => void handleRestoreLibraryItems(selectedFiles)}
              restoring={restoringLibrary}
            />
          ) : null}

          {activeTab === "todos" ? (
            <TodosView todos={todos} onBack={() => setActiveTab("home")} onAdd={(title) => { setLastUserMessage(`新增待办 ${title}`); addTodo(title); }} onToggle={toggleTodo} onAction={handleTodoAction} />
          ) : null}

          {activeTab === "ideas" ? (
            <IdeasView
              ideas={ideas}
              onBack={() => setActiveTab("home")}
              onAdd={(title) => { setLastUserMessage(`记录想法 ${title}`); addIdea(title); }}
              onAccept={(id) => setIdeas((current) => current.map((idea) => idea.id === id ? { ...idea, status: "accepted" } : idea))}
              onConvert={(idea) => {
                addTodo(idea.title);
                setIdeas((current) => current.map((item) => item.id === idea.id ? { ...item, status: "converted" } : item));
              }}
            />
          ) : null}
        </div>

        <WidgetTabs active={activeTab} onChange={setActiveTab} />

        {notificationsOpen ? (
          <NotificationSheet
            notices={notices}
            onClose={() => setNotificationsOpen(false)}
            onReadAll={() => setNotices((current) => current.map((notice) => ({ ...notice, read: true })))}
          />
        ) : null}

        {socialSettingsOpen ? (
          <SocialSettingsSheet
            shortcuts={customSocialShortcuts}
            accounts={socialAccounts}
            loading={socialAccountsLoading}
            onAdd={(name, url) => setCustomSocialShortcuts((current) => [...current, { id: makeId("social"), name, url }])}
            onRemove={(id) => setCustomSocialShortcuts((current) => current.filter((shortcut) => shortcut.id !== id))}
            onOpen={(shortcut) => void openCustomSocial(shortcut)}
            onConnect={async (platform) => {
              await openSocialSession(platform);
              addNotice(`${platformNames[platform]} 登录窗口已打开`, "请在 Edge 完成登录，再回到这里同步并验证账号。登录态由 Edge Profile 保存。", "action");
            }}
            onSyncSnapshot={async (platform) => {
              try {
                const saved = await syncSocialSnapshot(platform);
                setSocialAccounts((items) => [...items.filter((item) => item.platform !== saved.platform), saved]);
                addNotice(`${platformNames[platform]} 汇总已同步`, "只读取了当前页面可见的账号名称和汇总数字，没有读取私信正文或原始 Cookie。", "action");
                return saved;
              } catch (error) {
                const latest = await listSocialAccounts().catch(() => []);
                if (latest.length) setSocialAccounts(latest);
                throw error;
              }
            }}
            onSaveSnapshot={async (snapshot) => {
              const saved = await saveSocialSnapshot(snapshot);
              setSocialAccounts((items) => [...items.filter((item) => item.platform !== saved.platform), saved]);
            }}
            onDisconnect={async (platform) => {
              await disconnectSocialSession(platform);
              const latest = await listSocialAccounts();
              setSocialAccounts(latest);
              addNotice(`${platformNames[platform]} 窗口已断开`, "Edge Profile 仍保留，下次打开可继续使用登录状态。", "action");
            }}
            onClear={async (platform) => {
              await clearSocialSession(platform);
              const latest = await listSocialAccounts();
              setSocialAccounts(latest);
              addNotice(`${platformNames[platform]} 登录资料已清除`, "该平台的本地 Edge Profile 与汇总快照已删除。", "action");
            }}
            onClose={() => setSocialSettingsOpen(false)}
          />
        ) : null}

        {settingsOpen ? (
          <RestSettingsSheet
            enabled={restEnabled}
            workMinutes={workMinutes}
            restMinutes={restMinutes}
            nextRestAt={nextRestAt}
            onEnabled={updateRestEnabled}
            onWorkMinutes={updateWorkMinutes}
            onRestMinutes={setRestMinutes}
            onPreview={() => void triggerRest()}
            onClose={() => setSettingsOpen(false)}
          />
        ) : null}

        {petSettingsOpen ? (
          <PetSettingsSheet
            locale={locale}
            name={petName}
            species={petSpecies}
            theme={petTheme}
            evolution={effectiveEvolution}
            autoEvolution={autoEvolution}
            manualEvolutionStage={manualEvolutionStage}
            evolutionStage={effectiveEvolutionStage}
            evolutionPoints={evolutionState.points}
            evolutionStagePoints={evolutionState.stagePoints}
            evolutionStageSpan={evolutionState.stageSpan}
            motionMode={petMotionMode}
            visible={petVisible}
            layer={petLayer}
            focusMode={focusMode}
            dialogueEnabled={dialogueEnabled}
            voiceEnabled={voiceEnabled}
            connectorId={agentConnector}
            connectors={agentConnectors}
            connectorsLoading={agentConnectorsLoading}
            connectorScanMessage={agentConnectorScanMessage}
            agentWorkspace={activeAgentWorkspace}
            lastFeed={lastFeed}
            onName={setPetName}
            onSpecies={setPetSpecies}
            onTheme={setPetTheme}
            onEvolution={setEvolution}
            onEvolutionStage={setManualEvolutionStage}
            onAutoEvolution={setAutoEvolution}
            onMotionMode={setPetMotionMode}
            onVisible={setPetVisible}
            onLayer={setPetLayerState}
            onFocusMode={setFocusMode}
            onDialogueEnabled={(value) => {
              setDialogueEnabled(value);
              if (!value) {
                setDialogueOpen(false);
                setIsListening(false);
                setVoiceEnabled(false);
                if (isTauri()) void hidePetDialogue().catch(console.error);
              }
            }}
            onVoiceEnabled={setVoiceEnabled}
            onConnector={setAgentConnector}
            onAgentWorkspace={updateActiveAgentWorkspace}
            onRefreshConnectors={() => void refreshAgentConnectors()}
            onUndoFeed={() => void handleUndoFeed()}
            onClose={() => setPetSettingsOpen(false)}
          />
        ) : null}

        <ExclusionsSheet
          open={exclusionsOpen}
          exclusions={organizeExclusions}
          loading={exclusionsLoading}
          onClose={() => setExclusionsOpen(false)}
          onRemove={(nameKey) => void handleRemoveExclusion(nameKey)}
        />

        <OrganizeSheet
          phase={organizePhase}
          result={organizeResult}
          error={organizeError}
          canUndo={canUndoOrganize}
          organizedTotal={libraryFileCount}
          desktopIconState={desktopIconState}
          onClose={() => setOrganizePhase("idle")}
          onUndo={(confirmationToken) => void handleUndoDesktopOrganize(confirmationToken)}
          onReview={handleOrganizeReview}
          onRequestPublicConfirmation={requestPublicDesktopConfirmation}
          onOpenExclusions={openExclusions}
          onToggleDesktopIcons={handleToggleDesktopIcons}
          onOrganizePublicDesktop={(confirmationToken) => handleOrganizeDesktop(true, confirmationToken)}
        />

        <MinimalDialogue
          open={dialogueOpen && dialogueEnabled}
          petName={petName || "派派"}
          userMessage={lastUserMessage}
          reply={assistantMessage}
          messages={activeDialogueSession.messages}
          draft={activeDialogueSession.draft}
          attachmentPaths={activeDialogueSession.attachmentPaths}
          voiceEnabled={voiceEnabled}
          isListening={isListening}
          busy={activeConnectorBusy}
          busyElapsedMs={agentTaskTiming.elapsedMs}
          busyHeartbeatKnown={agentTaskTiming.heartbeatKnown}
          busyHeartbeatFresh={agentTaskTiming.heartbeatFresh}
          busyCancelling={agentTaskStatus?.connectorId === agentConnector && (agentTaskStatus.state === "cancelling" || agentTaskStatus.cancelRequested === true)}
          connectorId={agentConnector}
          connectors={agentConnectors}
          resultFiles={agentResultFiles}
          onClose={() => setDialogueOpen(false)}
          onSubmit={(submission) => { void handleCommand({ ...submission, connectorId: agentConnector }); }}
          onMic={handleMic}
          onConnector={setAgentConnector}
          onDraftChange={(draft) => updateDialogueSession(agentConnector, { draft })}
          onAttachmentPathsChange={(next) => updateDialogueSession(agentConnector, (current) => ({
            attachmentPaths: typeof next === "function" ? next(current.attachmentPaths) : next,
          }))}
          onOpenResultFile={handleOpenAgentResultFile}
          onStop={activeConnectorBusy ? () => void handleStopAgentTask() : undefined}
        />
      </main>
    </div>
  );
}

function App() {
  const windowKind = new URLSearchParams(window.location.search).get("window");
  const [locale, setLocale] = usePersistentState<AppLocale>("wormhole-pie.locale.v1", "zh-CN");
  useDocumentLanguage(locale);

  useEffect(() => {
    const title = locale === "zh-CN"
      ? windowKind === "pet"
        ? "虫洞派 · 宠物"
        : windowKind === "dialogue" || windowKind === "pet-dialogue"
          ? "虫洞派 · 对话"
          : "虫洞派"
      : windowKind === "pet"
        ? "Wormhole Pie · Pet"
        : windowKind === "dialogue" || windowKind === "pet-dialogue"
          ? "Wormhole Pie · Dialogue"
          : "Wormhole Pie";
    document.title = title;
    if (!isTauri()) return;
    let cancelled = false;
    void getCurrentWindow()
      .setTitle(title)
      .catch((error) => {
        if (!cancelled) console.error(error);
      });
    return () => { cancelled = true; };
  }, [locale, windowKind]);

  useEffect(() => {
    void setAppLocale(locale).catch(console.error);
  }, [locale]);

  useEffect(() => {
    let cleanup: undefined | (() => void);
    let cancelled = false;
    void subscribeToAppLocale((nextLocale) => setLocale((current) => current === nextLocale ? current : nextLocale)).then((unlisten) => {
      if (cancelled) unlisten();
      else cleanup = unlisten;
    }).catch(console.error);
    return () => {
      cancelled = true;
      cleanup?.();
    };
  }, [setLocale]);

  useEffect(() => {
    if (!isTauri()) return;
    const preventWebviewMenu = (event: MouseEvent) => {
      const target = event.target instanceof HTMLElement ? event.target : null;
      if (target?.closest("input, textarea, [contenteditable='true']")) return;
      event.preventDefault();
    };
    document.addEventListener("contextmenu", preventWebviewMenu);
    return () => document.removeEventListener("contextmenu", preventWebviewMenu);
  }, []);

  if (windowKind === "pet") return <DesktopPetWindow />;
  if (windowKind === "dialogue" || windowKind === "pet-dialogue") return <PetDialogueWindow />;
  return <WidgetApplication locale={locale} onLocale={setLocale} />;
}

export default App;
