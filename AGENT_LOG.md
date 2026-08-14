# 智能体协作日志

> 本文件记录课程项目中 agent 的技能、上下文、产出、人工校正、测试与评审证据。日志不保存 API key、Authorization header、原始思维链或其他秘密。凡原始记录缺失之处均明确写“未留存”，不从代码或提交标题反推过程完成。
>
> 根据提交者的文档约束，本日志使用可排序的“阶段”代替具体日期和时间戳；这是对课程时间戳要求的显式偏离，不表示原始时间证据完整留存。
>
> 除非标注“主分支”，下文的课程实现提交、WebUI、容器与 CI 证据均指 `task/course-closure` 分支或其未提交工作树；它们尚未合并到主分支。

## 一、规格形成阶段

### 任务与产品定位

主智能体把既有的外部 agent 统一启动器重新规划为 Project A Coding Agent Harness。用户选择 CLI 优先、治理与安全、供应商无关内核和 OpenAI 优先，并要求保留 Claude、Codex、OpenCode 等外部 agent。最终边界是：Orchester 自身是 Coding Agent，拥有模型、工具、审批、反馈、记忆、恢复和停止组成的主循环；调用其他 agent 只是其中一种可选能力。

### 使用的技能

- `superpowers:using-superpowers`：在行动前识别适用流程技能。
- `superpowers:brainstorming`：澄清目标、边界、方案取舍与验收标准。
- `superpowers:dispatching-parallel-agents`：并行核对课程要求、参考实现和仓库差距。
- `superpowers:writing-plans`：把确认后的规格拆成可测试任务。
- `openai-docs`：核对 Responses API 只作为单次 provider 请求边界，不采用高层 agent runner 代替自研循环。

### 主智能体上下文

- 用户要求 `~/.orchester/orchester.jsonc` 风格的完整配置，并希望用户配置与项目配置分离。
- 课程要求先完成 SPEC、PLAN 和 cold-start，再进入生产实现；要求 TDD、新鲜子智能体、规格评审、代码质量评审、WebUI、CI、三项 mock 演示和提交证据。
- 仓库已有 protocol、adapter、registry、conductor、session 与 CLI，这些可用于委托通道，但不能冒充自研 Coding Agent 内核。
- 公开 WebUI 与本地 Coding Agent 的威胁面不同，因此公开演示只开放三个固定 mock 场景。

### 并行子智能体

#### `/root/research_agent_loops`

提示范围是只读比较 Codex、OpenCode、Goose、Claude Code 的主循环、模型/工具边界、事件与取消。产出确认：harness 应拥有外层循环；transport retry 与新 step 必须区分；事件 projector 可以借鉴；外部工具调度框架不能作为自研内核。人工只采纳能由源码或正式文档支持的结论。

#### `/root/research_context_feedback`

提示范围是只读比较 Aider、Cline、Continue 等项目的上下文、压缩、反馈、循环检测与凭据范围。产出形成 canonical transcript 与 hash-bound compaction sidecar 分离、tool call/result 不拆分、validator failure 作为结构化 observation、run state 与长期 memory 分离等决定。人工否决了首期向量检索和明文全局 provider 配置。

#### `/root/course_compliance`

提示范围是完整阅读通用要求与 Project A 要求，列出硬要求、交付落点、证据和冲突。产出确认了至少五个 INVEST user stories、威胁模型、三项不同 mock 演示、公网 WebUI、过程日志和分发证据。课程原文要求 GitHub Actions 之外还提供 `.gitlab-ci.yml`，而用户后续明确选择 GitHub-only；这属于需披露且等待课程方接受的偏离。

#### `/root/repo_gap_map`

提示范围是只读分析 Cargo 工作区、CLI、协议和测试。产出确认既有 delegate 基础设施可保留，应抽取通用 stream driver，并指出 stderr 未排空、取消传播、EOF 成功和 session JSONL 并发等风险。人工没有把全部新能力塞进单一 crate，而是维持 `modell + laufzeit::harness` 边界。

#### `/root/governance_spec`

提示范围是研究策略、沙箱、审批与恢复。该子任务没有在约定范围内收束，主智能体中止任务，没有把未返回内容当作证据。教训是研究任务必须限制文件范围、证据数量与停止条件。

### 规格评审与人工决定

