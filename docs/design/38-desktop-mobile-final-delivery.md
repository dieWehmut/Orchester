# 桌面监控、手机共享字体与交付验收

本页功能基线为 main `6278e3b522100b5d7ccc9a4df69d8abceb49aac6`，构建及原生字体验收执行于 2026-11-10，门禁、原生窗口和终端、Linux 部署及文件复核于 2026-11-11（Asia/Shanghai）。便携包验收发现退出后 WSL 辅助进程残留，生命周期测试及修复已分别提交 `49d90ac`、`9dccb34` 并合入 main `8c24d0c73309e45738285cee4d0297ef4158810d`。默认安装更新和 Windows 部署仍待修复后的产物验证。

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

2026-11-11 生命周期修复前，完整前端 test script 按工作区串行复验，Vitest 1,445 项和工具/桌面安全/打包 Node 37 项通过；全工作区类型检查通过。Rust `--workspace --all-targets --locked --offline` 为 1,286 项通过、1 项忽略，格式与全工作区严格 Clippy 全部退出 0。此命令不包含 doctest。1,118 项代码及构建输入在门禁前后散列完全一致；期间提交仅归档文档。[逐项命令与退出记录](acceptance/desktop-mobile-final/current-gates.json)、[输入清单](acceptance/desktop-mobile-final/current-source-inputs.json)、[Rust 日志](acceptance/desktop-mobile-final/current-rust.log)、[前端日志](acceptance/desktop-mobile-final/current-frontend.log)。

Gitleaks 8.30.1 扫描 1,498 项当前跟踪文件，严格初扫退出 1，命中 5 项现存合成秘密测试或压缩代码标识。逐项人工复核并仅将这些精确 finding 作为 baseline 后，退出 0、新增命中 0；原始失败和逐项理由保留在[扫描复审](acceptance/desktop-mobile-final/secret-scan-review.json)。本轮新增验收文档另有独立扫描，没有目录或规则级忽略。

## Android 原生字体

APK 构建自 `6ead03b078d6d93891c3cf9693236f5191d4b6e6` 的隔离工作区。字体、图标、JS、CSS、favicon、首页共 6 项资源与 dist、Android、iOS 同步资源及 APK public 逐字节相等。APK 和测试包签名校验通过，并与原安装版证书一致。

Windows Android Studio AVD `emulator-5554` 的实际 base.apk 散列与上述 APK 一致。安装版 NativeBridgeTest 4 项通过，15.028 秒，测试进程正常退出。原生 WebView 的 computed 字体栈首选 `LXGW WenKai`、字重 400，`document.fonts` 中该 face 为 `loaded`。[安装、桥接及字体证明](acceptance/desktop-mobile-final/fonts-native.json)。16px / 400 的静态中英文测量宽度为 `137.47225952148438`，仅记录该环境的结果。

本次[实际设置页截图](acceptance/desktop-mobile-final/font-settings.png)已目视确认标题、正文、数字及底部导航使用共享字体，单/双栏控制显示正常。截图没有配对码、访问令牌或任务正文。

2026-11-11 在同一安装版和原生 WebView 中，通过产品设备表单临时配对隔离 Linux 17378 服务，再由产品界面创建实际 Bash PTY。终端等宽栈与桌面共享，computed 字号 13px，字体集及霞鹜文楷 face 均已加载；实际 32 字符测量得到单元宽 `7.797619342803955`、高 `15.2380952835083` CSS px。原生软键盘显示/隐藏时，界面实际网格和同一后端 PTY 均为 `45×28 → 45×8 → 45×28`，与测量后的可用屏幕面积相符。[26 项具体检查和网格结果](acceptance/desktop-mobile-final/android-terminal-fonts/fonts-terminal-native.json)、[原生软键盘及输入截图](acceptance/desktop-mobile-final/android-terminal-fonts/font-grid-input.png)。

`pwd` 使用 Android 原生输入事件，中文与 emoji 使用原生 WebView CDP `Input.insertText` 后发送原生 Enter，均经实际 xterm 和 PTY 回显；Unicode 结果不代表 IME 组合输入已验证。产品“关闭进程”后会话结束且 Bash PID 消失。临时设备由产品移除，原生凭据为空，服务端撤销返回 204、旧临时令牌请求返回 401；原 5 条设备 JSON 精确保留、靛蓝保留，检查内存已清除并回到控制台。自有 listener 正常 `quit` 退出 0、17378 释放、零自有后代，原 7337 服务保持运行。[退出截图](acceptance/desktop-mobile-final/android-terminal-fonts/font-grid-ended.png)、[逐文件原始散列复核](acceptance/desktop-mobile-final/android-terminal-fonts/provenance.json)。测试后端只按实际二进制散列绑定，不作为 Windows 候选源码验收；未调用模型或 Agent CLI，未重装或清除应用数据。

## Linux 当前部署

2026-11-11 已恢复 Linux `0.0.0.0:7337`，使用原状态目录 `/home/han/.local/state/orchester-remote` 和受控 stdin。实际二进制 SHA-256 为 `2384f245e8e34900fa6141dcf27ba8b3fa09b41d040f7732e445ed69518fc968`，构建于 `9a1d5b1`；Rust 输入与本页 main 基线一致，页面则为上述新版字体构建。

HTTP 返回的 6 项手机资源逐个与构建散列相等，未授权 info 返回 401；临时设备实际配对后 info/agents/projects 返回 200。检查后删除该测试设备，原 token 再请求为 401，配对窗口已关闭。原设备、认证与项目状态按内容保持一致，5 条任务记录精确保留（4 条 completed、1 条 cancelled）。此次部署没有调用模型。[部署及状态保留记录](acceptance/desktop-mobile-final/linux-deployment.json)。

## Windows 运行缺口与生命周期修复

