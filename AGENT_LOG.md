# 智能体协作日志

## 一、初步阶段

### 任务与产品定位

主智能体把既有的外部 agent 统一启动器重新规划为 Project A Coding Agent Harness。产品以 CLI、自研主循环、治理与安全、供应商无关内核为主体，同时保留 Claude、Codex、OpenCode 等外部 agent 的可选委派能力。Orchester 自己拥有模型、工具、审批、反馈、记忆、恢复和停止组成的主循环。

### 使用的技能

- `superpowers:using-superpowers`：在行动前识别适用流程技能。
- `superpowers:brainstorming`：澄清目标、边界、方案取舍与验收标准。
- `superpowers:dispatching-parallel-agents`：并行核对课程要求、参考实现和仓库差距。
- `superpowers:writing-plans`：把确认后的规格拆成可测试任务。
- `openai-docs`：核对 Responses API 的单次 provider 请求边界，不采用高层 agent runner 代替自研循环。

### 上下文

- `~/.orchester/orchester.jsonc` 需要覆盖完整个人配置，并与项目配置保持明确权限边界。
- 课程要求先完成 SPEC、PLAN 和 cold-start，再进入生产实现，同时要求 TDD、新鲜子智能体、规格评审、代码质量评审、固定机制演示、CI 和提交证据。
- 仓库已有 protocol、adapter、registry、conductor、session 与 CLI，可用于委派通道，但不能代替自研 Coding Agent 内核。
- `/status`、`/permissions`、`/resume`、`/model`、`/plugins` 等命令需要共享应用服务与持久状态。

### 并行子智能体

#### `/root/research_agent_loops`

只读比较 Codex、OpenCode、Goose、Claude Code 的主循环、模型与工具边界、事件和取消。产出确认 harness 应拥有外层循环，transport retry 与新 step 必须区分，外部工具调度框架不能作为自研内核。只采纳可由源码或正式文档支持的结论。

#### `/root/research_context_feedback`

只读比较 Aider、Cline、Continue 等项目的上下文、压缩、反馈、循环检测与凭据范围。产出形成 canonical transcript 与 hash-bound compaction sidecar 分离、tool call/result 不拆分、validator failure 作为结构化 observation、run state 与长期 memory 分离等决定。首期向量检索与无权限保护的全局 provider 配置没有进入规格。

#### `/root/course_compliance`

完整阅读通用要求与 Project A 要求，整理硬要求、交付落点、证据和冲突。产出确认至少五个 INVEST 需求故事、威胁模型、三项不同固定机制演示、过程日志和分发证据。课程原文同时出现 GitHub Actions 与 GitLab 配置文字，最终交付采用 GitHub Actions 并单独记录该差异。

#### `/root/repo_gap_map`

只读分析 Cargo 工作区、CLI、协议和测试。产出确认既有 delegate 基础设施可保留，应抽取通用 stream driver，并指出 stderr 未排空、取消传播、EOF 成功和 session JSONL 并发等风险。模块边界维持 `modell + laufzeit::harness`，没有把全部能力集中到单一 crate。

#### `/root/governance_spec`

研究策略、沙箱、审批与恢复。该子任务没有在约定范围内收束，主智能体中止任务，未返回内容没有作为证据。教训是研究任务必须限制文件范围、证据数量和停止条件。

### 规格评审与人工决定

独立规格评审首轮指出策略优先级、崩溃重试、间接变更、数据库 schema、发布校验和 Windows 首次运行处理等阻断问题。修订后补全安全限制优先级、unknown outcome、源码观察集合、数据库关键关系和发布校验策略。复审确认规格可以进入实现。

关键决定包括：采用自研主通道并保留外部 agent 可选委派；治理与安全优先；项目记忆必须审批；provider 只负责单次请求；CLI 命令通过统一应用服务访问状态。凭据方案从“只允许秘密引用”调整为“秘密引用优先，受保护个人配置可含字面 Key”。

## 二、任务 1 与 cold-start 补偿检查

### 任务 1：版本化 Harness 协议

- **阶段：**协议基线与后续加固。
- **技能/上下文：**历史实现的原始技能调用和子智能体 session 未完整留存；可核对上下文为早期 SPEC、PLAN、协议测试和补偿检查报告。
- **提交：**`b85ad9b`；后续 `7f6308d`、`db1973c`、`fec1532`、`fc4bbd1` 修订 wire contract、摘要边界、action origin 和 durable payload 去敏。
- **Red/Green/评审：**历史原始 Red/Green 与代码质量评审未留存。隔离试作先得到 unresolved import 红灯，随后任务测试在一次性工作树通过；该结果不代表当前工作树总门禁。
- **人工校正/教训：**增加 `ApprovalRequest.run_id`、统一 event envelope、限制摘要并去敏。类型能 round-trip 不代表持久化契约、安全边界和兼容面已经正确。

