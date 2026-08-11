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
<a href="#1-安裝">
  <img src="https://img.shields.io/badge/PLATFORM-WIN%20%7C%20MAC%20%7C%20LINUX-4C8BF5?style=flat-square&logo=windowsterminal&logoColor=white&labelColor=555555" alt="Platform">
</a>
<a href="https://github.com/dieWehmut/Orchester/blob/main/LICENSE-MIT">
  <img src="https://img.shields.io/badge/LICENSE-MIT%20OR%20APACHE--2.0-green?style=flat-square&logo=github&logoColor=white&labelColor=555555" alt="License">
</a>

</div>

<div align="center">

[简体中文](../README.md) | 繁體中文 | [English](README.en.md)

</div>

---

`Orchester` 是一個用 Rust 寫的**異質編碼 Agent 編排執行環境**。它把你已經裝好的 Claude Code、Codex CLI、OpenCode 等 Agent 當作子行程拉起來，把它們各自的 JSONL 輸出正規化成同一套事件協定，再用同一個 CLI、同一套生命週期驅動。

Orchester 本身**不是**另一個編碼 Agent，它不重新實作規劃、工具呼叫、記憶或上下文管理。護城河是**協定**：只要統一的 `Event` 串流設計對了，接上一個新 Agent 通常只需要一份 TOML 清單，不用寫 Rust。

## 範例

- 倉庫位址：<https://github.com/dieWehmut/Orchester>
- npm 套件：<https://www.npmjs.com/package/@orchester/cli>

## 功能

- 統一事件協定：`Task` 進，`Event` 串流出，`RunResult` 收尾
- 清單驅動的轉接器，新增 Agent 通常只寫一份 TOML
- 內建 `claude`、`codex`、`opencode`、`mock` 四個轉接器
- 工作階段擷取與 `--resume` 續跑
- `--json` 直接輸出 Orchester 自己的 JSONL，可以再餵給另一個 Orchester
- 互動式終端機：斜線指令、指令面板、啟動頭像
- 自建 Agent Harness：記憶、雜湊鏈稽核、憑證保管、策略引擎、人工審核
- `doctor` 檢查本機 Agent 是否可用
- 外掛管理
- 一行安裝腳本（macOS / Linux / Windows）與 npm 發佈

## 快速開始

### 1. 安裝

macOS 與 Linux：

```bash
curl -fsSL https://raw.githubusercontent.com/dieWehmut/Orchester/main/install.sh | sh
```

Windows PowerShell：

```powershell
irm https://raw.githubusercontent.com/dieWehmut/Orchester/main/install.ps1 | iex
```

安裝腳本會檢查 `git`、`curl`/`wget`、Rust/Cargo 和 C 連結器，缺什麼就用系統套件管理員或 rustup 補上，然後把 `orchester` 裝進 `~/.cargo/bin`。

在 Windows 上，安裝腳本還會把安裝目錄（預設 `%USERPROFILE%\.cargo\bin`）寫進使用者 `PATH`，並在 `%LOCALAPPDATA%\Microsoft\WindowsApps` 可寫入時建立 `orchester.cmd` 墊片，這樣目前的 `cmd.exe` 視窗就能直接用。如果墊片目錄不可用，安裝完成後開一個新終端機即可。

因為 `irm | iex` 無法傳參數，PowerShell 一行安裝改從環境變數讀設定：

```powershell
$env:ORCHESTER_INSTALL_ROOT = "D:\tools\orchester"   # 預設 %USERPROFILE%\.cargo
$env:ORCHESTER_NO_PATH_UPDATE = "1"                  # 不動 PATH
$env:ORCHESTER_REF = "main"                          # 分支、標籤或 commit
irm https://raw.githubusercontent.com/dieWehmut/Orchester/main/install.ps1 | iex
```

