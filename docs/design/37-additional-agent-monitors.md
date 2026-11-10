# Gemini、Antigravity、Cursor 与 DeepSeek Harness 监控

2026-11-10。Agent 列表增加 Gemini CLI、Antigravity、Cursor 和 DeepSeek Harness 四个监控项。DeepSeek 使用用户确认的官方 [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness)，命令简称 `dsh`。四项即使没有可启动的 adapter 也显示；已有同 provider 的 adapter 保留运行能力并共用监控行。

## 识别与界面

| 监控项 | 已核实的运行入口 |
| --- | --- |
| Gemini CLI | Node 的 `@google/gemini-cli/dist/index.js`；同入口、同参数的直接 supervisor/child 组合计一次。 |
| Antigravity | 品牌安装目录中的 `resources/bin/language_server`，或服务入口的明确 Antigravity user-agent 标记。 |
| Cursor | 桌面的 `cursor-agent-host/dist/agent-host-daemon/dist/bin/daemon.cjs`；官方 CLI 的 `cursor-agent/versions/<版本>/index.js`，包括 Linux 别名与 `--use-system-ca`；[Linux/Windows 产物证据](acceptance/cursor-cli-official-entrypoint.json)。 |
| DeepSeek Harness | Node 的 `@deepseek-ai/dsh/lib/bin.js`、官方源码 CLI 入口，以及 DSH Desktop 的专用 harness Node 入口。 |

识别规则检查真实入口，不在所有参数中搜索品牌字符串。普通 Node、Electron 窗口/renderer、工具/helper、插件管理命令与提示词中的路径不计为 Agent。Rust 与 WSL Python 共用规则；原始 PID、参数、路径和发行版名称留在采集内部，API 只公开 provider、数量、环境、采样时间及来源状态。官方资料和已安装包只读检查见[研究记录](acceptance/additional-monitors/official-research.json)；没有为了验收启动这些 Agent 或模型。

界面使用官方本地图标。只确认服务进程时显示“服务运行”；完整成功且为零时显示“未发现运行进程”。不能读取完整来源时保留已确认数量并显示“至少 N”，没有正计数则显示“状态未知 / —”；旧缓存标为 stale，不加入当前数量。仅监控的行不展示未探测的安装、登录和可启动性字段。进程数量不代表对话或子任务数量，后续会话活动接入需要另行验证。

OS 读取共用一个 worker 与三秒调用期限。调用超时后 worker 在实际读取结束前仍占用门禁，晚到结果被丢弃；无法读取一个候选不会清掉其他已确认进程。候选最多 512 个，每个入口检查最多 64 个参数、16 KiB；已退出进程和截断候选不会成为确认计数。

## 当前验证

集成基线 `ff5fc4d` 的 [Web 与协议完整门禁](acceptance/additional-monitors/frontend-validation.json)为 Web 814、协议 94，共 908 项通过；类型检查和 Web 生产构建通过，dist 共 38 项。相同基线的 [Rust workspace 门禁与复审](acceptance/additional-monitors/rust-review.json)为 1,287 项通过、1 项忽略，包含 19 项进程单元测试和 2 项 doctest；rustfmt 通过。该次全量门禁准确绑定基线，后续入口修正使用单独专项证据。

Cursor 官方启动器的静态核实发现其真正入口是版本目录内的 Node `index.js`，Linux 可带 `--use-system-ca`。补齐这两个识别条件后，8 个正例、16 个反例逐项在 Rust 与 WSL Python 对照通过，覆盖 Windows 路径、别名、相对入口、辅助脚本、运行时表达式和提示词中的路径。最终集成 `bee8ebd` 的[进程专项复验](acceptance/additional-monitors/cursor-integrated-validation.json)20 项全部通过；格式和 Python 语法检查通过。只下载与解析官方产物，没有执行 Cursor 安装器或 CLI。

[生产 Linux 只读取样](acceptance/additional-monitors/linux-production-snapshot.json)确认已有 Codex 3 个进程，同时保留 metadata unavailable 来源。其他 provider 在这次不完整采样中为未知，不能据零值宣称未运行。该取样没有启动模型，也不替代四项 provider 的实际运行验收。

Windows 最终 EXE、配套 Web、用户桌面安装及 7337/7338 后端部署尚待新产物验证。历史安装和便携包的 v2 运行记录仍保留在[来源验收](32-windows-wsl-activity.md)，不作为本次新增监控的已部署证明。
