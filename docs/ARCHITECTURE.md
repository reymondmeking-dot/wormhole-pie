# 虫洞派 / Wormhole Pie：技术架构

## 架构目标

虫洞派同时支持 Windows 与 macOS，以本地优先的桌面内核连接文件系统、语音、浏览器、社交平台和多个 Agent。架构必须确保：

- 对用户只交付必要确认与最终结果。
- Agent 和平台能力可插拔、可降级、可替换。
- 所有高风险动作都经过统一的本地权限代理。
- 浏览器凭据、桌面文件和宠物成长数据默认留在本机。
- 多宠物与分支进化不改变安全边界，也不生成可自行执行的代码。

## 总体分层

```text
Desktop Pet / Widgets / Voice / Result-only Conversation
                         │
                  Result Presentation Gate
                         │
        Conversation Controller / Intent Router
                         │
          Permission Broker / Confirmation Tokens
                         │
                 Capability Orchestrator
      ┌──────────┬───────────┬───────────┬──────────┐
      │          │           │           │          │
   Codex     Claude Code   Hermes     Cursor    OpenClaw
   Adapter      Adapter     Adapter    Adapter    Adapter
      │          │           │           │          │
      └──────────┴──────┬────┴───────────┴──────────┘
                        │
          System / Browser / WeChat Adapters
                        │
 File Index / SQLite / Audit / Notification / Evolution
                        │
             Windows & macOS Platform Layer
```

推荐应用壳为 `Tauri 2 + React + TypeScript`，核心能力使用 Rust。平台能力统一封装在 `PlatformServices` 后面：

```ts
interface PlatformServices {
  knownFolders(): Promise<KnownFolder[]>;
  watchFiles(roots: string[]): AsyncIterable<FileEvent>;
  openFile(fileId: string): Promise<ActionResult>;
  moveToTrash(fileId: string): Promise<UndoHandle>;
  listPrograms(): Promise<ProgramEntry[]>;
  launchProgram(programId: string): Promise<ActionResult>;
  positionWidget(anchor: "top-right"): Promise<void>;
  hideMainToTray(): Promise<void>;
  showPetContextMenu(): Promise<void>;
  showNotification(input: NotificationInput): Promise<void>;
  registerGlobalShortcut(shortcut: string): Promise<void>;
}
```

- Windows：Known Folder API、`ReadDirectoryChangesW`、`ShellExecuteExW`、系统托盘、原生菜单、回收站与 Windows Toast。
- macOS：URL/目录授权书签、FSEvents、`NSWorkspace`、菜单栏、原生菜单、废纸篓与 UserNotifications。
- macOS 的文件夹、麦克风、自动化与辅助功能权限必须按实际能力逐项申请，不使用宽泛预授权。

### 原生桌面壳

- 主组件是无边框透明窗口。首次显示时根据当前显示器工作区计算右上角坐标；用户拖动后可保存自定义位置。
- “向上收起”是隐藏主窗口并保留托盘/菜单栏进程，不是操作系统最小化，因此隐藏后不占任务栏或 Dock。
- 宠物右键通过 Rust/Tauri 原生菜单弹出，菜单项再向前端发送语义事件；不得用 DOM 浮层伪装系统菜单。
- 宠物窗口默认不进入任务栏。桌面底层模式启用点击穿透后无法接收拖动或右键，应从托盘或主组件恢复交互层级。
- 浏览器预览不具备托盘、工作区坐标、原生菜单和系统启动器，只用于布局与状态测试。

## 结果式极简对话网关

这是架构级硬边界，不是 UI 偏好。所有执行后端可以产生内部事件，但进入用户对话前必须经过 `ResultPresentationGate`。

用户可见事件仅允许：

```ts
type UserVisibleEvent =
  | { type: "user_message"; text: string }
  | { type: "permission_request"; request: PermissionRequest }
  | { type: "final_result"; result: FinalResult };
```

以下内容永远不得进入对话消息流：

- 任务树、子 Agent 名单与路由决策。
- 工具名称、参数、调用记录和命令输出。
- 步骤、计划、过程播报、百分比和 token 流。
- 模型内部草稿、思考链、隐藏推理或选择后端的理由。

内部可观测性与用户对话完全分离：

```text
AdapterEvent -> Internal Event Bus -> Audit / Metrics / Recovery
                             └──────> Result Aggregator
                                         └────> Result Presentation Gate
```

审计只保存动作、对象、权限、时间、结果、错误代码和可撤销信息，不保存模型私有思考链。执行中 UI 可播放无文字宠物动作，但不得把动作转换成“第几步”或“调用了什么工具”的状态提示。

`FinalResult` 应面向结果建模：

```ts
interface FinalResult {
  outcome: "completed" | "partial" | "failed" | "needs_input";
  message: string; // 如：这个活搞定啦，需要我帮主人打开看一眼嘛？
  artifacts?: ArtifactLink[];
  choices?: ResultChoice[];
  undo?: UndoHandle;
}
```

