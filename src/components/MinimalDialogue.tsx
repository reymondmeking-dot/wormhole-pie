import { ArrowUp, Bot, ChevronDown, ExternalLink, FileText, FileUp, LoaderCircle, Mic, MicOff, Paperclip, Square, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { FormEvent, useCallback, useEffect, useRef, useState } from "react";
import { MAX_DIALOGUE_ATTACHMENTS, normalizeDialogueAttachmentPaths } from "../dialogueSessions";
import type { DialogueMessage } from "../dialogueSessions";
import { isTauri, pickDialogueFiles, subscribeToWindowFileDrop } from "../lib/desktop";
import type { AgentConnectorStatus, AgentResultFile, DialogueConnectorId, DialogueSubmission } from "../lib/desktop";

type Props = {
  open: boolean;
  standalone?: boolean;
  petName: string;
  userMessage: string;
  reply: string;
  messages?: DialogueMessage[];
  draft: string;
  attachmentPaths: string[];
  voiceEnabled: boolean;
  isListening: boolean;
  busy?: boolean;
  busyElapsedMs?: number;
  busyHeartbeatKnown?: boolean;
  busyHeartbeatFresh?: boolean;
  busyCancelling?: boolean;
  connectorId: DialogueConnectorId;
  connectors: AgentConnectorStatus[];
  resultFiles?: AgentResultFile[];
  onClose: () => void;
  onSubmit: (submission: DialogueSubmission) => void | Promise<void>;
  onMic: () => Promise<string | null>;
  onConnector: (connectorId: DialogueConnectorId) => void;
  onDraftChange: (draft: string) => void;
  onAttachmentPathsChange: (next: string[] | ((current: string[]) => string[])) => void;
  onOpenResultFile?: (file: AgentResultFile) => void | Promise<void>;
  onStop?: () => void;
};

const EMPTY_RESULT_FILES: AgentResultFile[] = [];
const EMPTY_MESSAGES: DialogueMessage[] = [];
type DialogueResizeDirection = Parameters<ReturnType<typeof getCurrentWindow>["startResizeDragging"]>[0];

const resizeHandles: ReadonlyArray<readonly [string, DialogueResizeDirection]> = [
  ["north", "North"],
  ["north-east", "NorthEast"],
  ["east", "East"],
  ["south-east", "SouthEast"],
  ["south", "South"],
  ["south-west", "SouthWest"],
  ["west", "West"],
  ["north-west", "NorthWest"],
];

function attachmentName(path: string) {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? "附件";
}

function formatElapsedTime(elapsedMs: number) {
  const totalSeconds = Math.max(0, Math.floor(elapsedMs / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
}

function connectorReady(connector: AgentConnectorStatus) {
  return connector.detected && connector.available;
}

function connectorUnavailableLabel(connector: AgentConnectorStatus) {
  if (connector.configurationState === "permission_blocked") return "权限受限";
  if ((connector.configurationState as string) === "unavailable" || (connector.configurationState as string) === "probe_failed") return "当前不可用";
  if (connector.configurationState === "not_configured") return "配置待确认";
  return "未安装";
}

export function MinimalDialogue({
  open,
  standalone = false,
  petName,
  userMessage,
  reply,
  messages = EMPTY_MESSAGES,
  draft,
  attachmentPaths,
  voiceEnabled,
  isListening,
  busy = false,
  busyElapsedMs = 0,
  busyHeartbeatKnown = false,
  busyHeartbeatFresh = false,
  busyCancelling = false,
  connectorId,
  connectors,
  resultFiles = EMPTY_RESULT_FILES,
  onClose,
  onSubmit,
  onMic,
  onConnector,
  onDraftChange,
  onAttachmentPathsChange,
  onOpenResultFile,
  onStop,
}: Props) {
  const [attachmentNotice, setAttachmentNotice] = useState("");
  const [dropActive, setDropActive] = useState(false);
  const [pickerBusy, setPickerBusy] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const messagesRef = useRef<HTMLDivElement>(null);
  const busyRef = useRef(busy);
  const connectorIdRef = useRef(connectorId);
  busyRef.current = busy;
  connectorIdRef.current = connectorId;

  const addAttachments = useCallback((paths: string[]) => {
    const candidates = normalizeDialogueAttachmentPaths(paths);
    if (!candidates.length) return;
    onAttachmentPathsChange((current) => {
      const next = normalizeDialogueAttachmentPaths([...current, ...candidates]);
      const accepted = new Set(next.map((path) => path.toLocaleLowerCase()));
      setAttachmentNotice(candidates.some((path) => !accepted.has(path.toLocaleLowerCase()))
        ? `一次最多带 ${MAX_DIALOGUE_ATTACHMENTS} 个文件。`
        : "");
      return next;
    });
  }, [onAttachmentPathsChange]);

  useEffect(() => {
    if (open) window.setTimeout(() => inputRef.current?.focus(), 170);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const frame = window.requestAnimationFrame(() => {
      const messages = messagesRef.current;
      if (messages) messages.scrollTop = messages.scrollHeight;
    });
    return () => window.cancelAnimationFrame(frame);
  }, [busy, messages, open, reply, resultFiles.length, userMessage]);

  useEffect(() => {
    if (open) return;
    setAttachmentNotice("");
    setDropActive(false);
  }, [open]);

  useEffect(() => {
    setAttachmentNotice("");
    setDropActive(false);
  }, [connectorId]);

  useEffect(() => {
    if (!open) return;
    let cleanup: undefined | (() => void);
    let cancelled = false;
    void subscribeToWindowFileDrop((event) => {
      if (event.type === "leave") {
        setDropActive(false);
        return;
      }
      if (event.type === "enter" || event.type === "over") {
        if (!busyRef.current && connectorIdRef.current !== "local") setDropActive(true);
        return;
      }
      setDropActive(false);
      if (busyRef.current) return;
      if (connectorIdRef.current === "local") {
        setAttachmentNotice("先选择 Claude、Hermes 或 Codex，再把文件交给它。");
        return;
      }
      addAttachments(event.paths);
    }).then((unlisten) => {
      if (cancelled) unlisten();
      else cleanup = unlisten;
    }).catch(console.error);
    return () => {
      cancelled = true;
      cleanup?.();
    };
  }, [addAttachments, open]);

  if (!open) return null;

  const submitMessage = async (value: string) => {
    if (busy || submitting) return;
    const text = value.trim() || (attachmentPaths.length ? "请查看并处理这些文件。" : "");
    if (!text) return;
    if (attachmentPaths.length && connectorId === "local") {
      setAttachmentNotice("文件需要交给本机 Agent，请先选择 Claude、Hermes 或 Codex。");
      return;
    }
    setSubmitting(true);
    try {
      await onSubmit({ text, attachmentPaths: [...attachmentPaths] });
      onDraftChange("");
      onAttachmentPathsChange([]);
      setAttachmentNotice("");
    } catch (error) {
      console.error(error);
      setAttachmentNotice("文件和消息没有送出去，请再试一次。");
    } finally {
      setSubmitting(false);
    }
  };

  const submit = (event: FormEvent) => {
    event.preventDefault();
    void submitMessage(draft);
  };

  const listen = async () => {
    if (isListening) return;
    const transcript = await onMic();
    if (!transcript) return;
    await submitMessage(transcript);
  };

  const chooseAttachments = async () => {
    if (busy || pickerBusy) return;
    if (connectorId === "local") {
      setAttachmentNotice("先选择 Claude、Hermes 或 Codex，再上传文件。");
      return;
    }
    setPickerBusy(true);
    try {
      const paths = await pickDialogueFiles();
      addAttachments(paths);
    } catch (error) {
      console.error(error);
      setAttachmentNotice("没有打开文件选择器，请再试一次。");
    } finally {
      setPickerBusy(false);
    }
  };

  const startResize = (direction: DialogueResizeDirection) => {
    if (!standalone || !isTauri()) return;
    void getCurrentWindow().startResizeDragging(direction).catch(console.error);
  };

  const historyMessages: DialogueMessage[] = messages.length ? messages : [
    ...(userMessage ? [{ id: "legacy-user", role: "user" as const, text: userMessage, createdAt: 0, resultFiles: [] }] : []),
    ...((reply || !userMessage) ? [{ id: "legacy-assistant", role: "assistant" as const, text: reply || "想让我做什么？", createdAt: 0, resultFiles }] : []),
  ];
  const lastHistoryMessage = historyMessages.at(-1);
  const showPreviewReply = !busy
    && Boolean(reply)
    && lastHistoryMessage?.role !== "user"
    && (lastHistoryMessage?.text !== reply || JSON.stringify(lastHistoryMessage.resultFiles) !== JSON.stringify(resultFiles));
  const visibleMessages = showPreviewReply
    ? [...historyMessages, { id: "preview-assistant", role: "assistant" as const, text: reply, createdAt: Date.now(), resultFiles }]
    : historyMessages;

  return (
    <div className={standalone ? "dialogue-window-surface" : "dialogue-scrim"} onMouseDown={standalone ? undefined : onClose}>
      <section className={`minimal-dialogue ${standalone ? "is-standalone" : ""} ${dropActive ? "is-drop-active" : ""}`} onMouseDown={(event) => event.stopPropagation()} aria-label={`与${petName}对话`}>
        {standalone ? resizeHandles.map(([edge, direction]) => (
          <span
            key={edge}
            className={`dialogue-resize-handle is-${edge}`}
            onPointerDown={(event) => {
              if (event.button !== 0) return;
              event.preventDefault();
              event.stopPropagation();
              startResize(direction);
            }}
            aria-hidden="true"
          />
        )) : null}
        <header className="minimal-dialogue-heading" data-tauri-drag-region={standalone ? "" : undefined}>
          <span className="dialogue-face" aria-hidden="true"><i /><i /></span>
          <div className="dialogue-identity">
            <strong data-no-i18n>{petName}</strong>
            <label className="dialogue-connector" title="选择执行方式">
              <Bot size={11} />
              <select value={connectorId} onChange={(event) => onConnector(event.target.value as DialogueConnectorId)} aria-label="选择本机 Agent">
                <option value="local">本地助手</option>
                {connectors.map((connector) => (
                  <option key={connector.id} value={connector.id} disabled={!connectorReady(connector)}>
                    {connector.name}{connectorReady(connector) ? "" : ` · ${connectorUnavailableLabel(connector)}`}
                  </option>
                ))}
              </select>
              <ChevronDown size={10} />
            </label>
          </div>
          <button onClick={onClose} aria-label="关闭对话"><X size={16} /></button>
        </header>

        <div className="minimal-dialogue-messages" ref={messagesRef} aria-live="polite" aria-busy={busy}>
          {visibleMessages.map((message) => message.role === "user" ? (
            <p key={message.id} className="dialogue-user-message" data-no-i18n>{message.text}</p>
          ) : (
            <div className="dialogue-pet-response" key={message.id}>
              <p className="dialogue-pet-message" data-no-i18n={connectorId === "local" ? undefined : true}>{message.text}</p>
              {message.resultFiles.length ? (
                <div className="dialogue-result-files" aria-label={`Agent 交付了 ${message.resultFiles.length} 个文件`}>
                  {message.resultFiles.map((file) => (
                    <button key={file.path} type="button" onClick={() => void onOpenResultFile?.(file)} title={`打开 ${file.relativePath}`}>
                      <span><FileText size={13} /></span>
                      <span data-no-i18n><strong>{file.name}</strong><small>{file.relativePath}</small></span>
                      <ExternalLink size={11} />
                    </button>
                  ))}
                </div>
              ) : null}
            </div>
          ))}
          {busy ? (
            <p className="dialogue-pet-message is-busy">
              <span className="dialogue-agent-motion" aria-hidden="true"><i /><i /><i /></span>
              <span className="dialogue-agent-copy">
                <strong>{busyCancelling
                  ? "正在温柔停下来…"
                  : busyHeartbeatFresh
                    ? "Agent 仍在运行"
                    : busyHeartbeatKnown
                      ? "正在重新确认 Agent 状态…"
                      : "正在启动本机 Agent…"}</strong>
                <small>{busyHeartbeatFresh
                  ? `已确认运行 ${formatElapsedTime(busyElapsedMs)} · 完成后会送回来`
                  : `已等待 ${formatElapsedTime(busyElapsedMs)} · ${busyHeartbeatKnown ? "心跳暂未更新" : "正在建立连接"}`}</small>
              </span>
              {onStop && !busyCancelling ? (
                <button type="button" className="dialogue-stop-agent" onClick={onStop} aria-label="停止当前 Agent 任务"><Square size={9} />停止</button>
              ) : null}
            </p>
          ) : null}
        </div>

        <form className="minimal-dialogue-composer" onSubmit={submit}>
          {attachmentPaths.length ? (
            <div className="dialogue-attachments" aria-label={`已添加 ${attachmentPaths.length} 个文件`}>
              {attachmentPaths.map((path) => (
                <span className="dialogue-attachment-chip" key={path}>
                  <Paperclip size={10} aria-hidden="true" />
                  <span data-no-i18n>{attachmentName(path)}</span>
                  <button type="button" disabled={busy} onClick={() => onAttachmentPathsChange((current) => current.filter((item) => item !== path))} aria-label={`移除 ${attachmentName(path)}`}>
                    <X size={9} />
                  </button>
                </span>
              ))}
            </div>
          ) : null}
          {attachmentNotice ? <p className="dialogue-attachment-notice" role="status">{attachmentNotice}</p> : null}
          <div className="minimal-dialogue-input">
            <button
              type="button"
              className="dialogue-attach"
              onClick={() => void chooseAttachments()}
              disabled={busy || pickerBusy || connectorId === "local"}
              aria-label="上传文件给 Agent"
              title={connectorId === "local" ? "先选择 Claude、Hermes 或 Codex" : "上传文件，或直接拖进对话框"}
            >
              {pickerBusy ? <LoaderCircle size={15} className="is-spinning" /> : <FileUp size={15} />}
            </button>
            <input ref={inputRef} disabled={busy} value={draft} onChange={(event) => onDraftChange(event.target.value)} placeholder={busy ? "Agent 工作中…" : attachmentPaths.length ? "补充一句，或直接发送…" : "说一句…"} aria-label="输入消息" />
            <button
              type="button"
              className={`${isListening ? "is-listening" : ""} ${voiceEnabled ? "" : "is-unavailable"}`}
              onClick={() => void listen()}
              disabled={busy}
              aria-label={isListening ? "正在听" : "本地语音输入"}
              title={voiceEnabled ? "本地语音输入" : "本地语音未开启，点击查看提示"}
            >
                {isListening ? <MicOff size={16} /> : <Mic size={16} />}
            </button>
            <button type="submit" disabled={busy || submitting || (!draft.trim() && !attachmentPaths.length)} className="dialogue-send" aria-label="发送"><ArrowUp size={16} /></button>
          </div>
        </form>
        {dropActive ? (
          <div className="dialogue-drop-overlay" aria-hidden="true">
            <span><Paperclip size={20} /></span>
            <strong>松开，把文件交给 Agent</strong>
            <small>{`最多 ${MAX_DIALOGUE_ATTACHMENTS} 个文件`}</small>
          </div>
        ) : null}
      </section>
    </div>
  );
}
