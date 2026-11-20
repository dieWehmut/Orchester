# WorkBuddy 国际版 Linux 发布包核验（2026-11-20）

官方国际版原生 Linux 安装仍未完成。此次检查没有取得可用的官方国际版 Linux 包，不能验证其 ELF 身份或实际 Linux 配置命名空间。国内版未用作替代，现有国内版可执行文件 SHA-256 与此前核验值一致。

本次新增有效证据：官方文档已更新至 `main@432ddbdc516`（构建时间 `2026/11/19 19:21:55`）。国际版更新日志仍提及 Linux 安装/更新可靠性，但 Quickstart、FAQ、Changelog 没有直接 Linux 包链接；1,954 项元数据中没有 `workbuddy_download-history.md`。单独的国内版历史页有 303 个包链接，其中 Linux 包链接为 0。国际版首页与三个下载相关资源保持此前字节内容。

| 来源明确的检查 | 本次结果 |
| --- | --- |
| 国际版 x64 / arm64 的 deb / rpm，以及 UOS / Kylin x64，共 6 个更新渠道 | HTTP 200，仍返回 `5.5.2.37849279` |
| 上述渠道原样返回的 6 个官方 CDN 包地址 | Range GET 全部 HTTP 404 |
| 源码支持的企业 AppImage 更新接口 | HTTP 400，未找到 upgrade config |
| `version=5.9.4`、`version=5.5.2.37849279` | HTTP 204；此版本参数不是历史包选择器 |
| 来源支持的历史客户端版本 `version=5.2.7` | HTTP 200，仍返回同一个失效的 x64 deb 地址 |

`current-global-linux-result-safe.json` 是本次汇总；HTTP、接口、文档及源码绑定分别记录于同目录的 `current-*-safe.json` 和 `public-source-bindings-safe.json`。两次初始传输故障及其成功重试分别保留，未将传输错误写成 HTTP 404。

`expanded-20261116/` 完整归档此前扩展调查的安全 JSON，包括额外渠道、文档历史链接、版本敏感接口以及国际版 Windows 公共源码绑定。其原始清单中 10 个收据的字节数与 SHA-256 均已复核，详见 `historical-archive-integrity-safe.json`。Windows 公共源码只用于发现真实更新渠道，不作为实际 Linux 包身份或配置隔离的证明。

此次未执行应用、安装器、Windows 应用命令、登录或模型调用，未读取或写入任何私有用户配置。结论仅覆盖已观察到的公开渠道和版本，不主张所有历史版本、所有渠道均不存在，也不主张官方永久不支持 Linux。后续需要来源明确且可下载的官方国际版 Linux 包，再核验身份和隔离安装。
