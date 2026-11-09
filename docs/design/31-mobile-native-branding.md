# 手机同源图标、启动过渡与 Android 返回键验收

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

两台服务均为真实 Rust `orchester remote` listener，使用不同临时工作区和状态目录。项目 manifest 注册的 `qa-echo` 是隔离 Python CLI，仅回显真实 cwd / prompt，并提供测试续接 session。其 PATH 仅含 `/usr/bin:/bin`，不调用模型、不调用 Windows Codex，也不消耗模型额度。本轮配对与 HTTP 证据不代表真实模型任务或物理手机 Wi-Fi 已通过。模拟器原先保存的两台旧 QA 地址仍保留并显示未连接，没有改动其服务或提交任务。

持久截图均已目视核对，不含配对码或访问令牌：

- [Launcher 图标](acceptance/mobile-brand-icon-android.png)
- [启动过渡](acceptance/mobile-brand-launch-android.png)
- [底部设备选择面板](acceptance/mobile-picker-device-android.png)
- [Back 关闭后仍在任务页](acceptance/mobile-picker-back-android.png)

当前未完成的原生 UI 场景包括真实目录/文件附件、多工作栏请求、任务选择/停止/删除/续接的完整逐项操作，非默认六主题的逐屏视觉核对，以及物理手机网络和 iOS 安装。这些不能由浏览器 fixture 测试或旧 APK 截图替代；后续结果须按实际完成情况补充。

测试期间 Android 17 的 `uiautomator` 工具进程出现过一次 `UiAutomation` / Binder `Bad file descriptor` 异常，重试后可继续导出 UI 树；该日志属于工具进程，未观察到 Orchester 应用进程崩溃。因此不将整段 crash buffer 记作“为空”。
