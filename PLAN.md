# Orchester Project A 实现计划

> 本计划是课程提交用的简明执行账本。状态只依据仓库提交、当前工作树和可复核命令；缺少原始 Red/Green、评审或外部运行记录时，一律写“未留存”或“待验证”。

## 一、项目目标与边界

Orchester 是一个自研 Coding Agent。核心能力是由本项目拥有模型调用、动作解码、策略判断、人工审批、工具执行、反馈、记忆、恢复和停止条件组成的完整循环；调用 Claude、Codex、OpenCode 等其他 agent 只是可选的委托能力，不是产品本体，也不代替自研循环。

CLI 与 WebUI 复用 Application Service。自研通道为 `ContextAssembler -> LanguageModel -> ActionDecoder -> Policy/Approval/Audit -> ToolRuntime -> Feedback/Memory/Stop`；委托通道继续使用 Registry、Adapter 与 Conductor。SQLite 保存可恢复事实，去敏哈希链保存审计记录。公开 WebUI 只运行三个固定 mock 场景，不接收任意提示词或真实 provider 凭据。

技术范围为 Rust、Tokio、Serde、SQLite、reqwest/OpenAI Responses API、Axum/SSE、React/TypeScript/Vite、Vitest/Playwright、OCI/Docker 与 GitHub Actions。

## 二、执行规则与真实偏差

每项任务原则上采用 TDD：先增加能证明需求缺口的失败测试，再写最小实现，随后重构并运行任务级验证；之后分别做规格符合性评审和代码质量评审。仓库没有为所有任务留下完整的原始提示词、Red/Green 输出、PR 和双评审记录，因此本计划不补造过程证据。

强制 cold-start 原本应在生产实现前完成。实际顺序相反：全新隔离 session 后来只读取早期 `SPEC.md` 与 `PLAN.md`，在一次性 worktree 试作任务 1，未合并其产出，并发现协议冲突、测试不足、`ApprovalRequest.run_id` 缺失及 Windows 命令问题。该补偿检查已经执行并推动修订，但操作纪律仍属部分不合规，不能表述为事前 cold-start 门禁完成或通过。

Windows 本地规范门禁为：

```powershell
cargo +stable-x86_64-pc-windows-gnu run --locked --offline --target x86_64-pc-windows-gnu -p orchester-xtask -- test-all
```

最近一次已留存的 `task/course-closure` 完整门禁尝试未通过：`kisten/server/src/harness.rs` 中真实的 `HarnessApplication::decide` 参数过多，触发 `clippy::too_many_arguments`。主分支尚未合并 server/xtask 收尾切片；修复、合并并完整重跑前，不得宣称项目总门禁绿色。

依赖关系：

```text
T1 -> T2 -> T3 -> T4 -> T5 -> T6 -> T7 -> T8
T3..T8 -> T9 -> T10
T3 + T4 -> T11 -> T12
T2..T12 -> T13 -> T14 -> T15
既有委托运行时 -> T16
T13 + T15 + T16 -> T17 -> T18 -> T19 -> T20
T15 + T16 + T17 -> T21
T17 + T19 + T20 + T21 -> T22
```

可并行部分：T4 完成后，T11–T12 可与 T5–T10 并行；T16 可与自研主循环任务并行维护；满足各自依赖后，T20 与 T21 可并行；T22 最后统一收口。

## 三、任务计划与状态

### 任务 1：版本化 Harness 协议

- **依赖：**无。
- **目标：**定义稳定 ID、action、observation、feedback、approval、stop 与 event envelope，同时保留 legacy `Event` 兼容面。
- **文件：**`kisten/protokoll/src/harness.rs`、`lib.rs`、`tests/harness_roundtrip.rs`、`roundtrip.rs`。
- **预期失败测试：**验证 JSON round-trip，并拒绝空 ID、未知版本、无界摘要、缺少 `run_id` 或 action origin；类型不存在时协议测试应编译失败。
- **实现：**使用 newtype ID、严格 DTO、显式版本、摘要边界和兼容导出，持久化载荷不得包含秘密。
- **验证：**运行协议 crate 与 legacy round-trip 测试。
- **提交：**`b85ad9b`，后续修订 `7f6308d`、`db1973c`、`fec1532`、`fc4bbd1`。
- **真实状态：**核心能力已提交；补偿 cold-start 发现并推动修订，但原始规格评审与代码质量评审未完整留存。

