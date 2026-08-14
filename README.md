<h1 align="center">Orchester</h1>

<p align="center">
  <img src="https://count.getloli.com/get/@Orchester?theme=rule34" alt="Visitors">
</p>

<div align="center">

<a href="https://www.rust-lang.org/" target="_blank">
  <img src="https://img.shields.io/badge/RUST-1.80%2B-000000?style=flat-square&logo=rust&logoColor=white&labelColor=555555" alt="Rust">
</a>
<a href="https://www.npmjs.com/package/@orchester/cli" target="_blank">
  <img src="https://img.shields.io/badge/NPM-%40orchester%2Fcli-CB3837?style=flat-square&logo=npm&logoColor=white&labelColor=555555" alt="npm">
</a>
<a href="#1-安装">
  <img src="https://img.shields.io/badge/PLATFORM-WIN%20%7C%20MAC%20%7C%20LINUX-4C8BF5?style=flat-square&logo=windowsterminal&logoColor=white&labelColor=555555" alt="Platform">
</a>
<a href="https://github.com/dieWehmut/Orchester/blob/main/LICENSE-MIT">
  <img src="https://img.shields.io/badge/LICENSE-MIT%20OR%20APACHE--2.0-green?style=flat-square&logo=github&logoColor=white&labelColor=555555" alt="License">
</a>

</div>

<div align="center">

简体中文 | [繁體中文](docs/README.zh-TW.md) | [English](docs/README.en.md)

</div>

---

`Orchester` 是一个用 Rust 编写的**独立 Coding Agent**。它自行负责任务理解、上下文组织、计划、模型调用、工具执行、验证反馈、记忆、人工审批和持久化恢复。

Claude Code、Codex CLI、OpenCode 等外部 Agent 只是可选委派能力。Orchester 可以通过清单驱动它们并统一事件协议，但外部 Agent 不定义 Orchester 的身份，也不接管它的核心循环。

## 示例

- 仓库地址：<https://github.com/dieWehmut/Orchester>
- npm 包：<https://www.npmjs.com/package/@orchester/cli>

## 功能

- 独立 Agent 主循环：上下文 → 模型 → 动作 → 策略/审批 → 工具 → 反馈/记忆 → 停止
- 受治理的文件与命令工具，配套路径护栏、验证器和哈希链审计
- 持久化运行、项目记忆、一次性审批与 `--resume` 恢复
- 交互式终端：`/status`、`/permissions`、`/resume`、`/model`、`/plugins` 等命令
- 统一事件协议：`Task` 进，`Event` 流出，`RunResult` 收尾
- 可选外部 Agent 委派：清单驱动的 `claude`、`codex`、`opencode`、`mock` 适配器
- `--json` 直接输出 Orchester 自己的 JSONL，可以接给其他工具
- `doctor` 检查本地运行环境与可选外部 Agent
- 插件管理
- 一行安装脚本（macOS / Linux / Windows）与 npm 分发

## 快速开始

### 1. 安装

macOS 与 Linux：

```bash
curl -fsSL https://raw.githubusercontent.com/dieWehmut/Orchester/main/install.sh | sh
```

Windows PowerShell：

```powershell
irm https://raw.githubusercontent.com/dieWehmut/Orchester/main/install.ps1 | iex
```

安装脚本会检查 `git`、`curl`/`wget`、Rust/Cargo 和 C 链接器，缺什么就用系统包管理器或 rustup 补上，然后把 `orchester` 装进 `~/.cargo/bin`。

在 Windows 上，安装脚本还会把安装目录（默认 `%USERPROFILE%\.cargo\bin`）写进用户 `PATH`，并在 `%LOCALAPPDATA%\Microsoft\WindowsApps` 可写时创建 `orchester.cmd` 垫片，这样当前 `cmd.exe` 窗口就能直接用。如果垫片目录不可用，安装完成后新开一个终端即可。

因为 `irm | iex` 无法传参，PowerShell 一行安装从环境变量读配置：

```powershell
$env:ORCHESTER_INSTALL_ROOT = "D:\tools\orchester"   # 默认 %USERPROFILE%\.cargo
$env:ORCHESTER_NO_PATH_UPDATE = "1"                  # 不动 PATH
$env:ORCHESTER_REF = "main"                          # 分支、标签或提交
irm https://raw.githubusercontent.com/dieWehmut/Orchester/main/install.ps1 | iex
```