独立规格评审首轮指出策略优先级、崩溃重试、间接变更、数据库 schema、发布签名和 SmartScreen 决策等阻断问题。主智能体全部修订，并补充 session ownership、CSRF 和配额。第二轮只剩 Web session ownership 未进入数据库 schema；补充 actor/session/owner/capability 字段和跨 session 测试后，第三轮评审返回批准。

人工决策包括：采用自研主通道并保留外部 agent 可选委派能力；治理与安全优先；公开 WebUI 只跑固定场景；项目记忆必须审批；provider 只负责单次请求。早期凭据方案只允许 `${secret:OpenAI}` 与系统凭据存储，后续配置实现发生口径调整，详见“凭据口径校正阶段”。

## 二、任务 1 与 cold-start 补偿检查

### 任务 1：版本化 Harness 协议

- **阶段：**协议基线与后续加固。
- **技能/上下文：**历史实现的原始技能调用和子智能体 session 未完整留存；可核对上下文为早期 SPEC/PLAN、协议测试和后续补偿检查报告。
- **提交：**`b85ad9b`；后续 `7f6308d`、`db1973c`、`fec1532`、`fc4bbd1` 修订 wire contract、摘要边界、action origin 和 durable payload 去敏。
- **Red/Green/评审：**历史任务的原始 Red/Green 与代码质量评审未留存。隔离补偿试作得到 unresolved import 红灯，随后任务 1 新测试、旧协议测试和工作区测试在该一次性 worktree 通过；这些数字仅属于被丢弃试作，不是当前工作树绿色证据。
- **人工校正/教训：**增加 `ApprovalRequest.run_id`、统一 event envelope、限制摘要并去敏。类型能 round-trip 不代表持久化契约、安全边界和兼容面已经正确。

### 补偿 cold-start 事实

- 使用全新 Claude Opus session `729ca35c-7dc1-492c-8d1d-2a0e61387467`，无 plugin、无 MCP，只提供生产实现前版本的 `SPEC.md` 与 `PLAN.md`。
- 子智能体在一次性 worktree 尝试任务 1，没有提交、没有合并，完成后丢弃全部试产出；仓库只保留去敏结果摘要。
- 检查暴露了 event envelope 冲突、任务 1 测试不足、`ApprovalRequest.run_id` 缺失、Windows 规范命令缺失和任务状态记录失真。
- 该检查已执行并推动真实修订，但生产实现先于检查，因此操作纪律部分不合规。它不能被称为事前 cold-start 门禁完成、通过或合规。

## 三、任务 2–16 逐项过程追溯

以下记录基于提交历史、当前源代码和能够定位的测试。没有找到原始提示词、session、Red/Green 输出、PR 或双阶段评审时，明确记录缺失，而不是补写一段想象中的流程。

### 任务 2：单次模型边界与严格解码

- **阶段：**模型边界建立与诊断加固。
- **技能/上下文：**原始技能调用、子智能体和提示词未留存；当前可确认上下文是“单次请求必须与 agent loop 分离”和离线 ScriptedModel 要求。
- **提交：**`d1ab8dc`、`9ebc0ec`、`5292021`。
- **Red/Green/评审：**仓库存在模型边界、严格解码和有界 call ID 测试；原始失败输出、绿色命令、PR、规格评审与代码评审未留存，状态为待复核。
- **人工校正/教训：**后续限制非法 call ID 诊断长度。模型层只能返回结构化结果，不能偷偷执行工具或拥有循环。

### 任务 3：JSONC 配置与凭据

- **阶段：**安全配置、凭据生命周期和平台权限加固。
- **技能/上下文：**原始任务技能/session 未留存；可确认上下文包括用户/项目配置分层、严格者优先、profile/provider 分离和配置可修复性。
- **提交：**基础配置 `d0b1640`，凭据生命周期 CLI `f9d7136`；主分支另有 `/config`、`/login`、`/logout`、`ORCHESTER_HOME` 和 Windows ACL 加固提交。
- **Red/Green/评审：**存在配置合并、凭据状态、隐藏输入、权限和泄漏测试；历史逐步 Red/Green 与双评审未留存。当前另有配置模板的未提交工作，不能计为已交付。
- **人工校正/教训：**早期“仅秘密引用”结论后来被修订：受保护的用户级配置可以写 literal API key，但项目配置不可以；ACL/权限不安全时必须拒绝读取，所有显示必须去敏。安全性由存储位置、权限和传播边界共同决定，不能只看字段名。

### 任务 4：事务化 Run Store

