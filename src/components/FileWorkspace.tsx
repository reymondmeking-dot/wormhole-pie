import {
  Archive,
  ChevronDown,
  File,
  FileImage,
  FileText,
  Folder,
  MoreHorizontal,
  Play,
  RefreshCw,
  Search,
} from "lucide-react";
import { useDeferredValue, useMemo } from "react";
import { formatBytes, kindLabels, relativeTime } from "../lib/format";
import type { DesktopFile, FileKind } from "../types";

type Filter = FileKind | "all" | "unorganized";

type Props = {
  files: DesktopFile[];
  query: string;
  filter: Filter;
  isScanning: boolean;
  onQueryChange: (value: string) => void;
  onFilterChange: (value: Filter) => void;
  onRefresh: () => void;
  onOpen: (file: DesktopFile) => void;
  onCategoryChange: (fileId: DesktopFile["id"], category: string) => void;
};

const filters: Filter[] = ["all", "unorganized", "document", "image", "video", "folder", "archive"];

const iconByKind = {
  document: FileText,
  image: FileImage,
  video: Play,
  audio: Play,
  archive: Archive,
  application: File,
  folder: Folder,
  other: File,
} satisfies Record<FileKind, typeof File>;

export function FileWorkspace({
  files,
  query,
  filter,
  isScanning,
  onQueryChange,
  onFilterChange,
  onRefresh,
  onOpen,
  onCategoryChange,
}: Props) {
  const deferredQuery = useDeferredValue(query.trim().toLowerCase());
  const visibleFiles = useMemo(
    () =>
      files.filter((file) => {
        const matchesQuery = !deferredQuery || `${file.name} ${file.category}`.toLowerCase().includes(deferredQuery);
        const matchesFilter =
          filter === "all" ||
          (filter === "unorganized" ? file.category === "待整理" : file.kind === filter);
        return matchesQuery && matchesFilter;
      }),
    [deferredQuery, files, filter],
  );

  return (
    <section className="file-workspace" aria-labelledby="files-title">
      <div className="section-heading file-heading">
        <div>
          <h1 id="files-title">桌面文件</h1>
          <p>{files.length} 个项目 · 实时同步桌面</p>
        </div>
        <button className={`icon-button ${isScanning ? "is-spinning" : ""}`} onClick={onRefresh} aria-label="刷新桌面文件">
          <RefreshCw size={17} />
        </button>
      </div>

      <div className="file-toolbar">
        <label className="search-control file-search">
          <Search size={17} />
          <input
            value={query}
            onChange={(event) => onQueryChange(event.target.value)}
            placeholder="搜索桌面文件…"
            aria-label="搜索桌面文件"
          />
          <kbd>⌘ K</kbd>
        </label>
        <label className="select-control">
          <select value={filter} onChange={(event) => onFilterChange(event.target.value as Filter)} aria-label="文件类型筛选">
            {filters.map((item) => (
              <option key={item} value={item}>
                {kindLabels[item]}
              </option>
            ))}
          </select>
          <ChevronDown size={15} />
        </label>
      </div>

      <div className="category-rail" aria-label="快速分类">
        {filters.slice(0, 6).map((item) => {
          const count = files.filter((file) => {
            if (item === "all") return true;
            if (item === "unorganized") return file.category === "待整理";
            return file.kind === item;
          }).length;
          return (
            <button key={item} className={filter === item ? "is-selected" : ""} onClick={() => onFilterChange(item)}>
              {kindLabels[item]}
              <span>{count}</span>
            </button>
          );
        })}
      </div>

      <div className="file-table" role="table" aria-label="桌面文件列表">
        <div className="file-row file-table-head" role="row">
          <span role="columnheader">名称</span>
          <span role="columnheader">分类</span>
          <span role="columnheader">大小</span>
          <span role="columnheader">修改时间</span>
          <span aria-hidden="true" />
        </div>

        <div className="file-list" role="rowgroup">
          {visibleFiles.length ? (
            visibleFiles.map((file) => {
              const Icon = iconByKind[file.kind];
              return (
                <div className="file-row" role="row" key={file.id} onDoubleClick={() => onOpen(file)}>
                  <button className="file-name-cell" onClick={() => onOpen(file)} role="cell">
                    <span className={`file-icon kind-${file.kind}`}>
                      <Icon size={19} strokeWidth={1.8} />
                    </span>
                    <span className="file-name-copy">
                      <strong>{file.name}</strong>
                      <small>{file.extension ? file.extension.toUpperCase() : "文件夹"}</small>
                    </span>
                    {file.isNew ? <span className="new-marker">新</span> : null}
                  </button>
                  <label className="inline-category" role="cell">
                    <select value={file.category} onChange={(event) => onCategoryChange(file.id, event.target.value)}>
                      <option>待整理</option>
                      <option>工作项目</option>
                      <option>小红书素材</option>
                      <option>个人资料</option>
                    </select>
                    <ChevronDown size={13} />
                  </label>
                  <span className="muted-cell" role="cell">{formatBytes(file.size)}</span>
                  <span className="muted-cell" role="cell">{relativeTime(file.modifiedAt)}</span>
                  <button className="row-more" aria-label={`${file.name} 更多操作`}>
                    <MoreHorizontal size={17} />
                  </button>
                </div>
              );
            })
          ) : (
            <div className="empty-state">
              <Folder size={34} strokeWidth={1.4} />
              <strong>这里暂时没有文件</strong>
              <span>把文件放到桌面，或换个搜索词试试。</span>
            </div>
          )}
        </div>
      </div>

      <div className="file-footer">
        <span><i className="live-dot" />桌面监听中</span>
        <span>双击文件即可打开</span>
      </div>
    </section>
  );
}