### 当前会话隔离实现

- `local`、`codex`、`claude`、`hermes` 各自拥有版本化 `DialogueSession`，保存有界消息历史、草稿、附件路径、工作区、结果文件和活动任务 ID。
- 本地文件、整理、待办和社交动作始终写入 `local`；Agent 状态、完成结果、停止和错误按任务自身的 connector 回写，不读取当前选中的标签作为归属依据。
- 当前只允许一个本机 Agent 进程任务同时执行，但用户可在任务运行时切换查看其他会话；其他会话不会继承忙碌状态、草稿或附件。
- CLI 原生 resume 不可靠时，只向当前 provider 注入最近 14 条、最多 12,000 字符的纯文本历史。历史只提供上下文，不构成重新执行旧动作的授权。

## 统一适配器协议

执行适配器统一实现能力发现、任务执行、取消和恢复；其事件仅供内部编排使用：

```ts
type AdapterId =
  | "codex"
  | "claude-code"
  | "hermes"
  | "cursor"
  | "openclaw"
  | "wechat";

interface CapabilityAdapter {
  id: AdapterId;
  health(): Promise<AdapterHealth>;
  capabilities(): Promise<Capability[]>;
  start(input: TaskRequest, grant: PermissionGrant): AsyncIterable<InternalTaskEvent>;
  cancel(taskId: string): Promise<void>;
  resume(taskId: string, input: ResumeInput): AsyncIterable<InternalTaskEvent>;
}
```

适配器约束：

- **Codex**：只使用公开可调用的 CLI、SDK、MCP 或正式任务接口，不依赖受保护的应用内部二进制路径。
- **Claude Code**：使用结构化输出、明确工作目录和最小允许工具集合，禁止危险的全权限绕过。
- **Hermes**：优先 ACP/MCP/headless 服务，其次才是受限 one-shot 模式。
- **Cursor**：使用公开 CLI、深链接或正式扩展协议；不可直接修改 Cursor 私有会话存储。
- **OpenClaw**：只通过其公开协议、本地服务或明确配置的连接点接入。
- **微信**：优先官方开放能力和用户操作可见的桌面深链接；不得抓取加密数据库、会话密钥、验证码或绕过客户端保护。

适配器只能申请声明过的 capability。最终能否执行由本地 `PermissionBroker` 决定，而不是由模型自行决定。

## 本地桌面内核

### 文件监听与索引

```text
Known Folders / User-authorized Roots
                 │
       Initial Scan + Reconciliation
                 │
 Windows Watcher / macOS FSEvents
                 │
       Debounce + Single Writer Queue
                 │
        SQLite WAL + FTS5 Index
```

核心模块：

- `known_folders`：解析桌面、下载、文档以及用户授权目录。
- `file_watcher`：合并新建、修改、删除和重命名事件。
- `indexer`：初次扫描、启动对账和遗漏恢复。
- `classifier`：扩展名分类、自定义标签和虚拟分类。
- `search_engine`：文件名、时间、类型、全拼、拼音首字母和编辑距离打分。
- `safe_action_executor`：通过文件 ID 打开或整理文件，不接受任意 Shell 字符串。

普通文件只有在候选唯一、路径仍存在且位于授权目录时才可直接打开。可执行文件、脚本、快捷方式、应用包以及跨授权目录操作必须确认。

### 一键整理、安全复查与排除规则

```text
扫描桌面顶层 -> 生成批次与目标分类 -> 安全移动
            -> 展示复查清单
            -> 保留 / 放回并记住 / 撤销剩余批次
```

- 整理只处理桌面顶层普通文件和文件夹，跳过隐藏项、系统项、符号链接、重解析点和既有“虫洞派整理”目录。
- 每次整理保存短期批次 ID、原路径、目标路径和分类。中途失败应回滚，重名目标使用可预测的安全重命名。
- 复查选择“放回并记住”时，先校验项目仍属于该批次，再放回桌面并写入本机排除规则。
- 排除规则当前按规范化后的桌面顶层名称匹配；用户可在“已忽略整理”中删除规则。它不是内容哈希或智能判断。
- 复查后的未排除项目继续保留撤销能力；批次过期、路径越界或文件已变化时安全失败。

### Windows 桌面图标分层

