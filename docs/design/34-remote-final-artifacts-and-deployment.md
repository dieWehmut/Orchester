# 最终远程产物与本地部署

2026-11-10（Asia/Shanghai）。最终功能源码为 `48cae93`，包含手机项目/多设备、交互 PTY、Android 快速与混合输入修复，以及桌面项目库和原生独立会话窗口。原生 UI 的输入收尾验收继续在同一 Android Studio API 37 / `emulator-5554` 上完成；物理 Android 手机暂不可连接，iOS 缺少 macOS/Xcode 和目标设备，二者保留待办。

## 集成检查

在 main worktree 运行 Rust 全工作区、全目标测试：1,257 项通过、1 项 ignored；格式、严格 Clippy 和 TypeScript 全套通过。生产 Web/mobile/website 构建通过。前端按工作区串行执行全部原有 test script：Vitest 1,331 项通过，前端工具 26 项、桌面安全配置 1 项和桌面打包工具 10 项通过。

首轮 `pnpm test` 与 Gradle/MSVC 同时运行时，新增输入回归第一项超过 5 秒，其他手机 89 项通过。保留失败日志后，以 `pnpm --workspace-concurrency=1 -r test` 重跑完整前端，未改测试断言或超时设置，全部通过。Windows 额外执行当前源码的 CLI 22 项和实际 ConPTY 4 项测试，全部通过。功能源码 `48cae93` 的 [GitHub Pages 工作流](https://github.com/dieWehmut/Orchester/actions/runs/42741437315) 已完成且 conclusion 为 success；后续 main 的验收文档提交未改变站点输入。

[产物及门禁 JSON](acceptance/remote-final-20261110.json)记录了 339 项 Rust 输入的整体散列、root/main 内容相同、实际二进制和 APK hash；后续只更新验收文档时不重新宣称不同源码的二进制已验证。Rust 测试与模型验收分开，本轮没有发起模型任务，也没有启动 Windows Codex。此前 Linux OpenCode 真实首轮和续接的证据保留在[真实模型记录](27-remote-acceptance.md)。

## APK 与便携包

最终页面也已 `cap sync ios`，5 项 iOS public 文件与同一次 dist 完全一致，LaunchScreen Storyboard XML 可解析；未把同步成功计为 Xcode 编译或 iPhone 安装通过。

最终 APK SHA-256：`7cd8ab5d8b6236b15938d8d3a91aa5e83a04aa2f1aeaa1a722d9c66e69d70aef`，5,654,877 字节，debug 签名。5 项 dist 文件与 APK 内 `assets/public` 逐字节相同；APK 另外含 Capacitor 生成的 `cordova.js` / `cordova_plugins.js`。5554 实际安装 base.apk hash 完全一致，`NativeBridgeTest` 3 项通过、5.304 秒，覆盖 Credentials/Keystore、重建恢复、原生无 CORS 请求和主题恢复。

Windows x64 后端 SHA-256：`4569036d09a2279153f9e5fd3a362403212cd39b0ec68481b09fca7afe303cee`。默认交付目录为 `C:\Users\han\Downloads\OrchesterRemote`，包含 16 项文件，哈希清单覆盖除自身外的全部 15 项。独立验证确认精确文件集合、每项 SHA-256 和 APK/Web 一致性。使用实际包内启动器运行 14 项检查：配对、项目目录、Unicode/空格 cwd、Windows ConPTY 输入、顺序输出、96×31 resize、关闭、未连接终端的退出清理与端口释放，均通过。[便携运行记录](acceptance/remote-final-portable-runtime.json)。

旧默认目录完整保留为 `C:\Users\han\Downloads\OrchesterRemote-backup-20261110-before-main48cae93`；独立命名的已验证包也保留为 `OrchesterRemote-20261110-main48cae93-input-final`。[替换记录](acceptance/remote-final-portable-promotion.json)。安装文件是默认目录中的 `orchester-mobile-debug.apk`；验证命令为 Windows PowerShell 中的 `& "$env:USERPROFILE\Downloads\OrchesterRemote\verify.ps1"`。

## 当前本地部署

Linux 后端恢复为 `0.0.0.0:7337`，实际工作区 `/home/han/project/Orchester`，允许项目根 `/home/han/project`，状态继续使用 `/home/han/.local/state/orchester-remote`。首页与最终页面逐字节一致，未授权 `/remote/v1/info` 返回 401；用本轮临时测试设备实际配对，info/agents/projects 返回 200，目录包含实际项目。测试设备已 DELETE 撤销，原 token 再请求为 401；此前用户设备和任务历史保留。[Linux 检查](acceptance/remote-final-linux-deployment.json)。

Linux OpenCode 使用本机 API-key Responses provider 的进程环境覆盖，`codex-provider.py --check` 已通过，没有修改全局凭据或启动 Codex 客户端。设备列表仅反映已安装 CLI 的发现；本轮部署探测没有模型调用。

Windows 后端恢复为 `0.0.0.0:7338`，使用默认包内 EXE、相同 UNC 实际工作区与原 `%LOCALAPPDATA%\OrchesterRemote\state-home`。实际首页、未授权 401、临时设备配对和撤销通过。[Windows 检查](acceptance/remote-final-windows-deployment.json)。Windows 管理窗口保留，可在其 CLI 中输入 `pair` 生成新的单次配对码，`devices` 查看授权设备，`quit` 正常关闭；不是系统自启服务。

当前 Windows WLAN 地址为 `10.52.50.173:7338`，WSL NAT 地址为 `192.168.144.157:7337`，均是本次机器检查得到的快照。WSL 实际请求 Windows WLAN 的首页返回 200、内容与最终包完全一致，未授权 API 返回 401；WSL→Windows 的虚拟网关与以太网地址此次 TCP 未连通。[网络检查](acceptance/remote-final-network-probe.json)。这些结果仍不证明物理手机网络可达。手机接入、实际 Wi-Fi 断开/恢复以及 iOS 原生编译与安装仍需目标环境；此前用户已允许先继续本地 Android Studio 验收，不把这些待办改成通过。
