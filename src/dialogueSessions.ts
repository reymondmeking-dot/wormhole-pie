import type { AgentResultFile, DialogueConnectorId } from "./lib/desktop";

export const dialogueSessionStorageKey = "wormhole-pie.dialogue.sessions.v1";
export const dialogueSessionSchemaVersion = 5;
export const MAX_DIALOGUE_MESSAGES = 48;
export const MAX_DIALOGUE_ATTACHMENTS = 8;

const MAX_DIALOGUE_HISTORY_CHARACTERS = 48_000;
const MAX_DIALOGUE_MESSAGE_CHARACTERS = 16_000;
const MAX_PROVIDER_CONTEXT_MESSAGES = 14;
const MAX_PROVIDER_CONTEXT_CHARACTERS = 12_000;
const DEFAULT_REPLY = "想让我做什么？";
const SESSION_SEED_STORAGE_KEY = "wormhole-pie.dialogue.sessionSeed.v1";

export type DialogueMessageRole = "user" | "assistant" | "system";
export type DialogueMode = "chat" | "plan" | "execute" | "review";

export type DialogueMessage = {
  id: string;
  role: DialogueMessageRole;
  text: string;
  createdAt: number;
  resultFiles: AgentResultFile[];
  taskId?: string;
};

export type DialogueSession = {
  schemaVersion: typeof dialogueSessionSchemaVersion;
  appSessionId: string;
  providerSessionId: string | null;
  mode: DialogueMode;
  messages: DialogueMessage[];
  draft: string;
  attachmentPaths: string[];
  workspace: string;
  busy: boolean;
  activeTaskId: string | null;
  userMessage: string;
  reply: string;
  resultFiles: AgentResultFile[];
  fieldUpdatedAt: DialogueSessionFieldTimestamps;
  updatedAt: number;
};

const dialogueSessionFields = [
  "messages",
  "providerSessionId",
  "mode",
  "draft",
  "attachmentPaths",
  "workspace",
  "busy",
  "activeTaskId",
  "userMessage",
  "reply",
  "resultFiles",
] as const;

type DialogueSessionField = typeof dialogueSessionFields[number];
type DialogueSessionFieldTimestamps = Record<DialogueSessionField, number>;

export type DialogueSessions = Record<DialogueConnectorId, DialogueSession>;

type UnknownRecord = Record<string, unknown>;

let runtimeSessionSeed = "";

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function createId(prefix: string) {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return `${prefix}-${crypto.randomUUID()}`;
  }
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

function sessionSeed() {
  if (runtimeSessionSeed) return runtimeSessionSeed;
  try {
    const saved = localStorage.getItem(SESSION_SEED_STORAGE_KEY)?.trim();
    if (saved) {
      runtimeSessionSeed = saved;
      return saved;
    }
    runtimeSessionSeed = createId("app");
    localStorage.setItem(SESSION_SEED_STORAGE_KEY, runtimeSessionSeed);
    return runtimeSessionSeed;
  } catch {
    runtimeSessionSeed = createId("app");
    return runtimeSessionSeed;
  }
}

function createAppSessionId(connectorId: DialogueConnectorId) {
  return `${sessionSeed()}-${connectorId}`;
}

function normalizeResultFiles(value: unknown): AgentResultFile[] {
  if (!Array.isArray(value)) return [];
  return value
    .filter(isRecord)
    .map((file) => ({
      name: typeof file.name === "string" ? file.name : "",
      path: typeof file.path === "string" ? file.path : "",
      relativePath: typeof file.relativePath === "string" ? file.relativePath : "",
    }))
    .filter((file) => file.name && file.path)
    .slice(0, 12);
}

export function normalizeDialogueAttachmentPaths(value: unknown) {
  if (!Array.isArray(value)) return [];
  const seen = new Set<string>();
  const paths: string[] = [];
  for (const item of value) {
    if (typeof item !== "string") continue;
    const path = item.trim();
    const key = path.toLocaleLowerCase();
    if (!path || path.length > 4096 || seen.has(key)) continue;
    seen.add(key);
    paths.push(path);
    if (paths.length >= MAX_DIALOGUE_ATTACHMENTS) break;
  }
  return paths;
}

function normalizeMessageText(value: string) {
  return value.trim().slice(0, MAX_DIALOGUE_MESSAGE_CHARACTERS);
}

function boundMessages(messages: DialogueMessage[]) {
  const bounded = messages.slice(-MAX_DIALOGUE_MESSAGES);
  let characters = bounded.reduce((total, message) => total + message.text.length, 0);
  while (bounded.length > 1 && characters > MAX_DIALOGUE_HISTORY_CHARACTERS) {
    characters -= bounded.shift()?.text.length ?? 0;
  }
  return bounded;
}

