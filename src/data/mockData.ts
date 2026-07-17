import type { DesktopFile, Idea, Notice, Todo } from "../types";

const now = Date.now();

export const mockFiles: DesktopFile[] = [
  {
    id: "mock-1",
    name: "项目报告.docx",
    path: "C:\\Users\\你\\Desktop\\项目报告.docx",
    extension: "docx",
    kind: "document",
    category: "工作项目",
    size: 2_400_000,
    createdAt: now - 6 * 24 * 3_600_000,
    modifiedAt: now - 18 * 60_000,
  },
  {
    id: "mock-2",
    name: "封面设计.png",
    path: "C:\\Users\\你\\Desktop\\封面设计.png",
    extension: "png",
    kind: "image",
    category: "小红书素材",
    size: 8_800_000,
    createdAt: now - 3 * 24 * 3_600_000,
    modifiedAt: now - 42 * 60_000,
    isNew: true,
  },
  {
    id: "mock-3",
    name: "产品演示.mp4",
    path: "C:\\Users\\你\\Desktop\\产品演示.mp4",
    extension: "mp4",
    kind: "video",
    category: "待整理",
    size: 124_000_000,
    createdAt: now - 2 * 24 * 3_600_000,
    modifiedAt: now - 2 * 3_600_000,
  },
  {
    id: "mock-4",
    name: "品牌资料",
    path: "C:\\Users\\你\\Desktop\\品牌资料",
    extension: "",
    kind: "folder",
    category: "工作项目",
    size: 0,
    createdAt: now - 12 * 24 * 3_600_000,
    modifiedAt: now - 5 * 3_600_000,
  },
  {
    id: "mock-5",
    name: "产品演示素材.zip",
    path: "C:\\Users\\你\\Desktop\\产品演示素材.zip",
    extension: "zip",
    kind: "archive",
    category: "待整理",
    size: 46_000_000,
    createdAt: now - 24 * 3_600_000,
    modifiedAt: now - 24 * 3_600_000,
  },
  {
    id: "mock-6",
    name: "剪映专业版.lnk",
    path: "C:\\Users\\你\\Desktop\\剪映专业版.lnk",
    extension: "lnk",
    kind: "application",
    category: "个人资料",
    size: 2_048,
    createdAt: now - 14 * 24 * 3_600_000,
    modifiedAt: now - 30 * 60_000,
  },
];

export const seedTodos: Todo[] = [];

export const seedIdeas: Idea[] = [];

export const seedNotices: Notice[] = [];
