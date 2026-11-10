# Journey: Android Studio 安装版交互终端（首轮 APK）

日期：2026-11-10，Asia/Shanghai。本记录仅覆盖 Windows Android Studio 打开的 `Medium_Phone_API_37.0` / `emulator-5554`，Android 17 / API 37，1080×2400、420 dpi。全程明确指定 serial；另一个 `emulator-5556` 未操作。没有物理 Android 手机连接，本记录不宣称真实手机 Wi-Fi 已通过。

## 构建、安装与原生桥证据

构建从干净 main commit `35f78aa7a272e14219ad262fcd387739982c8aec` 开始；之后 main 的脚本、桌面和监控改动没有改变这份 mobile/native/Rust 终端源码。`pnpm install --frozen-lockfile --offline`、项目 `werkzeug/mobile/build-android.sh` 通过；JDK 21、SDK 36、Gradle 8.14.3。当前网页资源是 `index-CsFvA51o.js` / `index-BGBk7Bq9.css`。全部 5 个 dist 文件与 APK 内 `assets/public` 逐字节相同。

APK 路径：`apps/mobile/android/app/build/outputs/apk/debug/app-debug.apk`。SHA-256：`ba8d594e434223da0e8315abc0055ef908088d66ff23520ba169cc759d5ffb96`，debug 签名。Windows SDK `adb.exe install -r` 成功；首轮全部交互结束后再次从设备 `/data/app/.../base.apk` 计算 SHA-256，仍与构建 APK 一致。[构建与资源核对](build-evidence.json)、[最终实际安装 hash](installed-sha256-final.log)。

`:app:assembleDebugAndroidTest` 通过。同一安装 APK 的 `NativeBridgeTest` 重跑结果 `OK (3 tests)`、4.644 秒，覆盖原生 Credentials / Keystore、Activity 重建、设备 key 绑定、无 CORS 的原生 HTTP Bearer 请求、主题切换和重建恢复。[测试日志](native-bridge-after-reboot.log)。`cap sync ios` 通过，LaunchScreen Storyboard XML 可解析；没有 Xcode / iPhone 编译和安装验收。

首轮 instrumentation 在执行测试前被系统按启动 ANR 杀死。系统日志同时记录 Play Store / GMS 启动 ANR、zygote 启动 9–12 秒、CPU 总占用约 99%（内核约 93%），UIAutomator 也无法生成 XML。root 确认其他 Orchester 验收代理没有使用此 AVD，前台为 Launcher，随后仅 reboot `emulator-5554`，未 wipe / clear 应用数据。新 boot uptime 41 秒时 shell 恢复约 50 毫秒；同一 APK 的上述 3 个测试通过，后续交互未出现应用崩溃。原始失败记录保留为 [首次测试日志](native-bridge.log)，完整启动系统日志仍在 `/tmp/orchester-native-terminal-qa/test-startup-system.log`，不把首次失败抹掉。

Linux android CLI 直接连接 Linux `127.0.0.1:5037`，无法发现 Windows Studio AVD。已实际尝试 `android layout` 并用 strace 确认；随后按既有工具链回退到 Windows SDK `adb.exe` 的 `uiautomator dump` 和 `exec-out screencap -p`。未切换或停止 adb server，未改变其他项目服务。

## Results

下表的终端操作连接真实新编译 Rust listener `10.0.2.2:17367`，工作目录 `/tmp/orchester-native-terminal-host-qe5egq0h/native-terminal-project`，独立状态目录，PATH 仅 `/usr/bin:/bin`。fixture 是隔离本地 shell / Python 进程，不调用 Agent、模型或 Windows Codex。所有 PNG 均是实际安装版 1080×2400 原图，已经目视核对，不含配对码或访问令牌。

