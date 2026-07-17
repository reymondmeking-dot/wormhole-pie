import { ChevronDown, ExternalLink } from "lucide-react";
import { useState } from "react";

type Platform = "xiaohongshu" | "x" | "douyin";

type Props = {
  onOpen: (platform: Platform) => void;
};

export function SocialActions({ onOpen }: Props) {
  const [menuOpen, setMenuOpen] = useState(false);

  return (
    <section className="social-actions" aria-label="社交媒体快捷操作">
      <div className="social-copy">
        <strong>创作快捷入口</strong>
        <span>发布由浏览器完成，应用不保存账号密码</span>
      </div>
      <div className="social-buttons">
        <div className="split-action">
          <button className="xhs-action" onClick={() => onOpen("xiaohongshu")}>
            <span className="social-mark xhs-mark">红</span>
            发布小红书
            <ExternalLink size={14} />
          </button>
          <button className="xhs-menu-button" onClick={() => setMenuOpen((value) => !value)} aria-label="小红书更多操作">
            <ChevronDown size={15} />
          </button>
          {menuOpen ? (
            <div className="social-menu">
              <button onClick={() => onOpen("xiaohongshu")}>发布图文</button>
              <button onClick={() => onOpen("xiaohongshu")}>发布视频</button>
              <button onClick={() => onOpen("xiaohongshu")}>打开创作中心</button>
            </div>
          ) : null}
        </div>
        <button className="secondary-social" onClick={() => onOpen("x")}>
          <span className="social-mark x-mark">X</span>
          发布到 X
        </button>
        <button className="secondary-social" onClick={() => onOpen("douyin")}>
          <span className="social-mark douyin-mark">抖</span>
          打开抖音
        </button>
      </div>
    </section>
  );
}
