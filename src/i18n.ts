import { useEffect } from "react";

export type AppLocale = "zh-CN" | "en-US";

const exactTranslations = new Map<string, string>([
  ["虫洞派", "Wormhole Pie"],
  ["本地", "Local"],
  ["首页", "Home"],
  ["文件", "Files"],
  ["待办", "Tasks"],
  ["意见", "Ideas"],
  ["宠物", "Pet"],
  ["桌面资料库", "Desktop library"],
  ["社交媒体", "Social media"],
  ["今天", "Today"],
  ["查看全部", "View all"],
  ["文件资料库", "File Library"],
  ["桌面整理结果", "Desktop organization results"],
  ["一键整理", "One-click organize"],
  ["关闭整理结果", "Close organize results"],
  ["个人桌面已整理", "Personal desktop organized"],
  ["整理统计", "Organization summary"],
  ["本次新增", "New this time"],
  ["资料库已整理", "Organized in library"],
  ["跳过项", "Skipped"],
  ["隐藏桌面图标", "Hide desktop icons"],
  ["快速复查", "Quick review"],
  ["哪些不该收进去？", "What should stay on the desktop?"],
  ["加入常用", "Add to favorites"],
  ["勾选后放回桌面，并记住以后不再整理同名项目。", "Selected items return to the desktop and matching names will be excluded next time."],
  ["撤销整理", "Undo organization"],
  ["保存复查", "Save review"],
  ["文档", "Documents"],
  ["图片", "Images"],
  ["视频", "Videos"],
  ["音频", "Audio"],
  ["压缩包", "Archives"],
  ["代码", "Code"],
  ["其他", "Other"],
  ["今天待办", "Today's Tasks"],
  ["意见整理", "Idea Inbox"],
  ["常用程序", "Apps"],
  ["程序", "Apps"],
  ["文件夹", "Folders"],
  ["快捷", "Shortcuts"],
  ["快捷方式", "Shortcuts"],
  ["待整理", "Unsorted"],
  ["资料库", "Library"],
  ["· 资料库", "· Library"],
  ["全部分类", "All categories"],
  ["按类型", "By type"],
  ["按名称", "By name"],
  ["按创建时间", "By created time"],
  ["按首字母", "Alphabetical"],
  ["搜索文件…", "Search files…"],
  ["搜索桌面文件…", "Search desktop files…"],
  ["自动", "Auto"],
  ["已整理", "Organized"],
  ["桌面", "Desktop"],
  ["创建时间未知", "Created time unknown"],
  ["正在查找本机程序…", "Finding local apps…"],
  ["这个分类还没有项目", "No items in this category"],
  ["读取中", "Loading"],
  ["暂无", "None"],
  ["检查新增", "Check new"],
  ["整理中", "Organizing"],
  ["还没有个人文件", "No personal files yet"],
  ["正在查找常用程序…", "Finding common apps…"],
  ["整理复查时可加入常用", "Add favorites during organize review"],
  ["添加", "Add"],
  ["我的宠物", "My Pet"],
  ["名字", "Name"],
  ["给它起个名字", "Give it a name"],
  ["选择角色", "Choose character"],
  ["动作速率", "Motion frequency"],
  ["外观主题", "Appearance theme"],
  ["进化路线", "Evolution path"],
  ["任务 Agent", "Task Agents"],
  ["一键识别配置", "Detect configurations"],
  ["正在识别…", "Detecting…"],
  ["正在识别 Claude、Hermes 和 Codex…", "Detecting Claude, Hermes and Codex…"],
  ["本地助手", "Local Assistant"],
  ["文件、待办与社交入口", "Files, tasks and social shortcuts"],
  ["任务目录", "Task directory"],
  ["安装目录", "Install directory"],
  ["浏览", "Browse"],
  ["一键安装", "Install"],
  ["重新安装", "Reinstall"],
  ["安装中…", "Installing…"],
  ["自动安装所需依赖", "Install required dependencies automatically"],
  ["中国镜像源", "China mirror"],
  ["国际官方源", "Global official source"],
  ["API 配置", "API configuration"],
  ["接口地址", "Base URL"],
  ["API 密钥", "API key"],
  ["模型", "Model"],
  ["测试通断", "Test connection"],
  ["测试中…", "Testing…"],
  ["保存配置", "Save configuration"],
  ["已保存", "Saved"],
  ["对话窗口", "Dialogue window"],
  ["只显示你的话、必要确认和最终结果", "Only your messages, necessary confirmations and final results"],
  ["语音", "Voice"],
  ["默认关闭，开启后才显示麦克风", "Off by default; the microphone appears when enabled"],
  ["显示宠物", "Show pet"],
  ["独立透明窗口，可直接拖到任意位置", "A transparent window you can drag anywhere"],
  ["显示层级", "Display layer"],
  ["普通", "Normal"],
  ["软件可以盖住", "Apps can cover it"],
  ["桌面底层", "Desktop layer"],
  ["软件盖住，桌面可点", "Behind apps; desktop stays clickable"],
  ["置顶", "Always on top"],
  ["一直陪着你", "Always stays with you"],
  ["宠物设置", "Pet settings"],
  ["休息设置", "Break settings"],
  ["已忽略整理", "Organize exclusions"],
  ["暂时收起", "Hide for now"],
  ["退出组件", "Quit"],
  ["通知", "Notifications"],
  ["关闭", "Close"],
  ["返回首页", "Back to home"],
  ["新增待办…", "Add a task…"],
  ["进行中", "In progress"],
  ["已完成", "Completed"],
  ["待开始", "Not started"],
  ["今天的事情都完成了", "Everything is done for today"],
  ["想让我做什么？", "What would you like me to do?"],
  ["告诉派派要做什么…", "Tell Paipai what to do…"],
  ["对话已关闭，点我设置", "Dialogue is off; click to configure"],
  ["这件事我还不会，换句话试试吧。", "I can't do that yet. Try asking another way."],
  ["任务完成啦。", "Task complete."],
  ["任务已经停下来了。", "The task has stopped."],
  ["正在温柔停下来…", "Stopping safely…"],
  ["当前没有可停止的 Agent 任务。", "There is no active Agent task."],
  ["暂时没能停下来，再试一次吧。", "I couldn't stop it yet. Please try again."],
  ["这个交付文件暂时打不开。", "This delivered file cannot be opened right now."],
  ["撤回到桌面", "Restore to desktop"],
  ["一键撤回到桌面", "Restore all to desktop"],
  ["撤销最近一次桌面整理", "Undo the latest desktop organization"],
  ["撤销最近整理", "Undo latest organization"],
  ["文件已撤回桌面", "Files restored to desktop"],
  ["文件暂时没能撤回桌面。", "The files could not be restored to the desktop."],
  ["没有找到可撤回的资料库项目，请刷新文件列表后重试。", "No restorable library items were found. Refresh the file list and try again."],
  ["确认全部撤回", "Confirm restore all"],
  ["打开", "Open"],
  ["更多操作", "More actions"],
  ["物理分类筛选", "File type filter"],
  ["资料库分类和排序方式", "Library grouping and sort"],
  ["程序、文件夹和快捷方式", "Apps, folders and shortcuts"],
  ["语言", "Language"],
  ["中文", "Chinese"],
  ["英文", "English"],
  ["关闭宠物设置", "Close pet settings"],
  ["糯糯", "Nuonuo"],
  ["星星", "Xingxing"],
  ["桃桃", "Taotao"],
  ["波波", "Bobo"],
  ["糖糖", "Tangtang"],
  ["热情、治愈", "Warm and healing"],
  ["安静、黏人", "Quiet and affectionate"],
  ["轻快、好奇", "Lively and curious"],
  ["聪明、活泼", "Clever and playful"],
  ["温柔、可靠", "Gentle and dependable"],
  ["温柔低频", "Gentle and infrequent"],
  ["身体动作约 45–90 秒一次，待机呼吸约 8 秒一轮", "Body actions every 45–90 seconds; idle breathing cycles around 8 seconds"],
  ["安静陪伴", "Quiet companion"],
  ["不主动做身体动作，只保留视线与任务回应", "No autonomous body actions; keeps gaze and task responses"],
  ["轻快陪伴", "Lively companion"],
  ["身体动作约 25–50 秒一次，仍避免连续切换", "Body actions every 25–50 seconds without rapid switching"],
  ["星雾紫", "Nebula purple"],
  ["薄荷青", "Mint"],
  ["蜜桃橘", "Peach"],
  ["陪伴系", "Companion path"],
  ["更多回应与休息动作", "More responses and break animations"],
  ["灵感系", "Inspiration path"],
  ["更多创作与庆祝动作", "More creative and celebration animations"],
  ["守护系", "Guardian path"],
  ["更多整理与专注动作", "More organizing and focus animations"],
  ["未安装", "Not installed"],
  ["浏览器预览无法检测本机 CLI 与配置", "Browser preview cannot inspect local CLIs or configuration"],
  ["浏览器预览：接口地址可达", "Browser preview: endpoint is reachable"],
  ["专注模式", "Focus mode"],
  ["工作时立即隐藏宠物，结束后再恢复", "Hide the pet while working and restore it afterward"],
  ["切换对话窗口", "Toggle dialogue window"],
  ["切换语音", "Toggle voice"],
  ["切换宠物显示", "Toggle pet visibility"],
  ["切换专注模式", "Toggle focus mode"],
  ["凭据只在点击保存时写入所选 Agent 的默认配置；测试通断只检查接口地址，不发送密钥。任务只交给你选择的本机 CLI；对话里不会显示终端、思考或工作流。", "Credentials are written only when you save them to the selected Agent's default configuration. Connection testing checks the endpoint without sending the key. Tasks run only through the local CLI you choose; the dialogue never exposes terminals, reasoning, or workflow details."],
  ["桌面底层会待在其他软件下方，但回到桌面后仍可拖动、右键；也能从托盘一键找回。", "Desktop layer stays behind other apps, remains draggable on the desktop, and can be restored from the tray."],
  ["一键识别 Claude、Hermes 和 Codex 配置", "Detect Claude, Hermes and Codex configurations"],
  ["向上收起到系统托盘", "Hide to system tray"],
  ["收起到系统托盘", "Hide to system tray"],
  ["查看通知", "View notifications"],
  ["组件菜单", "Widget menu"],
  ["桌面内容", "Desktop content"],
  ["整理桌面上的新增项目", "Organize new desktop items"],
  ["刷新文件索引", "Refresh file index"],
  ["已收起的首页栏目", "Collapsed home sections"],
  ["组件页面", "Widget pages"],
  ["告诉", "Tell "],
  ["要做什么…", " what to do…"],
  ["分钟前", "minutes ago"],
  ["小时前", "hours ago"],
  ["刚刚", "Just now"],
  ["星期一", "Monday"],
  ["星期二", "Tuesday"],
  ["星期三", "Wednesday"],
  ["星期四", "Thursday"],
  ["星期五", "Friday"],
  ["星期六", "Saturday"],
  ["星期日", "Sunday"],
]);