function nextSessionUpdatedAt(current: number) {
  const incremented = current < Number.MAX_SAFE_INTEGER ? current + 1 : current;
  return Math.max(Date.now(), incremented);
}

function normalizeMessage(value: unknown): DialogueMessage | null {
  if (!isRecord(value)) return null;
  const role = value.role;
  const text = typeof value.text === "string" ? normalizeMessageText(value.text) : "";
  if ((role !== "user" && role !== "assistant" && role !== "system") || !text) return null;
  return {
    id: typeof value.id === "string" && value.id.trim() ? value.id : createId("message"),
    role,
    text,
    createdAt: typeof value.createdAt === "number" && Number.isFinite(value.createdAt) ? value.createdAt : Date.now(),
    resultFiles: normalizeResultFiles(value.resultFiles),
    taskId: typeof value.taskId === "string" && value.taskId.trim() ? value.taskId : undefined,
  };
}

function emptyFieldUpdatedAt(): DialogueSessionFieldTimestamps {
  return Object.fromEntries(dialogueSessionFields.map((field) => [field, 0])) as DialogueSessionFieldTimestamps;
}

function normalizeFieldUpdatedAt(value: unknown, fallback: number): DialogueSessionFieldTimestamps {
  const saved = isRecord(value) ? value : {};
  return Object.fromEntries(dialogueSessionFields.map((field) => {
    const timestamp = saved[field];
    return [field, typeof timestamp === "number" && Number.isFinite(timestamp) ? timestamp : fallback];
  })) as DialogueSessionFieldTimestamps;
}

function emptySession(connectorId: DialogueConnectorId): DialogueSession {
  return {
    schemaVersion: dialogueSessionSchemaVersion,
    appSessionId: createAppSessionId(connectorId),
    providerSessionId: null,
    mode: "chat",
    messages: [],
    draft: "",
    attachmentPaths: [],
    workspace: "",
    busy: false,
    activeTaskId: null,
    userMessage: "",
    reply: DEFAULT_REPLY,
    resultFiles: [],
    fieldUpdatedAt: emptyFieldUpdatedAt(),
    updatedAt: 0,
  };
}

function migrateLegacyMessages(saved: UnknownRecord, updatedAt: number) {
  const userMessage = typeof saved.userMessage === "string" ? normalizeMessageText(saved.userMessage) : "";
  const reply = typeof saved.reply === "string" ? normalizeMessageText(saved.reply) : "";
  const resultFiles = normalizeResultFiles(saved.resultFiles);
  const messages: DialogueMessage[] = [];
  if (userMessage) {
    messages.push({
      id: createId("legacy-user"),
      role: "user",
      text: userMessage,
      createdAt: Math.max(0, updatedAt - 1),
      resultFiles: [],
    });
  }
  if (reply && (reply !== DEFAULT_REPLY || userMessage || resultFiles.length)) {
    messages.push({
      id: createId("legacy-assistant"),
      role: "assistant",
      text: reply,
      createdAt: updatedAt,
      resultFiles,
    });
  }
  return messages;
}

function normalizeSession(connectorId: DialogueConnectorId, value: unknown): DialogueSession {
  const defaults = emptySession(connectorId);
  if (!isRecord(value)) return defaults;
  const updatedAt = typeof value.updatedAt === "number" && Number.isFinite(value.updatedAt) ? value.updatedAt : 0;
  const savedMessages = Array.isArray(value.messages)
    ? value.messages.map(normalizeMessage).filter((message): message is DialogueMessage => Boolean(message))
    : migrateLegacyMessages(value, updatedAt);
  const messages = boundMessages(savedMessages);
  const latestUser = [...messages].reverse().find((message) => message.role === "user");
  const latestAssistant = [...messages].reverse().find((message) => message.role !== "user");
  const savedReply = typeof value.reply === "string" && value.reply.trim() ? normalizeMessageText(value.reply) : DEFAULT_REPLY;
  const savedResultFiles = normalizeResultFiles(value.resultFiles);
  const activeTaskId = typeof value.activeTaskId === "string" && value.activeTaskId.trim() ? value.activeTaskId : null;
  return {
    schemaVersion: dialogueSessionSchemaVersion,
    appSessionId: typeof value.appSessionId === "string" && value.appSessionId.trim()
      ? value.appSessionId
      : defaults.appSessionId,
    providerSessionId: typeof value.providerSessionId === "string" && value.providerSessionId.trim()
      ? value.providerSessionId.trim().slice(0, 128)
      : null,
    mode: connectorId === "local" || !["chat", "plan", "execute", "review"].includes(String(value.mode))
      ? "chat"
      : value.mode as DialogueMode,
    messages,
    draft: typeof value.draft === "string" ? value.draft.slice(0, 16_000) : "",
    attachmentPaths: normalizeDialogueAttachmentPaths(value.attachmentPaths),
    workspace: typeof value.workspace === "string" ? value.workspace.slice(0, 4096) : "",
    busy: connectorId !== "local" && (value.busy === true || Boolean(activeTaskId)),
    activeTaskId,
    userMessage: latestUser?.text ?? (typeof value.userMessage === "string" ? normalizeMessageText(value.userMessage) : ""),
    reply: latestAssistant?.text ?? savedReply,
    resultFiles: latestAssistant?.resultFiles.length ? latestAssistant.resultFiles : savedResultFiles,
    fieldUpdatedAt: normalizeFieldUpdatedAt(value.fieldUpdatedAt, updatedAt),
    updatedAt,
  };
}

