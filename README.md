# 虫洞派（Wormhole Pie）

虫洞派是一个本地优先的桌面组件、电子宠物与本机 Agent 入口。当前版本以 Tauri 2、React、Rust 和 SQLite 实现，主组件保持 iOS 小组件式的透明、克制体验，复杂执行过程不会出现在用户对话中。

Windows 是当前主要验收平台；GitHub Actions 已配置 Windows NSIS/MSI 与 macOS Universal DMG 双系统安装包构建。macOS 原生能力仍需要在真实 Mac 上继续验收。

## 安装包

GitHub 仓库的 Actions 页面支持手动运行 `Build Wormhole Pie Installers`：

- `wormhole-pie-windows`：Windows NSIS `.exe` 与 MSI `.msi`。
- `wormhole-pie-macos-universal`：同时支持 Intel 与 Apple Silicon 的 Universal `.dmg`。

未配置 Apple 证书时仍可生成 macOS 测试安装包，但 Gatekeeper 会提示应用来自未认证开发者。正式分发需要在 GitHub Secrets 中配置 `APPLE_CERTIFICATE`、`APPLE_CERTIFICATE_PASSWORD`、`APPLE_SIGNING_IDENTITY`、`APPLE_ID`、`APPLE_PASSWORD` 和 `APPLE_TEAM_ID`。

## 当前已实现

### 本轮功能更新

- 全局中英文切换：主窗口、文件资料库、整理复查、宠物设置和 Agent 管理界面会同步切换语言；用户自己的文件名、任务名和宠物名保持原文。
- 原生语言同步：宠物右键菜单、系统托盘、三个窗口标题、桌面宠物提示和独立对话窗口会随语言切换更新。
- 对话会话隔离：本地助手、Codex、Claude Code 和 Hermes 分别保存有界消息历史、草稿、附件、工作区与任务状态；结果、停止和错误不会串入其他连接器。
- 社交账号中心：Windows 上为小红书、X、抖音分别启动受控 Edge Profile，可验证当前账号页面并聚合页面可见的粉丝、未读私信数量和通知数量；支持断开窗口或清除该平台的本地登录资料。
- 指标进化：自动模式依据整理、待办、Agent 成功、互动、喂食和活跃天数选择路线与阶段；手动模式可分别选择路线和阶段，撤销操作会回滚对应成长指标。
- Blender 资产管线：使用 Blender 5.1.2 生成带 Armature 的 3D 源模型，再离线渲染为透明 2D PNG；7 个动作族覆盖 44 个运行时动作，三条进化路线拥有独立配饰。
- Agent 管理：支持 Codex、Claude Code、Hermes Agent 的一键安装/重装、安装目录和任务目录选择；中文界面使用 npm/PyPI 国内镜像，英文界面使用官方源，并自动处理 Node/Python 依赖。
- Agent 配置：可填写 API 地址、模型和密钥，使用“测试通断”检查地址可达性，再将配置写入对应 Agent 的默认配置文件；密钥不会保存到前端 localStorage。
- 资料库撤回：文件资料库支持右键菜单单项“撤回到桌面”，以及带二次确认的一键批量撤回；重名文件会自动改名，撤回后索引和整理状态同步更新。
- 本地预览和生产构建都覆盖了整理、撤回、语言切换、Agent 安装模拟和 API 连通测试流程。

### 桌面组件与原生窗口

- 主组件约 `410 × 660`，无边框透明，首次启动位于当前显示器工作区右上角，可自由拖动。
- 主组件、宠物和宠物对话均不占任务栏；向上收起后保留 Windows 系统托盘入口。
- 应用采用单实例运行。重复启动会唤回现有主组件，并把宠物恢复到可见工作区。
- 宠物是独立透明窗口，支持普通、桌面底层和置顶三种层级。
- 桌面底层模式保持完整交互，仍然可以点击、右键、拖动和接收文件。
- 宠物右键使用 Windows 原生菜单；托盘提供“找回宠物（普通层级）”。
- 对话使用独立的 `pet-dialogue` 窗口，显示在宠物旁边，并会根据屏幕边缘自动选择左右位置。
- 对话窗口支持自由缩放、窗口边角拖拽、消息滚动条和自动滚动到最新结果。

### 桌面整理与资料库

