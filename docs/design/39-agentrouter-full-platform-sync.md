# AgentRouter 全目录与多平台同步验收

本轮扩展范围与此前[38 的桌面/手机交付历史](38-desktop-mobile-final-delivery.md)分别归档，不覆盖旧源码、失败或退出边界。最终冻结源码为 main `71def643cb08b208cc79687f79255c504eb14dc6`，执行日期为 2026-11-11（Asia/Shanghai）。目录已分别展示 WorkBuddy 国内、国际两版，共 21 providers、7 services、42 项主题映射和 24 份官方图标。前端构建、针对性验证和 Pages 实际发布已有对应证据；Linux 新版服务已恢复并完成验证；Windows 核心构建和 Android 新 APK 签名、安装及原生测试已有本轮记录。Windows 同用户交互桌面尚未证实，最终原生 Windows/Android 界面、IDE 同步及 OrchesterDesktop 重装仍待实际终态。初始 `a96a06e` 的 20 项阶段另保留历史绑定，不计为最终 21 项通过。

[AgentRouter 官方目录覆盖凭据](acceptance/agentrouter-platform-sync/sources/agentrouter/final-guide-scope-safe.json)逐项核对 17 个唯一 guide URL 的 HTTP、title、h1 与文章散列：Claude Code CLI / VS Code 为同一产品，guide 去重为 16 项；主页另列 Gemini CLI，因此官方目录共 17 个产品。最终 canonical 21 项为这 17 项加既有 DeepSeek Harness / Cursor / Antigravity 及新增 WorkBuddy 国际版，没有缺项。第三方 guide 的平台用词不作为厂商支持或本机安装证据。

## 目录、展示与官方图标

最终共享 `AGENT_IDENTITIES` 为 21 个 canonical 产品标识，Web、桌面、手机和官网使用同一元数据与资产。进程与服务计数分别显示；Cline、Roo、Kilo 共用编辑器来源时保留 presence unknown，不由同一编辑器进程推导各扩展已活动。安装、版本检查、监控识别和模型登录分别记录。

最终 21 个产品的深浅色映射为 42 项，实际去重文件仍为 24 项：17 SVG、7 PNG。24 份图标字节与此前已检查的资产相同，沿用 SVG XML、事件/脚本/外部引用边界及 PNG 签名/解码证明。Gemini 的官方 1645×1645 PNG 源散列和 Lanczos 到 64×64 的解码像素相等证明也保留。[最终 21 项 / 24 图标原始凭据](acceptance/agentrouter-platform-sync/sources/icons/final21-official-current-24-assets.json)绑定前端提交 `cca3bd9`；[独立最终源码绑定](acceptance/agentrouter-platform-sync/sources/final71-source-binding.json)再次逐项核对资产及元数据与 `71def643` blob 相等。[a96 阶段 40 项映射与媒体安全历史](acceptance/agentrouter-platform-sync/sources/icons/a96a06e-current-24-assets.json)保留原字节。以上不代替实际原生界面验收。