- **阶段：**可恢复状态、transcript 和迁移连续加固。
- **技能/上下文：**原始技能/session 未留存；上下文来自 SPEC 的 owner/project scope、append-only transcript 和 durable resume 约束。
- **提交：**`caffe3f` 起始，后续 migration、transcript、sanitization、resume binding 与 audit coherence 修订持续至 `e4d83fb`。
- **Red/Green/评审：**大量 run-store 与 migration 测试可见，但原始 Red/Green 顺序、PR 和双评审记录未留存；不能由提交数量反推流程合规。
- **人工校正/教训：**后续补上 lifecycle event、transcript range、project identity 和 audit checkpoint 绑定。恢复必须从耐久证据推导，不能只读取最后一个状态字符串。

### 任务 5：路径护栏与工作区锁

- **阶段：**跨平台路径安全加固。
- **技能/上下文：**原始技能/session 未留存；可确认上下文是 traversal、link/reparse、ADS、对象替换和 rename 竞态威胁。
- **提交：**首个能力 `4e1e985`，后续有 workspace capability、object identity、ADS、Win32 歧义路径和锁加固提交。
- **Red/Green/评审：**存在路径回归与平台条件测试；历史红灯、绿色输出和独立代码质量评审未留存。
- **人工校正/教训：**仅用字符串 canonicalize 不足以防止检查后替换；写入前还需绑定已打开对象和工作区能力。

### 任务 6：策略引擎与命令分类

- **阶段：**策略矩阵与 durable binding。
- **技能/上下文：**原始技能/session 未留存；上下文来自 ALLOW/ASK/DENY、硬不变量和 restriction lattice 设计。
- **提交：**`c4aec5e`、`6e9e38e` 及后续 network、sleep 和 durable policy binding 提交。
- **Red/Green/评审：**policy matrix 和治理执行测试存在；当时的预期失败、完整绿色命令和双评审未留存。
- **人工校正/教训：**网络探测与 bounded sleep 后来被纳入结构化分类。项目或会话层只能收紧，不能覆盖核心安全规则。

### 任务 7：审计链与执行前 Barrier

- **阶段：**去敏审计与副作用授权绑定。
- **技能/上下文：**原始技能/session 未留存；上下文为“无 durable action、policy、permit 和 audit checkpoint 就不得执行”。
- **提交：**`06dfc8a`、`ada2de9`。
- **Red/Green/评审：**审计篡改、审批与 secret scan 测试存在；原始 Red/Green、rotation 恢复评审和代码质量评审未留存。
- **人工校正/教训：**审计 JSONL 后续与 durable action 绑定。日志落盘本身不是 barrier，必须先验证绑定再调用 executor。

### 任务 8：HITL 审批与恢复

- **阶段：**审批状态机、一次性 capability 与恢复加固。
- **技能/上下文：**原始技能/session 未留存；上下文来自 action hash、owner、policy、workspace、generation 全绑定要求。
- **提交：**`06dfc8a`、`efc505f`、`399b9e7`。
- **Red/Green/评审：**仓库可见 drift、replay、owner 和恢复测试；原始 TDD transcript、PR 和双评审未完整留存。
- **人工校正/教训：**后续把 resume 与 action 绑定并限制一次消费。用户批准的是一个不可变动作，不是对后续相似命令的通用许可。

### 任务 9：治理工具与进程沙箱

- **阶段：**工具 registry、文件工具和进程生命周期实现。
- **技能/上下文：**原始技能/session 未留存；上下文为统一治理管线、有界 observation、环境去密和跨平台进程树终止。
- **提交：**`ca454e9`、`4bf01e8`、`b90a130`、`8b65fe3`、`2a3d2e5`、`65a091e`、`3148e39`。
- **Red/Green/评审：**read/write/patch/process/cancel 测试存在，机制演示也覆盖部分链路；逐提交红灯和平台代码评审未留存。
- **人工校正/教训：**后续加入 registry generation、permit-bound execution、CAS patch 和进程树终止。工具名称白名单不够，执行时必须复核本次 action 的全部能力绑定。

### 任务 10：校验器、变更代次与反馈

- **阶段：**结构化失败反馈与 validator-gated completion。
- **技能/上下文：**原始技能/session 未留存；上下文是测试失败必须进入下一模型步，且陈旧绿色不能授权完成。
- **提交：**`9b0e808`、`868e336`、`3b4682d`。
- **Red/Green/评审：**反馈、mutation generation 和 completion gate 测试存在；原始 Red/Green、PR 和双评审未留存。当前总门禁仍被 server Clippy 失败阻断。
- **人工校正/教训：**后续限制 source snapshot traversal，并将校验结果绑定 generation。一个测试曾经通过不等于当前工作树可以 `finish`。

