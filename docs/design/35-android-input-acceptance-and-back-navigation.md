# Android 输入修复版验收与子页返回缺口

2026-11-10，Windows Android Studio API 37 / Android 17，指定 `emulator-5554`，未操作 5556。输入修复源码 `48cae93` 的 APK SHA-256 为 `7cd8ab5d8b6236b15938d8d3a91aa5e83a04aa2f1aeaa1a722d9c66e69d70aef`。实际安装 base.apk hash 一致，[构建/安装记录](acceptance/mobile-terminal-input/build-evidence.json)。同一安装版 [NativeBridgeTest](acceptance/mobile-terminal-input/native-bridge.log) 为 `OK (3 tests)`，5.304 秒，测试专用 Credentials/主题在 finally 恢复。

## 已通过的真实输入与进程场景

全部连接实际 Rust PTY listener 17377、临时项目 `/tmp/orchester-native-terminal-host-c6y8iyfs/native-terminal-project`。PATH 仅 `/usr/bin:/bin`，不调用 Agent 或模型。首个调查 listener 误用了旧二进制，创建终端返回 404；确认进程实际路径后才切换当前 main 构建，并重新从产品 UI 创建终端，旧 404 不算 APK 失败或通过证据。配对状态保留，未清应用数据。

| 场景 | 安装版结果 |
| --- | --- |
| 默认模式快速硬件事件 `pwd` | 实际 bash 输出项目 cwd，无旧版 `pwdwdd`；[截图](acceptance/mobile-terminal-input/fast-pwd-default.png) |
| 默认模式重复字符 | Python `input()` 精确读到 `bookkeeper111111`，UTF-8 一致；[字节记录](acceptance/mobile-terminal-input/default-repeat-proof.json) |
| 辅助阅读模式重复字符 | 精确读到 `aaabbbccc111111`；[字节记录](acceptance/mobile-terminal-input/reader-repeat-proof.json)、[截图](acceptance/mobile-terminal-input/fast-pwd-repeat-reader.png) |
| 原生 Gboard ASCII | 在键盘已稳定、焦点已检查后，实际触摸 p/w/d/Enter 输出 cwd；[截图](acceptance/mobile-terminal-input/gboard-pwd-final-focused.png) |
| 原生 Gboard 组合输入 | 触摸 c/a/f，长按 e，原生菜单拖到 é 并提交；电脑读到 `café`、UTF-8 `636166c3a9`；[菜单](acceptance/mobile-terminal-input/gboard-unicode-popup-final.png)、[结果](acceptance/mobile-terminal-input/gboard-unicode-final.png)、[字节记录](acceptance/mobile-terminal-input/gboard-unicode-proof.json) |
| Ctrl+C | 手机点实际快捷键，电脑等待进程收到 SIGINT，写入 `SIGINT_OK` 并回到 bash；[记录](acceptance/mobile-terminal-input/sigint-proof.json)、[截图](acceptance/mobile-terminal-input/ctrl-c-final.png) |
| Back 收键盘后返回项目 | 第一次 Back 收键盘且仍在终端，第二次回项目；同一 Bash PID/cwd 存活；[记录](acceptance/mobile-terminal-input/back-process-proof.json)、[项目页](acceptance/mobile-terminal-input/back-projects-final.png) |
| 断开期间输出补读 | 真实进程离开后产生 `REPLAY_WHILE_AWAY`，再进入原会话显示该输出，同一 Bash PID 保留；[记录](acceptance/mobile-terminal-input/replay-process-proof.json)、[截图](acceptance/mobile-terminal-input/replay-reconnected-final.png) |

首次 Gboard 注入在刚关闭阅读模式、立即打开键盘后连续触摸，得到 `wd`，当时没有先检查键盘焦点与动画稳定；[失败截图](acceptance/mobile-terminal-input/gboard-pwd-final.png)保留。同坐标在焦点确认后重试得到准确 `pwd`。未把首次失败隐去，也未凭重试推断已排除全部首次聚焦竞态。

## 发现的子页系统 Back 缺口

终端与底部选择器已有 history 项；主题/电脑设备/关于等内联子页只修改 Vue screen，没有 history 项。进入主题、切换珊瑚后，实际系统 Back 直接返回 Launcher，而没有回设置。[实际 Launcher 截图](acceptance/mobile-terminal-input/after-theme-system-back.png)。这是可复现的产品导航缺口，当前输入修复版不计为全部原生验收完成。

Windows SDK adb 的 UIAutomator/get-state 曾有临时非零退出；再次 `devices -l`/指定 serial `get-state` 均确认 5554 仍在线，随后截图可取得，没有因此重启 AVD 或 listener。期间原 Bash 在断开 5 分钟后按产品空闲策略退出，不能把该自动退出记作用户明确关闭通过。六主题终端与明确关闭场景将在子页 Back 修复后针对新 APK 补验。

物理 Android Wi-Fi 与 iOS 原生安装依旧待目标环境；本记录只覆盖指定模拟器与实际本机 PTY，不替代跨设备网络或真实模型调用。
