# Orchester 实现计划

## 一、目标与边界

Orchester 是一个自研 Coding Agent。模型调用、动作解码、策略判断、人工审批、工具执行、反馈、记忆、恢复和停止条件均由本项目负责。调用 Claude、Codex、OpenCode 等其他 agent 只是可选委托能力，不代替主循环。

主通道为：

```text
ContextAssembler -> LanguageModel -> ActionDecoder
-> Policy/Approval/Audit -> ToolRuntime
-> Feedback/Memory/Stop
```

核心技术范围为 Rust、Tokio、Serde、SQLite、reqwest、OpenAI Responses API、命令行交互、PowerShell/POSIX 脚本与 GitHub Actions。所有副作用都经过路径护栏、策略、审批、审计和可恢复状态检查。

## 二、执行规则

- 每项任务先写能证明缺口的失败测试，再实现最小行为。
- 测试、策略、配置和持久化边界必须有独立单元测试与组合测试。
- 提交前运行受影响 crate 的测试、格式检查、Clippy 和秘密扫描。
- 需要网络或凭据的模型测试与离线 ScriptedModel 测试分离；默认测试不依赖真实凭据。
- 配置合并遵循“用户级安全边界优先于项目级覆盖”的限制格。
- 任务依赖如下：

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

T11–T12 可在 T4 后与 T5–T10 并行；T16 可与 T13–T15 并行；满足依赖后 T20 与 T21 可并行。

## 三、任务计划

### 任务 1：版本化 Harness 协议

- **依赖：**无。
- **目标：**定义稳定 ID、action、observation、feedback、approval、stop 和 event envelope，并保留 legacy `Event` 兼容面。
- **文件：**`kisten/protokoll/src/harness.rs`、`lib.rs`、`tests/harness_roundtrip.rs`、`roundtrip.rs`。
- **预期失败测试：**拒绝空 ID、未知版本、无界摘要、缺少 `run_id` 或 action origin；验证 JSON round-trip。
- **实现：**使用 newtype ID、严格 DTO、显式版本和摘要边界；持久化载荷不得包含秘密。
- **验证：**运行协议 crate、legacy round-trip 和 schema 测试。
- **提交：**协议类型、编码解码和兼容导出分别提交。

### 任务 2：单次模型边界与严格解码

- **依赖：**任务 1。
- **目标：**把单次模型请求与 agent loop 分开，提供无网络、无凭据的确定性 `ScriptedModel`。
- **文件：**`kisten/modell/src/{lib,types,decoder,scripted}.rs`、`tests/scripted_loop_parts.rs`。
- **预期失败测试：**覆盖响应顺序、非法 action、超长或重复 call ID、未知字段和取消传播。
- **实现：**定义 `LanguageModel`、不可变 request/response、严格 `ActionDecoder` 和线程安全脚本模型；模型层不执行工具或循环。
- **验证：**运行 modell crate 全部单元与集成测试。
- **提交：**接口、解码器、脚本模型和测试分步提交。

### 任务 3：JSONC 配置与受保护凭据

- **依赖：**任务 2。
- **目标：**支持用户级和项目级配置、provider profile 与安全策略合并，阻止低信任配置覆盖秘密或放宽治理。
- **文件：**`kisten/laufzeit/src/harness/config.rs`、`credentials.rs`、受保护文件模块、配置测试。
- **预期失败测试：**项目配置放宽 policy、写 provider secret 或覆盖端点时拒绝；权限不安全、状态错误和非 TTY secret 输入 fail closed。
- **实现：**支持 `${secret:OpenAI}`、系统凭据存储和仅用户可读配置中的 literal key；项目级 literal key 永远拒绝，展示和日志全部去敏。
- **验证：**运行 JSONC、ACL、凭据生命周期和泄漏测试。
- **提交：**配置解析、合并策略、凭据存取分别提交。

### 任务 4：事务化 Run Store 与 Schema

- **依赖：**任务 3。
- **目标：**持久化 owner/project scoped run、模型阶段、action、approval、validator、event 和 transcript，为恢复提供唯一事实源。
- **文件：**`harness/run_store/`、`store.rs`、`migrations/*.sql`、`tests/run_store*.rs`。
- **预期失败测试：**覆盖数据库重开、跨 owner 隔离、非法状态回退、损坏 transcript、未绑定事件和秘密字段拒绝。
- **实现：**使用事务、外键、row version、append-only transcript、关联约束和 sanitized persistence。
- **验证：**运行 store、migration、transcript 和 resume 测试。
- **提交：**schema、迁移、读写事务和恢复接口分步提交。

### 任务 5：工作区路径护栏与变更锁