### 任务 2：单次模型边界、严格解码与 Scripted LLM

- **依赖：**任务 1。
- **目标：**把单次模型请求与 agent loop 分开，提供无需网络和凭据的确定性模型替身。
- **文件：**`kisten/modell/src/{lib,types,decoder,scripted}.rs`、`tests/scripted_loop_parts.rs`。
- **预期失败测试：**覆盖 scripted response 顺序、非法 action、超长或重复 call ID、未知字段与取消传播；模块缺失时 `cargo test -p orchester-modell` 失败。
- **实现：**定义 `LanguageModel`、不可变 request/response、严格 `ActionDecoder` 与线程安全 `ScriptedModel`；模型层不执行工具或循环。
- **验证：**运行 modell 全部单元与集成测试。
- **提交：**`d1ab8dc`、`9ebc0ec`、`5292021`。
- **真实状态：**实现与加固已提交；逐步 Red/Green、PR 和双评审记录未留存，需按当前代码复核。

### 任务 3：JSONC 配置与受保护凭据

- **依赖：**任务 2。
- **目标：**支持完整的用户/项目配置、provider profile 与安全策略合并，确保项目配置不能放宽治理或覆盖用户秘密。
- **文件：**`kisten/laufzeit/src/harness/config.rs`、`credentials.rs`、protected-file 平台模块、CLI 配置与凭据测试。
- **预期失败测试：**项目配置尝试放宽 policy、写 provider secret 或覆盖 endpoint 时失败；显示与日志不得泄密；不安全文件权限、错误 set/update 状态和非 TTY secret 输入应 fail closed。
- **实现：**支持 `${secret:OpenAI}` 与系统凭据存储；同时允许在仅用户可读的用户级 `orchester.jsonc` 中写 literal API key，前提是 Windows ACL 或 Unix 权限门禁通过。项目级 literal key 永远拒绝，所有展示均去敏。
- **验证：**运行配置、ACL、凭据生命周期和泄漏测试，并在 disposable credential 上复核 release build。
- **提交：**基础 `d0b1640`，凭据 CLI `f9d7136`；主分支另有配置权限与 CLI 加固提交。
- **真实状态：**能力已部分提交且仍有未提交配置模板工作。早期“只允许 secret reference”的口径已由“受保护用户配置可例外使用 literal key”取代；外部完整验收尚缺。

### 任务 4：事务化 Run Store 与 Schema

- **依赖：**任务 3。
- **目标：**持久化 owner/project scoped run、模型阶段、action、approval、validator、event 与 transcript，为中断恢复提供唯一事实源。
- **文件：**`harness/run_store/`、`store.rs`、`migrations/*.sql`、`tests/run_store*.rs`。
- **预期失败测试：**覆盖数据库重开恢复、跨 owner 隐藏、非法状态回退、损坏 transcript、未绑定事件和秘密字段拒绝。
- **实现：**使用事务、外键、row version、append-only transcript、owner/project join、schema validation 与 sanitized persistence。
- **验证：**运行 run-store、migration、transcript 和 resume 测试。
- **提交：**`caffe3f`，后续连续加固至 `e4d83fb`。
- **真实状态：**恢复与持久化主链已提交；历史任务级 PR 和双评审未留存，当前 schema 仍需整体复核。

### 任务 5：工作区路径护栏与变更锁

- **依赖：**任务 4。
- **目标：**阻止 traversal、link/reparse、ADS、路径歧义、替换竞态与跨工作区写入。
- **文件：**`governance/path_guard.rs`、`workspace_lock.rs`、平台文件模块、`tests/path_guard.rs`、文件工具测试。
- **预期失败测试：**拒绝 `..`、越界绝对路径、symlink/reparse、Windows ADS 和对象替换，允许已验证的工作区内常规文件。
- **实现：**规范化路径，绑定 handle/object identity 与 workspace capability，在写入和 rename 前复验，并按对象加锁。
- **验证：**运行路径与文件治理测试，在 Windows/Linux 规则上分别复核。
- **提交：**首个主提交 `4e1e985`，后续有 capability、identity、ADS 与歧义路径加固。
- **真实状态：**核心能力已提交；跨平台独立质量评审记录未留存。

### 任务 6：策略引擎与结构化命令分类