基线 Windows 桌面实际安装候选 EXE 完成 16 项原生检查、7 次保存和 2 张目视截图，验证独立会话窗口、关闭后主窗口恢复及共享字体加载，页面错误为 0。验收程序随后按原句柄清理 GUI、Node 和自有后代，测试端口释放；这份基线记录不证明 GUI 通过产品菜单自然退出。[原生结果](acceptance/desktop-mobile-final/desktop-desktop-runtime-result.json)、[字体与保存结果](acceptance/desktop-mobile-final/desktop-native-windows-result.json)、[退出证明](acceptance/desktop-mobile-final/desktop-cleanup-result.json)。截图确认文件曾因非原子写入读取半截 JSON；改为临时文件与原子替换后完整复验通过，未将失败那次算作通过。

基线便携包的真实 ConPTY 输入、关闭及后端 `quit` 已执行，但后端退出码 0 后被动等待 5 秒，仍有其直接启动的 System32 WSL forwarder、MSI WSL 子进程和 conhost 存活。父链、创建时间、EXE 散列均绑定实际后端；真实 ConPTY 进程自然退出。只清理这些精确自有句柄后全部退出，后补清理不计为产品自然退出通过。[自然退出缺口](acceptance/desktop-mobile-final/portable-normal-shutdown-observation.json)、[失败记录](acceptance/desktop-mobile-final/portable-failure.json)、[隔离清理](acceptance/desktop-mobile-final/portable-cleanup-result.json)。

原生 Windows 回归测试使用三层进程均存活的 fixture 握手，没有启动真实 WSL 会话或模型。旧实现加测试 overlay 后，探针 1 项通过，成功、失败、超时、输出限额、后端退出跳过 Drop 的 5 项行为检查均因自有后代残留而失败，原生退出码 101；红例未计为产品通过。[原始红例日志](acceptance/desktop-mobile-final/wsl-helper-lifecycle/red/stdout.log)、[原生退出审核](acceptance/desktop-mobile-final/wsl-helper-lifecycle/red/native-audit.json)。

修复让 Windows 辅助进程在首线程运行前归入不继承句柄的自有 Job Object，随后恢复执行，并在返回及进程退出时限制整个辅助树生命周期。修复后同一原生生命周期套件 6 项、vertrag 原生契约套件 25 项通过，原生格式和严格 Clippy 通过；4 个门禁均实际退出 0、无超时。红例 367 项冻结输入和绿例 371 项冻结输入散列逐一复核一致；红例代码与 `49d90ac` 的测试 overlay 相同，绿例 369 项 Git 输入与 `9dccb34` blob 相同，6 项修复文件散列均与修复提交和 main `8c24d0c` 相同，其余为版本/patch 记录。[源绑定及红绿汇总](acceptance/desktop-mobile-final/wsl-helper-lifecycle/summary.json)、[绿例原始日志](acceptance/desktop-mobile-final/wsl-helper-lifecycle/green/helper-wsl/stdout.log)、[原生门禁结果](acceptance/desktop-mobile-final/wsl-helper-lifecycle/green/result.json)。

相同 6 项源文件的 Linux 专项为 261 项通过、1 项忽略、36 个测试程序，严格 Clippy、格式、根和桌面锁定离线 metadata 校验通过；仅这 6 项变更文件的秘密扫描为 0 命中。此 Linux 命令不包含 doctest，Windows 专属编译与行为由上述原生门禁单独证明。[Linux 专项结果](acceptance/desktop-mobile-final/wsl-helper-lifecycle/linux/result.json)、[测试日志](acceptance/desktop-mobile-final/wsl-helper-lifecycle/linux/tests.log)。归档仅选择判定记录和关键日志，未重复冻结源码或复制用户状态。[各原始文件和归档散列](acceptance/desktop-mobile-final/wsl-helper-lifecycle/provenance.json)。

## 尚待交付验证

main `8c24d0c` 的完整 Rust 集成复验出现真实失败：`concurrent_first_open_applies_each_memory_migration_once` 返回 `DatabaseBusy (database is locked)`；完整命令已执行到 698 项通过、1 项失败后停止，格式与严格 Clippy 退出 0。[原始失败和逐项门禁](acceptance/desktop-mobile-final/integration-memory-race/summary.json)、[Rust 失败日志](acceptance/desktop-mobile-final/integration-memory-race/rust-tests.log)。内存数据库并发初始化修复及完整复验待完成；上述修复前的 1,286 项全量通过和生命周期专项通过不能作为当前 main 全量通过证明。

上述 Windows 候选包尚不计为已更新用户默认安装。辅助进程修复后两项 EXE 均需重新构建并绑定新源码复验，再更新默认安装和 Windows 7338，补齐临时验收工具退出证据。旧包与历史运行记录继续保留，不作为本轮已部署证明。

修复后的冻结构建与 promotion 脚本按精确有序的 15 项门禁核验，补入原生契约和 WSL 生命周期测试；promotion 还要求同一源码、plan、final result、EXE、实际 runtime 和 cleanup 散列，以及完整旧目录 sibling 备份和外部状态保留。这里只记录[只读脚本复审](acceptance/desktop-mobile-final/wsl-helper-lifecycle/promotion-script-review.json)，不计为最终构建、原生运行或安装已通过。旧 13 门禁服务管理入口须另行准备适配版本后使用。

iOS 已同步 6 项资源并检查 LaunchScreen XML，缺少 macOS/Xcode 原生构建、签名与 iPhone 安装证据。实体 Android 的 Wi-Fi 网络、字体缩放和终端交互仍需目标手机验证。这些范围不由 AVD、资源相等或构建成功代替。

[证据清单](acceptance/desktop-mobile-final/manifest.json)保存本页选定文件的 SHA-256；后续运行及部署结果另行追加。