const dynamicTranslations: Array<[RegExp, (...groups: string[]) => string]> = [
  [/^本次新增整理了 (\d+) 个项目，跳过 (\d+) 个。$/, (moved, skipped) => `Organized ${moved} new item${moved === "1" ? "" : "s"}; skipped ${skipped}.`],
  [/^(\d+) 个项目已归入 (\d+) 个分类。$/, (items, categories) => `${items} item${items === "1" ? "" : "s"} organized into ${categories} categor${categories === "1" ? "y" : "ies"}.`],
  [/^检测到 (\d+) 个公共桌面图标，系统图标也可一并隐藏$/, (count) => `Found ${count} public desktop icon${count === "1" ? "" : "s"}; system icons can be hidden too`],
  [/^已撤回 (\d+) 个项目到桌面(?:；(\d+) 个重名项目已自动改名)?。$/, (count, conflicts) => `Restored ${count} item${count === "1" ? "" : "s"} to the desktop${conflicts ? `; renamed ${conflicts} conflicting item${conflicts === "1" ? "" : "s"}` : ""}.`],
  [/^(\d+) 个项目已安全放回桌面(?:；(\d+) 个重名项目已自动改名)?。$/, (count, conflicts) => `${count} item${count === "1" ? "" : "s"} safely restored to the desktop${conflicts ? `; renamed ${conflicts} conflicting item${conflicts === "1" ? "" : "s"}` : ""}.`],
  [/^确认撤回 (\d+) 项$/, (count) => `Confirm restoring ${count} item${count === "1" ? "" : "s"}`],
  [/^今天还有 (\d+) 件事$/, (count) => `${count} task${count === "1" ? "" : "s"} left today`],
  [/^点一下，和(.+)说句话$/, (name) => `Click to talk with ${name}`],
  [/^(\d+) 个项目$/, (count) => `${count} item${count === "1" ? "" : "s"}`],
  [/^待整理 (\d+) · 资料库 (\d+)$/, (pending, library) => `Unsorted ${pending} · Library ${library}`],
  [/^待整理 (\d+) · 资料库$/, (pending) => `Unsorted ${pending} · Library`],
  [/^常用程序 (\d+)$/, (count) => `Apps ${count}`],
  [/^整理新增 (\d+)$/, (count) => `Organize ${count} new`],
  [/^找到 (\d+) 个相似项，选一个吧。$/, (count) => `Found ${count} similar items. Choose one.`],
  [/^没有找到「(.+)」。$/, (keyword) => `Nothing matched “${keyword}”.`],
  [/^创建 (.+)$/, (time) => `Created ${time}`],
  [/^(.+) 分类$/, (name) => `${name} category`],
  [/^(.+) 更多操作$/, (name) => `${name} More actions`],
  [/^打开与(.+)的对话$/, (name) => `Open chat with ${name}`],
  [/^打开(.+)$/, (name) => `Open ${name}`],
  [/^与(.+)对话$/, (name) => `Talk with ${name}`],
  [/^告诉(.+)要做什么…$/, (name) => `Tell ${name} what to do…`],
  [/^(.+) 已完成任务$/, (name) => `${name} completed the task`],
  [/^(.+) 未完成任务$/, (name) => `${name} did not complete the task`],
  [/^(.+) 已停止$/, (name) => `${name} stopped`],
  [/^收起(.+)到右侧栏$/, (name) => `Collapse ${inlineEnglish(name)} to the sidebar`],
  [/^恢复(.+)$/, (name) => `Restore ${inlineEnglish(name)}`],
  [/^完成 (.+)$/, (name) => `Complete ${name}`],
  [/^(\d+) 分钟前$/, (count) => `${count} minutes ago`],
  [/^(\d+) 小时前$/, (count) => `${count} hours ago`],
  [/^(\d+) 天前$/, (count) => `${count} day${count === "1" ? "" : "s"} ago`],
  [/^(\d+)月(\d+)日 · 星期([一二三四五六日])$/, (month, day, weekday) => {
    const weekdays: Record<string, string> = { 一: "Mon", 二: "Tue", 三: "Wed", 四: "Thu", 五: "Fri", 六: "Sat", 日: "Sun" };
    return `${month}/${day} · ${weekdays[weekday]}`;
  }],
];