- 个人桌面与旧整理库由普通“一键整理”处理；该命令默认且始终不包含 Public Desktop。
- Public Desktop 使用独立纯共享批次，界面先显示跨用户影响，再二次确认并逐次请求 UAC。调用只接受主窗口，开发版、便携版及用户可写安装目录禁用。
- 公共提权计划包含 5 分钟时效、SHA-256、源卷序列号、file ID、类型、大小、mtime 与递归内容摘要；helper 完成后逐项验证来源消失、目标存在且身份/摘要匹配。
- 公共撤销和复查同样需要跨用户二次确认，并始终经过受控 helper。失败时若目标身份不再匹配，返回 `recovery_required`，不按裸路径把替换物回滚到 Public Desktop。
- 同卷优先原子重命名；`ERROR_NOT_SAME_DEVICE` 时复制到新目标、校验摘要，再移除或隔离来源。目录递归拒绝符号链接和重解析点。
- `HideIcons` 只改变当前用户的 Explorer 显示策略，可视觉隐藏个人、公共和系统图标，不代表任何文件被物理迁移。

### 常用程序目录

- 系统发现只读取受控应用入口，不遍历整块磁盘。Windows 适配器读取系统应用入口与开始菜单快捷方式；macOS 适配器使用 Launch Services/`NSWorkspace` 和 `.app` 包。
- 整理复查可把识别为可启动的项目加入用户收藏。当前分支的收藏是本地 UI 持久化，后续可迁移到 SQLite。
- 点击启动只接受系统发现或用户收藏返回的精确目标，不接受任意 Shell 字符串、额外参数或工作目录注入。
- 普通文件列表中的可执行文件继续走高风险规则，不因出现在桌面而自动获得启动权限。

### 本地意图与语音

```text
Microphone -> Wake/VAD -> Local ASR -> Text Normalization
           -> Negation -> Rules -> Entity Extraction -> Fuzzy Search
           -> Permission Broker -> Action -> Local TTS Result
```

桌面整理、文件检索、待办、意见、通知和社交页面跳转使用本地规则与拼音模糊匹配，无需大模型 API。开放式任务再交给适配器。

### SQLite 主要实体

```text
watched_roots
files / files_fts
categories / file_categories
tags / file_tags
inbox_items          // idea, note, todo, file_note
notifications
intent_aliases
organize_exclusions
program_favorites     // 目标持久层；当前分支仍使用 UI 本地存储
operation_log
permission_grants
confirmation_tokens
pet_profiles
pet_branches
evolution_events
adapter_sessions
```

`inbox_items` 是桌面意见整理与今日待办的统一收件箱，可关联文件、标签、到期时间和来源语音。

## 权限代理

权限只能由虫洞派本地策略判断：

```text
读取已授权目录             -> 可按会话或持久范围授权
修改当前项目               -> 显示目录、影响范围与撤销信息
打开普通文件               -> 路径校验后允许
启动已发现/已收藏常用程序  -> 校验精确目标后允许，不接受附加参数
执行其他程序、脚本或快捷方式 -> 单独确认
删除文件                   -> 默认进入回收站/废纸篓并提供撤销
上传、发布、发送消息       -> 动作发生前最终确认
读取浏览器页面             -> 站点、账号、动作、有效期授权
支付、账号与安全设置       -> 默认拒绝或要求强确认
```

确认使用一次性短时 token，并绑定动作、目标、账号、内容摘要、附件哈希和有效期。任何一项变化都必须重新确认。

## 浏览器会话安全

推荐浏览器扩展配合 Native Messaging 或受控 MCP 连接器：

```text
Wormhole Pie requests a named capability
-> Connector verifies domain, account and visible login state
-> User grants scoped access
-> Connector acts inside the browser profile
-> Connector returns a redacted structured result
```

安全要求：

- 登录会话始终由浏览器 Profile 和站点 Cookie Store 持有。
- 不导出、复制或记录原始 Cookie、OAuth token、密码、验证码或密码管理器内容。
- 授权按域名、账号、动作和有效期限定，禁止“一次授权全网可用”。
- 平台存在官方 API 时优先使用 API，不用页面自动化替代稳定 API。
- DOM 结构、账号或目标页面不符合预期时安全失败，不猜测元素、不盲点发布。
- 浏览器连接器日志必须脱敏，正文与附件仅保留任务所需范围。

### 当前 Windows 托管 Edge 连接器

- 小红书、X、抖音各自使用 `app_local_data_dir/browser-sessions/<platform>` 下的独立 Edge Profile，浏览器自身持有 Cookie 和登录状态。
- Edge 以 `--remote-debugging-pipe` 启动，通过私有继承管道通信，不开放本机 TCP 调试端口；进程树加入 `KILL_ON_JOB_CLOSE` Job。
- 只有当受控页面域名唯一匹配、账号页和选择器均确认、显示名称与账号标识可见时，数据库才写入 `connected=true`。
- 汇总字段分别标记 `visible-page`、`manual` 或 `unavailable`。同步未识别到字段时清零并标记不可用，不沿用旧数字；确认入口存在但没有 badge 时写入可见的零值。
- 页面脚本禁止访问 `document.cookie`、Cookie Store、local/session storage、IndexedDB、私信正文或整页 HTML。
- “断开”关闭受控窗口但保留 Profile；“清除登录资料”关闭进程并删除该平台 Profile 与快照。
- 自定义外链只允许 HTTPS，拒绝 credentials，移除 fragment 和敏感查询参数；日志只记录安全的 origin 与 path。

