# 远程控制验收记录（2026-11-09）

本轮基于当前未提交工作树继续修补，功能设计见 [26-remote-control.md](26-remote-control.md)。以下分别记录源码测试、浏览器真实 HTTP 联调、原生工程、真实模型调用与部署证据；一类验证不能替代另一类。仍有物理设备网络、iOS 和正式分发等外部验收未完成，下文明确列出。

2026-11-10 续接说明：本文保留 11 月 9 日早期验收的版本与历史结果。后续同源图标、主题选择器、Android Studio API 37 安装版操作及六主题逐屏验证见 [最终原生验收](31-mobile-native-branding.md)。用户随后明确要求不测试 Windows Codex、不消耗其额度；下文早期 Windows Codex 调用仅是历史记录，后续不再执行其模型验收，也不再把 Windows Codex 可用额度作为交付条件。Windows/WSL 活动监控仅只读发现进程和已有会话。

## 手机端修复

| 触发场景 | 修复后的行为 | 位置与验证 |
| --- | --- | --- |
| 浏览器刷新丢失内存凭据，随后点击另一台已保存电脑 | 配对表单立即切换到被点击电脑，清空属于旧电脑的配对码，并显示“需要配对” | `apps/mobile/src/composables/use-remote-control.ts`；控制器回归测试与双后端浏览器操作 |
| 任务提交失败后，另一个任务的后台输出轮询成功 | 保留提交错误，轮询只清除自己的断线提示 | 同上；429 后继续收到输出的回归测试 |
| 电脑撤销令牌或令牌过期 | 显示“需要配对”，停止 401 轮询重试，禁用需认证的任务操作；输入新码可恢复 | 控制器测试；本机 revoke 命令与浏览器重配 |
| 输入省略协议但显式包含 `:80` 的地址 | 保留端口 80，不会因 URL 自动移除默认端口而错误连接到 7337 | `apps/mobile/src/client.ts`；IPv4、主机名、方括号 IPv6 三项回归测试 |
| 发送请求尚未返回时编辑下一条任务草稿 | 不再清空新草稿 | 控制器回归测试 |
| Agent 启动或输出失败且没有产生任何事件 | 显示后端 `error_code` 对应的检查建议和“任务已结束，没有可显示的输出”，不再永久显示等待 | `client.ts`、`presentation.ts`、`App.vue`；无输出非零退出子进程联调 |
| 配对失败、任务数量达到上限或记录被其他设备删除 | 按请求场景给出配对码、容量或记录不存在的提示 | `client.ts`；浏览器配对/撤销路径与源码核对 |

`App.vue` 同时显示中文任务状态与断线重连状态，并处理长电脑地址换行。Agent 错误说明只给出排查方向，不把登录、权限或模型配置推断成已确定的错误原因。

## 手机源码门禁

在仓库根目录运行：

```sh
pnpm --filter @orchester/mobile test
pnpm --filter @orchester/mobile build
```

2026-11-09 12:57（Asia/Shanghai）结果：2 个测试文件、27 项测试通过；`vue-tsc --noEmit` 与 Vite production build 通过。本轮先运行新增回归测试，取得 6 个预期失败，再修复通过。最终页面资源为 `index-CE78Wwhf.js`（后续修改或构建后应重新核对，不应把此文件名当作固定发布接口）。

测试覆盖地址规范化、认证请求、非 JSON 错误响应、空删除响应、幂等键、任务输出尾页、断线游标、设备/任务切换时丢弃旧响应、凭据过期、资源释放，以及上述修复。测试中的网络和凭据接口由替身提供，不证明原生 Keystore/Keychain 或真实模型账户可用。

## 浏览器联调

使用 `frontend-testing-debugging` 技能。当前会话未提供 Browser plugin，采用已有 Playwright 工具。页面为 `http://127.0.0.1:4175/`，标题 `Orchester Remote`。后端均为真实 `orchester remote` TCP listener，使用两个独立临时工作区和状态目录：

- `http://127.0.0.1:46449`，内置 `mock` adapter。
- `http://127.0.0.1:33925`，工作区 manifest 注册的临时 `fixture-ui` Python 子进程 adapter，支持固定输出、长任务、续接和无输出非零退出。

这两个地址运行在同一台测试电脑，证明客户端能切换独立后端，不代表两台物理电脑或手机跨 LAN 已经连通。临时服务已在测试后停止，未终止此前监听 17337 的旧 QA 服务。

