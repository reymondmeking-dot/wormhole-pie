import {
  Bell,
  CheckSquare2,
  Files,
  Lightbulb,
  Settings,
  Sparkles,
} from "lucide-react";

export type Section = "desktop" | "todos" | "ideas" | "notifications";

type Props = {
  active: Section;
  unread: number;
  onSelect: (section: Section) => void;
  onSettings: () => void;
};

const items = [
  { id: "desktop" as const, label: "桌面", icon: Files },
  { id: "todos" as const, label: "待办", icon: CheckSquare2 },
  { id: "ideas" as const, label: "意见", icon: Lightbulb },
  { id: "notifications" as const, label: "通知", icon: Bell },
];

export function Navigation({ active, unread, onSelect, onSettings }: Props) {
  return (
    <aside className="navigation" aria-label="主要导航">
      <button className="brand-mark" aria-label="虫洞派桌面助手">
        <Sparkles size={20} strokeWidth={2.2} />
      </button>

      <nav className="nav-list">
        {items.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            className={`nav-item ${active === id ? "is-active" : ""}`}
            onClick={() => onSelect(id)}
            aria-current={active === id ? "page" : undefined}
          >
            <span className="nav-icon-wrap">
              <Icon size={19} strokeWidth={1.9} />
              {id === "notifications" && unread > 0 ? (
                <span className="nav-dot" aria-label={`${unread} 条未读通知`} />
              ) : null}
            </span>
            <span>{label}</span>
          </button>
        ))}
      </nav>

      <button className="nav-item nav-settings" onClick={onSettings}>
        <Settings size={19} strokeWidth={1.9} />
        <span>设置</span>
      </button>
    </aside>
  );
}