也可以从已克隆的仓库安装：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\werkzeug\install.ps1
```

### 2. npm 安装

打了 tag 的 npm 发布通过审核后，元包会自动挑选当前平台的原生包：

```bash
npm install -g @orchester/cli
pnpm add -g @orchester/cli
yarn global add @orchester/cli
bun add -g @orchester/cli
```

这个包没有生命周期下载脚本。发布流程会先发六个平台原生包，等它们在公共 registry 上可见之后才提交 `@orchester/cli`。

### 3. 从源码构建

不用安装脚本的话，需要 Rust 1.80 以上工具链：

```bash
git clone https://github.com/dieWehmut/Orchester.git
cd Orchester
cargo build --release
```

### 4. 第一次运行

内置的 `mock` 适配器不起子进程、不需要任何 API key，可以直接验证整条流水线：

```bash
orchester --version
orchester doctor
orchester --agent mock "hello"
```

从源码运行则是：

```bash
cargo run -p orchester-konsole -- list
cargo run -p orchester-konsole -- --agent mock "hello"
cargo run -p orchester-konsole -- --agent mock --json "hello"
```

### 5. 调用外部 Agent（可选）

本地装好并登录过对应的 Agent CLI 之后：

```bash
orchester --agent codex "列出这个仓库里的文件"
orchester --agent claude --resume <session-id> "再补上测试"
```

`--json` 会把每个事件按 Orchester 自己的协议一行一条写到 **stdout**（人类可读的收尾信息走 stderr），所以 Orchester 可以被管道接给别的工具，也可以接给另一个 Orchester。

### 6. 配置

Orchester 的家目录在所有平台上都是 `~/.orchester`，和它驱动的 agent 的 `~/.claude`、`~/.codex` 放在一起。`ORCHESTER_HOME` 会整体覆盖这个根目录，配置和状态始终跟着一起走。

| 路径 | 作用 |
|---|---|
| `~/.orchester/orchester.jsonc` | 用户级配置：模型、供应商、治理策略、插件 |
| `~/.orchester/sessions.jsonl` | 委派 Agent 的会话记录，`sessions` 读它 |
| `~/.orchester/state/runs.db` | Orchester 的运行记录，`/resume` 读它 |
| `~/.orchester/state/audit.jsonl` | 哈希链审计日志 |
| `<项目>/.orchester/project.jsonc` | 项目级配置，作为不可信输入校验，不能引入凭据或放宽安全策略 |

`orchester.jsonc` 支持注释，结构大致如下：

```jsonc
{
  "version": 1,
  "model_provider": "OpenAI",
  "model": "gpt-5.6-sol",
  "model_reasoning_effort": "high",
  "model_providers": {
    "OpenAI": {
      "name": "OpenAI",
      "base_url": "https://api.openai.com/v1",
      "wire_api": "responses",
      "api_key": "${secret:OpenAI}"
    }
  },
  "projects": {
    "/path/to/repo": { "trust_level": "trusted" }
  },
  "governance": { "approval_reviewer": "user" },
  "tui": { "status_line": ["current-dir", "model", "permissions"] },
  "plugins": { "example@source": { "enabled": true } }
}
```

字段值里的 `${secret:供应商名}` 会去凭据保管库取值，`${env:变量名}` 仍可用于旧配置或非供应商环境项。请直接在 `model_providers` 中配置当前供应商的 `base_url`、`wire_api` 和 `api_key`；只要存在 `api_key` 就会默认启用 Bearer 认证，不需要单独用 `env` 转接，也不需要 `requires_openai_auth`。字面量 `api_key` 只在**受保护的**用户配置文件里才被接受：Unix 上要求 `0600`、目录 `0700`，Windows 上要求收紧过的 ACL；序列化和报错时一律脱敏。

## 斜杠命令

交互模式下（直接运行 `orchester` 不带参数）可用：

| 命令 | 作用 |
|---|---|
| `/agent` | 选择或切换要委派的 Agent |
| `/model` | 查看 Orchester 的模型目录、切换配置档 |
| `/config` | 查看解析后的配置：两层来源、去敏内容、权限体检；配置读不进去时也照样报路径和原因 |
| `/permissions` | 查看当前生效的权限 |
| `/resume` | 查看可续跑的运行记录 |
| `/status` | 查看 Orchester 的工作区状态 |
| `/login` | 把服务商 API Key 存进系统钥匙串，配置里只留引用 |
| `/logout` | 忘掉已存的服务商 API Key |
| `/plugins` | 管理插件（`list` / `status` / `install` / `remove`） |
| `/claude` `/codex` `/opencode` | 直接拉起对应的原生 Agent |
| `/help` | 显示帮助 |
| `/quit` | 退出，`/exit`、`/q` 同义 |

输入 `/` 会弹出命令面板，方向键选择、回车确认。

## 命令行

| 命令 | 作用 |
|---|---|
| `orchester run <prompt>` | 用一个适配器跑一次任务，也是默认模式 |
| `orchester list` | 列出发现的适配器与能力 |
| `orchester doctor [--strict]` | 体检本地 Agent 是否可用 |
| `orchester sessions` | 列出本地记录的会话元数据 |
| `orchester config` | 打印解析后的配置，密钥只显示引用 |
| `orchester login [provider]` | 存入服务商 API Key，省略 provider 就用配置里当前激活的那个 |
| `orchester logout [provider]` | 删掉已存的服务商 API Key |
| `orchester plugin <list\|status\|install\|remove>` | 插件管理 |

全局参数：`--agent/-a`、`--resume`、`--model/-m`、`--json`。
`--agents`、`--parallel`、`--auto` 已经占好位但还没接线，调用会明确报「尚未实现」，而不是悄悄跑错。

## 添加外部 Agent（可选）

正常情况下加 Agent 是写**清单**，不是写代码。把一份 TOML 放进 `manifeste/`：

```toml
# manifeste/claude.toml（节选）
name    = "claude"
command = "claude"
args    = ["-p", "{prompt}", "--output-format", "stream-json", "--verbose"]
resume_args = ["-p", "{prompt}", "--resume", "{session_id}", "--output-format", "stream-json", "--verbose"]
kinds = ["code", "chat"]
supports_resume = true
streaming = true

