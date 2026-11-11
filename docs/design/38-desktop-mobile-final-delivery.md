# 桌面监控、手机共享字体与交付验收

本页功能基线为 main `6278e3b522100b5d7ccc9a4df69d8abceb49aac6`，构建及原生字体验收执行于 2026-11-10，全量门禁、Windows 桌面窗口、Linux 部署及文件复核于 2026-11-11（Asia/Shanghai）。便携包验收发现退出后 WSL 辅助进程残留，转入生命周期修复；默认安装更新和 Windows 部署仍待修复后的产物验证。

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

2026-11-11 完整前端 test script 按工作区串行复验，Vitest 1,445 项和工具/桌面安全/打包 Node 37 项通过；全工作区类型检查通过。Rust `--workspace --all-targets --locked --offline` 为 1,286 项通过、1 项忽略，格式与全工作区严格 Clippy 全部退出 0。此命令不包含 doctest。1,118 项代码及构建输入在门禁前后散列完全一致；期间提交仅归档文档。[逐项命令与退出记录](acceptance/desktop-mobile-final/current-gates.json)、[输入清单](acceptance/desktop-mobile-final/current-source-inputs.json)、[Rust 日志](acceptance/desktop-mobile-final/current-rust.log)、[前端日志](acceptance/desktop-mobile-final/current-frontend.log)。

Gitleaks 8.30.1 扫描 1,498 项当前跟踪文件，严格初扫退出 1，命中 5 项现存合成秘密测试或压缩代码标识。逐项人工复核并仅将这些精确 finding 作为 baseline 后，退出 0、新增命中 0；原始失败和逐项理由保留在[扫描复审](acceptance/desktop-mobile-final/secret-scan-review.json)。本轮新增验收文档另有独立扫描，没有目录或规则级忽略。

## Android 原生字体

APK 构建自 `6ead03b078d6d93891c3cf9693236f5191d4b6e6` 的隔离工作区。字体、图标、JS、CSS、favicon、首页共 6 项资源与 dist、Android、iOS 同步资源及 APK public 逐字节相等。APK 和测试包签名校验通过，并与原安装版证书一致。

Windows Android Studio AVD `emulator-5554` 的实际 base.apk 散列与上述 APK 一致。安装版 NativeBridgeTest 4 项通过，15.028 秒，测试进程正常退出。原生 WebView 的 computed 字体栈首选 `LXGW WenKai`、字重 400，`document.fonts` 中该 face 为 `loaded`。[安装、桥接及字体证明](acceptance/desktop-mobile-final/fonts-native.json)。16px / 400 的静态中英文测量宽度为 `137.47225952148438`，仅记录该环境的结果。

本次[实际设置页截图](acceptance/desktop-mobile-final/font-settings.png)已目视确认标题、正文、数字及底部导航使用共享字体，单/双栏控制显示正常。截图没有配对码、访问令牌或任务正文。

## Linux 当前部署

2026-11-11 已恢复 Linux `0.0.0.0:7337`，使用原状态目录 `/home/han/.local/state/orchester-remote` 和受控 stdin。实际二进制 SHA-256 为 `2384f245e8e34900fa6141dcf27ba8b3fa09b41d040f7732e445ed69518fc968`，构建于 `9a1d5b1`；Rust 输入与本页 main 基线一致，页面则为上述新版字体构建。

HTTP 返回的 6 项手机资源逐个与构建散列相等，未授权 info 返回 401；临时设备实际配对后 info/agents/projects 返回 200。检查后删除该测试设备，原 token 再请求为 401，配对窗口已关闭。原设备、认证与项目状态按内容保持一致，5 条任务记录精确保留（4 条 completed、1 条 cancelled）。此次部署没有调用模型。[部署及状态保留记录](acceptance/desktop-mobile-final/linux-deployment.json)。

## 尚待交付验证

基线 Windows 桌面实际安装候选 EXE 完成 16 项原生检查、7 次保存和 2 张目视截图，验证独立会话窗口、关闭后主窗口恢复及共享字体加载，页面错误为 0。GUI、Node 和自有后代均退出，测试端口释放。[原生结果](acceptance/desktop-mobile-final/desktop-desktop-runtime-result.json)、[字体与保存结果](acceptance/desktop-mobile-final/desktop-native-windows-result.json)、[退出证明](acceptance/desktop-mobile-final/desktop-cleanup-result.json)。截图确认文件曾因非原子写入读取半截 JSON；改为临时文件与原子替换后完整复验通过，未将失败那次算作通过。

便携包的真实 ConPTY 输入、关闭及后端 `quit` 已执行，但后端退出码 0 后被动等待 5 秒，仍有其直接启动的 System32 WSL forwarder、MSI WSL 子进程和 conhost 存活。父链、创建时间、EXE 散列均绑定实际后端；真实 ConPTY 进程自然退出。只清理这些精确自有句柄后全部退出，后补清理不计为产品自然退出通过。[自然退出缺口](acceptance/desktop-mobile-final/portable-normal-shutdown-observation.json)、[失败记录](acceptance/desktop-mobile-final/portable-failure.json)、[隔离清理](acceptance/desktop-mobile-final/portable-cleanup-result.json)。修复将限制辅助进程树的生命周期，不操作全局 WSL 或其他任务。

上述 Windows 候选包尚不计为已更新用户默认安装。辅助进程修复后两项 EXE 均需重新构建并绑定新源码复验，再更新默认安装和 Windows 7338，补齐临时验收工具退出证据。旧包与历史运行记录继续保留，不作为本轮已部署证明。

iOS 已同步 6 项资源并检查 LaunchScreen XML，缺少 macOS/Xcode 原生构建、签名与 iPhone 安装证据。实体 Android 的 Wi-Fi 网络、字体缩放和终端交互仍需目标手机验证。这些范围不由 AVD、资源相等或构建成功代替。

[证据清单](acceptance/desktop-mobile-final/manifest.json)保存本页选定文件的 SHA-256；后续运行及部署结果另行追加。