### 任务 11：项目记忆

- **阶段：**记忆存储、审批、遗忘和 CLI 补全。
- **技能/上下文：**原始技能/session 未留存；上下文为 owner/project scope、secret admission、批准后召回和可审计 forget。
- **提交：**`b82a681`、`e6b5cb3`、`4af75ab`、`5486dfe`。
- **Red/Green/评审：**memory store、迁移、并发和 CLI 测试可见；原始失败输出、绿色记录和 CLI 质量评审未留存。
- **人工校正/教训：**后续修正 ownership 与首次迁移并发。长期记忆不是 transcript 的别名，必须经过单独批准与秘密扫描。

### 任务 12：上下文组装与压缩

- **阶段：**安全预算、完整工具对与 transcript continuation。
- **技能/上下文：**原始技能/session 未留存；上下文来自研究阶段的 canonical transcript、完整 call/result pair 和 hash-bound summary 决定。
- **提交：**`08a2dea`、`5ad55e3`。
- **Red/Green/评审：**context assembler 与 paired continuation 测试存在；原始 Red/Green、预算评审和压缩质量评审未留存。
- **人工校正/教训：**后续明确工具调用与结果必须一起保留。压缩是可验证派生物，不能替换事实 transcript。

### 任务 13：自研主循环

- **阶段：**可恢复模型步、工具 continuation 与成功终态修复。
- **技能/上下文：**原始技能/session 未完整留存；可确认上下文是 Orchester 必须拥有 request→model→action→tool→feedback→stop 全循环。
- **提交：**`9da4355`、`7cfad8b`、`33bf066`，后续 `33aae0b`、`d4d7cb0` 修复成功终态并补 text completion 测试。
- **Red/Green/评审：**runtime、run-store 和机制演示测试覆盖主要路径；原始逐步 TDD 与双评审记录不完整。
- **人工校正/教训：**后续发现“模型返回文本”也必须耐久地结束 run，不能只处理工具动作。主循环所有边界都要能从持久化证据恢复。

### 任务 14：OpenAI Responses Provider

- **阶段：**单请求 HTTP transport 与错误边界实现。
- **技能/上下文：**规格阶段使用 `openai-docs` 核对正式 API 边界；具体实现子智能体/session 和提示词未留存。
- **提交：**provider 提交链自 `3927ac4` 延续至 `fccc8f9`。
- **Red/Green/评审：**mock HTTP、schema、重试和取消测试可见；真实 disposable credential smoke、原始 Red/Green 和独立评审证据缺失。
- **人工校正/教训：**provider 只完成一次网络请求，外层重试、循环和工具授权继续由 harness 控制；401 不应盲目重试。

### 任务 15：Application Service 与自研 CLI

- **阶段：**自研入口、slash command 和凭据/配置交互补全。
- **技能/上下文：**历史多个实现回合的完整技能/session 未留存；上下文可由 `/status`、`/permissions`、`/resume`、`/model`、`/plugins`、auth、config、memory 需求和提交链确认。
- **提交：**核心 service/CLI 提交链，凭据命令 `f9d7136`；主分支另有 status、permissions、model、resume、plugins、config、login/logout 的独立提交。
- **Red/Green/评审：**相关命令各有解析或运行时测试，但不能据此声称所有回合都严格 TDD；综合 CLI 评审未留存。`/resume` 目前主要列出安全可恢复 run，完整续跑交互仍待验收。
- **人工校正/教训：**旧日志曾写 `auth set/status/update/clear` 不存在，已由 `f9d7136` 证明过时。CLI 是自研 Coding Agent 的主入口，外部 agent picker 只是能力之一。

### 任务 16：委托兼容与生命周期

- **阶段：**既有 adapter/runtime 兼容和插件会话修复。
- **技能/上下文：**最早实现技能/session 未留存；上下文是保留 Registry/Conductor，同时让 self-agent 与 delegate 的命令和生命周期清晰分离。
- **提交：**基础 `ffb6f68`，后续有 plugin 状态、registry refresh、EOF、session 与进程生命周期修复提交。
- **Red/Green/评审：**adapter/CLI/session 测试存在；任务专属 Red/Green、PR 和双评审未找到，当前 namespace 与规格是否完全一致待复核。
- **人工校正/教训：**外部 agent 的 stdout、stderr、退出码和取消都必须被完整处理。委托失败不能被交互 shell 继续运行掩盖为成功。

