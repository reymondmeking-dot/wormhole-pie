import type { FileKind } from "../types";

export const kindLabels: Record<FileKind | "all" | "unorganized", string> = {
  all: "全部文件",
  unorganized: "待整理",
  document: "文档",
  image: "图片",
  video: "视频",
  audio: "音频",
  archive: "压缩包",
  application: "程序",
  folder: "文件夹",
  other: "其他",
};

export function formatBytes(value: number) {
  if (!value) return "—";
  if (value < 1024 * 1024) return `${Math.max(1, Math.round(value / 1024))} KB`;
  if (value < 1024 * 1024 * 1024) return `${(value / 1024 / 1024).toFixed(1)} MB`;
  return `${(value / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

export function relativeTime(timestamp: number) {
  const minutes = Math.max(1, Math.round((Date.now() - timestamp) / 60_000));
  if (minutes < 60) return `${minutes} 分钟前`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours} 小时前`;
  return `${Math.round(hours / 24)} 天前`;
}

export function todayLabel() {
  return new Intl.DateTimeFormat("zh-CN", {
    month: "long",
    day: "numeric",
    weekday: "long",
  }).format(new Date());
}
