import type { ReactNode } from "react";
import {
  CalendarDays,
  ChevronLeft,
  FolderKanban,
  Lightbulb,
  PawPrint,
  Share2,
} from "lucide-react";

export type HomeSectionId = "pet" | "today" | "library" | "social" | "ideas";
export type HomeSectionMotion = { id: HomeSectionId; direction: "absorb" | "eject" } | null;

const sectionMeta = {
  pet: { label: "宠物", icon: PawPrint },
  today: { label: "今天", icon: CalendarDays },
  library: { label: "桌面资料库", icon: FolderKanban },
  social: { label: "社交媒体", icon: Share2 },
  ideas: { label: "意见整理", icon: Lightbulb },
} satisfies Record<HomeSectionId, { label: string; icon: typeof PawPrint }>;

export const homeSectionOrder = Object.keys(sectionMeta) as HomeSectionId[];

type FrameProps = {
  id: HomeSectionId;
  motion: HomeSectionMotion;
  onCollapse: (id: HomeSectionId) => void;
  children: ReactNode;
};

export function HomeSectionFrame({ id, motion, onCollapse, children }: FrameProps) {
  const meta = sectionMeta[id];
  const motionClass = motion?.id === id ? `is-${motion.direction === "absorb" ? "absorbing" : "ejecting"}` : "";
  return (
    <div className={`home-section-frame ${motionClass}`} data-section={id}>
      {children}
      <button
        type="button"
        className="home-section-collapse"
        onClick={() => onCollapse(id)}
        aria-label={`收起${meta.label}到右侧栏`}
        title={`收起${meta.label}`}
      >
        <ChevronLeft size={12} />
      </button>
    </div>
  );
}

type RailProps = {
  collapsed: HomeSectionId[];
  motion: HomeSectionMotion;
  onRestore: (id: HomeSectionId) => void;
};

export function HomeSectionRail({ collapsed, motion, onRestore }: RailProps) {
  const visibleIds = homeSectionOrder.filter((id) => collapsed.includes(id));
  return (
    <aside className={`home-section-rail ${motion ? "is-active" : ""} ${visibleIds.length ? "" : "is-empty"}`} aria-label="已收起的首页栏目">
      <span className="section-rail-wormhole" aria-hidden="true"><i /><i /><i /></span>
      <div className="section-rail-items">
        {visibleIds.map((id) => {
          const meta = sectionMeta[id];
          const Icon = meta.icon;
          return (
            <button key={id} type="button" onClick={() => onRestore(id)} aria-label={`恢复${meta.label}`} title={`恢复${meta.label}`}>
              <Icon size={15} />
            </button>
          );
        })}
      </div>
    </aside>
  );
}
