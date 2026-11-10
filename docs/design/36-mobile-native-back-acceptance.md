# 手机子页面与 Android 系统返回验收

2026-11-10。修复 commit `5d8518f` 已合并、推送 main `7313c29`。在同一 Windows Android Studio API 37 / `emulator-5554` 安装升级，未清应用数据、未操作 5556。旧输入修复版的 [主题 Back 退出 Launcher](35-android-input-acceptance-and-back-navigation.md) 已针对新安装版复验。

## 导航修复及回归

主题、电脑设备、关于、目录、Agent、任务详情和终端共用子页 history，回到进入时的主导航。页内返回和 Android 系统返回与选择面板共用一次异步遍历；返回尚未完成时连续 Back 不会排出第二次遍历跨过根页面。主导航本身保留系统退出行为。卸载重建等待旧 child/picker 的完整清理，晚到目录提交、Agent/任务/配对请求不能覆盖新页面。

23 项新增测试挂载真实 App.vue、fleet、RemoteClient 与子组件，只替代 HTTP/WebSocket、history 和 jsdom 缺少的布局 API。同一测试在修复前 `ff1e066` 为 14 失败 / 9 通过，失败包含子页没有 history、重复返回越过 root 和晚到请求跳页；[修复前日志](acceptance/mobile-native-back/navigation-before.log)。评审额外发现循环 Promise 结束时的微任务空档，新增测试先失败（22 通过 / 1 失败），再修正；[该红测](acceptance/mobile-native-back/navigation-microtask-before.log)。最终手机完整套件 113 项通过，包含原有 18 项 Android 输入回归；[最终日志](acceptance/mobile-native-back/mobile-tests.log)。类型检查与生产构建通过，独立只读复审无阻塞。main `7313c29` 的完整前端原有脚本串行门禁随后通过：Vitest 1,354 项、工具 26 项、桌面安全 1 项和桌面打包 10 项；[门禁记录](acceptance/mobile-native-back/final-frontend-gate.json)。未修改断言或超时。

## 新 APK 与真实原生返回

APK SHA-256 `cac1edd8fed9c4b9c43fac5da8b9a9722147103c18e6fdcb334718182247ce67`，5,655,509 字节，debug 签名。实际安装 base.apk hash 一致，5 项 dist 文件与 APK public 字节一致；root/main/隔离构建页面一致。iOS 只完成 5 项资源同步与 LaunchScreen XML 检查；未宣称 Xcode/设备通过。[构建与安装记录](acceptance/mobile-native-back/navigation-build-evidence.json)。

实际安装版 NativeBridgeTest 4 项通过、8.003 秒。新增用例通过 Android OnBackPressedDispatcher 返回主题、设备和关于页，并在系统返回后立即切任务/项目，等待完成后仍在项目。原有 Keystore 加密与设备绑定、恢复、原生 HTTP 和主题持久化用例同次通过；[原生日志](acceptance/mobile-native-back/navigation-native-bridge.log)。

再次从实际产品 UI 进入主题后按系统 Back，[主题页](acceptance/mobile-native-back/navigation-theme-before-system-back.png) 回到 [设置](acceptance/mobile-native-back/navigation-theme-system-back-settings.png)，没有退出 Launcher。项目设备选择面板首次系统 Back 关闭面板且仍在 [项目页](acceptance/mobile-native-back/navigation-picker-back-projects.png)。原有六条设备记录仍存在。

## 交付包与实际部署

Windows 新命名包 `OrchesterRemote-20261110-native-back-final` 由实际 Windows 打包/验证脚本生成；Windows verifier 已检查其完整集合与 APK/Web 一致性。默认 `C:\Users\han\Downloads\OrchesterRemote` 已更新为同一集合：16 项文件、15 条散列与命名包全部一致，5 项 APK/Web 相等。完整旧默认包保存在 `OrchesterRemote-backup-20261110-before-native-back-7313c29`；[替换记录](acceptance/mobile-native-back/portable-promotion.json)。更新前后后端 EXE 为同一 `4569036d…`，没有重写 EXE 或重启用户服务，状态目录仍在包外。

实际 Windows WLAN `http://10.52.50.173:7338` 的首页及 5 项页面文件全部逐字节等于新默认包，未授权 info 仍为 401，原 4 项任务及状态保留；[Windows HTTP 与状态记录](acceptance/mobile-native-back/windows-live.json)。Linux 7337 的首页等于新版 main dist；[Linux 记录](acceptance/mobile-native-back/linux-live.json)。

本次 Windows 命名包的重复 runtime 复验进入工具等待后超时，未把它记为通过。代理曾在 5554 成功 get-state 和截图，但运行复验的父/后代输出等待没有完成；Windows 工具互操作又出现 `UtilAcceptVsock accept4 110`。后端自身的上次 14 项实际便携运行基线与本次 EXE 字节相同，记录仍保留在 [历史运行证据](acceptance/remote-final-portable-runtime.json)；这不替代新版原生终端 UI 的收尾。

临时 Linux 测试 listener 17377 已通过 stdin quit 正常退出，TCP 确认释放；手机临时 17377 设备记录保留待原生收尾后单独移除。Windows helper 已请求停止，退出及其可能后代仍需原生进程检查，未把请求停止计为退出通过；[清理状态](acceptance/mobile-native-back/tool-cleanup-pending.json)。

终端明确关闭、六主题终端截图、靛蓝主题恢复及临时工具进程的退出确认仍在收尾。物理 Android Wi-Fi 与 iOS 原生编译/安装仍待此前缺少的目标环境。
