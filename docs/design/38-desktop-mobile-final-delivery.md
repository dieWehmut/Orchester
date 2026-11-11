# 桌面监控、手机共享字体与交付验收

功能源码为 main `6278e3b522100b5d7ccc9a4df69d8abceb49aac6`，构建及原生字体验收执行于 2026-11-10，Linux 部署、证据整理及文件复核于 2026-11-11（Asia/Shanghai）。Windows 运行验收、默认安装更新和 Windows 部署仍需后续证据。

## 功能与源码

Gemini CLI、Antigravity、Cursor 和 DeepSeek Harness 的监控识别、官方图标及来源状态已合并；规则与历史测试基线见[新增监控说明](37-additional-agent-monitors.md)。进程数量按来源标明单位，只有服务证据时显示“服务运行”，不完整来源保留已确认数量并显示“至少 N”，没有正计数时显示“状态未知”。本轮没有为取样启动这些 Agent 或调用模型。

手机和桌面共用 `packages/design/src/fonts.css`、`fonts.ts` 及随包字体，默认中英文为霞鹜文楷 `LXGW WenKai`。手机终端读取同一等宽栈与内容字重，等待字体加载后才初始化网格，加载失败保留系统回退。终端等宽字形仍取决于各平台实际可用字体。

## 构建与门禁

Windows 产物使用 Rust/Cargo 1.99.1、`x86_64-pc-windows-msvc` 和 debug profile。13 个构建门禁均退出码 0，无超时，原生进程退出已记录；包括进程/活动/HTTP 监控、MCP、CLI、原生标题栏、桌面测试、严格 Clippy、格式和两项 EXE 构建。[逐项门禁与日志散列](acceptance/desktop-mobile-final/windows-build-gates.json)。它们证明构建和测试通过，不代替桌面窗口、真实便携启动器的运行验收。

复核冻结目录全部 444 项输入，其中 398 项 Git 输入逐个等于上述 main 的 blob，46 项为生成资源与版本标记；计划文件散列匹配完成记录。桌面候选包完整集合为 39 项，手机远程候选包为 17 项，每项散列与构建清单相等。[产物与输入复核](acceptance/desktop-mobile-final/artifacts.json)。

| 产物 | 字节 / SHA-256 |
| --- | --- |
| Windows 桌面 EXE | 40,699,392 / `4fc62550edfcee272b412ae2e4908173038e076bb1b2e6b6c58cea699748be13` |
| Windows 远程 EXE | 30,802,944 / `70d784a849713a843e0b51c7ec44cc9f757582f84b0091a20c93df0f9d1f1cab` |
| Android debug APK | 13,683,781 / `55530cbab620d6d5a7301650b300a8af90de3f936f6388584e4edeedc409b0ca` |
| 共享 woff2 字体 | 8,059,324 / `23685af09c203fdf70cfcc15595411162ace2f75120a056134387dc4681b898b` |

main 集成 Web 完整套件为 860 项，类型检查与生产构建通过；门禁保存的 342 项源码散列及日志散列在本次归档复核一致。手机功能分支的完整套件 124 项、Design 308 项通过，随后字体与调色板专项分别 5、32 项通过，Web appearance 专项 7 项通过。手机及 Design 分支 `6ead03b` 与上述 main 的源码和锁文件无差异。归档保留原始门禁来源与各自范围，不把重复专项相加作为新的全量总数。[前端记录](acceptance/desktop-mobile-final/frontend-gates.json)。

## Android 原生字体

APK 构建自 `6ead03b078d6d93891c3cf9693236f5191d4b6e6` 的隔离工作区。字体、图标、JS、CSS、favicon、首页共 6 项资源与 dist、Android、iOS 同步资源及 APK public 逐字节相等。APK 和测试包签名校验通过，并与原安装版证书一致。

Windows Android Studio AVD `emulator-5554` 的实际 base.apk 散列与上述 APK 一致。安装版 NativeBridgeTest 4 项通过，15.028 秒，测试进程正常退出。原生 WebView 的 computed 字体栈首选 `LXGW WenKai`、字重 400，`document.fonts` 中该 face 为 `loaded`。[安装、桥接及字体证明](acceptance/desktop-mobile-final/fonts-native.json)。16px / 400 的静态中英文测量宽度为 `137.47225952148438`，仅记录该环境的结果。

本次[实际设置页截图](acceptance/desktop-mobile-final/font-settings.png)已目视确认标题、正文、数字及底部导航使用共享字体，单/双栏控制显示正常。截图没有配对码、访问令牌或任务正文。

## Linux 当前部署

2026-11-11 已恢复 Linux `0.0.0.0:7337`，使用原状态目录 `/home/han/.local/state/orchester-remote` 和受控 stdin。实际二进制 SHA-256 为 `2384f245e8e34900fa6141dcf27ba8b3fa09b41d040f7732e445ed69518fc968`，构建于 `9a1d5b1`；Rust 输入与本页 main 基线一致，页面则为上述新版字体构建。

HTTP 返回的 6 项手机资源逐个与构建散列相等，未授权 info 返回 401；临时设备实际配对后 info/agents/projects 返回 200。检查后删除该测试设备，原 token 再请求为 401，配对窗口已关闭。原设备、认证与项目状态按内容保持一致，5 条任务记录精确保留（4 条 completed、1 条 cancelled）。此次部署没有调用模型。[部署及状态保留记录](acceptance/desktop-mobile-final/linux-deployment.json)。

## 尚待交付验证

上述 Windows 候选包尚不计为已更新用户默认安装；桌面独立会话窗口、便携包终端运行、Windows 7338 的新产物部署和临时验收工具退出需补齐当前证据。旧包与历史运行记录继续保留，不作为本轮已部署证明。

iOS 已同步 6 项资源并检查 LaunchScreen XML，缺少 macOS/Xcode 原生构建、签名与 iPhone 安装证据。实体 Android 的 Wi-Fi 网络、字体缩放和终端交互仍需目标手机验证。这些范围不由 AVD、资源相等或构建成功代替。

[证据清单](acceptance/desktop-mobile-final/manifest.json)保存本页选定文件的 SHA-256；后续运行及部署结果另行追加。