- **依赖：**任务 4。
- **目标：**阻止 traversal、link/reparse、ADS、路径歧义、替换竞态与跨工作区写入。
- **文件：**`governance/path_guard.rs`、`workspace_lock.rs`、平台文件模块、`tests/path_guard.rs`。
- **预期失败测试：**拒绝 `..`、越界绝对路径、symlink/reparse、Windows ADS 和对象替换；允许已验证工作区内文件。
- **实现：**规范化路径，绑定 handle/object identity 与 workspace capability，在写入和 rename 前复验并按对象加锁。
- **验证：**运行路径、文件治理和跨平台规则测试。
- **提交：**规范化、身份校验、锁和平台适配分别提交。

### 任务 6：策略引擎与结构化命令分类

- **依赖：**任务 5。
- **目标：**确定性产生 `ALLOW`、`ASK`、`DENY`、风险等级、规则 ID 和 effect class。
- **文件：**`governance/policy.rs`、`command.rs`、`tests/policy_matrix.rs`、`governed_execution.rs`。
- **预期失败测试：**覆盖只读、git write、依赖安装、解释器、破坏命令、网络和 bounded sleep；证明低信任配置不能放宽硬规则。
- **实现：**硬不变量优先，解析结构化 argv，使用 restriction lattice 合并策略，把 decision 与 durable action、workspace 绑定。
- **验证：**运行 policy matrix 与治理执行测试。
- **提交：**命令分类、限制格、决策输出和绑定逻辑分别提交。

### 任务 7：去敏哈希链审计与执行前 Barrier

- **依赖：**任务 4、6。
- **目标：**副作用执行前留下与 durable action 绑定且可验真的审计证据。
- **文件：**`harness/audit.rs`、`audit/record.rs`、`barrier.rs`、`tests/audit_approval.rs`。
- **预期失败测试：**Authorization、API key 和 known secret 必须去敏；篡改链可检测；缺 policy、permit 或 checkpoint 时执行器调用次数为零。
- **实现：**使用 canonical JSON、SHA-256 链、落盘同步、rotation 连接记录、secret scan 和 pre-execution barrier。
- **验证：**运行审计、审批和秘密扫描测试。
- **提交：**记录格式、哈希链、去敏器和 barrier 分步提交。

### 任务 8：持久化 HITL 审批状态机

- **依赖：**任务 4、7。
- **目标：**审批与 action hash、owner、policy、workspace、generation 绑定，可恢复且只能消费一次。
- **文件：**`approval.rs`、`run_store/resume.rs`、runtime 与审批测试。
- **预期失败测试：**覆盖非法状态迁移、action drift、replay、跨 owner、过期和并发双消费。
- **实现：**使用 row-version CAS、one-time capability、actor/reason 和 resume evidence；消费前重新验证全部绑定。
- **验证：**运行 approval、audit 和 runtime resume 测试。
- **提交：**状态机、绑定校验、消费接口和恢复测试分别提交。

### 任务 9：治理工具、文件操作与进程沙箱

- **依赖：**任务 3–8。
- **目标：**所有工具统一经过 registry generation、policy、permit、path guard 和 barrier，并把有界 observation 回灌循环。
- **文件：**`tools.rs`、`executor.rs`、`execution/`、`process_runtime.rs`、文件工具测试。
- **预期失败测试：**无 permit 不执行；拒绝 generation 漂移；限制输出；timeout/cancel 终止进程树；清除 secret 环境变量；CAS patch 拒绝陈旧内容。
- **实现：**提供 bounded read、atomic write、strict patch、环境 allowlist、Unix process group、Windows Job Object 和 sanitized observation。
- **验证：**运行工具、文件、进程、取消和治理组合测试。
- **提交：**工具注册、执行器、文件操作和进程生命周期分别提交。

### 任务 10：校验器、变更代次与反馈

- **依赖：**任务 9。
- **目标：**把 test、lint、typecheck 结果转为结构化反馈，禁止陈旧绿色结果授权 `finish`。
- **文件：**`feedback.rs`、`mutation.rs`、`validator.rs`、coordinator/run-store 和反馈测试。
- **预期失败测试：**校验失败改变下一动作；源码变更递增 generation；校验期间变更使结果作废；只有当前 generation 全部通过才能结束。
- **实现：**提供失败分类、有界输出、source fingerprint、validator state 和 durable generation binding。
- **验证：**运行 feedback、mutation、validator 与 runtime tests。
- **提交：**反馈结构、generation、校验器和结束门禁分别提交。

### 任务 11：项目级记忆、审批与秘密扫描