function inlineEnglish(value: string) {
  return exactTranslations.get(value.trim()) ?? value;
}

type RenderState = { source: string; rendered: string };
const textStates = new WeakMap<Text, RenderState>();
const attributeStates = new WeakMap<Element, Map<string, RenderState>>();
const translatedAttributes = ["aria-label", "title", "placeholder"] as const;

export function translateInterfaceText(source: string, locale: AppLocale): string {
  if (locale === "zh-CN" || !source.trim()) return source;
  const leading = source.match(/^\s*/)?.[0] ?? "";
  const trailing = source.match(/\s*$/)?.[0] ?? "";
  const core = source.slice(leading.length, source.length - trailing.length);
  const exact = exactTranslations.get(core);
  if (exact) return `${leading}${exact}${trailing}`;
  for (const [pattern, render] of dynamicTranslations) {
    const match = core.match(pattern);
    if (match) return `${leading}${render(...match.slice(1))}${trailing}`;
  }
  return source;
}

function translateTextNode(node: Text, locale: AppLocale) {
  const current = node.nodeValue ?? "";
  let state = textStates.get(node);
  if (!state || current !== state.rendered) {
    state = { source: current, rendered: current };
    textStates.set(node, state);
  }
  const next = translateInterfaceText(state.source, locale);
  state.rendered = next;
  if (current !== next) node.nodeValue = next;
}

