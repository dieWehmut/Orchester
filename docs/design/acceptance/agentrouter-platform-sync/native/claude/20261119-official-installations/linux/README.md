# Claude Linux 官方安装与原生窗口证据

2026-11-16 在 Ubuntu WSL 中实际安装了官方 claude-desktop 5.11492.5（amd64）；2026-11-19 复核了系统包完整性，并运行一次 UID 1000 的独立新 profile WSLg 窗口测试。安装、原生窗口和正常退出均有实际回执。窗口呈现官方地域限制页面，未进行登录或模型调用，在线功能可用性没有得到证明。

| 日期 | 已验证行为 | 主要回执 |
| --- | --- | --- |
| 2026-11-16 | 官方 InRelease 签名有效，amd64 Packages 的哈希和选定 deb 的哈希、大小形成绑定链 | [签名核验](evidence/20261116/signed-repository-verification-safe.json)、[完整下载](evidence/20261116/linux-deb-resumed-download-safe.json) |
| 2026-11-16 | APT 实际安装退出 0；dpkg 状态为 install ok installed，版本 5.11492.5；实际安装 ELF 与包内 ELF 哈希一致 | [安装回执](evidence/20261116/install-result-safe.json) |
| 2026-11-16 | 已安装 ELF 的 --version 实际退出 0、输出 5.11492.5；4 个持有 pidfd 对象均结束 | [版本回执](evidence/20261116/native-version-result-safe.json) |
| 2026-11-19 | dpkg -V 退出 0、差异 0；1200×800 窗口真实 mapped，XGetImage 成功；产品 Ctrl+Q 后退出 0，16 个原 pidfd 全部结束，当前原生进程 0 | [GUI 回执](evidence/20261119/native-gui-result-safe.json)、[最终复核](evidence/20261119/final-native-linux-verification-safe.json) |

[签名公钥](repository/key.asc)、[InRelease](repository/InRelease) 和 [Packages](repository/Packages-amd64) 保留官方原始下载字节。来源是 [官方 Linux 文档](https://code.claude.com/docs/en/desktop-linux) 及回执中记录的 downloads.claude.ai 官方仓库。签名指纹为 31DDDE24DDFAB679F42D7BD2BAA929FF1A7ECACE。InRelease 的签署日期为 2026-11-14，有效期至 2026-11-21；“未过期”是 2026-11-16 的历史观察，不是永久有效声明。

deb（183335272 字节）、安装 ELF、ASAR、原始日志、截图和所有 profile 目录均未归档。[省略制品摘要](omitted-artifacts-safe.json) 只记录安全哈希、大小和省略原因；ASAR 包内快照的大小是在本次归档时补充读取元数据所得，明确没有冒充原安装回执字段。离线脚本可重新验证保留的签名与索引字节及历史回执间的一致性，不能重新计算未包含的安装包、已安装程序或截图本体。

[来源映射](source-map.json) 区分 original-bytes、派生选择和派生源码导出。安全 JSON 原字节保持不变；局部 .gitattributes 使用 -text 保持提交后的原字节。controllers 中的 .py.txt 文件是历史审阅文本，本次归档未运行它们：安装和版本控制器代码字节未变，GUI 控制器仅替换两个本地账号字面量为 <LOCAL_ACCOUNT>，原 SHA256 和大小仍保留。原版本控制器含超时信号回退分支，实际版本测试回执的 signalCleanupActions 为空；GUI 回执也没有信号清理动作。

原 GUI 回执误把 /usr/lib/claude-desktop/version 的 Electron 运行时 45.2.1 标为应用版本。[原回执](evidence/20261119/native-gui-result-original-safe.json) 原字节保留；[纠正回执](evidence/20261119/native-gui-result-safe.json) 的 metadataCorrection 记录原 SHA256、纠正时刻及依据（已安装 ASAR package.json 和 dpkg），没有重跑窗口测试。应用版本是 5.11492.5。

在此目录运行以下命令，需 Python 3、gpg 和 gpgv；仅使用临时 keyring，不联网、不安装、不启动应用、不读取用户 profile：

~~~bash
python3 validate-linux-archive.py
~~~

本目录自身文件哈希见 [制品索引](artifact-index.json)。该索引本身不自哈希，外层总归档可以对它再做校验。

A derived complete GUI metadata-change audit additionally lists every changed, removed and added key between the retained original and corrected source receipts. Besides the package/runtime version label, the original userProfileReads=0 label became testControllerUserProfileReads=0; this counts controller reads and does not assert that the product never reads its fresh profile. Added fields corroborate dpkg identity, annotate the own-image regional page and narrow the success/availability scope. Original native launch, image, quit and process facts remain unchanged; neither source receipt was edited during this archive assembly.
