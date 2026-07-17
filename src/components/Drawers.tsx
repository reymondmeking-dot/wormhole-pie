import { Bell, Check, FolderOpen, Mic2, Moon, Settings, SlidersHorizontal, X } from "lucide-react";
import type { Notice } from "../types";
import { relativeTime } from "../lib/format";

type NotificationProps = {
  open: boolean;
  notices: Notice[];
  onClose: () => void;
  onRead: (id: string) => void;
  onReadAll: () => void;
};

export function NotificationDrawer({ open, notices, onClose, onRead, onReadAll }: NotificationProps) {
  if (!open) return null;
  return (
    <div className="drawer-scrim" onMouseDown={onClose}>
      <aside className="drawer notification-drawer" onMouseDown={(event) => event.stopPropagation()}>
        <div className="drawer-heading">
          <div><Bell size={19} /><h2>通知</h2></div>
          <button onClick={onClose} aria-label="关闭通知"><X size={18} /></button>
        </div>
        <button className="read-all" onClick={onReadAll}><Check size={14} />全部标为已读</button>
        <div className="notice-list">
          {notices.map((notice) => (
            <button className={`notice-item ${notice.read ? "is-read" : ""}`} key={notice.id} onClick={() => onRead(notice.id)}>
              <span className={`notice-icon notice-${notice.type}`}><Bell size={15} /></span>
              <span className="notice-copy">
                <strong>{notice.title}</strong>
                <span>{notice.message}</span>
                <small>{relativeTime(notice.createdAt)}</small>
              </span>
              {!notice.read ? <i /> : null}
            </button>
          ))}
        </div>
      </aside>
    </div>
  );
}

type SettingsProps = {
  open: boolean;
  opacity: number;
  quietMode: boolean;
  onOpacity: (value: number) => void;
  onQuietMode: (value: boolean) => void;
  onClose: () => void;
};

export function SettingsDrawer({ open, opacity, quietMode, onOpacity, onQuietMode, onClose }: SettingsProps) {
  if (!open) return null;
  return (
    <div className="drawer-scrim" onMouseDown={onClose}>
      <aside className="drawer settings-drawer" onMouseDown={(event) => event.stopPropagation()}>
        <div className="drawer-heading">
          <div><Settings size={19} /><h2>设置</h2></div>
          <button onClick={onClose} aria-label="关闭设置"><X size={18} /></button>
        </div>

        <div className="setting-group">
          <label><SlidersHorizontal size={17} /><span><strong>工作台透明度</strong><small>让桌面背景保持可见</small></span></label>
          <input type="range" min="70" max="100" value={opacity} onChange={(event) => onOpacity(Number(event.target.value))} />
          <span className="range-value">{opacity}%</span>
        </div>

        <div className="setting-group setting-row">
          <label><FolderOpen size={17} /><span><strong>监听目录</strong><small>桌面 · 实时同步</small></span></label>
          <button>管理</button>
        </div>

        <div className="setting-group setting-row">
          <label><Mic2 size={17} /><span><strong>本地语音模型</strong><small>接口已预留，当前使用文本指令</small></span></label>
          <span className="model-state">待安装</span>
        </div>

        <div className="setting-group setting-row">
          <label><Moon size={17} /><span><strong>勿扰模式</strong><small>暂停弹窗提醒</small></span></label>
          <button className={`switch ${quietMode ? "is-on" : ""}`} onClick={() => onQuietMode(!quietMode)} aria-label="切换勿扰模式"><i /></button>
        </div>
      </aside>
    </div>
  );
}
