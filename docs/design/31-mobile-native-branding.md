# 手机同源图标、启动过渡与 Android 原生交互验收

日期：2026-11-09（Asia/Shanghai）。本轮在 Windows Android Studio 2026.2.2 打开手机 Android 工程，使用其 Windows SDK 的 `Medium_Phone_API_37.0` AVD 实际安装和操作 APK。模拟器上报 Android 17 / API 37，serial 为 `emulator-5554`，物理分辨率 1080×2400，密度 420 dpi；WHPX 检查通过。构建使用 WSL JDK 21、SDK 36、Gradle 8.14.3 和 Capacitor 8.6.0。Android Studio 打开工程与模拟器，构建由项目脚本调用 Gradle 完成，未将其记作 Studio IDE 内点击 Build 通过。

## 本轮改动

桌面 `apps/desktop/src-tauri/icons/icon.png` 是唯一品牌图源，SHA-256 为 `c280f2cd646b1bd5008338081fb2fbd6444faf9f56649604272b6ec4bf1bef06`。`werkzeug/mobile/generate-brand-assets.py` 生成 Android 各密度的启动器图标、adaptive foreground、原生启动图和 iOS AppIcon / LaunchScreen 图；网页启动头像与桌面源文件逐字节相同，Android legacy 图标逐像素等于源图的 Lanczos 缩放，iOS 图标为不含透明通道的 1024×1024 PNG。普通构建使用已保存的生成资源，无需 Pillow；重新生成时需要 Pillow。

独立 `LaunchScreen.vue` 通过 `main.ts` 挂载一次性启动层，使用当前主题的头像光晕、轮廓、文字入场和节奏条，约 1.24 秒后完全卸载。实际应用同时启动设备重连；启动层显示期间正文使用 `inert`，避免误触不可见按钮。减少动态效果时缩短为约 180 毫秒；已有 WebView 从后台恢复不会重播。本轮原生录屏发现过 native Splash 到 WebView 的短暂白屏，已用深色 WebView 底色及页面加载后才退出原生 Splash 修补，修补后录屏未见白帧。

Android 实测打开底部选择器后按一次系统 Back，初版直接退出到 Launcher。原因是目前只安装 Capacitor core，没有插件提供 WebView 历史回退处理。`MainActivity` 现通过 `OnBackPressedDispatcher` 优先调用 WebView `goBack()`，由面板的 `popstate` 关闭；没有可回退历史时才交回 Activity 的默认返回行为。修补后同一面板按一次 Back 关闭，仍停留在任务页，`topResumedActivity` 仍为 `dev.orchester.mobile/.MainActivity`。

## 当前构建和安装证据

- 前端 `pnpm --filter @orchester/mobile build` 通过（包含 `vue-tsc --noEmit`）；当前页面资源为 `index-DIAAEXRO.js` / `index-BzmDNTRi.css`。所有 `dist` 文件与 APK 内 `assets/public` 逐字节核对一致。
- `:app:assembleDebug` 和 `:app:assembleDebugAndroidTest` 通过。
- 当前 APK：`apps/mobile/android/app/build/outputs/apk/debug/app-debug.apk`，SHA-256 `bf617dc6209d23726ab80eb17c5ce16f936d2211568f3788e410c55afe4c7af2`。这是 debug 签名测试包。
- 当前 APK 已安装 Windows AVD；在这份 APK 上复跑 `NativeBridgeTest`，结果 `OK (3 tests)`。日志 `/tmp/orchester-native-tests-bf617.log`。覆盖 Keystore 加密/Activity 重建/删除与设备 key 绑定、无 CORS 本地服务器的原生 HTTP Bearer 请求，以及主题切换和重建后恢复。
- 原生 UI 收尾时从模拟器 `/data/app` 的实际安装路径读取 APK，再次计算 SHA-256，确认仍为上述 `bf617dc6…` 包；整个交互验收沿用同一份安装包。
- `cap sync ios` 通过，Storyboard XML 可解析。iOS 未在 macOS/Xcode 编译、签名或安装，本轮不宣称 iPhone 运行通过。

Windows AVD 启动期间默认 adb 5037 端口曾由 WSL relay 转给无设备的 Linux adb server，导致 Windows adb 误连 Linux `localhost`。停止本轮启动的无设备 Linux server 后，Windows adb server 接管 5037，设备正常显示。测试使用 Windows SDK 的 `adb.exe`；不把 Linux `android studio check` 无法发现 Windows IDE 记作 Android Studio 未运行。Windows IDE 日志记录了该 UNC 工程路径。

## 当前已完成原生 UI 场景