也可以從已複製的倉庫安裝：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\werkzeug\install.ps1
```

### 2. npm 安裝

打了 tag 的 npm 發佈通過審核後，中介套件會自動挑選目前平台的原生套件：

```bash
npm install -g @orchester/cli
pnpm add -g @orchester/cli
yarn global add @orchester/cli
bun add -g @orchester/cli
```

這個套件沒有生命週期下載腳本。發佈流程會先發六個平台原生套件，等它們在公開 registry 上看得到之後才送出 `@orchester/cli`。

### 3. 從原始碼建置

不用安裝腳本的話，需要 Rust 1.80 以上的工具鏈：

```bash
git clone https://github.com/dieWehmut/Orchester.git
cd Orchester
cargo build --release
```

### 4. 第一次執行

內建的 `mock` 轉接器不起子行程、不需要任何 API key，可以直接驗證整條流程：

```bash
orchester --version
orchester doctor
orchester --agent mock "hello"
```

從原始碼執行則是：

```bash
cargo run -p orchester-konsole -- list
cargo run -p orchester-konsole -- --agent mock "hello"
cargo run -p orchester-konsole -- --agent mock --json "hello"
```

### 5. 接上真實 Agent

本機裝好並登入過對應的 Agent CLI 之後：

```bash
orchester --agent codex "列出這個倉庫裡的檔案"
orchester --agent claude --resume <session-id> "再補上測試"
```

`--json` 會把每個事件依 Orchester 自己的協定一行一筆寫到 **stdout**（人類可讀的收尾資訊走 stderr），所以 Orchester 可以被管線接給別的工具，也可以接給另一個 Orchester。

### 6. 設定

設定目錄由 `ORCHESTER_HOME` 決定；沒設定時 Windows 用 `%LOCALAPPDATA%\Orchester`，其他平台用 `~/.orchester`。

| 路徑 | 作用 |
|---|---|
| `.orchester/orchester.jsonc` | 使用者層級設定：模型、供應商、治理策略、外掛 |
| `.orchester/project.jsonc` | 專案層級設定，視為不可信輸入來驗證，不能引入憑證或放寬安全策略 |
| `state/runs.db` | 執行記錄，`/resume` 與 `sessions` 讀它 |
| `state/audit.jsonl` | 雜湊鏈稽核記錄 |

`orchester.jsonc` 支援註解，結構大致如下：

```jsonc
{
  "version": 1,
  "model_provider": "OpenAI",
  "model": "gpt-5.6-sol",
  "model_reasoning_effort": "high",
  "env": {
    // 引用憑證保管庫，而不是把明文寫進設定
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

欄位值裡的 `${secret:供應商名}` 會去憑證保管庫取值，`${env:變數名}` 會去別的環境項取值。字面值 `api_key` 只在**受保護的**使用者設定檔裡才被接受：Unix 上要求 `0600`、目錄 `0700`，Windows 上要求收緊過的 ACL；序列化與錯誤訊息一律去識別化。

## 斜線指令

互動模式下（直接執行 `orchester` 不帶參數）可用：

| 指令 | 作用 |
|---|---|
| `/agent` | 選擇或切換要委派的 Agent |
| `/model` | 檢視自建 Agent 的模型目錄、切換設定檔 |
| `/permissions` | 檢視目前生效的權限 |
| `/resume` | 檢視可續跑的執行記錄 |
| `/status` | 檢視自建 Agent 的工作區狀態 |
| `/plugins` | 管理外掛（`list` / `status` / `install` / `remove`） |
| `/claude` `/codex` `/opencode` | 直接拉起對應的原生 Agent |
| `/help` | 顯示說明 |
| `/quit` | 離開，`/exit`、`/q` 同義 |

輸入 `/` 會跳出指令面板，方向鍵選擇、Enter 確認。

## 命令列

| 指令 | 作用 |
|---|---|
| `orchester run <prompt>` | 用一個轉接器跑一次任務，也是預設模式 |
| `orchester list` | 列出發現的轉接器與能力 |
| `orchester doctor [--strict]` | 檢查本機 Agent 是否可用 |
| `orchester sessions` | 列出本機記錄的工作階段中繼資料 |
| `orchester plugin <list\|status\|install\|remove>` | 外掛管理 |

全域參數：`--agent/-a`、`--resume`、`--model/-m`、`--json`。
`--agents`、`--parallel`、`--auto` 已經佔好位但還沒接線，呼叫會明確回報「尚未實作」，而不是默默跑錯。

## 新增一個 Agent

正常情況下加 Agent 是寫**清單**，不是寫程式。把一份 TOML 放進 `manifeste/`：

```toml
# manifeste/claude.toml（節選）
name    = "claude"
command = "claude"
args    = ["-p", "{prompt}", "--output-format", "stream-json", "--verbose"]
resume_args = ["-p", "{prompt}", "--resume", "{session_id}", "--output-format", "stream-json", "--verbose"]
kinds = ["code", "chat"]
supports_resume = true
streaming = true

[parse]
discriminator = "type"        # 用哪個頂層欄位來分支
session_id    = "session_id"  # 點號路徑，命中一次就發 SessionStarted

[parse.map]
assistant = { event = "message", text = "message.content[0].text" }
result    = { event = "result",  text = "result" }
```

通用的 `ManifestAdapter` 會解讀任意這樣的檔案。只有廠商行為確實不規則時才寫 Rust，例如 Codex 的續跑是 `exec resume <id>` **子指令**，這種情況用一整條 `resume_args` 覆蓋即可，仍然是宣告式的。

磁碟上的清單同名時覆蓋內建清單，所以改內建 Agent 的參數不需要重新編譯。

## 運作方式

```
User ──▶ orchester (konsole) ──▶ Conductor (laufzeit)
                                     │
                                     ├─ Registry (verzeichnis) ── 內建 + manifeste/*.toml
                                     ├─ Session  (laufzeit)     ── Starting→Running→Completed/Failed
                                     └─ Adapter  (vertrag)      ── 起子行程，解析 stdout 的 JSONL
                                           │                        └─▶ 正規化 ─▶ protokoll::Event
                                           ▼
                              claude / codex / opencode / mock
```

支撐這套設計的關鍵觀察是：所有目標 Agent 的 headless 形態都收斂到同一個骨架——起子行程、傳 prompt、從 stdout 讀按行分隔的 JSON、抓一個 session id 用於續跑。所以 Orchester 就照這個骨架建模，把各家的 JSONL 對應到同一個廠商無關的 `Event` enum 上。

## 專案結構

crate 用德文角色名命名：

```text
kisten/            # Cargo workspace 成員
  protokoll/       # 核心：Task、Event、RunResult、Capability、SessionState
  modell/          # 供應商無關的單次呼叫語言模型邊界
  vertrag/         # 轉接器契約：AgentAdapter trait + ManifestAdapter 引擎
  adapter/         # 內建轉接器：mock + 編譯期內嵌的 claude/codex/opencode
  verzeichnis/     # 登錄表：發現內建 + 載入 manifeste/*.toml
  laufzeit/        # 執行環境：Conductor、Session，以及 harness/ 子系統
  konsole/         # orchester CLI 執行檔
manifeste/         # 宣告式轉接器定義
werkzeug/          # 安裝與開發輔助腳本
npm/               # npm 發佈套件
.github/           # CI 與發佈工作流程
```

`kisten/laufzeit/src/harness/` 是自建 Agent 的執行殼：設定、憑證、記憶、稽核、策略、審核、工具登錄表、行程沙箱契約、驗證器與回饋引擎。

## 常用指令

```bash
cargo build --release          # 建置
cargo test --workspace         # 全量測試
cargo fmt --all -- --check     # 格式檢查
cargo clippy --all-targets -- -D warnings
```

## 藍圖

- **v0.1（目前）統一呼叫**：單 Agent 執行、JSONL 與渲染兩種輸出、可被磁碟清單覆蓋的登錄表、工作階段擷取與續跑、用於確定性測試的 mock 轉接器。
- **v0.2 穩定的本機執行環境**：設定目錄、`doctor`、持久化工作階段中繼資料、更豐富的能力描述、更完整的 TUI、更多清單轉接器。
- **v0.5 多 Agent 編排**：平行執行、結果彙整與比較、PR review 工作流程、取消與逾時、Git 預檢、每個 Agent 一個 worktree。
- **v1.0 Agent 工作流程執行環境**：DAG 工作流程、檢查點與還原、人工審核中斷、MCP/ACP 橋接、依成本與延遲路由、選配 Web UI、清單之外的外掛體系。

> 設計原則：中心小（協定、轉接器契約、登錄表、執行環境），邊緣寬（清單、子行程轉接器、未來的 MCP/ACP 橋、工作流程與 UI 層）。不要重新實作 Agent 內部，而是用一個執行環境和一條事件串流把它們連起來。

## 貢獻指南

歡迎提交 Issue 和 Pull Request。為了讓維護順暢，請盡量遵守這些約定：

1. 大改動先開 Issue 說明動機、影響範圍和預期行為。
2. 從最新 `main` 開新分支，例如 `feat/manifest-timeout` 或 `fix/resume-id`。
3. 保持改動聚焦，一個 PR 只做一件事。
4. 遵循先寫失敗測試、確認 Red、再實作、確認 Green 的流程。
5. 提交前跑 `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace`。
6. 如果改動影響設定、安裝或使用方式，同步更新三份 README。

## 授權

MIT OR Apache-2.0，見 [LICENSE-MIT](../LICENSE-MIT) 與 [LICENSE-APACHE](../LICENSE-APACHE)。