### cold-start 补偿事实

- 使用全新 Claude Opus session `729ca35c-7dc1-492c-8d1d-2a0e61387467`，无 plugin、无 MCP，只提供生产实现前版本的 `SPEC.md` 与 `PLAN.md`。
- 子智能体在一次性工作树尝试任务 1，没有提交、没有合并，结束后丢弃试产出，只保留去敏结果摘要。
- 检查暴露 event envelope 冲突、任务 1 测试不足、`ApprovalRequest.run_id` 缺失、Windows 规范命令缺失和任务状态记录失真。
- 检查推动了真实修订，但发生在生产实现开始之后，不能记为事前门禁通过。

## 三、任务 2–16 逐项过程追溯

以下记录基于提交历史、当前源代码和能够定位的测试。没有找到原始提示词、session、Red/Green 输出、PR 或双阶段评审时，明确记录缺失。

### 任务 2：单次模型边界与严格解码

- **阶段：**模型边界建立与诊断加固。
- **技能/上下文：**原始技能调用、子智能体和提示词未留存；可确认上下文是单次请求必须与 agent loop 分离，以及离线 `ScriptedModel` 要求。
- **提交：**`d1ab8dc`、`9ebc0ec`、`5292021`。
- **Red/Green/评审：**模型边界、严格解码和有界 call ID 测试存在；原始失败输出、绿色命令、PR 与双评审未留存。
- **人工校正/教训：**后续限制非法 call ID 诊断长度。模型层只返回结构化结果，不能执行工具或拥有循环。

### 任务 3：JSONC 配置与凭据

- **阶段：**安全配置、凭据生命周期和平台权限加固。
- **技能/上下文：**原始任务技能和 session 未留存；上下文包括个人与项目配置分层、严格者优先、profile/provider 分离和配置可修复性。
- **提交：**基础配置 `d0b1640`，凭据生命周期 CLI `f9d7136`；另有 `/config`、`/login`、`/logout`、`ORCHESTER_HOME` 和 Windows ACL 加固提交。
- **Red/Green/评审：**配置合并、凭据状态、隐藏输入、权限和泄漏测试存在；历史逐步 Red/Green 与双评审未留存。配置模板另有未提交草稿，不能计为已交付。
- **人工校正/教训：**受保护的个人配置可以写字面 API Key，项目配置不可以；权限不安全时拒绝读取，所有显示必须去敏。

### 任务 4：事务化 Run Store

- **阶段：**可恢复状态、transcript 和迁移连续加固。
- **技能/上下文：**原始技能和 session 未留存；上下文来自 SPEC 的 owner/project scope、append-only transcript 和 durable resume 约束。
- **提交：**`caffe3f` 起始，后续 migration、transcript、sanitization、resume binding 与 audit coherence 修订持续至 `e4d83fb`。
- **Red/Green/评审：**大量 run-store 与 migration 测试可见，但原始 Red/Green 顺序、PR 和双评审记录未留存。
- **人工校正/教训：**后续补上 lifecycle event、transcript range、project identity 和 audit checkpoint 绑定。恢复必须从耐久证据推导，不能只读取最后一个状态字符串。

### 任务 5：路径护栏与工作区锁

- **阶段：**跨平台路径安全加固。
- **技能/上下文：**原始技能和 session 未留存；上下文是 traversal、link/reparse、ADS、对象替换和 rename 竞态威胁。
- **提交：**首个能力 `4e1e985`，后续有 workspace capability、object identity、ADS、Win32 歧义路径和锁加固提交。
- **Red/Green/评审：**路径回归与平台条件测试存在；历史红灯、绿色输出和独立代码质量评审未留存。
- **人工校正/教训：**字符串 canonicalize 不足以防止检查后替换，写入前还需绑定已打开对象和工作区能力。

### 任务 6：策略引擎与命令分类