| 场景 | 实际观察 |
| --- | --- |
| Launcher 图标 | 启动器显示与桌面同源头像；adaptive icon 适配圆形遮罩 |
| 冷启动 | 原生头像转为主题色启动层，再淡出到控制台；修补后录屏未见白帧 |
| 配对两台隔离服务 | 安装版通过 `10.0.2.2:17347` / `10.0.2.2:17348` 分别配对成功，均显示已连接与实际项目数量 |
| 重开 App | 已保存的两台隔离服务自动重连，没有输入新配对码 |
| 设备筛选面板 | 已连接和未连接设备都正确显示，当前选择有勾，背景遮罩覆盖页面，底部保留系统手势区域 |
| 系统 Back | 一次关闭真实底部面板，仍停留在任务页与同一 Activity |
| 真实目录新增项目 | 在第二台服务打开允许的根目录，进入 `project-beta` 并点击“使用此文件夹”；随后 Agent 页及重新查询的项目页显示真实 `computer-2/project-beta`，原来的 `project-alpha` 保留 |
| 真实文件附件 | 在第一台服务的 `project-beta/src` 选择 `hello.py`，发送 `qa-beta-attachment` 后，真实 Python 输出同时包含该项目 cwd 和参考文件绝对路径 |
| 附件移除 | 再次选择 `hello.py` 后点击对应移除按钮，附件名称和按钮消失，未创建新任务 |
| 任务选择 | 原生底部任务选择器显示已完成任务和当前勾选；选择较早的附件任务后重新读出其原始输出 |
| 续接 | 点“继续会话”后显示续接提示；发送 `qa-beta-resume-verified`，真实 CLI 输出确认 `resume=qa-session`，cwd 仍为原项目 |
| 删除记录 | 点“删除记录”后当前输出清空，任务选择器恢复占位；只读核对服务端 SQLite `remote_runs`，被删任务已移除，其他记录保留 |
| 停止任务 | `qa-stop-beta` 的隔离 Python 进程等待时显示“运行中”；手机点“停止任务”后显示“已停止”，服务端持久状态为 `cancelled`，已产生的输出保留 |
| 跨电脑、跨项目双栏 | 左栏为 `computer-1/project-beta`，右栏为 `computer-2/project-alpha`；两栏独立保留草稿，发送左栏不清空右栏；两台服务分别持久保存对应任务，真实输出 cwd 与输入逐项吻合 |
| 工作栏恢复 | 强制结束再启动 App 后，两台服务凭据自动重连，以上两栏和双栏布局恢复 |
| 六主题逐屏 | 珊瑚、玫瑰、海盐、森林、靛蓝、琥珀分别实际导航、打开选择器、浏览目录并读取任务输出；各 16 屏，共 96 张原始截图已目视核对 |

两台服务均为真实 Rust `orchester remote` listener，使用不同临时工作区和状态目录。项目 manifest 注册的 `qa-echo` 是隔离 Python CLI，仅回显真实 cwd / prompt，并提供测试续接 session。其 PATH 仅含 `/usr/bin:/bin`，不调用模型、不调用 Windows Codex，也不消耗模型额度。本轮配对与 HTTP 证据不代表真实模型任务或物理手机 Wi-Fi 已通过。模拟器原先保存的两台旧 QA 地址仍保留并显示未连接，没有改动其服务或提交任务。

停止场景仅给临时 `qa_echo.py` 增加 `qa-stop` 输入时等待 90 秒的分支，以便实际操作停止按钮；点击停止后该等待进程结束。续接 manifest 把 `{session_id}` 作为实际 CLI 参数传入，输出验证了 `qa-session`。此过程未重建、重新安装或替换已验证 APK，也未修改产品源码。两台隔离 listener 上报相同电脑名；本轮用端口及真实 `computer-1` / `computer-2` 路径区分设备。

持久截图均已目视核对，不含配对码或访问令牌：

- [Launcher 图标](acceptance/mobile-brand-icon-android.png)
- [启动过渡](acceptance/mobile-brand-launch-android.png)
- [底部设备选择面板](acceptance/mobile-picker-device-android.png)
- [Back 关闭后仍在任务页](acceptance/mobile-picker-back-android.png)
- [选择真实项目目录](acceptance/mobile-native-project-folder-android.png)
- [第二台电脑新增项目后](acceptance/mobile-native-project-added-android.png)
- [真实电脑文件选择](acceptance/mobile-native-file-browser-android.png)
- [附件任务真实输出](acceptance/mobile-native-attachment-output-android.png)
- [附件移除前](acceptance/mobile-native-attachment-before-remove-android.png)
- [附件移除后](acceptance/mobile-native-attachment-removed-android.png)
- [原生任务选择面板](acceptance/mobile-native-task-picker-android.png)
- [续接参数真实回显](acceptance/mobile-native-resume-verified-android.png)
- [删除后当前栏清空](acceptance/mobile-native-delete-android.png)
- [运行中停止按钮](acceptance/mobile-native-stop-running-android.png)
- [停止后的状态](acceptance/mobile-native-stop-completed-android.png)
- [两栏独立草稿](acceptance/mobile-native-double-drafts-android.png)
- [跨电脑、跨项目真实输出](acceptance/mobile-native-double-output-android.png)
- [重开 App 后恢复的两栏](acceptance/mobile-native-restored-panels-android.png)
- [六主题逐屏原图与联系图索引](acceptance/mobile-native-themes/README.md)
- [原生 UI 逐项操作记录](acceptance/mobile-native-ui-qa.md)

六主题分别核对了控制台及已连接设备卡片、单/双栏工作区、工作栏文件页、项目、目录与文件列表、Agent 选择、任务详情、设备选择器、已保存设备、空配对表单、设置、主题和关于页面。原始 PNG 为 1080×2400；联系图仅用于并排复核，未改变原图 UI。页面背景、主次按钮、选中状态、长路径换行、连接/任务状态和底部导航未见主题混色或遮挡导致的无法操作。当前产品提供的六种配色均为深色主题。

物理手机网络和 iOS 安装仍未完成；这些不能由模拟器、浏览器 fixture 或旧 APK 截图替代。

测试期间 Android 17 的 `uiautomator` 工具进程出现过 `UiAutomation` / Binder `Bad file descriptor` 异常，重试后可继续导出 UI 树；该日志属于工具进程，未观察到 Orchester 应用进程崩溃。收尾读取 crash buffer，包含 UiAutomation 记录，没有 `dev.orchester.mobile` 包名；同一 Activity 仍在前台。因此不将整段 crash buffer 记作“为空”。