function translateElementAttributes(element: Element, locale: AppLocale) {
  let states = attributeStates.get(element);
  if (!states) {
    states = new Map();
    attributeStates.set(element, states);
  }
  translatedAttributes.forEach((attribute) => {
    const current = element.getAttribute(attribute);
    if (current == null) return;
    let state = states!.get(attribute);
    if (!state || current !== state.rendered) {
      state = { source: current, rendered: current };
      states!.set(attribute, state);
    }
    const next = translateInterfaceText(state.source, locale);
    state.rendered = next;
    if (current !== next) element.setAttribute(attribute, next);
  });
}

function translateTree(root: Node, locale: AppLocale) {
  if (root.nodeType === Node.TEXT_NODE) translateTextNode(root as Text, locale);
  if (root.nodeType === Node.ELEMENT_NODE) translateElementAttributes(root as Element, locale);
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT);
  let node = walker.nextNode();
  while (node) {
    if (node.nodeType === Node.TEXT_NODE) translateTextNode(node as Text, locale);
    else translateElementAttributes(node as Element, locale);
    node = walker.nextNode();
  }
}

export function useDocumentLanguage(locale: AppLocale) {
  useEffect(() => {
    document.documentElement.lang = locale;
    translateTree(document.body, locale);
    const observer = new MutationObserver((mutations) => {
      mutations.forEach((mutation) => {
        if (mutation.type === "characterData") translateTextNode(mutation.target as Text, locale);
        else if (mutation.type === "attributes") translateElementAttributes(mutation.target as Element, locale);
        else mutation.addedNodes.forEach((node) => translateTree(node, locale));
      });
    });
    observer.observe(document.body, {
      subtree: true,
      childList: true,
      characterData: true,
      attributes: true,
      attributeFilter: [...translatedAttributes],
    });
    return () => observer.disconnect();
  }, [locale]);
}
