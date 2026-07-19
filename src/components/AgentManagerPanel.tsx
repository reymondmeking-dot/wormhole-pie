import { CheckCircle2, ChevronDown, Download, FolderOpen, KeyRound, LoaderCircle, RefreshCw, ServerCog, ShieldCheck, Shuffle, Wifi } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import type { AppLocale } from "../i18n";
import { usePersistentState } from "../hooks/usePersistentState";
import {
  applyCcSwitchProvider,
  getAgentDefaultInstallDirectory,
  getCcSwitchStatus,
  installAgent,
  pickDirectory,
  saveAgentApiConfig,
  testAgentApi,
  type AgentApiConfig,
  type AgentConnectorId,
  type AgentConnectorStatus,
  type CcSwitchStatus,
} from "../lib/desktop";

type Props = {
  locale: AppLocale;
  connectors: AgentConnectorStatus[];
  workspace: string;
  onWorkspace: (value: string) => void;
  onInstalled: () => void;
};

type PublicApiSettings = Record<AgentConnectorId, { baseUrl: string; model: string }>;

const defaultApiSettings: PublicApiSettings = {
  codex: { baseUrl: "https://api.openai.com/v1", model: "gpt-5.1-codex" },
  claude: { baseUrl: "https://api.anthropic.com", model: "claude-sonnet-4-5" },
  hermes: { baseUrl: "https://api.openai.com/v1", model: "hermes-4-405b" },
};

function copy(locale: AppLocale, zh: string, en: string) {
  return locale === "zh-CN" ? zh : en;
}