- **依赖：**任务 5。
- **目标：**确定性产生 ALLOW/ASK/DENY、风险等级、规则 ID 和 effect class。
- **文件：**`governance/policy.rs`、`command.rs`、`tests/policy_matrix.rs`、`governed_execution.rs`。
- **预期失败测试：**覆盖只读、git write、依赖安装、解释器、破坏命令、网络和 bounded sleep，并证明项目或会话配置不能放松硬规则。
- **实现：**硬不变量优先，解析结构化 argv，使用 restriction lattice 合并策略，把 decision 与 durable action、workspace 绑定。
- **验证：**运行 policy matrix 与治理执行测试。
- **提交：**`c4aec5e`、`6e9e38e` 及后续 binding 提交。
- **真实状态：**主要实现已提交；规格与质量评审证据未留存。

### 任务 7：去敏哈希链审计与执行前 Barrier

- **依赖：**任务 4、6。
- **目标：**所有副作用执行前，先留下与 durable action 绑定且可验真的去敏审计证据。
- **文件：**`harness/audit.rs`、`audit/record.rs`、`barrier.rs`、`tests/audit_approval.rs`。
- **预期失败测试：**Authorization、API key 和 known secret 必须去敏；篡改链可检测；缺 policy、permit 或 audit checkpoint 时 executor 调用次数为零。
- **实现：**canonical JSON、SHA-256 链、落盘同步、rotation 连接记录、secret scan 与 pre-execution barrier。
- **验证：**运行审计/审批测试与历史 secret scan。
- **提交：**`06dfc8a`、`ada2de9`。
- **真实状态：**能力已提交；rotation 恢复和独立评审证据待补。

### 任务 8：持久化 HITL 审批状态机

- **依赖：**任务 4、7。
- **目标：**审批与 action hash、owner、policy、workspace、generation 绑定，可恢复且只能消费一次。
- **文件：**`approval.rs`、`run_store/resume.rs`、service runtime 与审批测试。
- **预期失败测试：**覆盖非法状态迁移、action drift、replay、跨 owner、过期和并发双消费。
- **实现：**row-version CAS、one-time capability、actor/reason、resume evidence，消费前重新验证全部绑定。
- **验证：**运行 approval、audit 和 runtime resume 套件。
- **提交：**`06dfc8a`、`efc505f`、`399b9e7`。
- **真实状态：**action-bound resume 已提交；完整 TDD 与双评审过程未留存。

### 任务 9：治理工具、文件操作与进程沙箱

- **依赖：**任务 3–8。
- **目标：**所有工具统一经过 registry generation、policy、permit、path guard、barrier，并把有界 observation 回灌循环。
- **文件：**`tools.rs`、`executor.rs`、`execution/`、`process_runtime.rs`、workspace read/write/patch 模块及测试。
- **预期失败测试：**无 permit 不执行；拒绝 generation 漂移；限制输出；timeout/cancel 终止进程树；删除 secret 环境变量；CAS patch 拒绝陈旧内容。
- **实现：**bounded read、atomic write、strict patch、显式环境 allowlist、Unix process group、Windows Job Object 和 sanitized observation。
- **验证：**运行工具、文件、进程、取消和治理组合测试。
- **提交：**`ca454e9`、`4bf01e8`、`b90a130`、`8b65fe3`、`2a3d2e5`、`65a091e`、`3148e39`。
- **真实状态：**主要执行链已提交；平台沙箱复核和任务级评审记录仍缺。

### 任务 10：校验器、变更代次与反馈

- **依赖：**任务 9。
- **目标：**把 test/lint/typecheck 结果变成结构化反馈，禁止用陈旧绿色结果授权 `finish`。
- **文件：**`feedback.rs`、`mutation.rs`、`validator.rs`、coordinator/run-store 和反馈循环测试。
- **预期失败测试：**validator 失败后下一动作变化；源码变更递增 generation；校验期间变更使结果作废；只有当前 generation 全部通过才能完成。
- **实现：**失败分类、有界输出、source fingerprint/watch、validator state 和 durable generation binding。
- **验证：**运行 feedback、mutation、validator 与 runtime tests。
- **提交：**`9b0e808`、`868e336`、`3b4682d`。
- **真实状态：**validator-gated completion 已提交；项目总门禁仍被 server Clippy 问题阻断。

### 任务 11：项目级记忆、审批与秘密扫描

