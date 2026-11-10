# Windows 与 WSL 活动监控验收

2026-11-09。监控只读取已存在的会话、SQLite 元数据和进程名，不启动、提交或续接 Codex、Claude、OpenCode。Windows Codex 额度未用于本轮验证。

## 行为

- Windows 本地来源与正在运行的 WSL 发行版分别标记环境，保留独立会话 ID；相同目录名按真实 cwd 与环境分组，显示数量、状态和时间，支持环境筛选。
- Codex 无索引时保留全部发现的历史摘要；目录遍历仍有 12,000 项上限。仅最近 128 个会话保留正文摘录，单来源合计限制 8 MiB；删除文件会清理缓存，超过 128 项不会使整个缓存失效。
- 历史正文按需读取，分页正文缓存合计限制 16 MiB。CLI 详情在后台单任务加载，Esc 可返回，过期结果不能覆盖新选中的会话。
- WSL 命令输出通过隔离临时文件捕获，避免后代持有 stdout 导致等待线程无法退出。进程扫描共用 3 秒期限；停止服务的监控调用后不会留下永久进程扫描线程。
- SQLite 通过 WSL 内只读备份获取一致快照；后台来源分别更新，慢来源不阻挡其他来源详情。

首次扫描以及大型压缩历史的按需解码仍可能较慢；目录扫描不是后台增量索引。WSL 来源在首次后台读取完成之前可能显示不可用，不能仅据第一次取样判断其最终读取状态。

## 验证

在独立 worktree 中组合精确监控文件、对应 CLI、仅 activity 文案及桌面依赖锁文件，避免混入未提交的项目库和会话操作改动：

- Linux activity 33、process 6、CLI 13 项专项测试通过；netz/konsole 全 targets 严格 Clippy 与 rustfmt 通过。
- Windows MSVC：netz 32 个测试程序合计 204 项通过、1 项忽略；CLI command center 13 项通过。
- 前端 activity 13 项测试、Web 类型检查通过。
- 桌面独立 `Cargo.lock` 补齐 tempfile 及此前远程运行时遗漏的 fs2/futures/libc/windows-sys 依赖关系；`cargo metadata --locked` 通过。这不代表完整桌面原生打包通过。
- Windows 测试发现新增缓存夹具用未规范化路径比较缓存的规范化路径，误删了应保留的文件；修复为同一规范化表示，保留原断言后重跑上述 Windows 门禁通过。

额外运行 Windows 原生只读探针，使用应用同一个 `LocalAgentActivity` 与真实用户目录；等待 WSL 首次后台扫描后得到：

| 环境 | Provider | 会话数 | 来源状态 |
| --- | --- | ---: | --- |
| Windows | Codex | 31 | available |
| Windows | OpenCode | 2 | available |
| WSL · Ubuntu-22.04 | Codex | 73 | available |
| WSL · Ubuntu-22.04 | OpenCode | 3 | available |

这些数字是验收时的快照，不是固定目录容量。Claude 在这两个环境中未找到记录。探针只输出来源和数量，不输出会话正文、配对凭据或模型密钥；未调用任何模型。临时日志为 `/tmp/orchester-windows-monitor-live.log`，测试日志为 `/tmp/orchester-windows-monitor-tests.log` 与 `/tmp/orchester-windows-command-center-tests.log`。

## 2026-11-10：未知会话状态显示修复

只读审计发现：外部会话超过两分钟没有更新后，后端会把未结束记录标为 `unknown`，而侧栏原先把所有非 `active` 状态都显示为“未运行”。现在进行中仍显示“正在处理”，完成、中断和未知分别显示“已完成 / 已中断 / 状态未知”；英文与繁体中文同步，行标题也使用明确的未知状态文案。此修复保留后端状态与进程发现规则，不据进程数量推断某个历史会话仍在运行。

现有活动界面套件新增三种语言的真实组件 DOM 回归，覆盖 Windows/WSL 来源、四种状态和 `unknown` 经轮询恢复为 `active` 后的文字与运行指示。[修复前记录](acceptance/activity-state-labels-before.log)为三项失败，明确复现未知显示“未运行”；[修复后记录](acceptance/activity-state-labels-after.log)中活动界面和语言套件共 19 项通过。Web 类型检查与生产构建通过，未启动 Windows Codex 或调用模型。此前原生探针的会话数量只证明历史发现，不替代实际正在运行进程的取样。

## 2026-11-10：运行进程来源审计

历史验收中实际存在一次 Windows 已安装运行时 `/api/v1/agents/status` 的 `codexInstances=5` 取样；它来自 `active_windows`，与上述历史会话数量不同。[历史来源证明](acceptance/windows-activity-historical-source-proof.json)保留时间、安装文件哈希与证据关联，不能视为当前版本的新取样。