## 四、任务 17：三项机制演示

- **阶段：**离线可重复验收。
- **技能/上下文：**子智能体 `/root/mechanism_demos_retry` 使用 TDD、系统化调试和完成前验证；范围限定为固定 ScriptedModel 场景，不接 provider 网络。
- **提交：**课程分支上的 `c8da178`、`4c9a877`，主分支尚未合并。
- **Red：**默认 MSVC 工具链先因主机 linker 不可用失败，该环境错误不是有效功能红灯；切换 Windows GNU 工具链后，缺失机制入口构成可行动失败。
- **Green：**GNU 离线任务级测试曾记录通过，`werkzeug/run_mechanism_demos.ps1` 调用同一命令；后续端到端提交加强 guardrail、feedback-loop 和 approval-resume 断言。
- **评审/人工校正：**最终独立双评审未留存。教训是先区分环境失败和需求失败；局部机制绿色不能替代当前 `test-all` 总门禁。

## 五、任务 18–19：固定场景 Server 与 WebUI

### 任务 18：Axum Server

- **阶段：**固定场景 HTTP contract、session/SSE 与 durable runtime 对接。
- **技能/上下文：**并行子智能体 `/root/web_server` 负责 server/Web 范围；精确完整提示词未全部留存，已知边界是不得开放任意 prompt 或真实 provider 配置。
- **提交：**课程分支初版 `d4c95c5`；`task/course-closure` 工作树仍有大幅未提交修订，主分支未合并。
- **Red/Green/评审：**本地 server smoke 曾通过固定场景、SSE、审批恢复与 replay 拒绝；最近规范 Clippy 在 `kisten/server/src/harness.rs` 的真实 `HarnessApplication::decide` 处因参数过多失败。未提交修订尚未完成质量评审。
- **人工校正/教训：**HTTP 边界需固定 schema、owner join、CSRF 和一次性 capability。一个能启动的 server 不代表 lint、权限隔离和恢复契约都完成。

### 任务 19：React WebUI

- **阶段：**治理工作台与可访问性测试。
- **技能/上下文：**与 server 共用固定场景 contract；UI 只展示场景选择、事件、审批和反馈，不提供任意 prompt box。
- **提交：**课程分支初版与 `d4c95c5` 同批；后续修订仍未提交，主分支未合并。
- **Red/Green/评审：**Vitest、build、Playwright 测试存在；当前修订需在 server 修复后完整复跑。最终视觉、响应式与可访问性评审未留存。
- **人工校正/教训：**公开演示的价值是让治理机制可观察，而不是复刻本地全权限 Coding Agent。

## 六、任务 20–21：部署与分发

### 任务 20：OCI 演示与烟测

- **阶段：**容器、部署清单和双客户端 smoke。
- **技能/上下文：**子智能体 `/root/delivery_docs` 只处理分发与课程收尾，并从 server 子智能体获取固定 HTTP contract，不修改并行 server/Web 文件。
- **提交：**容器与部署文件主要仍在 course-closure 未提交工作树。
- **Red/Green/评审：**PowerShell AST 与 `sh -n` 通过；真实本地 server 上两套 smoke 曾通过健康检查、三场景、SSE、审批恢复、replay、严格输入和配额。Docker daemon 无法连接，因此没有镜像 build/run、digest、容器 smoke 或公网证据。
- **人工校正/教训：**修复了 strict JSON 状态码、PowerShell cookie/error response、SSE 等待 EOF、debug 时限和 `python3` app alias 等实际问题。本地进程 smoke 不能写成容器或公网 smoke。

### 任务 21：归档安装与卸载

- **阶段：**双平台 release archive installer。
- **技能/上下文：**`/root/delivery_docs` 使用执行计划、TDD 和完成前验证；remote helper 只允许显式 host/version/HTTPS base URL。
- **提交：**`f7fcec4`。
- **Red：**PowerShell fixture 在缺少 release installer 入口时失败；先修复测试自身的参数默认值和变量名冲突，才接受有效红灯。
- **Green：**PowerShell 与 POSIX fixture 覆盖 ZIP/archive + SHA-256、原子更新、错误摘要/结构不破坏旧 binary、receipt ownership、PATH 去重、默认保留配置和 purge 确认。
- **评审/人工校正：**外部 release、attestation 和授权远程主机验收未执行。教训是 receipt 只能删除安装器确实创建的对象，脚本存在不等于全新机器验收完成。