- **依赖：**任务 3、4。
- **目标：**只召回已批准、owner/project scoped 且通过 secret admission 的项目记忆。
- **文件：**`memory.rs`、memory migrations、`memory_store.rs`、`memory_cli.rs` 及测试。
- **预期失败测试：**未批准或跨项目记忆不可见；secret/PEM/token 拒绝且不回显；forget 清除内容与索引但保留 tombstone。
- **实现：**propose/approve/forget、FTS 或关键词检索、访问记录、secret scan、事务和审计事件。
- **验证：**运行 memory store、runtime 与 CLI tests。
- **提交：**`b82a681`、`e6b5cb3`、`4af75ab`、`5486dfe`。
- **真实状态：**存储和 CLI 已提交；CLI 独立质量评审未留存。

### 任务 12：上下文组装与哈希绑定压缩

- **依赖：**任务 2、4、11。
- **目标：**按安全优先级与预算组装模型输入，不拆分 tool call/result，并使摘要绑定 transcript prefix。
- **文件：**`context.rs`、`transcript.rs`、`tests/context_assembler.rs`。
- **预期失败测试：**验证来源顺序、预算下限、去重、完整 turn/tool pair；未授权文件、配置和凭据不得进入模型；摘要 hash 不匹配时拒绝。
- **实现：**来源排序、按 relevance/age 淘汰、完整 pair 保留、summary binding 与 secret exclusion。
- **验证：**运行 context、transcript 和 continuation tests。
- **提交：**`08a2dea`、`5ad55e3`。
- **真实状态：**核心实现已提交；预算与压缩评审记录未留存。

### 任务 13：自研主循环、停止条件与取消

- **依赖：**任务 2–12。
- **目标：**由 Orchester 自身拥有完整 agent loop，而不是把流程委托给高层 agent SDK 或外部 agent。
- **文件：**`agent_loop.rs`、`coordinator.rs`、`service/`、`session.rs` 和 self-agent tests。
- **预期失败测试：**ScriptedModel 驱动 text/tool/approval/feedback/finish；所有副作用都需授权；取消传播；限制 step、时间、usage 和重复循环；终态持久化。
- **实现：**执行 request→model→decode→persist→policy/approval/tool→observation→feedback→next step，并原子转换终态。
- **验证：**运行 loop、service、run-store 与 runtime tests。
- **提交：**`9da4355`、`7cfad8b`、`33bf066`，后续 `33aae0b` 与 `d4d7cb0` 修复终态并补测试。
- **真实状态：**自研循环和 durable completion 已提交；历史评审证据不完整。

### 任务 14：OpenAI Responses 单请求 Provider

- **依赖：**任务 2、3、13。
- **目标：**实现一次 Responses HTTP 调用边界，仅负责 provider transport，不接管 agent loop。
- **文件：**`provider/http/`、`provider/responses/`、`model_http_*`、`responses_*` tests。
- **预期失败测试：**覆盖 URL/origin、Bearer 目标限制、请求 schema、tool call/result 和有界错误；401 不重试，408/429/5xx 有限重试，取消立即生效。
- **实现：**校验 base URL，使用 rustls reqwest、严格 codec、状态与请求 ID 去敏、有限 backoff/cancel。
- **验证：**运行 provider tests；真实 smoke 只能用 disposable credential，并保存去敏结果。
- **提交：**provider 提交链自 `3927ac4` 延续至 `fccc8f9`。
- **真实状态：**适配能力已提交；真实 provider smoke 和独立评审证据缺失。

### 任务 15：Application Service 与自研 CLI

- **依赖：**任务 3–14。
- **目标：**提供运行、恢复、状态、权限、模型、插件、审批、auth、config、memory、audit 与 demo CLI。
- **文件：**`kisten/konsole/src/{args,main,self_agent}.rs` 及其子模块和 CLI tests。
- **预期失败测试：**覆盖 Clap grammar、TTY/non-TTY、JSONL、stderr warning、approval ID、auth 禁止 secret 参数、memory 权限、`/status`、`/permissions`、`/resume`、`/model`、`/plugins`。
- **实现：**由 Application Service 组合运行时，CLI 提供交互首页、稳定人类/JSON 输出、隐藏凭据输入和可恢复会话视图。
- **验证：**运行 CLI、self-agent service、status、permissions、model、resume、plugin 与 credential tests。
- **提交：**核心运行时与 CLI 提交链，凭据生命周期为 `f9d7136`；主分支另有各 slash command 的独立提交。
- **真实状态：**主要交互能力已存在；`/resume` 当前主要用于列出安全可恢复 run，完整交互续跑体验仍需验收；原计划 namespace 与当前 grammar 有差异。

