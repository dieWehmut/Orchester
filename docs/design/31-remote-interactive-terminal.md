# 手机远程交互终端

2026-11-09 开始，2026-11-10 更新，分支 `feat/remote-interactive-terminal`。此记录覆盖电脑后端与手机浏览器；原生打包、Android Studio 模拟器与最终 main 集成结果由交付时另行记录。

## 实现

`/remote/v1/terminals` 创建设备拥有的真实 PTY，手机通过独立 WebSocket 输入、调整窗口并读取 ANSI 输出。终端使用选中项目重新校验后的实际目录，不接受客户端命令、任意 cwd 或环境变量。

Linux 使用 `bash --noprofile --norc -i`，缺少 bash 时使用 `sh -i`；清除 `ENV`、`BASH_ENV` 启动钩子。Windows 使用关闭 AutoRun 的 `cmd /D /Q` 与 ConPTY。Windows canonical 路径转换为普通盘符路径，避免 cmd 将 `\\?\C:\…` 识别为 UNC 后退回 Windows 目录；真正 UNC 项目选择无 Profile 的 PowerShell。ConPTY 首次光标请求由后端在新空屏幕上初始化，未连接手机时也可以结束进程并释放读写线程。内部光标响应在关闭期间仍会发送，避免刚创建就关闭时卡住 ConPTY；用户输入在关闭后拒绝，输入线程等输出排空后释放。

Bearer 令牌仅用于 HTTP。WebSocket 使用随机、60 秒有效、一次性的票据，内存只存票据散列，绑定设备、终端与回放序号。另一个设备无法列出、打开或关闭该终端；同一终端最多一个连接。设备撤销会作废票据并结束其终端，每秒、每次输入及每块回放前复核设备授权。

PTY 读取按 UTF-8 拼接跨块字符，保留 ANSI，非法字节替换为 U+FFFD。输出带递增 `seq`，保留最多 256 KiB 和 512 块；重连按 `after` 回放，缓存缺口发送 `gap`。最后输出排空后发送 `exit`。JSON、Pong 和关闭帧的发送均限制为 5 秒，输入限制为每项 8192 UTF-8 字节和 16 项队列。

最多同时 4 个终端，终态快照总数最多 64 个，保存在内存。返回手机页面或断线保留 shell，5 分钟未连接、30 分钟无输入、主动关闭、设备撤销或服务停止会终止 PTY。Unix 清理同 PTY session 的前台、后台与 disown 作业，Windows 使用 Job Object 清理子孙进程。启动前取得可失败的 IO 句柄，异常清理会等待子进程以避免僵尸进程；服务停止等待终端读写线程排空，服务重启不恢复终端或输出。

完整请求与响应协议位于 [remote.openapi.yaml](../remote.openapi.yaml)。终端使用服务器用户权限；允许根目录约束初始项目选择，shell 本身并非目录沙箱。

## 手机终端

项目卡片增加“进入终端”。手机按电脑地址和项目分别记住最后选中的会话，支持新建、选择已有会话、返回后重新连接与主动关闭进程。浏览器返回键先关闭会话选择器，再退出终端页；返回仅释放 WebSocket，不删除 shell。

使用 `@xterm/xterm` 6.0.0 和 FitAddon 0.11.0 显示 ANSI，输入通过 `onData` 直接传给 PTY。Tab、Ctrl+C、Esc、方向键和 Enter 有手机按钮；视口和软键盘高度改变时重新调整字符行列，短视口仍保留提示符和快捷键。六套主题同时应用到终端背景、文字、选区、光标与 ANSI 颜色。

默认使用正常键盘输入模式。“辅助阅读”按需启用 xterm 的屏幕阅读输出。原版 6.0.0 在该模式下会拦截部分非 composition `insertText`；下述输入补丁同时修正这一事件路径，并验证两种模式下的快速输入。真实浏览器验证了中文和 emoji 输入。粘贴在发送前检查 8192 UTF-8 字节与 16 KiB JSON 帧限制，超过时提示分段粘贴并保留连接。

输出按序等待 xterm 的 write 回调后更新 `seq`。重连按已渲染游标请求一次性票据，忽略重复块；切换会话先排空已接受的写入，再清屏连接另一会话。旧连接的延迟断开不能触发新连接重试。401 或 404 会停止自动重试并显示重新配对或新建会话提示，HTTP 401 同时更新电脑设备状态。

## 后端验证

仅启动隔离 shell 和测试 fixture，没有启动、提交或续接真实 Agent，也没有调用模型服务。