WorkBuddy 国内官网为 [workbuddy.cn](https://www.workbuddy.cn/)，国际官网为 [workbuddy.ai](https://www.workbuddy.ai/)。Windows 原安装名分别为 WorkBuddy 5.9.3 / `WorkBuddy.exe` 和 WorkBuddy AI 5.9.4 / `WorkBuddyAI.exe`，两份既有安装保留。已兼容原 `workbuddy` 并显示“WorkBuddy 国内版”，新增 `workbuddy_global` 并显示“WorkBuddy AI 国际版”。两站各自 `rel=icon` 指向的 WorkBuddy 标记实际都是 4,142 字节，SHA-256 `10ccbc099e7bc3d5ff12c315c4edd7b21386aa7166c1ec19997b3d55010d5c7d`，与现有 `workbuddy.svg` 完全相等，共享品牌文件并分别保留官网来源。国际站自身引用的是 download.codebuddy.ai 下的 WorkBuddy 资产，不能据 CDN 名称改成 CodeBuddy 产品。[两版官方发行、品牌与命名研究](acceptance/agentrouter-platform-sync/sources/workbuddy/official-regional-release-research.json)、[既有 Windows 注册及快捷方式目标](acceptance/agentrouter-platform-sync/native/windows/workbuddy-registration-paths-readonly-safe.json)。

Windows 两份公开包的 npm 名称都为 `@genie/workbuddy-desktop`，但产品名、版本与官网不同；仅按包名不能合并两版身份。国际 EXE 的签名为 Valid、publisher 为 Tencent，SHA-256 `15880ca7859f417bf8a6a639bacd1da6b63ced83fbead710317dad38b8f6fcc8`。[公开包渠道差异](acceptance/agentrouter-platform-sync/native/windows/editions/workbuddy/public-package-channel-proofs-safe.json)、[国际 EXE 与目录存在性](acceptance/agentrouter-platform-sync/native/windows/editions/workbuddy/global-native-public-metadata-safe.json)只记录 `.workbuddy` / `.workbuddy-ai` 两目录存在，未读取内容；此观察还不能替代国际 Linux 包内实际 profile 命名空间的核验。

| 产品标识 | 当前 canonical 文件 | 官方来源 |
| --- | --- | --- |
| `codex` / Codex | `codex.png`, `codex-light.svg` | [深色](https://openai.gallery.vsassets.io/_apis/public/gallery/publisher/openai/extension/chatgpt/latest/assetbyname/Microsoft.VisualStudio.Services.Icons.Default), [浅色](https://openai.gallerycdn.vsassets.io/extensions/openai/chatgpt/26.51106.51428/1794372105882/Microsoft.VisualStudio.Services.VSIXPackage) |
| `claude` / Claude Code | `claude.png` | [官方资产](https://anthropic.gallery.vsassets.io/_apis/public/gallery/publisher/anthropic/extension/claude-code/latest/assetbyname/Microsoft.VisualStudio.Services.Icons.Default) |
| `deepseek` / DeepSeek Harness | `deepseek-harness-dark.svg`, `deepseek-harness-light.svg` | [深色](https://raw.githubusercontent.com/deepseek-ai/deepseek-harness/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/web/public/favicon-dark.svg), [浅色](https://raw.githubusercontent.com/deepseek-ai/deepseek-harness/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/web/public/favicon.svg) |
| `opencode` / OpenCode | `opencode.svg` | [官方资产](https://raw.githubusercontent.com/anomalyco/opencode/e56eff2bc01649b3aa21224876b8874e77dea4b0/packages/ui/src/assets/favicon/favicon.svg) |
| `gemini` / Gemini CLI | `gemini.png` | [官方资产](https://geminicli.com/icon.png) |
| `antigravity` / Antigravity | `antigravity.svg` | [官方资产](https://antigravity.google/favicon.svg) |
| `cursor` / Cursor | `cursor-dark.svg`, `cursor-light.svg` | [深色](https://cursor.com/marketing-static/favicon.svg), [浅色](https://cursor.com/marketing-static/favicon-light.svg) |
| `workbuddy` / WorkBuddy | `workbuddy.svg` | [官方资产](https://assets.workbuddy.cn/web/workbuddy/5611ca0b566ae70e862617fecf6f3c8b05e5515a/assets/logo.svg) |
| `workbuddy_global` / WorkBuddy AI 国际版 | `workbuddy.svg` | [国际官网关联资产](https://download.codebuddy.ai/web/workbuddy/05fd33c3609bc9cf5ae1fc387a142a2ad0fab64e/assets/logo.svg) |
| `qwen` / Qwen Code | `qwen.svg` | [官方资产](https://github.com/QwenLM/qwen-code/blob/e19e1ebe7881960ab2e3319d677c491254600571/packages/desktop/bootstrap/qwen-code-logo.svg) |
| `openclaw` / OpenClaw | `openclaw.svg` | [官方资产](https://github.com/openclaw/openclaw/blob/44a2948a9eaf4345069d4b652ae382d10dd08f62/ui/public/favicon.svg) |
| `cline` / Cline | `cline-dark.svg`, `cline-light.svg` | [深色](https://raw.githubusercontent.com/cline/cline/c5df41f937c496ce8c8e689ba58776593c703dfa/assets/branding/logos/cline-icon-white.svg), [浅色](https://raw.githubusercontent.com/cline/cline/c5df41f937c496ce8c8e689ba58776593c703dfa/assets/branding/logos/cline-icon-dark.svg) |
| `roo` / Roo Code | `roo.png` | [官方资产](https://RooVeterinaryInc.gallery.vsassets.io/_apis/public/gallery/publisher/RooVeterinaryInc/extension/roo-cline/latest/assetbyname/Microsoft.VisualStudio.Services.Icons.Default) |
| `kilo` / Kilo Code | `kilo.svg` | [官方资产](https://raw.githubusercontent.com/Kilo-Org/kilocode/2649e61b9eca9619f0dcbd3b2c53deb04c8b26ab/packages/kilo-docs/public/favicon/favicon.svg) |
| `hermes` / Hermes Agent | `hermes-dark.svg`, `hermes-light.svg` | [深色](https://raw.githubusercontent.com/NousResearch/hermes-agent/9373dc1e315a68446f9767afd7acd15d4c0642b8/assets/icon-master-dark.svg), [浅色](https://raw.githubusercontent.com/NousResearch/hermes-agent/9373dc1e315a68446f9767afd7acd15d4c0642b8/assets/icon-master.svg) |
| `pi` / Pi | `pi.svg` | [官方资产](https://pi.dev/logo.svg) |
| `crush` / Crush | `crush.png` | [官方资产](https://raw.githubusercontent.com/charmbracelet/crush/1b7582dbba81e0f716e50f20fe44bd04faf7251f/internal/ui/notification/crush-icon-solo.png) |
| `claude_app` / Claude App | `claude.png` | [官方资产](https://anthropic.gallery.vsassets.io/_apis/public/gallery/publisher/anthropic/extension/claude-code/latest/assetbyname/Microsoft.VisualStudio.Services.Icons.Default) |
| `trae` / Trae | `trae.png` | [官方资产](https://lf-cdn.trae.com.cn/obj/trae-com-cn/trae_website_prod_cn/favicon.png) |
| `zcode` / ZCode | `zcode.png` | [官方资产](https://zcode.z.ai/favicon-192x192.png?v=20260707-transparent) |
| `craft` / Craft Agents | `craft.svg` | [官方资产](https://raw.githubusercontent.com/craft-ai-agents/craft-agents-oss/3ce9fc12837541d688a59a8f0a2e9755ceb89ac2/apps/electron/resources/icon.svg) |

Codex 浅色图标初次使用了官方文档 book，目视复核发现其不对应 Codex 产品；已从 OpenAI 官方扩展 VSIX 的 `extension/resources/blossom-black.svg` 提取真实 blossom，原始字节 2,008，SHA-256 `8652777b55544c572ba445dd230a90c2e248611e92eaa77de5805e8d0561fc30`。ZIP byte range 与条目的原凭据保留。[纠正记录](acceptance/agentrouter-platform-sync/sources/icons/history/codex-visual-correction.json)、[官方扩展条目](acceptance/agentrouter-platform-sync/sources/icons/history/codex-product-icons-range-receipt.json)。Codex 深色仍用官方发布者 PNG；Claude Code 与 Claude App 共用官方 Claude 品牌 PNG，后者不宣称该图标取自桌面安装包。

Pi 初次使用跟随系统主题的 favicon，在应用主题不同于系统主题时会隐形；现保留官方 `https://pi.dev/logo.svg` 彩色 logo 原始字节 334，SHA-256 `abd66e7868b2d24f0f0895f9237ee8a6dcb22337583b0dc54aeb595acecb4d6b`。[Pi 纠正记录](acceptance/agentrouter-platform-sync/sources/icons/history/pi-visual-correction.json)。[旧资产凭据](acceptance/agentrouter-platform-sync/sources/icons/history/historical-official-assets-before-corrections.json)和[Pi 纠正前提交绑定](acceptance/agentrouter-platform-sync/sources/icons/history/binding-before-pi-correction.json)保留为历史，不能据其旧 book/favicon 散列宣称最终 24 图标相等。

## 前端门禁与构建

Web 完整命令初次退出 1，940 项通过、3 项 monitor 断言失败；修正后扩大 monitor 专项 61 项通过，另两项受影响文件原有 31 项通过。未把重复专项与完整套件通过数相加，未将初次命令改称退出 0。Design 完整命令为 310 项通过、1 项现存 CodePreview 超时；源码未变的 5 项用单 worker 重试通过。Mobile 全套 130 项、Protokoll 全套 97 项及四项类型检查均退出 0。原失败、重试、各命令范围及日志散列保留在[前端原始凭据](acceptance/agentrouter-platform-sync/tests/frontend/validation-summary.json)。随后资产纠正保持元数据导入路径不变，最终图片散列由上面的新证明绑定。

`a96a06e` 独立发现的非核心工具详情缺口已在 `6eb4117` 修复：WorkspaceView 在 `closeActivity` 后恢复被点击的 ID，详情保留对应工具。最终针对性验证为 Web 76、Mobile 6、Design 4 项通过，三项 typecheck 均退出 0；源码在各命令前后稳定，并与最终冻结输入相等。[修正后的原始凭据](acceptance/agentrouter-platform-sync/tests/frontend/workbuddy-editions/corrected-result.json)。新增 21 项目录的首次 Web 命令曾为 75 项通过 / 1 项失败，原因是测试保留了硬编码 20 项期望；[初次失败原字节](acceptance/agentrouter-platform-sync/tests/frontend/workbuddy-editions/initial-failed-result.json)和[失败/修正边界](acceptance/agentrouter-platform-sync/tests/frontend/final21-delivery-safe-summary.json)保留。专项不合并为重新执行的完整 Web 套件，也不代替原生 runtime。

Web / Mobile / 官网三个生产构建实际退出 0、进程已退出；分别输出 37 / 17 / 26 项 public 文件。363 项前端输入在构建前后相等，原构建提交 `cca3bd9` 的输入逐项等于最终 `71def643`；当前 dist 字节再次逐项与构建记录核对相等。[独立构建记录](acceptance/agentrouter-platform-sync/tests/frontend/final21-builds-independent-review.json)、[最终输入绑定](acceptance/agentrouter-platform-sync/sources/final71-source-binding.json)。手机主 JS 为 `index--zaH3Y7H.js`，SHA-256 `7e115c9d4b6236a5de0338b7499f0e09341df65ee03067ad71cd01b578a10a49`；CSS 为 `0ccaec9543d9001c1e528918446afa1170c46d9fc676c46ba33f1a21a5fbbf20`。

## Rust 与 Linux 构建

两版 WorkBuddy 的功能验证实际 58 项通过，含 33 个 process/identity 测试和 25 个 runtime/routes/socket/catalog/remote DTO/auth 测试；strict netz all-target Clippy 与格式退出 0。首次编译曾因新测试多传一个 `instance_key` 参数退出 101，原失败摘要、日志 SHA 及[关键编译失败日志](acceptance/agentrouter-platform-sync/tests/backend/workbuddy-editions/first-compile-failure.log)保留。[功能/拆分独立门禁记录](acceptance/agentrouter-platform-sync/tests/backend/workbuddy-editions/gates-independent-review.json)明确区分失败、修正与复用范围，不将编译失败归为模型或产品 runtime 失败。

随后把 19 个 identity 测试移至专用 `tests/identity.rs`，生产部分 548 行前缀逐字节不变，逻辑模块路径及原 33 个 process 测试名均保留；`identity.rs` 为 551 行，测试文件为 1,096 行。拆分后重新运行 33 个 process 测试、strict netz Clippy 和格式均退出 0，复用未改功能范围，不把 58 与 33 相加。[迁移证明](acceptance/agentrouter-platform-sync/tests/backend/workbuddy-editions/test-split/migration-proof.json)、[测试发现证明](acceptance/agentrouter-platform-sync/tests/backend/workbuddy-editions/test-split/test-discovery-proof.json)。功能阶段 366 项输入、拆分后 367 项输入分别稳定，差异仅为原 identity 文件和新增测试文件；最终 367 项后端加 363 项前端输入共 730 个互异文件，已独立核对 `71def643` blob 和当前字节相等。

本轮冻结 `71def643` 的 [持久完整 Rust 门禁](acceptance/agentrouter-platform-sync/tests/backend/final71-full-workspace/summary.json)已实际退出 0：103 个测试程序、1,304 项通过、1 项忽略、0 失败，strict workspace Clippy / fmt 和正确 `orchester-konsole` native 构建均通过。368 个输入在前后稳定；相比此前 367 输入仅扩大纳入未改变的 `.cargo/config.toml`，前端 363 与当前 Rust 368 共 731 个互异输入。最终 ELF 为 236,091,496 字节，SHA-256 `d983bceea7c0797c995c7ffb038eaa984323ee1106293da18433f2a3f22d6649`。之前临时阶段曾误用 package 名 `orchester-cli` 而退出 101，后续修正报告保留为阶段背景；本轮持久四门禁结果独立记录，不依赖已消失的临时 summary。

原完整 `cargo test --workspace --all-targets --locked --offline` 为 1,301 项通过、1 项忽略、103 个测试程序，严格全工作区 Clippy、格式与 Linux CLI 构建均退出 0；该结果在最后三项已安装产品身份修正之前执行。之后仅 `identity.rs`、`rules.json`、`wsl_query.py` 三项 Rust 输入不同，当前专项为 31 个 process/identity fixture 加 22 个 catalog/status/socket/routes 测试，共 53 项通过；严格 netz all-target Clippy 和格式退出 0。每份输入在自身命令前后稳定，当前 366 项 Rust 输入逐个等于 `a96a06e` blob。[独立范围与源码复核](acceptance/agentrouter-platform-sync/tests/backend/a96a06e-gates-independent-review.json)。上述范围只描述 a96 历史阶段，不是冻结 71def643 的完整套件结果；这些命令均不包含 doctest。

`a96a06e` Linux 原生链接成功，debug ELF 为 236,091,336 字节，SHA-256 `1a14692fec65687f6798bfe7f706da76d299c1c45288fa4fef92a59d4eb645c5`；部署前门禁明确复用上述完整与专项范围。[本轮部署门禁汇总](acceptance/agentrouter-platform-sync/tests/backend/a96a06e-deployment-gates-summary.json)。Linux 7337 此阶段已实际更新，PID 1244652 / start ticks 15063739 的 image 与上述 ELF 相等，schema 3/20 providers、17 项静态资源、临时鉴权撤销与 tasks / auth / projects 保留已记录；原 device 数量解释见本轮澄清；模型调用为 0。当时 supervisor session 88100 保留，旧 manager session 27081 正常退出 0；本轮重新观察时这些历史进程与监听器均已消失，不能据旧 session 判断当前存活。[实际部署记录](acceptance/agentrouter-platform-sync/deployments/linux/a96a06e-final-deployment.json)、[独立 live 复核](acceptance/agentrouter-platform-sync/deployments/linux/a96a06e-independent-live.json)。这份 20 providers 结果不代替新版 21 项服务部署。

本轮还区分持久 device 条目与有效注册表：原状态存在 host 3 条加 legacy 2 条，其中 legacy 与 host 重复，所以是 5 条保存记录、3 个互异 token / device IDs。原条目不删除或改写，当前程序按既有 host 优先规则读取 3 个有效设备；后续 live 验收按 3 计数，不能把 5 条保存记录说成 5 个当前有效设备。旧原始凭据字段保留；当前服务将使用原持久状态和当前环境恢复，旧进程已退出，不能再声称恢复了其已失去的 argv / env 内存。

[当前 Linux 7337 实际恢复](acceptance/agentrouter-platform-sync/deployments/linux/current-final21/final-safe.json)已通过：PID 1023019 / start ticks 256519 / ELF `d983bcee…` 对应冻结 71，21 项 schema 3、两版 WorkBuddy 分别显示、7 类 service 与 3 类 shared-editor 边界、17 个 public 资源散列均已核验。无鉴权访问 401，临时自有设备配对和四项读取成功，删除后 token 为 401、配对关闭；原 3 个有效设备及全部 5 条保存条目的值和 expiry 保留，5 条任务完整 raw rows 不变。remote state 的 JSON 语义相等，SQL 中 JSON 文本格式/顺序并非逐字节相等；不能混用这两个结论。恢复使用当前环境及原 state-root，未声称旧进程 argv / env 内存仍可恢复，也未声称已有用户 token 已在网络实际通过；[独立 live 复核](acceptance/agentrouter-platform-sync/deployments/linux/current-final21/independent-live-safe.json)确认当前 PID / image / 7337 listener。私人一致备份仅保存受限目录，未归档内容。

[主代理再次独立观察](acceptance/agentrouter-platform-sync/deployments/linux/current-final21/root-independent-live-safe.json)核对相同 PID / start ticks / image、公开首页字节和两条受保护路由 401。根工具查询 session 10496 返回 Unknown，并不等于服务停止；当前进程和 HTTP 已直接确认存活。[控制交接](acceptance/agentrouter-platform-sync/deployments/linux/current-final21/controller-handoff-safe.json)记录原自有 stdin pipe、controller 1023013 与 fd 4 的身份，后续正常管理需按该交接核对，不声称根工具能够继续 poll 10496。

## 原生 Agent 安装范围

WSL 有 19 个已完成验证单元：10 个 npm CLI 真实 `--version` 退出 0、Hermes 0.23.1 退出 0、4 个 VS Code remote-server 扩展实际安装/列出、4 个官方 GUI 包解包与 native ELF/runtime/ldd 检查。验证单元有产品重叠，19 不代表 19 个互异产品都通过 GUI 交互。CLI 安装版本分别为 Codex 0.167.1、Claude Code 2.1.322、Gemini 0.68.0、Qwen 0.27.2、OpenCode 1.19.5、OpenClaw 2026.11.3、Pi 1.5.2、Crush 0.100.0、Cline 3.0.74、Kilo 7.8.27；扩展为 Claude Code、Cline、Roo、Kilo。[WSL 原始摘要](acceptance/agentrouter-platform-sync/native/wsl/install-summary.json)、[真实版本记录](acceptance/agentrouter-platform-sync/native/wsl/native-version-receipts.json)、[扩展安装记录](acceptance/agentrouter-platform-sync/native/wsl/extensions-receipts.json)。

WSL GUI 包为 ZCode 3.17.2、Craft Agents 0.15.9、WorkBuddy Linux 5.6.2.40046327、Trae CN 3.4.17。实际 ELF 与官方包/产品版本已核，vendor ElectronRunAsNode 退出 0，未缺共享库；未启动这些产品的 GUI 会话或模型。[仅 native runtime 的独立记录](acceptance/agentrouter-platform-sync/native/wsl/gui-native-runtime-reviewed.json)、[官方包来源/散列](acceptance/agentrouter-platform-sync/native/wsl/official-package-source-map.json)。Claude App 官方 Linux 包或平台文档因 TLS EOF 未能核验，未以社区包或 Windows interop 代替，也未据此推断官方不支持 Linux。安装结束时配置保留和随后外部配置变化分别记录，未恢复过期备份或复制配置内容。

[本轮 WSL 安装矩阵](acceptance/agentrouter-platform-sync/native/wsl/current-matrix/current-native-installation-matrix-safe.json)只读复核为 19 条记录、16 个互异产品：10 CLI、Hermes、4 扩展与 4 GUI，Claude / Cline / Kilo 各有 CLI/扩展重叠。已记录的 15 个入口、4 个 GUI launcher 和相关 ELF 散列与历史相等，10 个 npm 公开版本和扩展 ID/version 相同；原扩展没有完整 payload 散列，因此不宣称所有扩展文件字节未变。原 17 份安装/版本凭据仍与归档相等，本轮没有重跑所有 runtime 或安装命令。已知 7 个 legacy 公共路径未找到 DeepSeek / Cursor / Antigravity 不能当作全系统未安装证明。

上述 WSL WorkBuddy 是国内版。国际官网的官方 Linux 更新接口返回独立 deb 5.5.2.37849279，但 `sha256hash` 为空；实际 deb 下载返回 HTTP 404 / curl exit 22 / 零字节，RPM 下载遇到 TLS 错误 / exit 35。国际 Linux 包、native ELF 和 profile 命名空间均未验证、未安装，国内安装保持原状，没有用 Windows interop 或社区包替代，也不据此推断官方永远不支持 Linux。[实际可用性终态](acceptance/agentrouter-platform-sync/native/wsl/workbuddy-global-final-availability.json)、[国际官方 Linux 更新原字节](acceptance/agentrouter-platform-sync/sources/workbuddy/global-linux-update.json)、[国内官方 Linux 更新原字节](acceptance/agentrouter-platform-sync/sources/workbuddy/cn-linux-update.json)。该凭据记录了实际下载尝试，未完成国际版 Linux 安装。

本轮进一步读取 [国际官方下载界面与发行状态](acceptance/agentrouter-platform-sync/native/wsl/current-official-availability/final-official-install-status-safe.json)：国际客户端公开下载项为 macArm64 / macX64 / Windows / online，Linux tile 标记 domesticOnly；主页结构化平台信息为 Windows / macOS。旧 Linux 更新地址仍返回 5.5.2.37849279，但新完整 GET 与 range GET 均为 404、零字节。此结果仅证明当前未提供可用国际 Linux 下载，不推断永久平台政策；国内原 ELF 与共享库保持原状。Claude Linux 的官方页面、curl 与 Chromium 通道仍有传输错误，官方 Claude Code CLI 的 GitHub 资料也不能替代 Claude App 桌面安装。商业网站完整 HTML/JS 保留在原研究目录，归档仅记录安全来源与散列。

Windows 已有 9 项真实 native CLI 版本命令退出 0、原子进程均退出、无超时；失败的数字版本解析原记录保留。[真实 Windows CLI 版本](acceptance/agentrouter-platform-sync/native/windows/nine-native-cli-versions.json)。新 ZCode 3.17.2 与 Craft Agents 0.15.9 已按官方包及原 EXE/资源身份核验，[安全安装身份记录](acceptance/agentrouter-platform-sync/native/windows/new-gui-installed-identities.json)。较新既有 WorkBuddy 5.9.3 与 Trae 3.6.13 保留，不降级；四项 Windows VS Code 扩展与既有版本的统一最终安全凭据待 Windows 代理补齐。版本检查不代表模型鉴权或任务执行已验证。

Claude 的官方下载 TLS 失败后改走官方 Microsoft Store 产品 `9P6K58THS811`、publisher `Anthropic, PBC`、package family `AnthropicPBC.Claude_fnn82j28hfe8t`。Store worker 退出 0，但当前只读 GetAppx 记录的匹配包数为 0，因此实际安装与 EXE 仍未闭合。[只读观察](acceptance/agentrouter-platform-sync/native/windows/claude-store-readonly-observation.json)。

本轮 [Claude Windows 官方渠道复核](acceptance/agentrouter-platform-sync/native/windows/claude-current-official-source/handoff-safe-summary.json)区分 Store family `AnthropicPBC.Claude_fnn82j28hfe8t` 与官方下载 MSIX family `Claude_pzs8sxrjxfjjc`；当前用户注册、对应机器命名空间、StartApps、已知产品和 EXE 的只读匹配数均为 0，查询未出错。官方 Microsoft Store 前端给出的 bootstrapper 已实际下载（813,088 字节、Microsoft Authenticode Valid），但它是 Store Installer，不是 Claude payload；准备、安装和最终 EXE 核验分别记录，不能因 bootstrapper 签名或下载成功改称 Claude 已安装。

## Windows 构建、重装与服务

`a96a06e` 原生源码导出的实际 1,681 Git blob、Web dist 37 项和 Mobile dist 17 项已核对；Git archive 的属性/EOL 转换已恢复为原 blob。[原冻结导出凭据](acceptance/agentrouter-platform-sync/native/windows/source-export-a96a06e.json)有一项历史元数据澄清：`apps/mobile/android/gradlew.bat` 的 `gitBlobs.sha256` 误保留恢复前值 `1d297e00…`，实际导出字节、计划 inventory 与原 Git blob 的 SHA-256 都为 `94102713eb8fb22d032397924c0f38ab2da783ba60d07054339f1190a0c4e2cd`，本次只读再次逐字节核对相等；不重写被冻结的审计文件。[构建安全汇总中的澄清](acceptance/agentrouter-platform-sync/native/windows/a96a06e-build/native-build-safe-summary.json)。新 producer 固定 16 项有序门禁，7 个脚本有固定散列。APK 在核心计划中显式 deferred/null，额外 RuntimeInputs 需要另行冻结实际同源码 Android 验收与 APK 字节，不能倒灌旧 APK。[prepared producer](acceptance/agentrouter-platform-sync/native/windows/producer-prepared-a96a06e.json)。

`a96a06e` 的 16 项 native build 已全部退出 0 / 无超时 / 原子进程退出 / 未尝试终止，原 worker 308308 于 native 终态观察时不存在。Desktop EXE SHA-256 `26a3449c079be61e634a273decc07d5ec80d91ea2e743173026f672f5166f45f` 对应 38 项核心文件，Remote EXE SHA-256 `f13efea300472bcfa4b81c88aa44f3470698807af40b00930787e8e7a9cb60fc` 对应 18 项核心文件，尚不含 APK。[原构建 final](acceptance/agentrouter-platform-sync/native/windows/a96a06e-build/final-result.json)、[16 项结果与 native 审计的清单](acceptance/agentrouter-platform-sync/manifest.json)。这批构建仍含上述 a96 详情缺口，且目录随后扩为 21 项，不能作为新源码 GUI 或重装通过。

最终 71 源码导出已有独立持久凭据：[精确 Git 导出](acceptance/agentrouter-platform-sync/native/windows/final71-source-export.json)为 1,682 个 Git blob 加 Web 37 / Mobile 17 文件，`gradlew.bat` 审计字段与实际 Git 字节都为 `94102713…`。APK 仍显式 deferred，源码导出与后续构建、runtime 和安装分别验收。

本轮 [Windows 71 核心构建结果](acceptance/agentrouter-platform-sync/native/windows/final71-build/final-result.json)实际 16 项门禁全部退出 0，无超时，各子进程已退出且未尝试终止。Desktop EXE SHA-256 为 `e4aa765befa85c7fb053075844be5693685c00677dd62b4cfff4592ec68c1d05`、38 项核心文件；Remote EXE 为 `24c8f9806256cc54b239b03f6b4adc976c3fee50c30bc0f2ecf601971c22ce2a`、18 项核心文件。[原冻结计划](acceptance/agentrouter-platform-sync/native/windows/final71-build/plan.json)散列为 `8645dca6…`，source 为 71，APK 仍为 deferred/null。worker 16008 写入 finishedAt，但其自身原 HANDLE 终态观察遇到 Interop 110，单凭核心构建结果不能证明 workerGone 或父 executor 退出 0，也不能代替 runtime18、正常托盘退出和正式重装。

随后 [新的原生只读 worker 观察](acceptance/agentrouter-platform-sync/native/windows/final71-build/current-worker-observation-safe.json)实际确认 PID 16008 已不存在，16 项已完成门禁均为 0 / 无超时 / 子进程已退出，进程信号为 0。原 HANDLE 因 PID 已不存在而未捕获、未验证；这份新观察补证当前 worker 不存活，不把此前 Interop 110 改称成功，也不推出父 executor 退出码或 runtime18 / 重装已完成。[终态聚合](acceptance/agentrouter-platform-sync/native/windows/final71-build/native16-core-terminal-safe.json)分别绑定先前 core summary 与此新观察，原 summary 的未观察终态字段保持原字节。C 盘可用空间 41,338,273,792 字节仅为此时快照。

[独立核心构建复核](acceptance/agentrouter-platform-sync/sources/audit/current-native16-review-safe.json)确认 1,682 个 Git / C 导出文件相等，计划 967 项为 911 Git 输入、54 public 文件及 2 个 task proof，全部散列绑定；54 个当前 Web / Mobile 资源和 C core 文件相等。当前 `gradlew.bat` 根据 Git `text eol=crlf` 属性签出为 CRLF，规范化 LF 后与原 Git / 导出 `94102713…` 相等；它不在上述 731 个编译输入中，没有改写文件。初次只读观察未分类 task proof、误要求工作树 raw LF 的 false 结果单独保留；这属于观察器范围修正。EXE 内容未由此独立审查再次读取，EXE 散列来自 producer 原结果，不能将两种复核范围混用。

[Windows portable 文件包准备](acceptance/agentrouter-platform-sync/native/windows/file-preparation/final21-portable-package-safe.json)已实际通过 28 文件及 17 Web 资源的 file verifier，绑定 71 包装源码、Remote EXE `24c8f980…`、新 21 项 APK `f83fa2a75c712a3d2e1f3f203cf73ce89204fb4ca0bce4d8c2ef0c4fe3596d3c`（13,960,333 字节）和原证书 `df6df435…`。此阶段只完成文件准备；Android final acceptance binding、runtime inputs freeze、portable 14 项 runtime 和默认安装均为 false，没有升级 7338。APK 的实际 IDE / UI / JUnit / 凭据保留终验仍由 Android 本轮安全凭据另证，APK 和 installer 二进制不归入 Git。

新版 native runtime、21 产品官方图标/单位/共享编辑器边界、真实托盘菜单自然退出、完整用户桌面备份/重装、Windows remote 7338 更新和原状态保留均待实际安全终态。新 producer / runtime-input guard 固定 typed bool/int、精确 16 门禁 / 21 产品 / 7 services、原 Android nested receipt 与最终 APK / 17 dist 绑定；原始 in_progress / 旧 20 项 receipt 必须拒绝。42 张主题图片加载、实际 provider 映射与每个 instance 计数、真实 theme 切换证明已纳入谓词。[独立安全审计](acceptance/agentrouter-platform-sync/native/windows/final71-build/producer-safe-audit-summary.json)记录 Native PowerShell 62 个用例及实际 JavaScript 24 个用例通过；首次 task-only wrapper 序列化失败退出 1 的边界保留。此审计没有执行 GUI 或安装，不计为 runtime18 通过。上一阶段 native interop 在执行 producer 前遇到 UtilAcceptVsock 110。本轮重新观察到原 Desktop / Remote / Studio / ADB / AVD 与助手进程均已退出，正在恢复现有资料并准备新验收；旧锁文件不代表运行进程，不能据此记录旧托盘菜单自然退出。新桌面重装及服务结果需以本轮实际凭据证明。

本轮 [原进程只读观察](acceptance/agentrouter-platform-sync/native/windows/final71-preflight/original-processes-absent.json)记录旧 Orchester / Remote / 工作助手进程已不存在、相关监听器为空，没有发送进程信号；进程缺失不能证明旧托盘菜单自然退出。[离线一致备份](acceptance/agentrouter-platform-sync/native/windows/final71-preflight/offline-original-backup-safe.json)已实际验证安装 156 文件、`.orchester` 9 文件、WebView 487 文件、Remote 安装 17 文件及其 state-home 2 文件，源文件检查前后稳定且复制散列相等。完整内容与清单只在受限私人备份目录，归档仅保留安全计数与散列；实际自然托盘退出和重装仍待执行。

[当前交互桌面限制](acceptance/agentrouter-platform-sync/native/windows/interactive-current/final21-readiness-blocked-safe.json)实际记录 InputDesktop 与鼠标位置 API 均为 Error 5，原自有进程在 Session 1、active console 为 Session 3，WTS 用户名与 domain 为空、查询错误为 0。当前既未证明相同账号，也未证明不同账号，不能据此改称已解锁或已具备可交互桌面。native18、正常托盘退出、默认桌面重装、7338 更新与 Claude payload 安装均为 false；原私人资料已离线备份，产品启动、进程信号与会话变化均为 0。

## Android Studio、同步与 Pages

Windows Android Studio 2026.2.2 / IDE JBR 25、Gradle JDK 21 的原生构建已实际 99 tasks、54 秒、退出 0，17 项资源与 dist/mirror/APK public 相等。先发现 Windows 标准 debug key 与原安装证书不同，停止错误覆盖；随后使用原安装证书对应密钥重签 app/test APK，保留两份原密钥与应用数据。该 20 项历史阶段 APK 为 13,960,545 字节、SHA-256 `d0d1d4cddd0914e7b6e5800dfe66d32b7cf10e562fb687193da22492a12ac8a8`，证书 SHA-256 `df6df435ccf10a685fb109668da8cdf7b022b481ba346f3143190aeda0c4468e`；实际 install-r app/test 均退出 0，NativeBridge 4 项通过，44.919 秒。此段为 Android 代理安全阶段记录，UI 20 图标、实际 PTY/input/resize、临时凭据移除及原 5 devices 保留还未终验；待最终 immutable 安全 receipt 后归档，不读取旁边的私人基线/DB/log。

本轮 Android 21 的 [原生源码镜像](acceptance/agentrouter-platform-sync/native/android/current21-partial/final21-source-mirror-safe.json)为 41 个 Git 输入与 17 public 资源，源码为 71，Gradle JDK 配置为 21.0.13+9，旧 20 镜像保留。[当前 APK 来源](acceptance/agentrouter-platform-sync/native/android/current21-partial/final21-apk-provenance-safe.json)记录 native Gradle 退出 0，app 为 `f83fa2a7…` / 13,960,333 字节、test 为 `b5c137aa…` / 574,161 字节，使用 Linux 官方 Android SDK apksigner 与原证书 `df6df435…` 签名和验证均为 0，原 key 未变、17 APK public 资源相等。新 [app 安装](acceptance/agentrouter-platform-sync/native/android/current21-partial/install-native-final21-app-safe.json)及 [test 安装](acceptance/agentrouter-platform-sync/native/android/current21-partial/install-native-final21-tests-safe.json)均成功退出 0、进程退出；初次失败仍保留，install-r 要求保数据不代表已证明原凭据保留。

[本轮 NativeBridge 紧凑安全结果](acceptance/agentrouter-platform-sync/native/android/current21-partial/native-bridge-tests-final21-current-apk-result.json)为 OK / exit 0；生产者报告 4 项测试通过，原 stdout 只保留散列引用，紧凑 JSON 本身没有解析后的测试数、SID 或 IDE 同步证明。当前 Studio 自有项目进程在 Session 1，InputDesktop Error 5，Session 3 与同用户 SID 尚未证实；不能据进程或 CLI 成功称 IDE sync / 原生 UI 已通过。21 providers / 42 图片、PTY、原保存凭据的身份、解密、回滚与 cleanup 仍待最终安全记录，不在原 AVD 身份核验前推断旧 5 条凭据无法恢复。

最终 21 项 iOS cap sync 实际退出 0，17 public resources 与最终手机 dist 逐字节相等；没有 Xcode 原生构建、签名或 iPhone 安装证明。[实际同步凭据](acceptance/agentrouter-platform-sync/native/ios/final21-public-sync.json)。此前 Android 20 项阶段已实际通过 UI / PTY，但同一原 5554 后来掉线，临时 Bash / device 与监听器 cleanup 尚未闭合；新版 21 项 APK 原生 UI / PTY 必须重新绑定最终源码，不由旧 20 项结果代替。

最终 21 项官网 Pages Actions run `42999110960` / source `4cee1f8fe40ee8213f0f69a07682fa030f7c93ab` 实际 success，177 项 site/design/workflow 输入与最终 `71def643` 相等；26 个 live resources 全部 HTTP 200、字节与该 CI artifact 相等。[独立 CI / live 记录](acceptance/agentrouter-platform-sync/deployments/pages/final21-ci-live-independent-review.json)。首次冷加载 Crush 图片为 false 的[原 DOM 观察](acceptance/agentrouter-platform-sync/deployments/pages/visual/site-final21-live-dom.json)保留；等待加载后的[实际 DOM](acceptance/agentrouter-platform-sync/deployments/pages/visual/site-final21-live-loaded-dom.json)为 21 products / 42 images 全部 loaded，390×844 无横向 overflow。本地 dark 截图是[受控 root theme 属性](acceptance/agentrouter-platform-sync/deployments/pages/visual/site-final21-controlled-dark.json)，不宣称通过了产品主题设置流程；[dark 网格](acceptance/agentrouter-platform-sync/deployments/pages/visual/site-final21-dark-grid.png)与[live 手机宽度网格](acceptance/agentrouter-platform-sync/deployments/pages/visual/site-final21-live-phone-grid.png)已独立目视，国内/国际两项名称与相同品牌图标清晰可辨。此前 20 项 Pages 成功另保留[历史记录](acceptance/agentrouter-platform-sync/deployments/pages/89e577-live-ci-resource-verification.json)。

本轮 [当前线上独立验收](acceptance/agentrouter-platform-sync/deployments/pages/current-live/verification-summary.json)再次实际请求 26 个资源，均 HTTP 200，字节数与 SHA-256 匹配原 CI 快照；其中 15 个还有保留的 CI 字节可直接比较，其余 11 个使用已记录的散列与大小，没有声称执行新 CI。实际 DOM 为 21 项、42 张图片全部 loaded、24 份资产映射一致，桌面与 390 像素手机宽度无横向溢出，两版 WorkBuddy 名称分别显示。25 个图标引用为原始字节，17 个小 SVG 引用经 Vite 解码规范化后相等；初次 URL 字符串观察器错误拒绝 XML 引号编码的失败结果保留，没有改动图片。隔离页面正常关闭；默认 favicon 仍返回 404，没有宣称控制台零错误。

## 维护与并发上限

已实际清理 73 个完全 ancestor merged 远程 ref 和 3 个本地 merged 分支，保护 attached/current/PR refs，使用 lease 与 atomic，原控制进程退出 0、远程不存在复核通过。[逐项安全清理记录](acceptance/agentrouter-platform-sync/maintenance/merged-branch-cleanup.json)。

[当前 worktree 状态只读复核](acceptance/agentrouter-platform-sync/maintenance/current-worktree-cleanup-state-safe.json)确认四个旧 worktree 路径与对应本地分支均不存在，四份保留提交仍为 main ancestor，当前注册 worktrees 为 15。原临时删除凭据已缺失，本轮只证明当前状态，不重建历史删除操作或字节数，也不把 Linux worktree 逻辑文件大小计作 C 盘物理释放量。

C 盘精确已知删除量为 7,219,765,510 字节：36 个旧 PDB 4,846,878,720 字节，加 4,048 个 incremental 文件 2,372,886,790 字节。保留最新构建依赖、rollback、用户资料、SDK、AVD 与 keystore；没有进程信号。最终 native free space 快照为 8,641,257,472 字节；并行安装影响前后读数，不将差额冒充净释放量，初始未量化部分也不并入该精确合计。[安全维护 receipt](acceptance/agentrouter-platform-sync/maintenance/windows-approved-cache-cleanup.json)。

随后另删 3 个已安装并核对身份的官方 installer / VSIX cache，共 587,458,817 字节、skip 0，已安装产品 SHA 不变、用户 profiles 未改、无进程信号。首次 worker 退出 1 的 ledger 边界与恢复 worker 退出 0 分别保留原 source pins，不改成一次成功；[独立阶段安全 receipt](acceptance/agentrouter-platform-sync/maintenance/completed-official-installer-cache-cleanup.json)只计这 3 个明确文件，不把 native free-space 前后差额算为净释放。

本轮另有 [自有包装缓存清理](acceptance/agentrouter-platform-sync/maintenance/owned-package-preparation-cache-cleanup-safe.json)：只删除此任务 package-source 的 25 个文件、23,373,069 逻辑文件字节，目录已不存在，28 个准备包文件及 producer 输入、原 profile 未改，进程信号为 0。此逻辑字节数单独记录，没有实测 C volume 净空闲差额，不并入上述物理释放量结论。

本地 Codex `max_threads` 配置此前按主代理记录调为 8、原 0600 私人备份保留；旧阶段工具上限为 4，曾拒绝第 5 个 spawn。[本轮安全并发元数据](acceptance/agentrouter-platform-sync/maintenance/current-concurrency-safe.json)实际记录配置值 8 / 0600 及工具 9 个并发槽位（含主代理），已按职责分派独立验收任务。配置值、旧上限与当前工具上限分别记录，不据时间先后推断配置导致热扩容。此页不读取私人配置、key 或环境内容。

[安全证据来源](acceptance/agentrouter-platform-sync/sources/receipt-provenance.json)逐项记录选定原文件 SHA-256 与归档字节。后续原生/runtime/部署/Pages终态只追加新的 source epoch；历史失败和 pending 不被改成通过。[逐项需求验收审计](acceptance/agentrouter-platform-sync/sources/requirement-delivery-audit.json)分别列出 13 项用户要求、已证范围和剩余实际验收，不把当前部分完成说成所有平台交付完毕。[当前范围摘要](acceptance/agentrouter-platform-sync/summary.json)与[文件散列清单](acceptance/agentrouter-platform-sync/manifest.json)放在顶层，其余按职责归档。旧临时源文件缺失不改写归档原字节或原源码字段；[本轮独立源码与产物复核](acceptance/agentrouter-platform-sync/sources/audit/final71-current-source-bindings.json)核对当时 730 项源码和 80 项构建文件均相等，当前持久完整 Rust 纳入 `.cargo/config.toml` 后为 731 项联合输入。[初始归档完整性快照](acceptance/agentrouter-platform-sync/sources/audit/initial-archive-integrity.json)保留归档原散列、链接与秘密扫描边界：

| 目录 | 材料职责 |
| --- | --- |
| `sources/` | 官方图标来源、纠正历史、WorkBuddy 两版研究、receipt provenance |
| `tests/backend/`, `tests/frontend/` | 命令范围、源码输入、失败与重试判定 |
| `native/windows/`, `native/wsl/`, `native/android/`, `native/ios/` | 原生构建、安装身份、平台验收与同步边界 |
| `deployments/linux/`, `deployments/pages/` | 已执行服务部署、CI artifact 与 live 字节核验 |
| `maintenance/` | 分支、精确缓存清理、后续并发配置安全摘要 |