[parse]
discriminator = "type"        # 用哪个顶层字段来分支
session_id    = "session_id"  # 点号路径，命中一次就发 SessionStarted

[parse.map]
assistant = { event = "message", text = "message.content[0].text" }
result    = { event = "result",  text = "result" }
```

通用的 `ManifestAdapter` 会解释任意这样的文件。只有厂商行为确实不规整时才写 Rust，比如 Codex 的续跑是 `exec resume <id>` **子命令**，这种情况用一整条 `resume_args` 覆盖即可，仍然是声明式的。

磁盘上的清单同名时覆盖内置清单，所以改内置 Agent 的参数不需要重新编译。

## 工作原理

```
Developer ──▶ orchester CLI ──▶ Application Service
                                      │
                                      ├─▶ Independent Agent Runtime
                                      │     Context → Model → Action
                                      │     → Policy/Approval → Tool
                                      │     → Feedback/Memory/Stop
                                      │
                                      └─▶ Optional Delegation
                                            Registry → Adapter
                                            → claude / codex / opencode / mock
```

Orchester 的独立主循环始终拥有执行权和停止决定。外部 Agent 委派是另一条可选路径：适配器负责启动子进程、解析 JSONL 和保留会话元数据，并把结果转换成统一 `Event` 流。

## 项目结构

crate 按职责拆分：

```text
kisten/            # Cargo workspace 成员
  protokoll/       # 核心：Task、Event、RunResult、Capability、SessionState
  modell/          # 供应商无关的单次调用语言模型边界
  vertrag/         # 适配器契约：AgentAdapter trait + ManifestAdapter 引擎
  adapter/         # 内置适配器：mock + 编译期内嵌的 claude/codex/opencode
  verzeichnis/     # 注册表：发现内置 + 加载 manifeste/*.toml
  laufzeit/        # 运行时：独立 Agent 主循环、Conductor、Session 与治理子系统
  konsole/         # orchester CLI 二进制
manifeste/         # 声明式适配器定义
werkzeug/          # 安装与开发辅助脚本
npm/               # npm 分发包
.github/           # CI 与发布工作流
```

`kisten/laufzeit/src/harness/` 是独立 Agent 核心：配置、凭据、上下文、模型边界、记忆、审计、策略、审批、工具注册表、进程沙箱契约、校验器与反馈引擎。

## 常用命令

```bash
cargo build --release          # 构建
cargo test --workspace         # 全量测试
cargo fmt --all -- --check     # 格式检查
cargo clippy --all-targets -- -D warnings
```

## 路线图

- **v0.1（当前）独立 Agent 基础**：自主主循环、受治理工具、审批、反馈、记忆、恢复、JSONL 与确定性 mock 测试。
- **v0.2 稳定本地 Agent**：完整配置目录、`doctor`、持久化运行、更完整的终端交互、验证器和插件。
- **v0.5 高级委派**：可选外部 Agent 并行运行、结果聚合与对比、PR review 工作流、取消与超时、Git 预检和独立 worktree。
- **v1.0 Agent 工作流运行时**：DAG 工作流、检查点与恢复、人工审批中断、MCP/ACP 桥接、按成本与延迟路由、清单之外的插件体系。

> 设计原则：Orchester 拥有自己的主循环、执行权和停止条件。协议、清单和适配器用于扩展可选委派能力，不把核心智能体控制权交给外部进程。

## 贡献指南

欢迎提交 Issue 和 Pull Request。为了让维护顺畅，请尽量遵守这些约定：

1. 大改动先开 Issue 说明动机、影响范围和预期行为。
2. 从最新 `main` 新建分支，例如 `feat/manifest-timeout` 或 `fix/resume-id`。
3. 保持改动聚焦，一个 PR 只做一件事。
4. 遵循先写失败测试、确认 Red、再实现、确认 Green 的流程。
5. 提交前跑 `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace`。
6. 如果改动影响配置、安装或使用方式，同步更新三份 README。

## 许可

MIT OR Apache-2.0，见 [LICENSE-MIT](LICENSE-MIT) 与 [LICENSE-APACHE](LICENSE-APACHE)。