Linux Rust 1.96.0 的以下命令均通过：

```sh
cargo test --locked -p orchester-netz --lib remote::terminal
cargo test --locked -p orchester-netz --test remote_terminals
cargo test --locked -p orchester-netz --test remote_routes
cargo fmt --all -- --check
cargo clippy --locked -p orchester-netz -p orchester-vertrag --all-targets -- -D warnings
cargo build --locked -p orchester-konsole --bin orchester
git diff --check
```

3 项 PTY 单元测试验证每个 UTF-8 字节边界、内存与块数上限，以及断线、空闲、设备撤销、manager Drop 后的 shell 和 IO 线程释放。5 项真实 PTY 集成测试验证实际 cwd、中文、等待输入的程序、窗口调整、输出回放、退出码、票据与设备隔离、容量和后台子进程终止。新增覆盖带空格及中文的项目目录、项目过滤和目录重校验、准确游标重连、非法 WebSocket 消息以及超出 256 KiB 缓存后的 gap 和保留输出回放。18 项既有 remote 路由回归通过。

OpenAPI 文件已通过 YAML 解析，并检查四个终端路径和 WebSocket 的独立票据认证声明。

2026-11-10，Windows 使用 Rust 1.99.1 和 `x86_64-pc-windows-msvc`，独立 target 目录执行 4 项单元测试和 4 项真实 ConPTY 集成测试，均通过。测试直接输入 `set /p` 并检查输出，不依赖浏览器提供首次 DSR 应答；检查了普通盘符与带空格、中文项目中的实际 cwd、中文、resize、replay、`exit 7` 和 Job 子孙进程终止。Windows 专用测试重复 24 次刚创建就关闭 ConPTY，确认首次光标请求及读写线程正常释放；大输出 gap 集成测试仅在 Unix 执行。

```powershell
cargo test --locked --target x86_64-pc-windows-msvc -p orchester-netz --lib remote::terminal
cargo test --locked --target x86_64-pc-windows-msvc -p orchester-netz --test remote_terminals
cargo clippy --locked --target x86_64-pc-windows-msvc -p orchester-netz -p orchester-vertrag --all-targets -- -D warnings -A clippy::double_must_use -A clippy::large_enum_variant
```

Windows 原始 `-D warnings` 检查被已有源码阻挡：Rust 1.99 对 `modell/src/types.rs` 和 `vertrag/src/adapter.rs` 的 `async_trait` 展开报告 `double_must_use`，Windows 下 `netz/src/mcp.rs` 的 `Transport` 报告 `large_enum_variant`。上述命令仅在检查时豁免这两个既有 lint，未修改源码以规避检查。Linux 严格检查无需豁免。

额外真实 UNC 探针通过 HTTP 创建 `\\wsl.localhost\Ubuntu-22.04\tmp\orchester-terminal-unc-project` 项目终端，确认选择无 Profile 的 PowerShell。经一次性票据连接后，输入 `(Get-Location).ProviderPath`，验证真实目录、标准输出与 `exit 7`。探针仅使用临时目录和隔离 shell，完成后已从源码树移除。

## 手机浏览器验证

2026-11-10，以下命令通过：

```sh
pnpm --filter @orchester/mobile test
pnpm --filter @orchester/mobile typecheck
pnpm --filter @orchester/mobile build
git diff --check
```

6 个测试文件共 72 项通过，终端的 17 项测试覆盖输入和窗口调整、渲染游标重连、重复输出、缓存缺口、结束状态、切换会话排空、过期配对或会话、主动关闭轮询、迟到票据、延迟断开竞态与 UTF-8/JSON 粘贴限制。依赖仅新增两个锁定版本的 xterm 包。

Browser 插件未提供，使用已安装的 Playwright Chromium；首次启动缺少 `libnspr4.so`，随后从已存在的浏览器库缓存设置 `LD_LIBRARY_PATH` 后通过，没有新增安装。页面为 `http://127.0.0.1:4177/`，连接此前已运行的隔离电脑服务 `http://127.0.0.1:7349` 与 `/tmp/orchester-terminal-ui-project`。测试仅运行临时 shell，没有启动模型 Agent。

真实 PTY 交互验证了 shell 的 TTY 和项目 cwd、ANSI 输出、中文命令、`read` 等待并接收中文、emoji、Tab、屏幕 Enter 和历史方向键、Ctrl+C 中断 `sleep`、游标重连不重复、返回保留进程、未连接期间产生的输出回放、超长粘贴提示及后续正常输入、主动关闭与终态禁用输入。辅助阅读可切换，开启后 ASCII 输入仍正常。六套主题经界面选择后重新进入同一终端，保留真实输出且终端颜色匹配主题。

