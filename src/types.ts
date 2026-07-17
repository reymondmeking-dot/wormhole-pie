export type FileKind =
  | "document"
  | "image"
  | "video"
  | "audio"
  | "archive"
  | "application"
  | "folder"
  | "other";

export type OrganizedCategory =
  | "待整理"
  | "文件夹"
  | "文档"
  | "图片"
  | "视频"
  | "音频"
  | "压缩包"
  | "代码"
  | "快捷方式"
  | "程序"
  | "其他";

export type DesktopFile = {
  id: number | string;
  name: string;
  path: string;
  extension: string;
  kind: FileKind;
  category: string;
  organizedCategory?: OrganizedCategory | string;
  size: number;
  createdAt?: number;
  modifiedAt: number;
  isNew?: boolean;
};

export type FavoriteProgram = {
  name: string;
  path: string;
  addedAt: number;
};

export type ProgramEntry = {
  name: string;
  path: string;
  source: string;
};

export type OrganizeExclusion = {
  nameKey: string;
  displayName: string;
  isDirectory: boolean;
  createdAt: number;
};

export type TodoStatus = "pending" | "doing" | "done";

export type Todo = {
  id: string;
  title: string;
  date?: string;
  time: string;
  priority: "high" | "medium" | "low";
  status: TodoStatus;
  actionType?: "social_publish" | "open_category";
  actionTarget?: "xiaohongshu" | "x" | "douyin" | "unorganized";
};

export type IdeaStatus = "pending" | "doing" | "accepted" | "converted" | "archived";

export type Idea = {
  id: string;
  title: string;
  status: IdeaStatus;
  tags: string[];
  source: "manual" | "voice" | "file";
  linkedFileId?: DesktopFile["id"];
};

export type Notice = {
  id: string;
  title: string;
  message: string;
  type: "file" | "todo" | "idea" | "action";
  read: boolean;
  createdAt: number;
};

export type AssistantResult = {
  kind: "success" | "not_found" | "candidates" | "blocked";
  message: string;
  files?: DesktopFile[];
};
