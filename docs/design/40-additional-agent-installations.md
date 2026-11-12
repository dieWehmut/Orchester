# Windows 与 WSL 追加 agent 安装

2026-11-12 已追加六种 CLI，Windows 原生与 WSL 各安装一份，共 12 个安装单元。两端的实际安装入口和注册后的常用命令均通过 `--version`，退出码全部为 0。

| Agent | 常用命令 | 已核验版本 | Windows 原生 | WSL |
| --- | --- | --- | --- | --- |
| GitHub Copilot CLI | `copilot` | `1.0.109` | 已安装 | 已安装 |
| Continue CLI | `cn` | `1.5.47` | 已安装 | 已安装 |
| Aider | `aider` | `0.86.2` | 已安装 | 已安装 |
| Goose | `goose` | `1.59.0` | 已安装 | 已安装 |
| Amp | `amp` | `0.0.1794456033-gb2ff22` | 已安装 | 已安装 |
| Auggie | `auggie` | `0.37.1` | 已安装，版本兼容性已测 | 已安装，符合厂商平台说明 |

WSL 命令位于 `~/.local/bin`，链接至各自的 `~/.local/share/orchester-agents/` 安装目录。Windows 命令位于 `%LOCALAPPDATA%\OrchesterAgents\bin`，该目录已加入用户 PATH；关闭旧终端并重新打开后使用。完整入口见[命令矩阵](acceptance/agentrouter-platform-sync/native/additional-agents/root-integration/command-entry-matrix-safe.json)。

Copilot 使用官方 `@github/copilot`，额外安装的便携 PowerShell `7.7.0` 为其 Windows 前置依赖。Continue 使用 `@continuedev/cli`，Amp 使用 `@ampcode/cli`，Auggie 使用 `@augmentcode/auggie`。PowerShell 与废弃的 `@sourcegraph/amp` 别名不计入新增 agent 数量。

本轮完成安装与命令可运行性验证，未执行登录或模型任务。Auggie 的厂商说明支持 Windows 上的 WSL，本轮另实测了原生 Windows 版本命令，完整原生 Windows 模型运行支持仍未验证。这六种追加安装单独记录；Orchester 当前 21 项监控目录的产品覆盖范围仍见[原平台验收记录](39-agentrouter-full-platform-sync.md)。

Windows 注册后的六项[版本检查](acceptance/agentrouter-platform-sync/native/additional-agents/root-integration/registered-command-version-gates-safe.json)与 WSL 六项[版本检查](acceptance/agentrouter-platform-sync/native/additional-agents/root-integration/wsl-registered-command-version-gates-safe.json)均保存实际退出码。官方来源、安装与首次失败记录分别保存在 [Copilot / Continue](acceptance/agentrouter-platform-sync/native/additional-agents/copilot-continue/final-additional-agent-installations-safe.json)、[Aider / Goose](acceptance/agentrouter-platform-sync/native/additional-agents/aider-goose/final-four-installations-safe.json)及 [Amp / Auggie](acceptance/agentrouter-platform-sync/native/additional-agents/amp-auggie/final-safe.json) 子目录。Goose 首次 Windows 验证控制器的退出及 Authenticode 未证；后续独立版本成功记录单独保留，未宣称所有历史安装进程均已清理。

[独立复核](acceptance/agentrouter-platform-sync/native/additional-agents/independent-review-safe.json)核对了 54 份历史公共 JSON 和 53 条散列/大小引用，结果一致；该审查未重跑安装或版本命令，注册后命令的实际运行由上面的独立记录证明。