另一组浏览器 fixture 验证两台电脑、各两个项目、多个终端，以及不同电脑返回相同终端 ID 时的会话隔离、选择器切换和返回键顺序。两组均无控制台警告或异常，无框架错误覆盖层，页面标题与非空内容正确。

视口覆盖 390×844、320×568、1000×900；模拟 `visualViewport.height = 350` 时，提示符、快捷键和底部导航均保持在可见范围。截图与临时脚本保留在 `/tmp/orchester-terminal-verified-*`，未加入源码树。此处软键盘验证是 Chromium 视口模拟，实际 Android 软键盘、原生返回与打包结果待最终 main 集成后验证。

## Android 快速输入修复

2026-11-10，在 Android Studio 的 `emulator-5554`、Android 17 / WebView Chrome 145 中，旧 APK 的实际受信任硬件事件复现 `adb shell input text pwd` 变为 `pwdwdd`。按键顺序为 `keydown(keyCode=229)`、`beforeinput`、`input` 和 `keyup`；约 6 ms 内完成全部输入。xterm 的 `_handleAnyTextareaChanges` 分别捕获空文本、`p`、`pw`，延迟回调均读取最终 `pwd`，因此 `onData` 发出 `pwd`、`wd`、`d`。这与上游 [6045](https://github.com/xtermjs/xterm.js/issues/6045) 描述的延迟快照和输入事件重叠问题相符；上游 [5438](https://github.com/xtermjs/xterm.js/issues/5438) 也记录了 Android 物理键盘重复输入。

通过 pnpm `patchedDependencies` 固定 `@xterm/xterm` 6.0.0 输入补丁。非 composition 的 229 按键不再安排延迟文本差异回调，由实际 `input` 事件同步提交文本；以 composition、待提交 composition 和已处理的硬件 keydown/keypress 状态明确区分输入来源。辅助阅读沿用同一输入路径。没有字符去重、时间防抖或按字符内容猜测的逻辑，因此重复字符仍逐次保留。Android 的后向删除由对应输入事件发送 DEL，正常控制键和 bracketed paste 沿用终端处理。硬件键松开同时清除已处理标记，避免辅助阅读模式吞掉下一次独立 input 事件。

补丁保留可读 TypeScript、生成的 ESM 与 UMD 运行时及对应 source map。`werkzeug/mobile/rebuild-xterm-input-patch.mjs` 使用工作区已有 esbuild 0.28.2，从 pnpm patch 编辑目录中的源文件重建，保持原有 CommonJS、AMD 和浏览器全局导出。源码和生成物都由补丁散列写入 lockfile；不是只修改 `node_modules` 的临时修正。补丁约 8.1 MiB，主要来自两份生成运行时与 source map；重新生成后的四个文件逐字节一致。

新增 18 项回归直接创建实际 xterm，并回放 Android DOM 事件，覆盖两种阅读模式下的快速输入、连续重复字符、中文与 emoji、input-before-keydown 的重叠与跨计时器按键、composition 提交后紧邻字符、后向删除、Ctrl+C、Enter、方向键、硬件 keypress/input 去重、硬件键松开后的 input-only / input-before-keydown emoji 及 bracketed paste。未修改依赖时 `pwd` 两种模式都稳定复现为 `pwdwdd`；修正后工作区移动端 7 个测试文件共 90 项通过，类型检查和生产构建通过。

已通过 `pnpm install --frozen-lockfile --offline`、CommonJS/AMD/浏览器全局导出检查与补丁重建一致性检查。使用修正后的移动页面重新运行真实 PTY 浏览器交互，中文 read、emoji、控制键、重连/回放、关闭与六套主题均通过。

设备分析使用同一台 5554 WebView 的临时终端 fixture 加载补丁 ESM，实际受信任快速 `pwd` 得到 `onData = [p, w, d]`；默认模式下 `ppp` 和辅助阅读模式下 `aaabbbccc` 均逐字保留。日志仅包含终端输入事件，未记录网络认证信息，fixture 不请求后端。该验证证明补丁在真实 Android 事件链路中的行为；安装的旧 APK 并未被替换，最终应用验证必须在 main 合并后重新构建、安装新 APK，不能以旧 APK 的慢速 Gboard 通过结果替代。