本轮只读原生探针直接调用生产 `SystemAgentProcessSource`。两次返回均在约 3.03 秒后给出空聚合，却没有提供来源可用性。相同运行中 WSL 的独立 `/proc/*/comm` 样本发现 Codex 3；Windows CIM 名称取样为零。[生产及补充取样](acceptance/windows-process-source-before/manifest.json)保留原记录与检测脚本。补充 PowerShell 脚本纠正进程句柄与退出码读取后，WSL 枚举成功，单发行版查询仍在 3 秒和 15 秒期限下立即失败；复杂 shell 参数或脱离终端的句柄行为尚未排除，因此不把该补充失败当作完整来源计数。这里没有启动任何 Agent、模型或用户服务。

源码确认的独立缺陷是：WSL 枚举失败被转换成空列表，单发行版读取失败被略过，过期缓存退成空聚合；随后运行状态把这些空聚合当成真实零。来源失败不能证明未运行。后续修复与新桌面文件验收需同时覆盖来源状态、计数和界面，旧安装目录与隔离桌面包不能自动替代新版本交付。[安装文件与候选集合](acceptance/desktop-installed-candidate-audit.json)仅为只读文件清单，尚未更新用户安装目录。

## 运行进程来源可用性修复

运行状态协议升级为 v3，同一 HTTP/WebSocket snapshot 原子携带当前计数与 `process_sources`。成功来源提供当前计数和采样时间；失败来源没有计数；缓存的最后成功值明确标为 `stale`，保留原时间，不再加入当前聚合。WSL 枚举成功且没有运行中的发行版才确认该来源为零；有发行版时分别保留每次读取结果，失败不会清掉其他成功来源。采样仍共用原来的 3 秒期限，所有查询子进程仍有界且仅归本次读取所有。操作系统枚举为空或后台读取任务失败也会公布来源不可用。

完整成功采样才能展示精确零。部分成功且存在进程显示“至少 N”；部分采样为零时总运行状态显示“状态未知”。受管运行和审批状态保留，详情显示来源与最后成功时间；CLI 是否可用独立展示，不能遮住真实 WSL 运行或未知状态。未知进入需注意分组，行、总计和 ARIA 文案同步。新客户端仍接受严格旧 v2，将其来源标为未观测；旧版本零不证明所有 Windows/WSL Agent 未运行。v3 强制 nullable 字段存在、验证 RFC3339 时间，并校验外部当前计数只等于成功来源之和。

[后端修复前](acceptance/process-source-status-before.log)复现首次部分零、质量变化未广播和审批来源缺失三项失败；[协议修复前](acceptance/process-source-protocol-before.log)复现缺 nullable 键、非法时间、计入过期数量三项失败。[后端门禁](acceptance/process-source-status-rust-integration.log)中进程、监控、runtime、HTTP、WebSocket 共 18 项通过；[协议门禁](acceptance/process-source-protocol-after.log)9 项通过；[缓存复验](acceptance/process-source-status-rust-cache.log)4 项通过。此前相关 lib 38 项亦通过，最后缓存行展示调整后重跑上述缓存专项。[严格 Clippy](acceptance/process-source-status-clippy.log)通过 netz/protokoll 全 targets，使用两个构建任务；工作区与桌面独立 Cargo.lock 同步，桌面 `cargo metadata --locked --offline` 通过。

[前端验证](acceptance/process-source-frontend-validation.json)包含协议 15 项、Web 43 项专项通过及协议类型检查、Web/website 生产构建的实际记录与文件清单。额外复核发现旧 v2 WebSocket 帧仍被版本检查拒绝，以及初始受管零的通用窗口指标仍宣称精确零；分别保留一项和两项失败记录后修复。新旧协议 HTTP/WebSocket 均使用共同严格解析，来源未观测或退化时通用零指标显示“—”，受管正数与审批继续保留。早期混合树测试和被中断的高并发 Rust 全量链接均不作最终门禁；完整最新 main 门禁仍由整合验收记录承担。本轮没有调用模型或启动 Windows Codex。

更新后的[生产源 Linux 取样](acceptance/process-source-status-after/linux-production-snapshot.json)于 UTC 06:23:12 直接读取已有 Codex 3 个进程；来源为本地且完整可用，附实际成功时间，无原始 argv、凭据或模型输入。这是生产 `SystemAgentProcessSource` 的新版本取样，不替代尚待执行的 Windows/WSL 原生诊断。

当前已安装桌面 `9688bae1…` 与旧隔离桌面 `372365d3…` 都是 v2 运行时；它们只能验收新 Web 的旧协议未知分支。完整 v3 原生来源验收需新的 Windows EXE 与配套 Web。相同 Rust 改动也影响 7337/7338 使用的远程后端：此前 `4569036d…` 便携包与 Linux 运行记录仍是历史版本，不能由只替换 Web 得到新后端验收。手机使用独立 `/agents` Remote API 和 mobile dist，本次 fleet v3 不单独要求重制既有 APK。尚未把 Windows 句柄诊断、新原生文件、用户安装目录或正在运行后端升级记为通过。