- **依赖：**任务 3、4。
- **目标：**只召回已批准、owner/project scoped 且通过 secret admission 的项目记忆。
- **文件：**`memory.rs`、memory migrations、`memory_store.rs`、`memory_cli.rs` 及测试。
- **预期失败测试：**未批准或跨项目记忆不可见；secret、PEM、token 拒绝且不回显；forget 清除内容与索引并保留 tombstone。
- **实现：**提供 propose、approve、forget、关键词检索、访问记录、secret scan、事务和审计事件。
- **验证：**运行 memory store、runtime 与 CLI tests。
- **提交：**存储、检索、审批、遗忘和命令接口分别提交。

### 任务 12：上下文组装与哈希绑定压缩

- **依赖：**任务 2、4、11。
- **目标：**按安全优先级和预算组装模型输入，不拆分 tool call/result，并使摘要绑定 transcript prefix。
- **文件：**`context.rs`、`transcript.rs`、`tests/context_assembler.rs`。
- **预期失败测试：**验证来源顺序、预算下限、去重、完整 turn/tool pair；未授权文件、配置和凭据不得进入模型；摘要 hash 不匹配时拒绝。
- **实现：**提供来源排序、按 relevance/age 淘汰、完整 pair 保留、summary binding 和 secret exclusion。
- **验证：**运行 context、transcript 和 continuation tests。
- **提交：**组装器、预算淘汰、摘要绑定和泄漏测试分别提交。

### 任务 13：自研主循环、停止条件与取消

- **依赖：**任务 2–12。
- **目标：**由 Orchester 自身拥有完整 agent loop，不把流程交给高层 agent SDK 或外部 agent。
- **文件：**`agent_loop.rs`、`coordinator.rs`、`service/`、`session.rs` 和 self-agent tests。
- **预期失败测试：**ScriptedModel 驱动 text、tool、approval、feedback、finish；副作用均需授权；取消传播；限制 step、时间、usage 和重复循环；终态持久化。
- **实现：**执行 request→model→decode→persist→policy/approval/tool→observation→feedback→next step，并原子转换终态。
- **验证：**运行 loop、service、run-store 与 runtime tests。
- **提交：**循环骨架、停止条件、取消传播和终态持久化分别提交。

### 任务 14：OpenAI Responses 单请求 Provider

- **依赖：**任务 2、3、13。
- **目标：**实现一次 Responses API 调用边界，只负责 provider transport，不接管 agent loop。
- **文件：**`provider/http/`、`provider/responses/`、`model_http_*`、`responses_*` tests。
- **预期失败测试：**覆盖端点来源、Bearer 目标限制、请求 schema、tool call/result 和有界错误；401 不重试，408/429/5xx 有限重试，取消立即生效。
- **实现：**校验端点地址，使用 rustls reqwest、严格 codec、状态与请求 ID 去敏、有限 backoff/cancel。
- **验证：**运行 provider tests；真实凭据仅用于隔离 smoke，结果必须去敏。
- **提交：**传输边界、请求 codec、重试策略和取消测试分别提交。

### 任务 15：Application Service 与自研 CLI

- **依赖：**任务 3–14。
- **目标：**提供运行、恢复、状态、权限、模型、插件、审批、auth、config、memory、audit 和 demo 命令。
- **文件：**`kisten/konsole/src/{args,main,self_agent}.rs` 及子模块和 CLI tests。
- **预期失败测试：**覆盖 Clap grammar、TTY/non-TTY、JSONL、stderr warning、approval ID、auth 禁止 secret 参数、memory 权限以及 `/status`、`/permissions`、`/resume`、`/model`、`/plugins`。
- **实现：**由 Application Service 组合运行时，CLI 提供稳定人类/JSON 输出、隐藏凭据输入和可恢复会话视图。
- **验证：**运行 CLI、service、status、permissions、model、resume、plugin 和 credential tests。
- **提交：**参数解析、服务组合、命令处理和输出格式分别提交。

### 任务 16：外部 Agent 委托兼容与生命周期

- **依赖：**既有 adapter/runtime 与任务 15。
- **目标：**保留 Claude、Codex、OpenCode 等委托能力，并与自研 Coding Agent 主通道分界。
- **文件：**`kisten/adapter/`、`conductor.rs`、`interactive.rs`、`process.rs`、CLI/session tests。
- **预期失败测试：**覆盖 legacy 路由、显式 delegate、stdout/stderr 排空、EOF 结果、cancel/drop 终止进程树和退出码保留。
- **实现：**复用 Registry、Conductor 与流驱动器，把外部 agent 作为可选工具；保留兼容别名 `/agent`。
- **验证：**运行 adapter、CLI、session 和 process lifecycle tests。
- **提交：**适配器、委托命令、流处理和生命周期测试分别提交。

### 任务 17：三项确定性机制演示

