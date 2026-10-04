# CLI 界面优化与本地安装验收

验收日期：2026-10-04。起点为 `c97a602`；功能分支为 `feat/cli-interface-polish`，Windows 构建分支为 `build/windows-cli-polish`。

## 参考与结果

参考 [Codex CLI 官方项目](https://github.com/openai/codex) 和其 [输入区实现](https://github.com/openai/codex/blob/main/codex-rs/tui/src/bottom_pane/chat_composer.rs) 的紧凑布局、草稿编辑、输入回看和粘贴处理。本轮修改现有 Rust CLI。

- 首页改为简短的产品、目录和模型信息，移除大边框、头像及没有实际数据的活动提示。
- 对话使用自然段留白；用户内容采用主题强调色，回复沿用终端前景色，状态信息弱化。
- 长回复按显示宽度换行，保留中文与完整文本，支持滚动阅读。
- 输入区支持光标移动、按词编辑、多行草稿、粘贴、当前进程内的输入回看，以及 `Tab` 命令补全。草稿最多显示四行，并跟随光标滚动。
- Windows 粘贴兼容 ConHost 的 CRLF 和松键字符事件，包含中文、连接 emoji 和组合字符。多行粘贴的首行 `/quit` 不会提前执行。
- 缩小窗口时重新绘制有效行，修复标题错位和状态行被反复清除的问题；普通输入继续使用局部刷新。

快捷键见三个语言版本的 README。输入回看只存在于进程内存；测试均使用独立 `ORCHESTER_HOME`。

## 验证

| 检查 | 结果 |
|---|---|
| Rust 工作区测试 | 1123 项通过，0 项失败、0 项忽略 |
| 其中 CLI 测试 | 259 项通过，含 210 项单元测试和 5 项 Windows ConPTY 测试 |
| npm 启动器测试 | 19 项通过 |
| 格式与严格 Clippy | `cargo fmt --all -- --check` 与工作区所有目标 `-D warnings` 通过 |
| Windows Release 构建 | `cargo build --locked --release -p orchester-konsole` 通过 |
| 实际终端交互 | 80×24 → 40×12 缩放、长回复、历史输入、命令编辑、面板开关、退出与终端恢复通过 |
| 两轮模型协议测试 | 本地模拟 Responses SSE 服务收到恰好 2 次请求；第一条中文/连接 emoji 多行提示与预期逐字相同 |
| 安装后启动器 | 原有 `orchester` 命令启动新版；版本检查、5 条纯 JSON 事件、命令修正和多行粘贴通过 |
| 安装校验 | 构建、交付与安装的程序 SHA-256 完全一致 |

最终工作区测试日志为 `final-workspace-tests.log`，代码检查日志为 `final-clippy.log`。模型协议测试和终端截图使用本地模拟服务；本轮未调用实际模型服务。组合 emoji 的文本保留已验证，最终字形由终端与字体决定。

## 本地交付

程序构建对应提交 `a92e0606ed075d0393bc5e6a186f9a6dc6a2bbbf`。沿用 CLI 包版本 `0.1.2`；这是本地源码更新构建，没有发布 npm 新版本。

```text
SHA-256: 054a0238fbcf5c62773f7b2a810dcd01a3483a3030899fc29de7f81e5c68dbe7
安装位置: npm 全局 @orchester/cli 的 @orchester/cli-win32-x64/bin/orchester.exe
```

已备份旧程序为任务目录中的 `installed-cli-before-polish.exe`，安装目录也保留 `orchester.before-cli-polish.exe`。正在运行的旧实例可以继续使用旧程序；重新执行 `orchester` 时加载新构建。

任务输出目录包含：

- `orchester.exe` 和 `orchester.exe.sha256`：Windows 可执行程序及校验。
- `installation-verification.json`、`installed-tui-verification.json`、`installed-smoke.jsonl`：安装与实际启动器验收证据。
- `cli-preview.html`：可离线打开的终端记录回放，使用真实 ConPTY 输出，明确标注本地模拟模型。
- `cli-conversation-replay.jpg`、`cli-compact-replay.jpg`：80×24 对话和 40×12 窄窗口的回放截图。

代码按布局、输入编辑、窗口缩放、文档验收分步提交；功能与构建分支保留，检查后快进合并到 `main` 并推送远程。
