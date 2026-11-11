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

## 最新 APK 的终端收尾

终端最终输出修复 `45739e2` 与测试、说明已合并并推送 main `8de9efe`。HTTP 关闭响应先禁用输入，WebSocket 继续接收最终输出与 exit；晚到 HTTP 响应不能覆盖已经完成的退出。手机完整套件现在为 119 项，终端相关 23 项通过；[修复与门禁](31-remote-interactive-terminal.md)。

实际 `emulator-5554` 安装升级 APK SHA-256 `dba3137147b9fb4bdad4ecbd43d1145f7436b73fddf7f633d36b3d3ba2e13b98`，5,655,501 字节，保留原应用数据；[构建产物](acceptance/mobile-terminal-final/build-artifacts.json)。新版原生桥 4 项通过，14.303 秒；[原生输出](acceptance/mobile-terminal-final/native-bridge.json)。

真实 Bash `2765586` 通过原生 Gboard 启动测试脚本。手机返回项目页且截图目视确认后，电脑侧 QA 标记才触发输出；重连同一仍存活的 Bash 时，终端显示离开期间产生的 `NATIVE_CURRENT_OUTPUT_WHILE_AWAY` 和返回的 shell prompt。这是受控触发的真实进程和 WebSocket 回放，不是定时器或模拟终端。前一次离开过久退出的会话不计为通过。

珊瑚、玫瑰、海盐、森林、靛蓝、琥珀六种主题的实际已连接终端截图均已目视确认。珊瑚使用上述回放会话；其余五种及靛蓝恢复使用新 Bash `2797559`，主题切换期间该 PID 保持存活。关闭确认框目视核对后点击确认，约 0.52 秒内实际进程消失；页面显示“会话已结束”，输入快捷键禁用。最终恢复靛蓝。

临时 17377 设备通过产品的“从本机移除”删除，本机保留另外五条设备记录，并在同一截图确认全部地址。该按钮负责本机记录和凭据移除；临时服务的唯一授权另由正常 admin `revoke` 撤销至零。随后 listener `2646618` 经 stdin `quit` 正常退出，父进程、listener 和 TCP 17377 均确认释放，默认 Linux 7337 保持运行。[安全截图与完整验收](acceptance/mobile-terminal-final/manifest.json)、[服务撤销](acceptance/mobile-terminal-final/temporary-server-revoke.json)、[正常退出](acceptance/mobile-terminal-final/temporary-listener-cleanup.json)。

截至该次验收，Windows V2 工具仍供新版桌面和后端构建验收使用；停止和退出证明在该验收结束后补齐。默认 Windows 便携包与桌面 EXE 的该次更新尚待新原生门禁，不把旧版部署当作新版交付。物理 Android Wi-Fi 与 iOS 原生目标仍待环境。

后续 Windows 桌面、便携包、默认部署及工具退出结果统一见[桌面与手机交付记录](38-desktop-mobile-final-delivery.md)。
