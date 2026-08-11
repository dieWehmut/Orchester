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

`Orchester` 是一个用 Rust 写的**异构编码 Agent 编排运行时**。它把你已经装好的 Claude Code、Codex CLI、OpenCode 等 Agent 当作子进程拉起，把它们各自的 JSONL 输出归一化成同一套事件协议，再用同一个 CLI、同一套生命周期驱动。

Orchester 本身**不是**另一个编码 Agent，它不重新实现规划、工具调用、记忆或上下文管理。护城河是**协议**：只要统一的 `Event` 流设计对了，接入一个新 Agent 通常只需要一份 TOML 清单，不用写 Rust。

## 示例

- 仓库地址：<https://github.com/dieWehmut/Orchester>
- npm 包：<https://www.npmjs.com/package/@orchester/cli>

## 功能

- 统一事件协议：`Task` 进，`Event` 流出，`RunResult` 收尾
- 清单驱动的适配器，新增 Agent 通常只写一份 TOML
- 内置 `claude`、`codex`、`opencode`、`mock` 四个适配器
- 会话捕获与 `--resume` 续跑
- `--json` 直接输出 Orchester 自己的 JSONL，可以再喂给另一个 Orchester
- 交互式终端：斜杠命令、命令面板、启动头像
- 自建 Agent Harness：记忆、哈希链审计、凭据保管、策略引擎、人工审批
- `doctor` 体检本地 Agent 可用性
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

### 5. 接入真实 Agent

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
| `~/.orchester/state/runs.db` | 自建 Agent 的运行记录，`/resume` 读它 |
| `~/.orchester/state/audit.jsonl` | 哈希链审计日志 |
| `<项目>/.orchester/project.jsonc` | 项目级配置，作为不可信输入校验，不能引入凭据或放宽安全策略 |

`orchester.jsonc` 支持注释，结构大致如下：

```jsonc
{
  "version": 1,
  "model_provider": "OpenAI",
  "model": "gpt-5.6-sol",
  "model_reasoning_effort": "high",
  "env": {
    // 引用凭据保管库，而不是把明文写进配置
    "OPENAI_API_KEY": "${secret:OpenAI}"
  },
  "model_providers": {
    "OpenAI": {
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

字段值里的 `${secret:供应商名}` 会去凭据保管库取值，`${env:变量名}` 会去别的环境项取值。字面量 `api_key` 只在**受保护的**用户配置文件里才被接受：Unix 上要求 `0600`、目录 `0700`，Windows 上要求收紧过的 ACL；序列化和报错时一律脱敏。

## 斜杠命令

交互模式下（直接运行 `orchester` 不带参数）可用：

| 命令 | 作用 |
|---|---|
| `/agent` | 选择或切换要委派的 Agent |
| `/model` | 查看自建 Agent 的模型目录、切换配置档 |
| `/permissions` | 查看当前生效的权限 |
| `/resume` | 查看可续跑的运行记录 |
| `/status` | 查看自建 Agent 的工作区状态 |
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
| `orchester login [provider]` | 存入服务商 API Key，省略 provider 就用配置里当前激活的那个 |
| `orchester logout [provider]` | 删掉已存的服务商 API Key |
| `orchester plugin <list\|status\|install\|remove>` | 插件管理 |

全局参数：`--agent/-a`、`--resume`、`--model/-m`、`--json`。
`--agents`、`--parallel`、`--auto` 已经占好位但还没接线，调用会明确报「尚未实现」，而不是悄悄跑错。

## 添加一个新的 Agent

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
User ──▶ orchester (konsole) ──▶ Conductor (laufzeit)
                                     │
                                     ├─ Registry (verzeichnis) ── 内置 + manifeste/*.toml
                                     ├─ Session  (laufzeit)     ── Starting→Running→Completed/Failed
                                     └─ Adapter  (vertrag)      ── 起子进程，解析 stdout 的 JSONL
                                           │                        └─▶ 归一化 ─▶ protokoll::Event
                                           ▼
                              claude / codex / opencode / mock
```

支撑这套设计的关键观察是：所有目标 Agent 的 headless 形态都收敛到同一个骨架——起子进程、传 prompt、从 stdout 读按行分隔的 JSON、抓一个 session id 用于续跑。所以 Orchester 就照这个骨架建模，把各家的 JSONL 映射到同一个厂商无关的 `Event` 枚举上。

## 项目结构

crate 用德语角色名命名：

```text
kisten/            # Cargo workspace 成员
  protokoll/       # 核心：Task、Event、RunResult、Capability、SessionState
  modell/          # 供应商无关的单次调用语言模型边界
  vertrag/         # 适配器契约：AgentAdapter trait + ManifestAdapter 引擎
  adapter/         # 内置适配器：mock + 编译期内嵌的 claude/codex/opencode
  verzeichnis/     # 注册表：发现内置 + 加载 manifeste/*.toml
  laufzeit/        # 运行时：Conductor、Session，以及 harness/ 子系统
  konsole/         # orchester CLI 二进制
manifeste/         # 声明式适配器定义
werkzeug/          # 安装与开发辅助脚本
npm/               # npm 分发包
.github/           # CI 与发布工作流
```

`kisten/laufzeit/src/harness/` 是自建 Agent 的执行壳：配置、凭据、记忆、审计、策略、审批、工具注册表、进程沙箱契约、校验器与反馈引擎。

## 常用命令

```bash
cargo build --release          # 构建
cargo test --workspace         # 全量测试
cargo fmt --all -- --check     # 格式检查
cargo clippy --all-targets -- -D warnings
```

## 路线图

- **v0.1（当前）统一调用**：单 Agent 运行、JSONL 与渲染两种输出、可被磁盘清单覆盖的注册表、会话捕获与续跑、用于确定性测试的 mock 适配器。
- **v0.2 稳定本地运行时**：配置目录、`doctor`、持久化会话元数据、更丰富的能力描述、更完整的 TUI、更多清单适配器。
- **v0.5 多 Agent 编排**：并行运行、结果聚合与对比、PR review 工作流、取消与超时、Git 预检、每个 Agent 一个 worktree。
- **v1.0 Agent 工作流运行时**：DAG 工作流、检查点与恢复、人工审批中断、MCP/ACP 桥接、按成本与延迟路由、可选 Web UI、清单之外的插件体系。

> 设计原则：中心小（协议、适配器契约、注册表、运行时），边缘宽（清单、子进程适配器、未来的 MCP/ACP 桥、工作流与 UI 层）。不要重新实现 Agent 内部，而是用一个运行时和一条事件流把它们连起来。

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