| 操作 | 当前证据 |
| --- | --- |
| 输入地址和一次性配对码 | `/pair`、`/info`、`/agents`、`/runs` 经过真实 HTTP；页面显示已连接和 Agent 列表 |
| 提交 mock 任务 | 显示 `mock received`、`mock done` 和已完成；完整读取终态输出 |
| 删除终态任务 | 真实 DELETE 后任务记录及输出区域消失 |
| 长任务期间浏览器离线再恢复 | 页面显示正在重连和电脑任务继续运行提示；联网后从原游标继续读取并显示已连接 |
| 停止运行中任务 | 真实取消接口后显示已停止，保留既有输出 |
| 继续已停止会话 | 新任务传入 `ui-fixture-session`，子进程返回相同 session ID 并正常完成 |
| 子进程未输出就以 23 退出 | 后端返回 failed / agent_stream_failed，页面显示失败说明；等待输出提示不存在 |
| 切换两台已保存电脑 | 地址、Agent 列表和任务记录随目标更新；第一后端 0 条记录，第二后端 3 条记录，未串接旧输出 |
| 本机撤销设备后重新配对 | 本机 revoke 成功；客户端显示需要配对与正确地址；新码恢复已连接 |
| 刷新后凭据丢失，再点击另一台电脑 | 地址切换到选定电脑，旧配对码清空，显示请输入配对码 |

页面在 390×844 与 1280×900 视口均非空、无框架错误 overlay、无横向溢出。正常流程没有 console error；故障注入期间出现两条预期网络错误：主动离线导致 `ERR_INTERNET_DISCONNECTED`，本机撤销令牌导致 `/info` 返回 401。未观察到应用运行时异常。

截图证据位于本机临时目录（可能随环境清理失效，后续验收需重新生成）：

- `/tmp/orchester-mobile-control-390.png`：390 像素宽视口，已配对、mock 任务与完整输出。
- `/tmp/orchester-mobile-devices-1280.png`：1280 像素宽视口，两台已保存后端与当前连接地址。

上述临时脚本没有调用模型；续接验证证明 session 参数与界面流程，不能证明 Codex/OpenCode 保存了真实会话上下文。真实账户结果见后续小节。

## 验收与启动脚本修补

`werkzeug/remote/smoke.py` 的幂等重试原先位于清理逻辑之外：首次提交成功、重试连接失败时，已启动任务不会被取消。本轮将重试纳入异常处理；取消或撤销设备失败时保留最初的验收错误，并额外输出不含凭据的清理失败事件。若验收本身成功而撤销失败，脚本仍以失败退出。

`werkzeug/remote/start.sh` 原先固定运行 `target/debug/orchester`，而构建可能受到 `CARGO_TARGET_DIR` / `CARGO_BUILD_TARGET` 影响输出到其他目录。本轮显式选择 `rustc -vV` 的本机 host target，并把构建目录和运行路径绑定。相对 `CARGO_TARGET_DIR` 按调用脚本时的目录解析，包含空格的路径也经过测试。新路径为 `<target-dir>/<host>/debug/orchester`，不再假定构建产物直接位于 `target/debug`。

```sh
bash -n werkzeug/remote/start.sh
python3 -B -m unittest discover -s werkzeug/remote -p 'test_*.py' -v
```

结果：Bash 语法检查通过；5 项离线测试通过，覆盖已启动任务的清理、保留原始异常、撤销失败仍应失败，以及目标环境变量/空格路径与最终执行路径一致性。启动路径测试使用临时 rustc/cargo/pnpm/二进制替身，没有重建后端、打开端口或调用模型；实际部署启动仍需单独验收。PowerShell 启动检查由后端/部署负责人记录。

## Android 与 iOS

本轮使用 JDK 21、Android SDK 36、Gradle 8.14.3 与 Capacitor 8.6.0 构建，已将最终手机资源 `index-CE78Wwhf.js` 同步进 APK。

- `:app:assembleDebug`、`:app:assembleDebugAndroidTest` 成功。
- 安装包：`apps/mobile/android/app/build/outputs/apk/debug/app-debug.apk`，SHA-256 `d3dbfd8c91a7ae4f686f5ee8bd095eecb8fc30d7ac6bed306811534cabb1a1a7`，APK v2 签名验证通过。这是 debug 签名测试包。
- APK 与测试 APK 均已实际安装至 Android 16 / API 36 的 `orchester-qa` 模拟器，`adb install` 返回 `Success`，应用正常启动，无 crash buffer 记录。
- 已删除模板中的 `2 + 2` 测试及错误包名测试，加入 `NativeBridgeTest`。实际运行结果为 `OK (2 tests)`，耗时 7.124 秒，日志 `/tmp/orchester-native-tests-20261109.log`。
- 原生测试穿过打包 WebView 的 JavaScript bridge：验证 Keystore 凭据加解密、Activity 销毁重建后读取、磁盘不含明文令牌、密文无法移到另一个设备 key、凭据删除；另验证 Capacitor 原生 HTTP 对无 CORS 头的本地测试服务器发送 Bearer 请求并读取 JSON 响应。

