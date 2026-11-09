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