### 任务 16：外部 Agent 委托兼容与生命周期修复

- **依赖：**既有 adapter/runtime 与任务 15。
- **目标：**保留 Claude、Codex、OpenCode 等委托能力，并与自研 Coding Agent 主通道清晰分界。
- **文件：**`kisten/adapter/`、`conductor.rs`、`interactive.rs`、`process.rs`、CLI/session tests。
- **预期失败测试：**覆盖 legacy 路由、显式 delegate、stdout/stderr 排空、EOF 结果、cancel/drop 终止进程树和退出码保留。
- **实现：**复用 Registry/Conductor 与流驱动器，只把外部 agent 作为可选工具；保留兼容别名 `/agent`。
- **验证：**运行 adapter、CLI、session 和 process lifecycle tests。
- **提交：**既有基础 `ffb6f68` 及后续插件、会话和生命周期修复提交。
- **真实状态：**委托能力可验证存在；任务专属 PR 与双评审未找到，当前 grammar 是否完全满足修订规格仍待确认。

### 任务 17：三项确定性演示与一键测试

- **依赖：**任务 13、15、16。
- **目标：**离线演示治理拦截、校验失败后修正、审批持久化与恢复三个不同机制。
- **文件：**`demo.rs`、`kisten/xtask/`、`werkzeug/run_mechanism_demos.ps1` 和机制测试。
- **预期失败测试：**guardrail 场景 executor 调用为零；feedback-loop 从 failed 变 passed 且 action hash 改变；approval-resume 只执行一次并拒绝 drift/replay。
- **实现：**固定 ScriptedModel/fixture、JSON 输出、`orchester demo` 和 xtask `test-all/demo/secret-scan`。
- **验证：**运行规范 `test-all` 和三项独立 demo。
- **提交：**`c8da178`、`4c9a877`。
- **真实状态：**机制演示已提交到课程分支，且曾有局部 GNU 测试通过记录；尚未合并主分支，课程分支完整门禁仍先被 server Clippy 阻断。

### 任务 18：Axum Server、Session、SSE 与配额

- **依赖：**任务 4、8、13、17。
- **目标：**只开放三个固定 mock scenario，并保证 run、approval、SSE 均与 owner/session 绑定。
- **文件：**`kisten/server/src/{lib,main,routes,session,sse,harness}.rs`、`tests/web_auth.rs`。
- **预期失败测试：**跨 session 404、CSRF、one-time capability、replay 409、未知字段 422、任意 run route 404、quota/TTL 和 SSE reconnect。
- **实现：**cookie session、owner join、固定 schema、quota/TTL、durable-runtime adapter 和 SSE。
- **验证：**运行 server tests 与 `cargo clippy ... -p orchester-server --all-targets -- -D warnings`。
- **提交：**初版 `d4c95c5`。
- **真实状态：**初版只存在于未合并的课程分支，`task/course-closure` 工作树还有未提交修订；最近验证在 `HarnessApplication::decide` 的参数数量 lint 失败，需修复、重跑、评审、提交并合并。

### 任务 19：React WebUI

- **依赖：**任务 18。
- **目标：**提供固定场景、事件时间线、审批卡和反馈状态组成的可访问治理工作台。
- **文件：**`web/src/`、`web/e2e/demo.spec.ts`、Vite/Vitest/Playwright 配置。
- **预期失败测试：**覆盖三场景、事件反馈、审批一次提交、错误/过期、键盘操作、响应式布局、跨 session 拒绝和零 console error。
- **实现：**typed client、固定场景 `RunComposer`、`EventTimeline`、`ApprovalCard`、`FeedbackPanel`；不提供任意 prompt box 或 provider 配置。
- **验证：**运行 `npm test -- --run`、build 与 E2E。
- **提交：**初版 `d4c95c5`。
- **真实状态：**初版只存在于未合并的课程分支，未提交修订需在 server 绿色后复验、评审、提交并合并；公开部署证据缺失。

### 任务 20：OCI 公共演示与部署烟测