- **阶段：**策略矩阵与 durable binding。
- **技能/上下文：**原始技能和 session 未留存；上下文来自 `ALLOW/ASK/DENY`、硬不变量和 restriction lattice。
- **提交：**`c4aec5e`、`6e9e38e` 及后续 network、sleep 和 durable policy binding 提交。
- **Red/Green/评审：**policy matrix 和治理执行测试存在；当时的预期失败、完整绿色命令和双评审未留存。
- **人工校正/教训：**网络探测与 bounded sleep 被纳入结构化分类。项目或会话层只能收紧，不能覆盖核心安全规则。

### 任务 7：审计链与执行前 Barrier

- **阶段：**去敏审计与副作用授权绑定。
- **技能/上下文：**原始技能和 session 未留存；上下文为缺少 durable action、policy、permit 或 audit checkpoint 时不得执行。
- **提交：**`06dfc8a`、`ada2de9`。
- **Red/Green/评审：**审计篡改、审批与 secret scan 测试存在；原始 Red/Green、rotation 恢复评审和代码质量评审未留存。
- **人工校正/教训：**审计 JSONL 后续与 durable action 绑定。日志落盘本身不是 barrier，必须先验证绑定再调用 executor。

### 任务 8：HITL 审批与恢复

- **阶段：**审批状态机、一次性 capability 与恢复加固。
- **技能/上下文：**原始技能和 session 未留存；上下文来自 action hash、owner、policy、workspace、generation 全绑定要求。
- **提交：**`06dfc8a`、`efc505f`、`399b9e7`。
- **Red/Green/评审：**drift、replay、owner 和恢复测试可见；原始 TDD transcript、PR 和双评审未完整留存。
- **人工校正/教训：**后续把 resume 与 action 绑定并限制一次消费。一次批准只对应一个不可变动作，不是对相似命令的通用许可。

### 任务 9：治理工具与进程沙箱

- **阶段：**工具 registry、文件工具和进程生命周期实现。
- **技能/上下文：**原始技能和 session 未留存；上下文为统一治理管线、有界 observation、环境去密和跨平台进程树终止。
- **提交：**`ca454e9`、`4bf01e8`、`b90a130`、`8b65fe3`、`2a3d2e5`、`65a091e`、`3148e39`。
- **Red/Green/评审：**read/write/patch/process/cancel 测试存在，固定机制演示覆盖部分链路；逐提交红灯和平台代码评审未留存。
- **人工校正/教训：**后续加入 registry generation、permit-bound execution、CAS patch 和进程树终止。工具名称白名单不够，执行时必须复核本次 action 的全部能力绑定。

### 任务 10：校验器、变更代次与反馈

- **阶段：**结构化失败反馈与 validator-gated completion。
- **技能/上下文：**原始技能和 session 未留存；上下文是测试失败必须进入下一模型步骤，陈旧绿色不能授权完成。
- **提交：**`9b0e808`、`868e336`、`3b4682d`。
- **Red/Green/评审：**反馈、mutation generation 和 completion gate 测试存在；原始 Red/Green、PR 和双评审未留存。整树门禁需以重新执行结果为准。
- **人工校正/教训：**后续限制 source snapshot traversal，并将校验结果绑定 generation。历史测试通过不等于当前工作树可以 `finish`。

### 任务 11：项目记忆

- **阶段：**记忆存储、审批、遗忘和 CLI 补全。
- **技能/上下文：**原始技能和 session 未留存；上下文为 owner/project scope、secret admission、批准后召回和可审计 forget。
- **提交：**`b82a681`、`e6b5cb3`、`4af75ab`、`5486dfe`。
- **Red/Green/评审：**memory store、迁移、并发和 CLI 测试可见；原始失败输出、绿色记录和 CLI 质量评审未留存。
- **人工校正/教训：**后续修正 ownership 与首次迁移并发。长期记忆不是 transcript 的别名，必须经过单独批准与秘密扫描。

### 任务 12：上下文组装与压缩

- **阶段：**安全预算、完整工具对与 transcript continuation。
- **技能/上下文：**原始技能和 session 未留存；上下文来自 canonical transcript、完整 call/result pair 和 hash-bound summary。
- **提交：**`08a2dea`、`5ad55e3`。
- **Red/Green/评审：**上下文组装与 paired continuation 测试存在；原始 Red/Green、预算评审和压缩质量评审未留存。
- **人工校正/教训：**工具调用与结果必须一起保留。压缩是可验证派生物，不能替换事实 transcript。

### 任务 13：自研主循环