export function AgentManagerPanel({ locale, connectors, workspace, onWorkspace, onInstalled }: Props) {
  const [installDirectory, setInstallDirectory] = usePersistentState("wormhole-pie.agent.installDirectory.v1", "");
  const [apiSettings, setApiSettings] = usePersistentState<PublicApiSettings>("wormhole-pie.agent.apiPublic.v1", defaultApiSettings);
  const [selected, setSelected] = useState<AgentConnectorId>("codex");
  const [apiKey, setApiKey] = useState("");
  const [installing, setInstalling] = useState<AgentConnectorId | null>(null);
  const [installMessage, setInstallMessage] = useState("");
  const [testing, setTesting] = useState(false);
  const [apiMessage, setApiMessage] = useState("");
  const [saving, setSaving] = useState(false);
  const [ccStatus, setCcStatus] = useState<CcSwitchStatus | null>(null);
  const [ccLoading, setCcLoading] = useState(false);
  const [ccApplying, setCcApplying] = useState<string | null>(null);
  const [ccMessage, setCcMessage] = useState("");

  useEffect(() => {
    if (installDirectory.trim()) return;
    getAgentDefaultInstallDirectory().then(setInstallDirectory).catch(() => undefined);
  }, [installDirectory, setInstallDirectory]);

  const refreshCcSwitch = useCallback(async () => {
    setCcLoading(true);
    setCcMessage("");
    try {
      setCcStatus(await getCcSwitchStatus());
    } catch (error) {
      setCcMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setCcLoading(false);
    }
  }, []);

  useEffect(() => { void refreshCcSwitch(); }, [refreshCcSwitch]);

  const selectedConnector = useMemo(
    () => connectors.find((connector) => connector.id === selected),
    [connectors, selected],
  );
  const selectedApi = apiSettings[selected] ?? defaultApiSettings[selected];
  const sourceLabel = copy(locale, "中国镜像源", "Global official source");

  const updateApi = (patch: Partial<PublicApiSettings[AgentConnectorId]>) => {
    setApiSettings((current) => ({
      ...current,
      [selected]: { ...(current[selected] ?? defaultApiSettings[selected]), ...patch },
    }));
    setApiMessage("");
  };

  const chooseDirectory = async (kind: "install" | "workspace") => {
    const title = kind === "install"
      ? copy(locale, "选择 Agent 安装目录", "Choose Agent install directory")
      : copy(locale, "选择 Agent 任务目录", "Choose Agent task directory");
    const selectedDirectory = await pickDirectory(title);
    if (!selectedDirectory) return;
    if (kind === "install") setInstallDirectory(selectedDirectory);
    else onWorkspace(selectedDirectory);
  };

  const handleInstall = async (connectorId: AgentConnectorId) => {
    if (!installDirectory.trim() || installing) return;
    setInstalling(connectorId);
    setInstallMessage(copy(locale, "正在检查依赖并安装，请保持窗口开启…", "Checking dependencies and installing. Keep this window open…"));
    try {
      const result = await installAgent({ connectorId, locale, installDirectory: installDirectory.trim() });
      setInstallMessage(result.detail);
      if (result.success) onInstalled();
    } catch (error) {
      setInstallMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setInstalling(null);
    }
  };

  const currentConfig = (): AgentApiConfig => ({
    connectorId: selected,
    apiKey: apiKey.trim(),
    baseUrl: selectedApi.baseUrl.trim(),
    model: selectedApi.model.trim(),
  });

  const handleTest = async () => {
    setTesting(true);
    setApiMessage("");
    try {
      const result = await testAgentApi(currentConfig());
      setApiMessage(result.detail);
    } catch (error) {
      setApiMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setTesting(false);
    }
  };

  const handleSave = async () => {
    setSaving(true);
    setApiMessage("");
    try {
      await saveAgentApiConfig(currentConfig());
      setApiKey("");
      setApiMessage(copy(locale, "已写入 Agent 默认配置。", "Saved to the Agent's default configuration."));
      onInstalled();
    } catch (error) {
      setApiMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setSaving(false);
    }
  };

  const handleCcApply = async (providerId: string) => {
    if (ccApplying) return;
    setCcApplying(providerId);
    setCcMessage("");
    try {
      await applyCcSwitchProvider(selected, providerId);
      setCcMessage(copy(locale, "已安全应用到本机 Agent；CC Switch 数据库未被修改。", "Applied safely to the local Agent; the CC Switch database was not modified."));
      onInstalled();
    } catch (error) {
      setCcMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setCcApplying(null);
    }
  };

  const selectedCcProviders = ccStatus?.providers.filter((provider) => provider.appType === selected) ?? [];

  return (
    <div className="agent-manager-panel" data-testid="agent-manager-panel">
      <div className="agent-directory-grid">
        <label>
          <span><Download size={13} />{copy(locale, "安装目录", "Install directory")}</span>
          <div><input value={installDirectory} onChange={(event) => setInstallDirectory(event.target.value)} spellCheck={false} /><button type="button" onClick={() => void chooseDirectory("install")}><FolderOpen size={13} />{copy(locale, "浏览", "Browse")}</button></div>
        </label>
        <label>
          <span><FolderOpen size={13} />{copy(locale, "任务目录", "Task directory")}</span>
          <div><input value={workspace} onChange={(event) => onWorkspace(event.target.value)} spellCheck={false} /><button type="button" onClick={() => void chooseDirectory("workspace")}><FolderOpen size={13} />{copy(locale, "浏览", "Browse")}</button></div>
        </label>
      </div>

      <div className="agent-source-note"><ServerCog size={14} /><span>{sourceLabel} · {copy(locale, "自动安装所需依赖", "Required dependencies are installed automatically")}</span></div>

      <div className="agent-install-list">
        {connectors.map((connector) => {
          const isInstalling = installing === connector.id;
          return (
            <article key={connector.id} className={connector.available ? "is-ready" : ""}>
              <div><strong>{connector.name}</strong><small>{connector.detail}</small></div>
              <button type="button" disabled={Boolean(installing) || !installDirectory.trim()} onClick={() => void handleInstall(connector.id)}>
                {isInstalling ? <LoaderCircle className="is-spinning" size={14} /> : connector.available ? <CheckCircle2 size={14} /> : <Download size={14} />}
                {isInstalling ? copy(locale, "安装中…", "Installing…") : connector.available ? copy(locale, "重新安装", "Reinstall") : copy(locale, "一键安装", "Install")}
              </button>
            </article>
          );
        })}
      </div>
      {installMessage ? <p className="agent-manager-feedback" role="status">{installMessage}</p> : null}

      <details className="agent-api-details">
        <summary><span><KeyRound size={14} />{copy(locale, "API 配置", "API configuration")}</span><ChevronDown size={14} /></summary>
        <div className="agent-api-body">
          <div className="agent-api-tabs">
            {connectors.map((connector) => <button type="button" key={connector.id} className={selected === connector.id ? "is-active" : ""} onClick={() => { setSelected(connector.id); setApiKey(""); setApiMessage(""); }}>{connector.name}</button>)}
          </div>
          <label><span>{copy(locale, "接口地址", "Base URL")}</span><input value={selectedApi.baseUrl} onChange={(event) => updateApi({ baseUrl: event.target.value })} placeholder="https://api.example.com/v1" spellCheck={false} /></label>
          <label><span>{copy(locale, "API 密钥", "API key")}</span><input type="password" value={apiKey} onChange={(event) => { setApiKey(event.target.value); setApiMessage(""); }} placeholder={selectedConnector?.configured ? copy(locale, "已保存；留空则保持原密钥", "Already saved; leave blank to keep it") : "sk-…"} autoComplete="off" /></label>
          <label><span>{copy(locale, "模型", "Model")}</span><input value={selectedApi.model} onChange={(event) => updateApi({ model: event.target.value })} spellCheck={false} /></label>
          <div className="agent-api-actions">
            <button type="button" disabled={testing || !selectedApi.baseUrl.trim()} onClick={() => void handleTest()}>{testing ? <LoaderCircle className="is-spinning" size={14} /> : <Wifi size={14} />}{testing ? copy(locale, "测试中…", "Testing…") : copy(locale, "测试通断", "Test connection")}</button>
            <button type="button" disabled={saving || !selectedApi.baseUrl.trim()} onClick={() => void handleSave()}>{saving ? <LoaderCircle className="is-spinning" size={14} /> : <ShieldCheck size={14} />}{copy(locale, "保存配置", "Save configuration")}</button>
          </div>
          {apiMessage ? <p className="agent-manager-feedback" role="status">{apiMessage}</p> : null}
        </div>
      </details>

      <details className="agent-api-details cc-switch-details">
        <summary><span><Shuffle size={14} />{copy(locale, "CC Switch 兼容配置", "CC Switch compatible profiles")}</span><ChevronDown size={14} /></summary>
        <div className="agent-api-body cc-switch-body">
          <div className="agent-api-tabs">
            {connectors.map((connector) => <button type="button" key={connector.id} className={selected === connector.id ? "is-active" : ""} onClick={() => { setSelected(connector.id); setCcMessage(""); }}>{connector.name}</button>)}
          </div>
          <div className="cc-switch-toolbar">
            <p>{copy(locale, "只读发现本机配置；密钥不会进入前端，也不会修改 CC Switch 当前状态。", "Profiles are discovered read-only; secrets never enter the UI and CC Switch state is not changed.")}</p>
            <button type="button" disabled={ccLoading} onClick={() => void refreshCcSwitch()} aria-label={copy(locale, "刷新 CC Switch", "Refresh CC Switch")}>
              <RefreshCw size={12} className={ccLoading ? "is-spinning" : ""} />
            </button>
          </div>
          {!ccStatus?.detected ? (
            <p className="cc-switch-empty">{ccLoading ? copy(locale, "正在检测…", "Detecting…") : copy(locale, "未检测到 ~/.cc-switch/cc-switch.db", "No ~/.cc-switch/cc-switch.db detected")}</p>
          ) : selectedCcProviders.length ? (
            <div className="cc-switch-list">
              {selectedCcProviders.map((provider) => (
                <article key={provider.id} className={provider.isCurrent ? "is-current" : ""}>
                  <div>
                    <strong data-no-i18n>{provider.name}</strong>
                    <small data-no-i18n>{[provider.model, provider.endpoint].filter(Boolean).join(" · ") || "—"}</small>
                  </div>
                  {provider.isCurrent ? <span>{copy(locale, "CC 当前", "CC current")}</span> : null}
                  <button type="button" disabled={Boolean(ccApplying)} onClick={() => void handleCcApply(provider.id)}>
                    {ccApplying === provider.id ? <LoaderCircle className="is-spinning" size={12} /> : <Shuffle size={12} />}
                    {copy(locale, "应用", "Apply")}
                  </button>
                </article>
              ))}
            </div>
          ) : <p className="cc-switch-empty">{copy(locale, `CC Switch 中没有 ${selectedConnector?.name ?? selected} 配置`, `No ${selectedConnector?.name ?? selected} profiles in CC Switch`)}</p>}
          {ccMessage ? <p className="agent-manager-feedback" role="status">{ccMessage}</p> : null}
        </div>
      </details>
    </div>
  );
}