- **依赖：**任务 18、19。
- **目标：**构建非 root、只读根、固定 mock 场景容器，并用 PowerShell 与 POSIX 客户端验收。
- **文件：**`Dockerfile`、`.dockerignore`、`deploy/render.yaml`、`smoke-web.ps1`、`smoke-web.sh`。
- **预期失败测试：**非 demo mode 拒绝启动；镜像无源码和凭据；任意 prompt 字段拒绝；三场景、审批恢复、replay 拒绝和 provider 字段泄漏扫描均通过。
- **实现：**Rust/Web 多阶段构建，最终镜像只含 server/static/CA，以非 root 用户运行，仅临时目录可写。
- **验证：**Docker build/run 后执行两套 smoke，并记录镜像摘要与公网 HTTPS URL。
- **提交：**容器与部署文件主要位于 course-closure 未提交工作树。
- **真实状态：**真实本地 server 上两套 smoke 曾通过，但 Docker daemon 不可用；没有容器构建、公网 URL 或公网 smoke 证据。

### 任务 21：归档安装、发布验证与卸载

- **依赖：**任务 15–17。
- **目标：**让全新 Windows/Linux 主机无需 Git/Rust 即可安装固定版本，并能按 receipt 安全卸载。
- **文件：**根 `install.*`/`uninstall.*`、`werkzeug/release-*`、安装 fixture 与 remote helper。
- **预期失败测试：**缺入口、错误 checksum/结构不破坏旧 binary、PATH 不重复、receipt 只拥有自己创建项、默认保留 config、purge 必须确认。
- **实现：**OS/arch 选择、HTTPS archive+sha256、stage/atomic replace、可选 attestation、精确 receipt 和隔离 remote helper。
- **验证：**运行 PowerShell/POSIX install-uninstall fixtures；发布后再执行授权远程验收。
- **提交：**`f7fcec4`。
- **真实状态：**本地双平台 fixture 能力已提交到课程分支，尚未合并主分支；release、sha256/attestation 和全新远程主机证据仍缺。

### 任务 22：GitHub Actions、中文课程文档与收尾

- **依赖：**任务 17、19、20、21。
- **目标：**使用 GitHub Actions 运行提交门禁，完成中文课程文档，并只记录真实交付证据。
- **文件：**`.github/workflows/ci.yml`、`README.md`、`SPEC.md`、`PLAN.md`、`SPEC_PROCESS.md`、`AGENT_LOG.md`、`REFLECTION.md`、`docs/RELEASE_CHECKLIST.md`、`docs/evidence/`。
- **预期失败测试：**`verify-course-files` 实际只检查指定必需文件存在、`REFLECTION.md` 非空白字符数、公网 WebUI HTTPS URL，以及带 `status: pass` 和公网 HTTPS run URL 的 hosted CI 证据；它不检查 README 章节、GitHub job 名称或 AI 披露文本。
- **实现：**复核 GitHub `unit-test`、`web-test`、`container-build`、`secret-scan`，统一文档状态，删除 `.gitlab-ci.yml`，不得用计划或占位文件冒充外部证据。
- **验证：**先修 server Clippy，再完整运行 `test-all`、`verify-course-files`、Web、installer、Docker/smoke 和 secret scan；随后核对 exact commit 的 GitHub run。
- **提交：**CI 基础 `4706576` 只存在于课程分支；中文文档正在主分支统一提交，push/PR CI 尚待合并。
- **真实状态：**用户明确选择 GitHub Actions only，删除 GitLab 配置；这偏离课程原文对 `.gitlab-ci.yml` 的要求，须如实披露并由课程方决定是否接受。主分支当前只有手动触发的发布工作流，没有 push/PR 课程测试工作流。`REFLECTION.md` 已由 AI 辅助生成并披露辅助范围，仍待提交者逐句复核事实和个人判断。hosted CI、公网 URL、release/attestation 和远程验收证据均未完成。

## 四、完成定义

只有同时满足以下条件才可宣称完成：22 项任务的已提交、未提交、待评审与外部阻塞均标注准确；规范本地门禁退出码为零；cold-start 顺序偏差持续披露；exact release commit 的 GitHub Actions 全绿；release 摘要与 attestation、全新机器安装卸载、公开 WebUI smoke、授权远程验收和全历史 secret scan 都有去敏证据；`REFLECTION.md` 的 AI 辅助披露真实且已由提交者复核。

GitHub-only 仍是等待课程方接受的显式偏离。局部测试通过、脚本存在、YAML 存在或模型输出 `finish` 都不等于项目完成。
