DeepSeek Harness 官方 Windows 安装已于 2026-11-20 原生只读复核：完整 EXE 哈希与原安装一致、Authenticode 为 Valid、签名者为 Hangzhou DeepSeek Artificial Intelligence Co., Ltd.、HKCU 版本为 0.2.2-rc.1。PE 数字资源版本 0.2.2.0 与应用语义版本分别记录。原安装执行及其首次版本验证失败仍见 [原安装纠正记录](../deepseek-official-installed-20261112/corrected-verification-safe.json)。本次未重新安装。

本次原生 PowerShell 实际退出 0，发现 4 个仍在运行的原 DataElement DSH Desktop 进程，均匹配原安装映像。原 DataElement 0.13.0 注册和 EXE 仍在；未停止或发送信号。原共享目录的 Process/User/Machine DSH_HOME 覆盖均未设置，输入桌面可访问，但这些条件不证明现在可以安全启动第二个共享目录应用。

官方 GUI 本次启动次数为 0；没有窗口成功、正常退出或启动后原共享目录保留结论。准备好的 v2 控制器还有独立缺陷：原生 C# 编译失败，缺少 NodeProof、SnapshotResult.Nodes 和 OriginalPhysicalRoot 声明。最小修订见 [声明修订](prepared-retention-v3-declarations.diff)，仅准备在私有后继源码中；随后 Windows 卷出现 errno 5 I/O 错误，无法复制后继源码进行原生编译，未将该问题归因为 DeepSeek 应用失败。原 v2 源码未修改。

准备控制器在未识别界面或无法正常退出时会保留原句柄并等待自然结束，且其已有进程检查只覆盖官方映像，不能排除本次观察到的 DataElement 共享目录写入者。主窗口 Close 会隐藏至后台；活动、计划或未知工作可能使产品退出要求确认。本次没有盲目执行控制器、操作确认框、强制退出、替换 profile 或修改原数据。

明确的原生安装要求已有历史安装和本次身份复核支持。GUI 与启动后共享目录保留属于进一步的运行验证，目前仍未完成；无需为安装结论附加账号登录条件。后续安全短测需原共享目录应用均已自然结束、修正后的数据保留控制器通过原生编译、当前原目录基线和实际所属正常退出路径完成绑定。仅比较历史基线，或在别处新建 profile，均不能替代原目录启动后的保留证据。

[summary-safe.json](summary-safe.json) 界定结论；[current-preflight-safe.json](current-preflight-safe.json) 是原生安全 stdout 原字节，[执行回执](current-preflight-execution-safe.json) 保留实际退出码；[审阅记录](preparation-review-safe.json) 和 [来源映射](source-map.json) 保留来源摘要。未包含私有目录路径、环境值、凭据、用户文件名/内容、窗口标题或截图；未进行登录、模型调用或 Claude/Codex 进程操作。运行 `python3 validate.py` 可离线核对归档和这些结论，脚本不会启动 Windows 程序。