## 七、任务 22：GitHub CI 与课程收尾

### CI 与文档状态

- **阶段：**中文文档统一、GitHub Actions 门禁和证据缺口核对。
- **技能/上下文：**使用需求矩阵、实现审计和参考对话审计三个只读子任务并行核对；没有把任何外部状态推测为已通过。
- **提交：**课程分支的 CI 基础为 `4706576`；主分支目前只有手动发布工作流，push/PR 课程测试工作流尚待合并。
- **Red/Green/评审：**最近规范门禁失败于 `HarnessApplication::decide` 的 `clippy::too_many_arguments`，因此不能记录 `test-all` 绿色。托管 GitHub run、公网 URL、release/attestation 和远程验收证据均缺失。
- **人工校正/教训：**`verify-course-files` 的实际职责仅是检查指定文件存在、反思字符数、公网 WebUI URL 和 hosted CI 的 PASS+URL；它不检查 README 章节、GitHub job 名称或 AI 披露。文档不得扩大脚本真实检查范围。

### GitHub-only 偏离

用户明确要求删除 `.gitlab-ci.yml`，只保留 GitHub Actions。课程原文仍要求 GitLab 配置与精确 `unit-test` job，因此 GitHub 绿色不能替代被省略项。本项目会如实标注这项偏离，是否接受由课程方决定。

### AI 辅助反思

`REFLECTION.md` 已由 AI 辅助生成，并在文件中披露所用工具/模型及提纲、起草或润色范围；它不能被描述为仅由学生撰写。该文件仍待提交者逐句复核技术事实、个人经历与最终判断后再提交。课程原文禁止 AI 代写反思，因此这同样是已披露、等待课程方判断的偏离。

### 凭据口径校正阶段

早期设计因课程安全要求采用“仅秘密引用”，即 `${secret:OpenAI}` + keyring/platform secret store。后续用户要求并实际实现了一个受约束例外：literal API key 可以写入用户级 `~/.orchester/orchester.jsonc`，但仅在文件通过 Windows ACL 或 Unix 私有权限检查时读取；项目级配置不能写 literal key，CLI、日志、诊断和 effective config 必须去敏。旧的“绝不允许配置文件出现 literal key”不再是当前规格，日志保留这次决定变化以避免前后矛盾。

## 八、当前验证与证据缺口

- `task/course-closure` 的规范 `orchester-xtask test-all` 不能记为通过，已知阻断是 server Clippy 参数数量问题；主分支尚未合并该门禁实现。
- cold-start 补偿检查已执行，但执行顺序不合规，不能补写为事前门禁通过。
- 任务 2–16 的能力和提交多数可定位，但很多原始 Red/Green、子智能体提示词、PR、规格评审和代码质量评审没有留存。
- 任务 20 缺 Docker build/run、镜像摘要、公开 HTTPS URL 和公网 smoke。
- 任务 21 缺正式 release、checksum/attestation 与全新授权远程主机证据。
- 任务 22 缺 exact commit 的 hosted GitHub PASS 记录；课程分支有 push/PR 工作流，主分支尚未合并；GitHub-only 和 AI 辅助反思均是显式课程偏离。
- 所有缺口必须以新运行的去敏命令输出或外部链接补齐，不能以计划、脚本存在、提交标题或 agent 自述替代。

## 九、过程教训

1. 课程硬要求应在产品偏好和实现前建立矩阵，否则会产生冷启动顺序、CI 平台和反思作者规则等难以事后修复的偏离。
2. 提交证明代码变化，不证明当时严格执行 TDD、独立评审或外部验收；过程证据必须随任务保存。
3. 自研 Coding Agent 的边界必须清楚：外部 agent 可以是委托工具，但策略、审批、执行、反馈、恢复与停止必须由 Orchester 掌控。
4. 凭据安全不能简化成“配置文件里有没有 key”；要同时验证配置层级、文件权限、传播路径、展示去敏和项目层限制。
5. 本地 smoke、容器 smoke、公网 smoke、托管 CI 和远程安装是不同证据，任何一个都不能替代其他项。
6. 补偿检查可以发现并修复技术问题，但不能抹除原本未遵守的操作顺序；最可靠的日志是同时记录能力进展和流程偏差。