- 同时索引 Windows 桌面和 `Documents\虫洞派资料库`，整理后文件不会从组件列表中消失。
- 一键整理将桌面顶层项目按文件夹、文档、图片、视频、音频、压缩包、代码、快捷方式、程序和其他类型移入资料库。
- 旧版桌面整理目录中的内容会在整理时迁移到 `Documents\虫洞派资料库`，不再把整理库留在桌面上。
- 支持“整理新增项目”，不会因为上一批仍可撤销而阻止第二次整理。
- 整理批次持久化保存；可撤销最近一批，并在重启应用后恢复撤销状态。
- 整理结果提供安全复查：可将项目放回桌面并记住以后不再整理；忽略规则可随时移除。
- 对重名、越界、符号链接、重解析点、隐藏或系统项目执行安全检查；失败时尽量整批回滚。
- 普通“一键整理”只处理个人桌面；公共桌面项目使用单独的跨用户警告、二次确认和按次 UAC，并且仅在安装到 `Program Files` 的正式版本中启用。
- 公共桌面提权计划带时效、SHA-256、Windows 文件身份及移动前后摘要校验；跨卷文件/目录使用复制、校验、隔离来源的回退流程。
- Windows 桌面图标隐藏/恢复仅修改当前用户的 `HideIcons`，可把回收站、此电脑、不可迁移项目和公共快捷方式做“视觉收纳”，不会谎报为物理移动。
- “常用程序”合并 Windows 开始菜单、桌面、资料库和用户收藏，可直接交给系统启动器打开。

### 宠物、对话与本地语音

- 宠物眼睛跟随鼠标，并包含眨眼、招手、伸懒腰、探头、进食和完成反馈等动作。
- 支持糯糯柯基、星星猫、桃桃兔、波波水獭和糖糖熊五个角色。Blender 源文件包含 15 套阶段模型、真实 Armature/骨骼和路线配饰，运行时使用 256×256 RGBA 逐帧渲染图。
- 自动进化同时决定路线与阶段；关闭自动模式后，可手动选择陪伴、灵感、守护路线以及幼苗、成长、进化阶段。
- 极简对话只显示用户消息、必要确认和最终结果，不展示任务树、工具调用或思考过程。
- 可一键识别本机 Codex、Claude Code 与 Hermes Agent 配置，并从宠物对话框直接交付任务。
- Agent 长任务采用 12 小时安全上限、进程心跳与运行计时；前台不会再因 120 秒而误报失败。
- 对话支持将文件直接拖入或通过上传按钮选择；最多 8 个附件，发送前可移除。
- 工作区外附件会复制为任务级安全快照，任务结束后自动清理；文件内容和绝对路径不会写入公开日志。
- Agent 返回的工作区文件会显示为可点击文件卡片；脚本以文本方式安全打开，可执行文件不会被直接运行。
- 纯文字对话可独立使用，也可以关闭整个对话功能。
- Windows 语音输入已经接入本机 `System.Speech`，不调用大模型 API。它使用已安装的中文 Windows Speech Recognizer 和默认麦克风，单次识别时间约 8 秒。
- 语音默认关闭；若系统缺少 `zh-CN` 识别器、麦克风权限或音频设备，界面会返回明确错误。当前只有语音转文字，没有 TTS 语音播报。
- 桌面或 `Documents\虫洞派资料库` 内的文件都可以拖给宠物；文件会进入 Windows 回收站，不做永久删除，并可撤销最近一次喂食。
- 强制休息不会隐藏或移动真实鼠标，支持可见的长按 3 秒按钮和 `Esc` 紧急退出。

### 待办、意见与快捷入口

- 今日待办支持创建、完成以及与本地动作关联。
- 意见整理支持记录想法、反馈、灵感，并可转为待办。
- 小红书、X、抖音提供托管 Edge 登录、可见汇总同步、创作页快捷入口和本地关键词匹配。发布页复用同一平台 Profile，但不会代替用户完成最终发布。
- 文件、待办、意见和应用内通知保存在本机；文件检索当前主要基于名称、类型、分类与模糊匹配，不读取文件正文。

## 本地数据与安全边界

- 桌面索引、资料库索引、整理批次、忽略规则和操作记录保存在本地 SQLite 或本地设置中。
- 文件正文、原始浏览器 Cookie、账号令牌和密码不会写入对话、SQLite 或操作日志；登录状态始终由 Edge Profile 与站点 Cookie Store 持有。
- 普通文件通过索引 ID 和授权路径校验后打开；程序入口只接受受控扫描结果或用户明确收藏的路径。
- “喂食”只接受桌面和虫洞派资料库内的项目，并使用系统回收站。
- 社交连接只读取已验证账号页面上可见的账号名称和汇总数字，不读取私信正文、Cookie Store、localStorage、sessionStorage 或 IndexedDB。
- 自定义外链只允许 HTTPS，拒绝 URL credentials，并在打开和日志记录前移除 fragment 与敏感查询参数。
- 社交快捷入口只打开目标页面，不会代替用户执行最终发布。

