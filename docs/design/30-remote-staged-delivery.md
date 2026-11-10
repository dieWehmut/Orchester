# 远程控制分阶段交付

2026-11-09。提交分支 `feat/mobile-remote-multidevice`，基于本地 `12e6068`。
桌面端历史与第一阶段后端已集成到 `main`；2026-11-09 已验证并推送 `c24e9be`。

## 第一阶段：电脑后端

- 默认注册 Claude、Codex、OpenCode 三种真实 CLI。脚本测试适配器只在测试配置中提供，不注册进生产目录。
- 电脑级配对与持久设备凭据；旧工作区配对凭据迁移到电脑级身份。
- `/projects` 持久保存实际目录。`/folders` 读取工作区和重复 `--allow-root` 指定的目录树，支持分页。
- Agent 目录、任务 cwd、续接与幂等请求按项目隔离；设备可列出所有已选项目的任务。
- 选择旧工作区时导入历史，兼容 v1 幂等请求；目录被替换时拒绝执行。
- 共用 Unix 进程组和 Windows Job Object，CLI 关闭 stdin，限制原始与规范事件输出。

验证在从 Git HEAD 导出的临时目录内运行，使用精确待提交源码与锁文件，未包含工作目录中尚未提交的桌面端或项目库依赖。

```sh
cargo test --locked --offline -p orchester-netz --test remote_routes --test agent_catalog \
  -p orchester-konsole --test remote_cli --test cli
cargo test --locked --offline -p orchester-vertrag -p orchester-adapter \
  -p orchester-verzeichnis --lib
```

第一组 43 项、第二组 25 项通过。路由测试包括真实临时目录和 CLI 子进程；CLI 回归使用隔离 manifest 子进程。此记录不将协议测试 fixture 等同于实际模型或 Android 操作验收。
OpenAPI v2 已通过 YAML 解析。

### main 集成验证

在干净的 `main` worktree、提交 `c24e9be` 上重新执行以下门禁，均返回退出码 0：

```sh
cargo test --locked --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
pnpm install --offline --frozen-lockfile
pnpm -r typecheck
pnpm test
```

Rust 共 98 个测试程序，1,229 项通过、1 项忽略；前端 Vitest 共 1,203 项通过，根测试命令中的前端工具、桌面安全配置及打包工具检查也通过。远程 `main` 已核对为 `c24e9be57ac33154b18bd94c83604f2f3cde4379`。这些结果覆盖已提交的集成源码，不包含手机端和 Windows/WSL 监控的未提交改动，也不代表 Windows 原生构建或模型调用验收。

## 第二阶段：手机端

已提交并合入远程 `main`：`1b14c45` 提供四个导航、多设备、独立项目 Agent 栏、远程目录/文件和六主题；`ea71b9f` 提供同源图标、启动过渡和 Android 返回键桥接。最终 `bf617dc6…` APK 在 Windows Android Studio API 37 AVD 完成双设备、目录、附件、续接、删除、停止、双栏恢复和六主题共 96 屏验收；详见 [原生验收](31-mobile-native-branding.md)。这是模拟器 debug 包验收，物理手机网络与 iOS 安装仍待目标设备验证。

## 第三阶段：部署

源码启动、Windows 便携打包、完整文件清单和 APK/Web 一致性检查已提交到 main。2026-11-10 从最终功能提交 `48cae93` 生成 APK 与 Windows x64 后端，默认 `C:\Users\han\Downloads\OrchesterRemote` 已更新，原目录完整保留为 `OrchesterRemote-backup-20261110-before-main48cae93`。15 项哈希、精确文件集合、5 项 APK/Web 内容和真实便携启动器 ConPTY 流程均通过。两端常驻部署已恢复并实际核对；见 [最终交付记录](34-remote-final-artifacts-and-deployment.md)。iOS 原生编译和安装仍需要 macOS/Xcode 与目标设备。

## 后续独立任务与验收边界

- Windows 与 WSL 的既有 Agent 活动监控：`fc53429` 已通过专项测试并合入远程 `main`。Windows 原生只读探针已发现 Windows Codex 31、OpenCode 2 与 WSL Codex 73、OpenCode 3 个会话；本地会话读取与进程发现不得启动、提交或续接 Windows Codex，不消耗其额度。
- 交互终端入口：`fe4a59d` 后端与 `61fb9e3` 手机入口已合入 main；`7751366` 修复快速 Android 输入，`d186c47` 清除终态遗留提示，最终功能合并为 `48cae93`。最终 APK 的 bridge/hash/快速输入已通过，原生组合输入与收尾场景继续补验。功能与验收边界见 [终端记录](31-remote-interactive-terminal.md)。终端范围按此前确认的手机远程进入电脑推进；桌面终端页签仍为占位，不将任务输出流算作交互终端。
- 桌面项目库、会话操作、共享认证与原生独立窗口已分步提交并合入 main，浏览器与 Windows 原生 HWND 生命周期验收见 [桌面最终记录](33-desktop-library-and-windows-acceptance.md)。
- 每个独立任务验证后提交并推送功能分支，及时合入和推送 `main`；手机产物必须在最终 UI 源码完成后重新生成。
