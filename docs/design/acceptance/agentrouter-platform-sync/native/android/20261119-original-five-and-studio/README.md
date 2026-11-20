# Windows Android 原五条凭据与 Android Studio 验收

2026-11-19 的实际原生验收已完成。本归档只保存状态、数量、摘要及公开源码；原凭据、密文、设备名称/地址/ID、localStorage 值、用户截图和原始 IDE 输出均不复制。绝对本机路径已替换为模板；`manifest.json` 记录公开字节 SHA-256/大小、原始来源 SHA-256/大小、派生或省略方式。

原 Windows AVD 的已有 AES 别名验证通过，未生成密钥、重配对、重置或重装应用。原五条凭据 GET 与解密均为 5 成功、0 失败；保存的 7337 来源仅作一次显式信息请求，返回 200 且名称匹配。实际产品控制台显示 21 个规范 Agent、42 张已加载的明暗图标；当前仓库项目的自有终端完成输入、输出和正常 `exit`。48 个 ADB creator 均已实际终止，其中唯一退出 1 是预期的 Dex 不可写探针，其余退出 0；自有 forward、Dex 和产品 Back 退出均已验证。

原 XML 的前后 SHA-256 都等于历史原五条摘要。保留的六条 XML 历史副本在内存移除唯一额外元素后得到同一摘要；未恢复或公开原 XML。最早 metadata 整文件摘要 `e6e6b72a2617b84821736d55a3e2f39cd7c5f5f9747df1e87571653c4620f46c` 对应文件已缺失，因此不宣称重新验证该整文件。当前全部四项持久化 key/value 的排序摘要、五条设备记录和主题，与已锁定历史 QA 副本仅保留原五条设备后的派生摘要匹配；补充存储检查的 before/after 也完全一致，18 个 ADB creator 全部退出 0，存储写入为 0。

Android Studio 实际分析 `CredentialsPlugin.java` 退出 0，错误 0、警告 0、提示 3。help 与分析两个 Java creator 均退出 0；原 Studio 的 PID、birth、镜像及会话身份在分析前后通过句柄验证。CLI stdout 未报告 Studio PID，归档保留 `stdoutReportsStudioPid=false`；绑定依据是显式 PID 参数与原句柄检查。

各项实际运行日期与范围另见 [requirements-coverage-safe.json](requirements-coverage-safe.json)。同一个 Studio 进程在 2026-11-16 完成实际项目同步，耗时 44.193 秒；当日另行执行的 `:app:testDebugUnitTest` 为 **NO-SOURCE**、测试报告 0，不能记作单元测试通过。2026-11-11 的正确 Windows AVD 上另有 4 项原生 instrumentation 测试通过，安装 APK SHA-256 与本次读取完全相同；测试由限定 SDK 调用，未从 IDE Run 执行。补充的历史回执保留上述边界和既有失败码。

本次 2026-11-19 自有 PTY 只观测输入、输出和正常 `exit`，没有重测 resize。2026-11-12 的历史真实键盘触摸使原生视口从约 914 缩至 578，后端同一个自有 PTY 从 45×28 变成 45×8，未注入浏览器仿真尺寸；该次 resize 使用自有临时 **7339** 后端。本次没有据此宣称 Linux7337 的当前 resize 通过。2026-11-16 的 Linux 实际内核重启/自启回执与本次 Linux owner 回执的 boot ID、listener PID/birth、controller 和 unit 相同；Windows 使用已固定的 Linux owner 回执，本身没有独立复核 Linux 进程身份。

原 manager 接收精确绑定的正常退出控制。server、head 和 QEMU 实际退出 0，原 creator/query 句柄及四个私有端口已释放，未强制终止。manager 的实际退出码保留为 **1**：它保留了 2026-11-17 在 QEMU 镜像摘要读取时发生的历史 `MemoryError`，不能改写成退出 0。2026-11-19 06:22:07 UTC 的 Win32 复核仍能打开原五个进程的内核对象，birth 匹配且句柄全部为 terminal；`originalProcessAbsent` / `allFiveOriginalNativeProcessesAbsent` 表示原进程已不存活，不表示内核对象不存在。四端口当前 TCP 行数为 0。

检查公开归档：

```sh
python3 -I -B validate-archive.py
```

校验脚本只读取归档，核对完整盘点、字节哈希、来源引用、脱敏约束、Python 语法和实际验收断言。它不产生 pycache，不写文件、不 spawn、不联网、不连接设备，也不重新执行原生验收。`sources/` 中的运行源码与路径脱敏模板用于审查历史方法，**不可直接运行**。13 份模板在入口处明确禁用原生执行；其余公开 CDP、JavaScript、Java 与固定摘要的诊断 Dex 是原字节副本，Dex 仅包含静态别名检查辅助代码。

归档复核补强了逐文件 provenance 公共字节绑定、原始来源引用的角色绑定、AVD/安装 APK/Linux owner 身份和历史同步/测试范围断言。哈希清单用于发现字节和引用不一致，不是外部签名；公开脱敏文件不能独立重建被省略的私密原件。原始回执未改写，新增历史副本只做逐字段声明的路径脱敏及 JSON 序列化；2026-11-19 原验收中的 `MemoryError` 与 manager 退出 1 均保留。

自动启动重连请求没有在本次单次显式请求范围内测量；安装证书由已知 APK 字节完全一致绑定，本次没有独立重新解析证书。历史收尾故障、metadata 整文件缺失和这些测量范围均保留在原证据或明确标注的派生证据中。

UTC 校验兼容本机 Python 3.10 与 3.14。Windows 七位秒小数只在临时日期比较字符串中转换为六位；原回执 UTC 字符串、exactBirth 和 FileTime 身份值不修改。两个解释器都只读取归档：

```sh
/usr/bin/python3 -B validate-archive.py
python3 -B validate-archive.py
```
