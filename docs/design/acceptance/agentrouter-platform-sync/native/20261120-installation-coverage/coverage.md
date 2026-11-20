# Installed agent coverage audit — 2026-11-20

This audit reconciles **21 original products** (AgentRouter 17, legacy DeepSeek/Cursor/Antigravity 3, and WorkBuddy Global 1) and **6 additional products / 12 installation units**. It does not claim 54 currently reexecuted platform installations. Windows evidence is historical native evidence; the present worker has no `/mnt/c/Windows` or `/mnt/c/Users/han`, so Windows cannot be freshly observed here. WSL checks read selected public files only.

The companion `current-public-entry-audit-safe.json` contains 86 planned file checks: 45 currently accessible WSL files, 41 Windows paths unavailable to this runtime, and no missing accessible selected files. Two WSL Codex files changed from their selected historical baseline: package metadata now says `0.171.0`; no executable version was run. Other available files with a selected baseline retain that baseline. A hash establishes only the specific file checked.

| Product | Windows source-backed installation | WSL current public entry / supported scope |
| --- | --- | --- |
| Claude Code | CLI 2.1.322; extension 2.1.322 | CLI/package 2.1.322; Linux extension 2.1.322 |
| Codex | Historical CLI/package 0.167.1; current Windows unobserved | Current package metadata 0.171.0; prior executed-version 0.167.1 no longer proves current version |
| Gemini CLI | CLI/package 0.68.0 | CLI/package 0.68.0 |
| Qwen Code | CLI/package 0.27.2 | CLI/package 0.27.2 |
| OpenCode | CLI/package 1.19.5 | CLI/package 1.19.5 |
| OpenClaw | CLI/package 2026.11.3 | CLI/package 2026.11.3 |
| Pi | CLI/package 1.5.2 | CLI/package 1.5.2 |
| Crush | CLI/package 0.100.0 | CLI/package 0.100.0 |
| Hermes | Historical executed version 0.23.1+1899.g60f8163 | Existing Hermes CLI public entry, historical executed version 0.23.1; no new version run |
| Cline | VS Code extension 4.1.26 | Linux extension 4.1.26 and extra CLI 3.0.74 |
| Roo Code | VS Code extension 3.54.0 | Linux extension 3.54.0 |
| Kilo Code | VS Code extension 7.8.27 | Linux extension and extra CLI 7.8.27 |
| ZCode | PE product metadata 3.17.2.9661 | Linux desktop historical package 3.17.2, current selected entry/launcher unchanged |
| Craft Agents | PE product metadata 0.15.9.0 | Linux desktop historical package 0.15.9, current selected entry/launcher unchanged |
| WorkBuddy domestic | Tencent WorkBuddy 5.9.3.0 | Domestic WorkBuddy 5.6.2.40046327 entry/launcher unchanged; does not satisfy Global |
| Trae | Windows product metadata 3.6.13 | **Trae CN** 3.4.17 entry/launcher unchanged; Windows metadata alone does not establish identical distribution/region scope |
| Claude application | Official Store registration 5.11492.5.0 / app 5.11492.5, 2026-11-19 frozen proof | Official APT package 5.11492.5, 2026-11-19 frozen proof; GUI reached regional unavailability page; login/model availability unproven |
| DeepSeek | Official **DeepSeek Harness** 0.2.2-rc.1 installed; PE numeric version 0.2.2.0 | Official `@deepseek-ai/dsh` 0.2.2-rc.1 Harness Web/CLI; current package manifest and launcher present; Linux desktop unavailable in original official packaging evidence |
| Cursor | Existing Windows desktop PE metadata 3.29.15 | Native Linux desktop 3.29.15, selected entry unchanged; vendor recommends Windows application for WSL integration; native GUI not accepted by this audit |
| Google Antigravity | Desktop 2.31.0, historical public PE proof | **Antigravity Desktop** 2.31.0 native entry unchanged; distinct Antigravity IDE 26 is not installed or claimed |
| WorkBuddy Global | Distinct **WorkBuddyAI** 5.9.4.0 installed entry and historical registration | Open gap assigned to independent WorkBuddy Global owner; no Global Linux payload inferred from domestic application |

Additional product registration is a separate six-product scope. All six WSL user commands currently resolve to their recorded installation targets. Historical Windows wrapper version gates and WSL registered-command version gates each passed all six commands with actual exit code zero on 2026-11-12. The current audit confirms public file presence/binding; it does not reexecute these commands or claim complete current payload identity.

| Product | Registered command | Historical Windows/WSL executed version | Current WSL binding |
| --- | --- | --- | --- |
| GitHub Copilot CLI | `copilot` | 1.0.109 | `/home/han/.local/bin/copilot` → recorded Copilot target |
| Continue CLI | `cn` | 1.5.47 | `/home/han/.local/bin/cn` → recorded Continue target |
| Aider | `aider` | 0.86.2 | `/home/han/.local/bin/aider` → recorded venv target |
| Goose | `goose` | 1.59.0 | `/home/han/.local/bin/goose` → recorded native target |
| Amp | `amp` | 0.0.1794456033-gb2ff22 | `/home/han/.local/bin/amp` → recorded package target |
| Auggie | `auggie` | 0.37.1 | `/home/han/.local/bin/auggie` → recorded package target |

Auggie's Windows gate proves its native version command only. It does not establish provider support for a complete native Windows development workflow or model execution. No entry in either table proves authentication, quota, model capability, or login/service availability.

The historical Windows matrix's “Claude application unconfirmed” conclusion is superseded by the frozen `claude/20261119-official-installations` receipts. The early Windows legacy metadata's DeepSeek installer observation is superseded by `windows/deepseek-official-installed-20261112/corrected-verification-safe.json`: this identifies the official DeepSeek-signed installed application and current-user registration. DataElement's **DSH Desktop 0.13.0** is a different existing product and must not satisfy DeepSeek Harness installation.

Evidence selection and exact source digests are embedded in `current-public-entry-audit-safe.json`. All sources are repository-relative to the parent `native/` directory. No Windows Codex command, model/auth/quota operation, installer, GUI, private profile, credential read, process cleanup, or registration mutation occurred. Windows fresh observation and current executable versions for changed WSL Codex remain unproven by this audit; they must not be silently promoted from historical evidence or public metadata.