## QA

- [`.github/workflows/quality.yml`](.github/workflows/quality.yml) 会在 `main` 推送和 Pull Request 上并行执行前端生产构建、Windows Rust 严格检查与 macOS Rust 严格检查。
- 发布工作流会在生成安装包前重复执行目标平台检查，避免仅在 Windows 本地通过、到 macOS 打包阶段才暴露条件编译错误。
- Windows 回收站往返测试需要交互式 Explorer 回收站，默认测试集中标记为忽略；在真实 Windows 会话中使用 `cargo test windows_pet_feed_round_trip_uses_the_system_recycle_bin -- --ignored` 单独验收。
- 浏览器预览用于本地 DOM、状态切换和布局检查，不代表 Windows 原生窗口已经通过验收。
- Windows 原生 QA 使用 WebView2 CDP 的 `Runtime.evaluate` 检查 `main`、`pet`、`pet-dialogue` DOM，并通过只读 Win32 P/Invoke 检查窗口标题、可见性、矩形、右上角位置、扩展样式、任务栏状态及宠物与对话的相邻关系。
- 原生 QA 不点击一键整理，不删除文件，也不打开社交网页；每次最终 EXE 构建后仍需重新运行。

## 开发运行

带热更新的桌面 Debug 模式（推荐）：

```powershell
pnpm install
pnpm debug
```

需要一个不依赖 `localhost:1420`、可直接双击运行的独立 Debug EXE：

```powershell
pnpm debug:standalone
& '.\src-tauri\target\debug\wormhole-pie.exe'
```

不要使用 `cargo run`、`cargo build` 或 IDE 的普通 Rust 启动按钮来生成桌面 Debug 版。这些方式会绕过 Tauri CLI：Debug WebView 会指向 `http://localhost:1420`，但 Vite 没有启动，因此会出现 `ERR_CONNECTION_REFUSED`。`pnpm debug` 会自动管理 Vite；`pnpm debug:standalone` 则会把 `dist` 前端资源嵌入 Debug EXE。

仅运行浏览器预览：

```powershell
pnpm dev
```

类型检查：

```powershell
pnpm check
```

重新生成 Blender 3D 源文件与透明 PNG 精灵：

```powershell
& 'C:\Program Files\Blender Foundation\Blender 5.1\blender.exe' `
  --background --factory-startup `
  --python tools\blender\generate_pet_models.py
python tools\blender\validate_pet_renders.py
```

生成器会更新 `assets/blender/wormhole-pets.blend`、`public/pets/blender-rendered/` 和 `src/pet/blenderMotionMap.json`。运行时不会调用 Blender。

构建当前系统安装包：

```powershell
pnpm tauri build --bundles nsis,msi
```

macOS Universal DMG（需要在 macOS 上执行）：

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
pnpm tauri build --target universal-apple-darwin --bundles dmg
```

仓库也包含 [`.github/workflows/build-installers.yml`](.github/workflows/build-installers.yml)，可以直接使用 GitHub macOS runner 生成 DMG。

## 当前限制与尚未实现

- macOS 安装包已具备 CI 构建能力；菜单栏、目录授权、应用发现、语音、窗口层级和安装体验尚未完成与 Windows 同等的真实设备验收。
- 社交聚合目前是 Windows 托管 Edge 的实验性只读连接器，不是平台官方 API。页面 DOM 变化、多账号同域页面或未进入受支持的账号页时会安全失败；不会读取私信正文。
- Edge Profile 可长期保持登录，但虫洞派不会提取、复制或保存原始 Cookie。macOS 对应的浏览器会话连接器尚未实现。
- Codex、Claude Code 与 Hermes 已接入本机 CLI；Cursor、OpenClaw 和微信的执行适配仍待接入。
- 当前自动进化是“指标驱动选择已签入的路线、阶段和 Blender 渲染资产”，不会在运行时生成任意模型、执行自修改代码或扩大权限。
- 尚未实现 TTS 语音播报。
- Blender 模型和骨骼动画用于离线生产资产；桌面运行时仍采用透明 2D PNG 与 DOM 合成，而不是实时 3D 渲染引擎。
- 公共桌面移动会影响所有 Windows 用户，必须使用正式安装版逐次授权；开发版、便携版和用户可写安装目录会主动禁用该能力。真实双账号/UAC 回归应只在一次性 VM 中进行。