| 场景 | 实际结果与截图 |
| --- | --- |
| 新增隔离电脑与原有设备保留 | 17367 配对、连接成功。设备选择器共 5 台：此前 4 台地址保留，均未删除或提交任务。[设备选择器](device-picker.png) |
| 系统 Back 关闭设备面板 | 一次 Back 关闭真实底部选择器，仍在项目页。[关闭后](device-picker-back.png) |
| 从真实项目创建 bash | 项目页“进入终端”→“新建终端”，显示真实连接状态与项目 cwd，创建会话前缀 `120098fb`。[终端与原生 Gboard](terminal-keyboard.png) |
| 真实 Android ASCII 键盘 | 在 Gboard 上分别触摸 p / w / d / Enter，真实 bash 输出精确匹配选择的项目 cwd。[实际 pwd](terminal-native-pwd.png) |
| 原生 Unicode 组合输入与 stdin | 以 Gboard 输入 c / a / f，长按 e，拖到原生弹出菜单的 é，提交给真实 Python `input()`。电脑文件精确为 `café`，UTF-8 `636166c3a9`。[原生字符弹出菜单](gboard-unicode-popup.png)、[实际读取结果](terminal-native-unicode.png)、[字节证据](native-unicode-proof.log) |
| Ctrl+C | 真实等待进程显示 `WAITING_FOR_CTRL_C`；手机点终端快捷键 Ctrl+C 后收到 SIGINT、写入 `SIGINT_OK`，终端返回新的 bash prompt。[实际中断](terminal-native-ctrl-c.png)、[进程证据](native-sigint-proof.log) |
| 键盘触发真实 PTY resize | 打开 Gboard 时 `stty size` 为 `8 45`；一次系统 Back 收起键盘而未离开终端，等待命令读取为 `28 45`。[PTY resize](terminal-keyboard-resize.png) |
| 主题与终端会话选择器 | 当前靛蓝主题的页面、终端、快捷键、设备面板与会话面板颜色一致，选中勾、运行状态可见。[会话选择器](terminal-session-picker.png)。此轮没有重复之前 APK 的六主题 96 屏验收；NativeBridgeTest 另外验证海盐主题切换与重建。 |
| Back 返回项目，电脑进程保留 | 运行延迟 fixture 后先 Back 收键盘，再 Back 返回项目页；电脑上同一个 Bash PID 仍存活、cwd 未变。[返回项目](terminal-back-projects.png)、[PID 证据](native-back-proof.log) |
| 回到相同会话并补读断开期间输出 | 再进入终端，仍选中 `120098fb`，同一 Bash PID 保留；离开期间真正产生的 `REPLAY_WHILE_AWAY` 显示出来。[真实补读](terminal-reconnected.png)、[复接证据](native-replay-proof.log) |
| 明确关闭进程 | 点“关闭进程”先显示保留/关闭确认，确认后 UI 为“已关闭 / 会话已结束”，快捷键禁用；原 Bash PID 已退出。[确认](terminal-close-confirmation.png)、[关闭后](terminal-closed.png)、[PID 结束](native-close-proof.log) |

上述命令均通过 Windows SDK `adb.exe -s emulator-5554`。原生 Gboard 字母和 Enter 是按截图坐标执行 `shell input tap`；é 是 `shell input motionevent DOWN` → 原生字符菜单 → `MOVE` / `UP`，不是注入 JavaScript composition/input 事件。xterm 的隐藏 textarea 不出现在 UIAutomator 的 EditText 节点中，键盘焦点另由 Android `dumpsys input_method` 的 `mInputShown=true`、`mServedInputConnection` / CapacitorWebView 焦点记录和实际键盘输入结果核实。[原生焦点记录](terminal-focus.log)。

## 未通过的快速硬件事件输入

快速 `adb shell input text pwd` 在默认 `辅助阅读=false` 状态产生 `pwdwdd`，真实 shell 报 command not found。[默认状态实际截图](adb-fast-input-default.png)。此前在 `辅助阅读=true` 状态对 `python3 qa_read.py` 注入同类事件，也产生 `python3ython3thon3...` 等重复后缀。两种模式均观察到问题，不能缩小为仅辅助阅读问题，也不能凭真实 Gboard 成功就宣布物理硬件键盘通过。

真实 Gboard 的逐键触摸及 é 的 IME 组合输入已经分别通过；快速硬件事件注入的根因与补丁交给前端代理另行处理。本首轮 APK 尚未修复此问题；修补后的新 APK 必须另写 hash 并针对输入回归，不能把本轮旧截图记作补丁后的运行证据。

本轮使用了一个隔离 listener 和临时测试文件；没有更改产品源码、提交、推送或合并。原生终端已在界面明确关闭。为后续输入问题抓取，listener 暂交给 root 的调查代理，最终由该代理结束；此前四台设备记录和其他 Android Studio 项目不受影响。