export function createDialogueSessions(): DialogueSessions {
  return {
    local: emptySession("local"),
    codex: emptySession("codex"),
    claude: emptySession("claude"),
    hermes: emptySession("hermes"),
  };
}

export function normalizeDialogueSessions(value: unknown): DialogueSessions {
  const saved = isRecord(value) ? value : {};
  return {
    local: normalizeSession("local", saved.local),
    codex: normalizeSession("codex", saved.codex),
    claude: normalizeSession("claude", saved.claude),
    hermes: normalizeSession("hermes", saved.hermes),
  };
}

export function mergeDialogueSessions(currentValue: unknown, incomingValue: unknown): DialogueSessions {
  const current = normalizeDialogueSessions(currentValue);
  const incoming = normalizeDialogueSessions(incomingValue);
  const mergeSession = (connectorId: DialogueConnectorId, currentSession: DialogueSession, incomingSession: DialogueSession) => {
    const mergedFields = Object.fromEntries(dialogueSessionFields.map((field) => {
      const currentTimestamp = currentSession.fieldUpdatedAt[field];
      const incomingTimestamp = incomingSession.fieldUpdatedAt[field];
      if (incomingTimestamp > currentTimestamp) return [field, incomingSession[field]];
      if (incomingTimestamp < currentTimestamp) return [field, currentSession[field]];
      const currentKey = JSON.stringify(currentSession[field]);
      const incomingKey = JSON.stringify(incomingSession[field]);
      return [field, incomingKey > currentKey ? incomingSession[field] : currentSession[field]];
    })) as Pick<DialogueSession, DialogueSessionField>;
    const fieldUpdatedAt = Object.fromEntries(dialogueSessionFields.map((field) => [
      field,
      Math.max(currentSession.fieldUpdatedAt[field], incomingSession.fieldUpdatedAt[field]),
    ])) as DialogueSessionFieldTimestamps;
    return normalizeSession(connectorId, {
      ...currentSession,
      ...mergedFields,
      fieldUpdatedAt,
      updatedAt: Math.max(currentSession.updatedAt, incomingSession.updatedAt),
    });
  };
  return {
    local: mergeSession("local", current.local, incoming.local),
    codex: mergeSession("codex", current.codex, incoming.codex),
    claude: mergeSession("claude", current.claude, incoming.claude),
    hermes: mergeSession("hermes", current.hermes, incoming.hermes),
  };
}

export function updateDialogueSessionFields(
  connectorId: DialogueConnectorId,
  session: DialogueSession,
  patch: Partial<DialogueSession>,
): DialogueSession {
  const updatedAt = Math.max(Date.now(), session.updatedAt + 1, patch.updatedAt ?? 0);
  const changedFields = dialogueSessionFields.filter((field) => {
    if (!(field in patch)) return false;
    return JSON.stringify(session[field]) !== JSON.stringify(patch[field]);
  });
  const fieldUpdatedAt = { ...session.fieldUpdatedAt };
  changedFields.forEach((field) => { fieldUpdatedAt[field] = updatedAt; });
  return normalizeSession(connectorId, {
    ...session,
    ...patch,
    fieldUpdatedAt,
    updatedAt,
  });
}

export function appendDialogueUserMessage(session: DialogueSession, text: string): DialogueSession {
  const message = normalizeMessageText(text);
  if (!message) return session;
  const last = session.messages.at(-1);
  const messages = last?.role === "user" && last.text === message
    ? session.messages
    : boundMessages([...session.messages, {
      id: createId("user"),
      role: "user",
      text: message,
      createdAt: Date.now(),
      resultFiles: [],
    }]);
  return {
    ...session,
    messages,
    userMessage: message,
    resultFiles: [],
    updatedAt: nextSessionUpdatedAt(session.updatedAt),
  };
}