- **阶段：**可恢复模型步骤、工具 continuation 与成功终态修复。
- **技能/上下文：**原始技能和 session 未完整留存；可确认上下文是 Orchester 必须拥有 request → model → action → tool → feedback → stop 全循环。
- **提交：**`9da4355`、`7cfad8b`、`33bf066`，后续 `33aae0b`、`d4d7cb0` 修复成功终态并补 text completion 测试。
- **Red/Green/评审：**runtime、run-store 和固定机制演示测试覆盖主要路径；原始逐步 TDD 与双评审记录不完整。
- **人工校正/教训：**模型返回文本也必须耐久结束 run，不能只处理工具动作。主循环所有边界都要能从持久证据恢复。

### 任务 14：OpenAI Responses Provider

- **阶段：**单请求传输与错误边界实现。
- **技能/上下文：**规格阶段使用 `openai-docs` 核对正式 API 边界；具体实现子智能体、session 和提示词未留存。
- **提交：**provider 提交链自 `3927ac4` 延续至 `fccc8f9`。
- **Red/Green/评审：**mock transport、schema、重试和取消测试可见；一次性凭据 smoke、原始 Red/Green 和独立评审证据缺失。
- **人工校正/教训：**provider 只完成一次请求，外层重试、循环和工具授权继续由 harness 控制；鉴权失败不应盲目重试。

### 任务 15：Application Service 与自研 CLI

- **阶段：**自研入口、slash command 和凭据与配置交互补全。
- **技能/上下文：**历史多个实现回合的完整技能和 session 未留存；上下文可由 `/status`、`/permissions`、`/resume`、`/model`、`/plugins`、auth、config、memory 需求和提交链确认。
- **提交：**核心 service/CLI 提交链，凭据命令 `f9d7136`；另有 status、permissions、model、resume、plugins、config、login/logout 的独立提交。
- **Red/Green/评审：**相关命令各有解析或运行时测试，但不能据此声称所有回合都严格 TDD；综合 CLI 评审未留存。`/resume` 目前主要列出安全可恢复 run，完整续跑交互仍待验收。
- **人工校正/教训：**CLI 是自研 Coding Agent 的主入口，外部 agent 选择只是其中一项能力。

### 任务 16：委派兼容与生命周期

- **阶段：**既有 adapter/runtime 兼容和插件会话修复。
- **技能/上下文：**最早实现技能和 session 未留存；上下文是保留 Registry/Conductor，同时让 self-agent 与 delegate 的命令和生命周期清晰分离。
- **提交：**基础 `ffb6f68`，后续有 plugin 状态、registry refresh、EOF、session 与进程生命周期修复提交。
- **Red/Green/评审：**adapter、CLI 和 session 测试存在；任务专属 Red/Green、PR 和双评审未找到，当前 namespace 与规格是否完全一致待复核。
- **人工校正/教训：**外部 agent 的 stdout、stderr、退出码和取消必须完整处理。委派失败不能被交互 shell 继续运行掩盖为成功。

## 四、任务 17：三项固定机制演示

- **阶段：**离线可重复验收。
- **技能/上下文：**子智能体 `/root/mechanism_demos_retry` 使用 TDD、系统化调试和完成前验证；范围限定为固定 `ScriptedModel` 场景，不连接 provider 网络。
- **提交：**`c8da178`、`4c9a877`。
- **Red：**默认 MSVC 工具链因主机 linker 不可用失败，该环境错误不是有效功能红灯；切换 Windows GNU 工具链后，缺失机制入口构成可行动失败。
- **Green：**GNU 离线任务级测试曾记录通过，`werkzeug/run_mechanism_demos.ps1` 调用同一命令；后续断言加强 guardrail、feedback-loop 和 approval-resume。
- **评审/人工校正：**最终独立双评审未留存。局部机制绿色不能替代整树总门禁。

## 五、任务 18–19：CLI 与离线机制验证

### 任务 18：确定性轨迹与重放检查

- **阶段：**固定场景的事件顺序、去敏摘要、审批绑定和重放拒绝检查。
- **技能/上下文：**沿用任务 17 的 `ScriptedModel`、持久运行状态和机制断言，不连接真实 provider，也不接收任意提示输入。
- **提交：**与任务 17 的机制提交共享基础，没有找到可独立归属任务 18 的完成提交。
- **Red/Green/评审：**guardrail、feedback-loop、approval-resume 的局部断言可定位；独立端到端输出与双评审未留存，因此不单独记录任务完成。
- **人工校正/教训：**机制验证必须固定输入、动作和预期事件，避免模型随机性掩盖治理错误。轨迹通过只证明对应场景，不代表所有工具组合均已覆盖。

### 任务 19：CLI 命令契约检查