此外通过 `adb input` 操作实际安装 App 的界面（坐标来自每一步的 UI tree），完成：

1. 输入电脑 `192.168.144.157:7337` 与新的一次性配对码，显示“已连接”，加载真实 Agent 列表与后端任务历史。
2. 选择 Codex，提交只回显验证标记的任务；实际模型返回 `ORCHESTER_ANDROID_NATIVE_20261109`，界面显示“已完成”。
3. 点击“继续会话”，后续提示只要求回忆上一轮标记，不再包含标记本身；发送后强制停止 App 进程。
4. 重新启动 App，仅点击已保存电脑（没有输入新配对码），Keystore 凭据重连成功；打开已完成续接任务，仍显示相同会话 ID 和正确标记。电脑任务在 App 关闭期间继续执行。

以上首次任务及续接均使用 session `01a1beb6-c2c5-7681-b5f7-dc7c24f676c6`。本次原生界面流程未再次验收停止按钮、设备撤销或物理 Wi-Fi 断线；相关浏览器、后端测试见其他小节。流程结束后 Android crash buffer 仍为空。

持久截图：[安装后启动界面](acceptance/mobile-remote-android-launch.png)、[强退重连后恢复真实 Codex 续接输出](acceptance/mobile-remote-android-resume.png)。截图不含配对码、访问令牌或模型凭据。

原生测试复跑命令见 [手机工具链](../../werkzeug/mobile/README.md)。当前环境可用 WSL KVM，但普通用户没有 `/dev/kvm` 访问权限；此次仅让模拟器进程加入已有 `kvm` 组，没有修改系统用户组或设备权限。Windows Android Studio 及 API 37 AVD 已探测到，WHPX 检查通过，但 C 盘剩余约 4.3 GB，小于该 AVD 所需数据分区，未完成 Windows AVD 启动；最终测试使用已有 Linux API 36 AVD。不要将该结果记作 API 37 或物理手机通过。

iOS 工程尚未在 macOS/Xcode 构建、签名或运行。物理 Android 手机跨 Wi-Fi 到电脑、正式签名分发均未验收。

## 真实 Codex / OpenCode 调用

Linux 后端监听 `0.0.0.0:7337`，工作区 `/home/han/project/Orchester`，状态目录 `/home/han/.local/state/orchester-remote`。通过 `werkzeug/remote/smoke.py` 实际调用本机 Codex CLI 0.167.0：

- 首轮以 `---` 开头的提示词原样提交，返回随机验证词。
- 同一幂等键重试返回同一任务 ID，没有创建第二条任务。
- 第二轮不重发验证词，使用第一轮 session ID 成功回忆该词；两轮 session ID 均为 `01a1beab-d94a-7fc3-b947-1e6b839f164a`。
- 两轮任务状态均为 completed，输出读取至终态空页。
- 临时测试设备在结束后撤销，后续请求返回 401。

首轮任务 `f2c34c51b7a98ffdb88be71c0b8a42c1b467de6868c0e49042f2ed6f429f2bdd`，续接任务 `5d0f68b362da56fbfc37f9dc9df513816b6ffb70e3fafdaebb7866c76d794291`；对应记录仍在部署状态库中。Android 安装版发起的独立真实任务由原生验收小节记录。

OpenCode 1.19.3 已在 Linux 隔离工具目录 `/home/han/.local/share/orchester-tools` 安装并确认可执行，部署进程 PATH 优先指向该目录，避免调用 Windows npm shim。`opencode auth list` 显示没有配置账户；真实任务使用默认 `opencode/big-pickle`，供应商返回 `Free usage limit reached`。CLI 没有及时结束，验收脚本在 150 秒超时后取消任务，后端确认 cancelled、CLI 进程退出，测试设备已撤销。**真实 OpenCode 成功返回与续接尚未通过**，需要可用模型服务配置；未重复尝试其他免费模型规避额度。

Windows 原生后端也实际启动 Codex，成功拿到 session ID 并收到供应商错误事件，但 Windows 账户返回 `You’ve hit your usage limit`，任务正确标记 failed / agent_reported_error。这证明 Windows 原生启动和事件链路可工作，不证明 Windows 账户当前能成功完成模型任务。Windows 与 WSL 使用各自的 CLI 账户。

后端回归证据：Linux remote_routes 9、remote_cli 2、vertrag 15、adapter 5、原 harness 进程运行 8 和插件包 5 项通过；Windows vertrag 22 项通过，包含 npm shim、无控制台启动、Job Object 取消父子进程。Windows MSVC 后端构建通过。原有 Web 界面 131 个文件 / 780 项测试通过，`cargo fmt --all -- --check`、`git diff --check` 通过。

