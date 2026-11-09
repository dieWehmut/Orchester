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

待交付：控制台、任务、项目、设置四个导航，多设备与独立项目 Agent 栏，远程目录/文件选择，六主题；源码已在工作区实现，需通过手机测试、构建及 Android 原生操作验收后提交。继续实现主题化底部选择面板、与桌面同源的应用图标及启动过渡，并使用 Android Studio 模拟器验证当前安装包。

## 第三阶段：部署

待完成：更新本地部署、Windows 便携包、Android APK、iOS 源码同步与验收文档。iOS 原生编译需要 macOS/Xcode。

## 后续独立任务与验收边界

- Windows 与 WSL 的既有 Agent 活动监控：现有未提交实现继续独立修复、测试及提交；本地会话读取与进程发现不得启动、提交或续接 Windows Codex，不消耗其额度。
- 交互终端入口：现有桌面终端页签仍为占位，没有交互进程、输入和窗口大小协议；需要另行实现，不能将任务输出流算作交互终端。
- 保留工作区中桌面项目库、会话操作与窗口相关改动，按实际依赖另行验证提交。
- 每个独立任务验证后提交并推送功能分支，及时合入和推送 `main`；手机产物必须在最终 UI 源码完成后重新生成。