- **依赖：**任务 13、15、16。
- **目标：**离线演示治理拦截、校验失败后修正、审批持久化与恢复三个不同机制。
- **文件：**`demo.rs`、`kisten/xtask/`、`werkzeug/run_mechanism_demos.ps1` 和机制测试。
- **预期失败测试：**guardrail 场景执行器调用为零；feedback-loop 从 failed 变 passed 且 action hash 改变；approval-resume 只执行一次并拒绝 drift/replay。
- **实现：**使用固定 ScriptedModel/fixture、JSON 输出、`orchester demo` 和 xtask `test-all/demo/secret-scan`。
- **验证：**运行规范 `test-all` 和三项独立 demo。
- **提交：**每项演示与总入口分开提交。

### 任务 18：CLI 会话控制与恢复体验

- **依赖：**任务 13、15、17。
- **目标：**让长任务可暂停、查询、审批、恢复和安全终止，所有操作都基于持久化 run。
- **文件：**`kisten/konsole/src/session.rs`、`resume.rs`、`status.rs`、`permissions.rs`、对应 CLI tests。
- **预期失败测试：**未知 run、跨 owner run、过期审批、action drift、重复恢复和取消后继续执行均被拒绝；JSONL 输出稳定。
- **实现：**提供统一会话选择器、run 摘要、权限视图、恢复确认和终止原因记录。
- **验证：**运行交互、非交互、恢复、审批和取消测试。
- **提交：**会话查询、恢复流程、终止流程和输出契约分别提交。

### 任务 19：插件、模型与权限治理命令

- **依赖：**任务 3、6、15、16。
- **目标：**让插件发现、模型切换和权限查看都经过配置校验、registry generation 和审计。
- **文件：**`kisten/konsole/src/plugins.rs`、`model.rs`、`permissions.rs`、插件 registry 与测试。
- **预期失败测试：**未知插件、generation 漂移、未授权模型端点、项目级策略放宽和秘密参数均拒绝；列举结果不得泄密。
- **实现：**提供只读列表、显式选择、确认提示、稳定 JSON schema 和审计事件；插件不能绕过治理工具。
- **验证：**运行 plugin、model、permissions、config 和 audit tests。
- **提交：**发现、选择、权限校验和审计事件分别提交。

### 任务 20：本地验收脚本与证据归档

- **依赖：**任务 17–19。
- **目标：**提供无需远程服务的重复验收入口，统一保存测试摘要、机制输出和秘密扫描结果。
- **文件：**`werkzeug/run_local_acceptance.ps1`、`werkzeug/run_local_acceptance.sh`、`docs/evidence/`、xtask 验收模块。
- **预期失败测试：**缺少任一机制输出、测试失败、摘要字段缺失或发现秘密时返回非零；重复运行不污染工作区。
- **实现：**按固定顺序运行 crate tests、CLI smoke、三项 demo、安装 fixture 和 secret scan，输出去敏 JSON 证据。
- **验证：**在 Windows 与 POSIX shell 各运行一次并检查退出码、文件清单和去敏内容。
- **提交：**脚本、证据 schema、清理逻辑和验收测试分别提交。

### 任务 21：归档安装、发布验证与卸载

- **依赖：**任务 15–17、20。
- **目标：**让全新 Windows/Linux 主机无需 Git/Rust 即可安装固定版本，并按 receipt 安全卸载。
- **文件：**根 `install.*`/`uninstall.*`、`werkzeug/release-*`、安装 fixture 与隔离辅助程序。
- **预期失败测试：**缺入口、错误 checksum/结构不破坏旧 binary、PATH 不重复、receipt 只拥有自己创建项、默认保留 config、purge 必须确认。
- **实现：**OS/arch 选择、归档校验、stage/atomic replace、可选 attestation、精确 receipt 和隔离辅助程序。
- **验证：**运行 PowerShell/POSIX install-uninstall fixtures，并复核升级、回滚、保留配置和 purge。
- **提交：**安装器、卸载器、receipt、校验和发布脚本分别提交。

### 任务 22：GitHub Actions 与文档

- **依赖：**任务 17、19、20、21。
- **目标：**使用 GitHub Actions 运行提交门禁，完成文档并保持证据可复核。
- **文件：**`.github/workflows/ci.yml`、`README.md`、`SPEC.md`、`PLAN.md`、`SPEC_PROCESS.md`、`AGENT_LOG.md`、`REFLECTION.md`、`docs/RELEASE_CHECKLIST.md`。
- **预期失败测试：**必需文件缺失、文档为空白、测试任务失败、秘密扫描命中或证据 schema 不合法时工作流失败。
- **实现：**配置 push/PR 的 unit-test、CLI smoke、mechanism-demo、installer、secret-scan jobs
- **验证：**依次运行本地验收、GitHub Actions workflow、文档检查、安装 fixture 和 secret scan，并核对同一提交的工作流结果。
- **提交：**工作流、课程文档、发布清单和证据索引分别提交。

