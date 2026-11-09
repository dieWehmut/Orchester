# 远程控制分阶段交付

2026-11-09。提交分支 `feat/mobile-remote-multidevice`，基于本地 `12e6068`。
远程 `main` 已有未合入的桌面端变更，本分支保留它们的历史；后续合入时需另行验证兼容性。

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

## 第二阶段：手机端

待完成：控制台、任务、项目、设置四个导航，多设备与独立项目 Agent 栏，远程目录/文件选择，六主题。需通过手机测试、构建及 Android 原生操作验收后提交。

## 第三阶段：部署

待完成：更新本地部署、Windows 便携包、Android APK、iOS 源码同步与验收文档。iOS 原生编译需要 macOS/Xcode。