export function appendDialogueAssistantMessage(
  session: DialogueSession,
  text: string,
  resultFiles: AgentResultFile[] = [],
  taskId?: string,
): DialogueSession {
  const message = normalizeMessageText(text) || DEFAULT_REPLY;
  const files = normalizeResultFiles(resultFiles);
  const nextMessage: DialogueMessage = {
    id: createId("assistant"),
    role: "assistant",
    text: message,
    createdAt: Date.now(),
    resultFiles: files,
    taskId,
  };
  let messages = session.messages;
  const taskIndex = taskId
    ? messages.findIndex((item) => item.role !== "user" && item.taskId === taskId)
    : -1;
  if (taskIndex >= 0) {
    messages = messages.map((item, index) => index === taskIndex ? { ...nextMessage, id: item.id } : item);
  } else {
    const last = messages.at(-1);
    const duplicate = last?.role !== "user"
      && last?.text === message
      && JSON.stringify(last.resultFiles) === JSON.stringify(files);
    if (!duplicate) messages = boundMessages([...messages, nextMessage]);
  }
  return {
    ...session,
    messages,
    reply: message,
    resultFiles: files,
    busy: taskId && (session.activeTaskId === taskId || session.activeTaskId === "pending") ? false : session.busy,
    activeTaskId: taskId && (session.activeTaskId === taskId || session.activeTaskId === "pending") ? null : session.activeTaskId,
    updatedAt: nextSessionUpdatedAt(session.updatedAt),
  };
}

export function setDialogueSessionPreview(
  session: DialogueSession,
  reply: string,
  resultFiles: AgentResultFile[] = session.resultFiles,
): DialogueSession {
  return {
    ...session,
    reply: normalizeMessageText(reply) || DEFAULT_REPLY,
    resultFiles: normalizeResultFiles(resultFiles),
    updatedAt: nextSessionUpdatedAt(session.updatedAt),
  };
}

export function buildProviderTaskWithHistory(session: DialogueSession, currentRequest: string) {
  const request = normalizeMessageText(currentRequest);
  const history = [...session.messages];
  const trailing = history.at(-1);
  if (trailing?.role === "user" && trailing.text === request) history.pop();

  const selected: DialogueMessage[] = [];
  let characters = 0;
  for (const message of history.slice(-MAX_PROVIDER_CONTEXT_MESSAGES).reverse()) {
    const remaining = MAX_PROVIDER_CONTEXT_CHARACTERS - characters;
    if (remaining <= 0) break;
    const text = message.text.length > remaining ? message.text.slice(-remaining) : message.text;
    selected.push(text === message.text ? message : { ...message, text });
    characters += text.length;
  }
  selected.reverse();
  const modeProtocol: Record<DialogueMode, string> = {
    chat: "Discuss, explain, and help interactively. Do not modify files or make external changes unless the current request explicitly asks you to do so.",
    plan: "Analyze and produce a concrete plan only. Do not modify files, execute commands, or make external changes.",
    execute: "Execute the current request. You may modify workspace files and run commands when useful, and you must verify non-trivial work before reporting completion.",
    review: "Review and diagnose. Prioritize concrete findings, risks, and verification. Do not modify files unless the current request explicitly asks for fixes.",
  };
  const protocol = [
    "[WORMHOLE PIE CONVERSATION MODE]",
    `MODE: ${session.mode.toUpperCase()}`,
    "This mode applies to the CURRENT USER REQUEST and takes precedence over behavior implied by older transcript text.",
    modeProtocol[session.mode],
  ].join("\n");

  const renderTask = () => {
    const transcript = selected.map((message) => {
      const role = message.role === "user" ? "USER" : message.role === "system" ? "SYSTEM" : "ASSISTANT";
      return `${role}: ${JSON.stringify(message.text)}`;
    }).join("\n");
    return [
      protocol,
      "[WORMHOLE PIE SESSION CONTEXT - PLAIN TEXT]",
      "This transcript belongs only to the current provider session. Use it for conversational continuity, but do not repeat or re-run earlier actions solely because they appear in history. Only the CURRENT USER REQUEST authorizes new actions.",
      transcript,
      "[CURRENT USER REQUEST]",
      request,
    ].join("\n\n");
  };

  let task = renderTask();
  while (selected.length && Array.from(task).length > 4_000) {
    selected.shift();
    task = selected.length ? renderTask() : `${protocol}\n\n[CURRENT USER REQUEST]\n\n${request}`;
  }
  return selected.length ? task : `${protocol}\n\n[CURRENT USER REQUEST]\n\n${request}`;
}