对之前未提交的项目文件夹与会话历史改动额外复核：`orchester-anwendung` 的 library 3 / session_history 6 项，以及 `orchester-netz` 的 library_routes 1 / session_history_routes 5 项全部通过，包含跨重启持久化、工作区范围、CSRF、分页和错误去敏。

## 本地部署

2026-11-09 本轮实际运行状态：

| 环境 | 已验证的运行状态 | 未覆盖 / 外部条件 |
| --- | --- | --- |
| WSL Linux | `0.0.0.0:7337`，首页通过 `http://192.168.144.157:7337` 返回 200；Android 模拟器输入该 IP 后配对并提交真实 Codex 成功 | 此 IP 是 WSL NAT 地址，不能由此推断物理手机 Wi-Fi 可达 |
| Windows 原生 | `0.0.0.0:7338`，UNC 工作区与 UNC 页面资源均正常；Windows 本机 HTTP 首页 200；CLI 能启动并上报模型额度错误 | 从 WSL 请求 Windows 的 `192.168.144.1`、WLAN `10.52.50.173`、以太网 `10.49.6.45` 的 7338 均未连通。当前 Windows 进程无管理员权限，接口为 Public；尚未配置防火墙放行 |

Linux 进程由本轮早期版本 `start.sh` 启动，实际使用 `target/debug/orchester`；脚本随后修复了跨编译环境变量下的产物路径，今后重启会显式构建并启动 `target/<host>/debug/orchester`。Windows 最初使用源码启动脚本，收尾时已正常 `quit` 后切换为 `C:\Users\han\Downloads\OrchesterRemote\start.ps1` 启动包内 EXE，端口仍为 7338，UNC 工作区相同，状态仍位于 `%LOCALAPPDATA%\OrchesterRemote\state-home`。均为前台常驻进程，未注册系统自启服务。

`start.ps1` 已修复 PowerShell UNC provider-qualified 路径导致的 Windows error 161；修复后全 UNC 工作区/静态页面启动并返回 HTTP 200。`werkzeug/remote/allow-lan.ps1` 已准备并通过 PowerShell 语法解析，只为指定 EXE、电脑 IP、TCP 端口、本地子网建立规则。因为本会话没有 Windows 管理员权限，**未执行防火墙修改，也未宣称物理手机跨 LAN 已通过**。可由管理员在目标电脑运行后，再从实际手机验收。

部署辅助脚本另有 5 项离线测试，覆盖真实 host/产物目录、幂等重试失败后取消，以及取消/撤销失败不遮盖原始故障。它们不是对真实模型或系统防火墙的替代验证。

Windows 便携包位于 `C:\Users\han\Downloads\OrchesterRemote`：包含约 28 MiB Windows 后端、Web 页面、4.1 MiB Android debug APK、独立启动器、防火墙辅助、README、两份许可证及 SHA256SUMS。11 项文件哈希已核对，拒绝覆盖已有目录的行为已验证。便携启动器使用独立测试状态目录在 17340 成功返回首页 HTTP 200，随后 `quit` 正常退出并确认没有遗留监听；之后正式用包内 EXE 启动 7338。目标 Windows 需自备 VC++ x64 Runtime，未将本机验证扩大为全新系统验证。

## 后续验收条件

本轮没有将整体目标标记完成。以下仍需外部条件或目标设备参与：

1. 为 OpenCode 配置有可用额度的模型服务，再运行真实首轮与续接验收。Windows Codex 受后续用户禁令约束，不再测试或消耗额度；Linux Codex 历史成功也不证明其他账户可用。
2. Windows 防火墙放行后，从物理手机输入 Windows 局域网地址并完成配对、发送、断线恢复及停止。当前 WLAN IP 为 `10.52.50.173`，运行端口 7338；IP 改变后应更新参数。下列命令需在 **Windows 管理员 PowerShell** 执行，仅针对当前运行的便携 EXE：

   ```powershell
   & "$env:USERPROFILE\Downloads\OrchesterRemote\allow-lan.ps1" -Executable "$env:USERPROFILE\Downloads\OrchesterRemote\orchester.exe" -LocalAddress 10.52.50.173 -Port 7338
   ```

   若改用其他位置的 EXE，应使用其路径和实际监听端口建立规则，不能用另一份二进制的规则替代。本轮未执行上述管理员命令。
3. iOS 工程需要 macOS/Xcode、开发团队签名和设备运行验收。当前仅保留完整工程与 Keychain 实现，不能生成或声称已有可安装 IPA。

Android debug APK 已可安装测试，正式发行签名及商店分发不在本轮验收证据内。Windows、Linux 服务都需要保持运行；自启服务、跨公网 HTTPS、更多工作区和角色权限属于设计文档中明确的后续扩展。