- **阶段：**`/status`、`/permissions`、`/resume`、`/model`、`/plugins` 的解析、状态读取和错误边界核对。
- **技能/上下文：**复用任务 15 的应用服务，通过临时配置与隔离状态目录检查命令行为，不读取真实凭据。
- **提交：**能力分布在任务 15 的 CLI 提交链中，没有找到可独立归属任务 19 的完成提交。
- **Red/Green/评审：**解析和局部运行时测试存在；跨命令状态一致性、完整恢复交互和独立质量评审仍需补充证据。
- **人工校正/教训：**命令名称存在不等于功能完整。每个命令都需验证成功路径、无配置路径、权限拒绝、去敏输出和持久状态一致性。

## 六、任务 20–21：打包与分发

### 任务 20：OCI 打包与 CLI 烟测

- **阶段：**容器归档、最小权限运行和固定离线 CLI 场景。
- **技能/上下文：**子智能体 `/root/delivery_docs` 处理分发收尾；容器只执行固定机制演示与健康退出检查，不加载个人凭据。
- **提交：**容器与运行清单主要位于独立课程工作树，未找到可绑定完整验收的提交链。
- **Red/Green/评审：**PowerShell AST 与 `sh -n` 通过；Docker daemon 无法连接，因此没有镜像 build/run、digest 或容器内 CLI smoke 证据。
- **人工校正/教训：**脚本语法通过不等于镜像可构建、可运行或满足最小权限。容器结论必须由实际 daemon 运行证明。

### 任务 21：归档安装与卸载

- **阶段：**双平台 release archive installer。
- **技能/上下文：**`/root/delivery_docs` 使用执行计划、TDD 和完成前验证；远程 helper 只允许显式主机、版本和受信归档源。
- **提交：**`f7fcec4`。
- **Red：**PowerShell fixture 在缺少 release installer 入口时失败；测试参数默认值和变量名冲突先被修正，随后才得到有效红灯。
- **Green：**PowerShell 与 POSIX fixture 覆盖 ZIP/archive + SHA-256、原子更新、错误摘要、失败时保留旧 binary、receipt ownership、PATH 去重、默认保留配置和 purge 确认。
- **评审/人工校正：**正式 release、attestation 和授权远程主机验收未执行。receipt 只能删除安装器确实创建的对象，脚本存在不等于全新机器验收完成。

## 七、任务 22：GitHub CI 与文档

### CI 与文档

- **阶段：**文档统一、GitHub Actions 门禁和证据缺口核对。
- **技能/上下文：**通过需求矩阵、实现审计和参考记录审计三个只读子任务并行核对，没有把外部状态推测为已通过。
- **提交：**课程 CI 基础提交为 `4706576`；测试工作流是否位于目标分支，需要在最终提交上重新核对。
- **Red/Green/评审：**整树门禁需要在目标提交上重新执行；托管 GitHub run、正式 release、attestation 和远程验收证据不能由本地文件代替。
- **人工校正/教训：**课程文件校验脚本的职责只按脚本源码认定。文档不得扩大脚本的检查范围，也不得把工作流配置存在写成托管运行成功。

### 凭据口径校正

早期设计采用“仅秘密引用”，即 `${secret:OpenAI}` 与平台凭据存储。后续实现增加受约束例外：字面 API Key 可以写入个人 `~/.orchester/orchester.jsonc`，但只有在 Windows ACL 或 Unix 私有权限检查通过后才读取；项目配置不能写字面 Key，CLI、日志、诊断和 effective config 必须去敏。日志保留这次规格变化以避免前后矛盾。


## 八、过程教训

1. 课程硬要求应在生产实现前建立需求矩阵，否则会产生冷启动顺序和 CI 平台等难以事后修复的差异。
2. 提交证明代码变化，不证明当时严格执行 TDD、独立评审或外部验收；过程证据必须随任务保存。
3. 自研 Coding Agent 的边界必须清楚：外部 agent 可以是委派工具，但策略、审批、执行、反馈、恢复与停止必须由 Orchester 掌控。
4. 凭据安全不能简化成配置文件里有没有 Key，还要验证配置层级、文件权限、传播路径、展示去敏和项目层限制。
5. 局部测试、整树门禁、容器 CLI smoke、托管 CI 和远程安装是不同证据，任何一项都不能替代其他项。
6. 补偿检查可以发现并修复技术问题，但不能抹除原本未遵守的操作顺序；可靠日志需要同时记录能力进展与过程偏差。