## 外部发布与消息最终确认

准备草稿和打开创作页可以自动完成；真正对外发布、发送微信消息、上传附件、删除内容或修改可见范围必须停在最后一步，向用户展示：

- 平台与账号。
- 接收人或公开可见范围。
- 标题、正文摘要和链接。
- 附件名称、数量和敏感提示。
- 即将执行的唯一动作。

用户确认后才签发一次性 token。确认不可复用，超时、内容变化、账号变化或页面变化均使 token 失效。结果消息只说明是否发布成功及可打开的结果链接，不展示自动化点击步骤。

## 多宠物与分支自进化

多宠物是呈现和路由层，不是额外权限主体。每只宠物可以拥有不同角色与能力偏好，但所有动作共用 `PermissionBroker`。

```ts
interface PetProfile {
  id: string;
  role: "organizer" | "maker" | "messenger" | "explorer" | "custom";
  activeBranchId: string;
  voiceProfileId?: string;
  userOverrides: Record<string, unknown>;
}

interface PetBranch {
  id: string;
  petId: string;
  parentBranchId?: string;
  version: number;
  traits: TraitSet;
  assetPackIds: string[];
  unlockedCapabilities: string[];
  status: "preview" | "active" | "archived";
}
```

进化引擎规则：

- 根据任务类别、完成质量、使用习惯和明确反馈提出分支候选。
- 分支资产版本化、可解释、可预览、可回退，用户选择后才激活。
- 多只宠物可沿不同分支成长，保留共同基础能力。
- 不允许生成或执行自修改代码，不允许修改权限策略。
- 外观和动作资产必须经过签名、尺寸、透明度、敏感内容和性能预算检查。
- 分支变化不得改变结果式极简对话规则。

当前实现采用受控资产选择，而不是运行时任意生成：

- 自动模式根据完成待办、整理文件、Agent 成功、互动、喂食和活跃天数计算路线分数与总成长点，同时选择 `companion | creator | guardian` 和 `seedling | growing | evolved`。
- 手动模式完全脱离指标，可分别指定路线和阶段；待办反选、撤销整理、复查放回和撤销喂食会扣回相应指标。
- Blender 5.1.2 源文件包含 5 物种 × 3 阶段、每套 Armature/骨骼和路线配饰。7 个稳定动作族映射全部 44 个业务动作。
- 桌面运行时不嵌入实时 3D 引擎，而加载 Blender 离线渲染的 256×256 RGBA PNG；manifest 记录 Blender 版本、脚本 SHA-256、动作映射和回退路径。

## 平台状态与限制

- 当前主要构建与验收环境是 Windows。整理批次、排除规则、托盘、程序发现、窗口定位与原生右键菜单仍应随最终桌面包做一次联合验收。
- macOS 复用同一领域模型，但菜单栏、应用发现、目录授权书签、签名/公证及辅助功能权限尚未完成同等实现与发布验证。
- 浏览器模式中的程序列表、整理结果和菜单行为是模拟数据，不可作为原生能力通过的证据。
- 多显示器右上角定位应以窗口所在显示器的工作区为准，并避开任务栏、Dock 与菜单栏；不同缩放比例需要分别测试。

## 推荐实施阶段

1. **Windows 桌面内核验收**：窗口定位、托盘收起、原生宠物菜单、文件索引、真实整理复查、常用程序与安全打开。
2. **结果式对话网关**：三类可见事件、权限卡片、最终结果卡片与统一自然话术。
3. **本地语音与检索**：本地 ASR、关键词/拼音匹配、歧义选择和本地 TTS。
4. **macOS 平台对齐**：菜单栏、应用发现、目录权限、签名公证和双平台行为测试。
5. **Agent 适配器**：先完成一个端到端适配器，再依次接入 Codex、Claude Code、Hermes、Cursor 与 OpenClaw。
6. **浏览器与微信**：先支持只读状态、打开目标页面和准备草稿，再开放带最终确认的发送/发布。
7. **多宠物与分支进化**：固定角色、版本化分支和签名资产包，验证稳定后再加入生成式外观。

## 架构验收底线

- 任意适配器产生的任务树、工具调用、步骤、进度或思考文本都无法进入用户对话流。
- Windows 与 macOS 对同一权限动作给出一致语义和相同确认信息。
- 无原始浏览器凭据进入 SQLite、日志、适配器输入或结果卡片。
- 未取得最终确认时，无法发布内容、发送微信消息或上传附件。
- 宠物切换、协作与进化不能扩大已授予权限。
- 适配器不可用时能给出简洁最终结果并安全降级，不暴露内部错误栈。
