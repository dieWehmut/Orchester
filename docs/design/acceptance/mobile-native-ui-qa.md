# Journey: Android 安装版远程目录与 Agent 工作栏

日期：2026-11-09。环境、品牌改动、构建来源及完整限制见 [手机原生验收](../31-mobile-native-branding.md)。本记录全部来自安装在 Windows `emulator-5554` 的 APK，SHA-256 为 `bf617dc6209d23726ab80eb17c5ce16f936d2211568f3788e410c55afe4c7af2`。

两台测试电脑是不同端口、不同工作区、不同状态目录的真实 Rust listener；`qa-echo` 只执行隔离 Python CLI。服务均上报相同电脑名，所以使用 `17347` / `computer-1` 和 `17348` / `computer-2` 区分。测试没有调用模型。

## Results

每次操作先用 Windows SDK `adb.exe -s emulator-5554 shell uiautomator dump` 导出当前 UI 树，再以节点 bounds 中心执行 `shell input tap`。`q.tap()` / `q.select()` 是 `/tmp/orchester_native_qa.py` 与 `/tmp/orchester_native_qa_resume.py` 的节点定位辅助函数；截图由 `exec-out screencap -p` 直接保存。首次启动 WebView 的 UI 树曾只包含容器，按当前截图点击主导航后恢复；偶发 UiAutomation / Binder 导出错误经重试恢复。

### Action: 从第二台电脑的真实目录新增项目 ✅

- **Commands**: 项目页选择 `17348` → 点击“选择目录” → 点击该电脑允许的根目录 → `q.select(text='project-beta 文件夹')` → `q.select(text='使用此文件夹')` → 返回项目页重新查询。
- **Comment**: 新项目自动打开 Agent 选择页；项目页同时显示第二台电脑的 `project-alpha` 和 `project-beta`，路径归属 `computer-2`。
- **Screenshot**: [选择真实项目目录](mobile-native-project-folder-android.png)、[查询新增后的两个项目](mobile-native-project-added-android.png)。

### Action: 从真实电脑目录选择文件并提交附件 ✅

- **Commands**: `q.tap(text='选文件')` → 滚动目录 → `q.tap(text='src 文件夹')` → `q.tap(text='hello.py 文件')` → 聚焦任务输入 → `q.type_focused('qa-beta-attachment')` → Android Back 收起键盘 → `q.tap(text='发送')`。
- **Comment**: UI 显示 `hello.py` 附件，接受任务后草稿和附件清空。真实结果同时包含 `computer-1/project-beta` cwd 和该项目 `src/hello.py` 的绝对参考路径。
- **Screenshot**: [目录中的真实文件](mobile-native-file-browser-android.png)、[附件任务结果](mobile-native-attachment-output-android.png)。

### Action: 移除尚未发送的文件附件 ✅

- **Commands**: 再次从第一台电脑的 `project-beta/src` 选择 `hello.py` → `q.select(desc='移除文件 hello.py')`。
- **Comment**: 附件名称与移除按钮消失，任务输入保留为空，未发送新任务。
- **Screenshot**: [移除前](mobile-native-attachment-before-remove-android.png)、[移除后](mobile-native-attachment-removed-android.png)。

### Action: 在两台电脑、两个项目的双栏中独立提交任务 ✅

- **Commands**: 项目页设备选择器选择 `17348` → `q.select(text='打开 Agent 栏')` → `q.select(contains='qa-echo')` → `q.select(text='双栏')` → 分别输入 `qa-double-left` / `qa-double-right` → `q.select(text='发送', idx=0)` → 核对右栏草稿保留 → `q.select(text='发送', idx=1)`。
- **Comment**: 左栏请求与结果属于 `17347` / `computer-1/project-beta`；右栏属于 `17348` / `computer-2/project-alpha`。两侧 SQLite 任务记录和真实 cwd 一致，均为 `completed`。
- **Screenshot**: [两栏草稿](mobile-native-double-drafts-android.png)、[两栏真实输出](mobile-native-double-output-android.png)。

### Action: 在任务选择器中切回较早任务 ✅

- **Commands**: `q.tap(text='查看任务 qa-beta-resumed')` → 从当前 UI 树取 `qa-beta-attachment … 已完成` 选项中心 → `q.tap_node(node)`。
- **Comment**: 当前勾选与任务状态正确；选中较早附件任务后重新读出其原始输入和输出。
- **Screenshot**: [原生任务选择器](mobile-native-task-picker-android.png)。

### Action: 继续已完成任务的会话 ✅

- **Commands**: `q.tap(text='继续会话')` → 核对“继续当前会话”提示 → 输入 `qa-beta-resume-verified` → `q.tap(text='发送')`。
- **Comment**: 测试 manifest 的续接模板实际传入 `{session_id}`；真实 CLI 输出 `resume=qa-session`，cwd 仍为原来的 `computer-1/project-beta`。这是测试会话参数验证，未调用真实模型恢复上下文。
- **Screenshot**: [实际续接参数回显](mobile-native-resume-verified-android.png)。

### Action: 删除已结束任务记录 ✅

- **Commands**: `q.tap(text='删除记录')` → 只读 SQLite `remote_runs` 查询。
- **Comment**: 当前输出清空；被删任务 `qa-beta-resume-verified` 从持久记录中消失，附件任务和较早续接记录保留。
- **Screenshot**: [删除后的当前栏](mobile-native-delete-android.png)。

### Action: 停止正在运行的隔离进程 ✅

- **Commands**: 输入并发送 `qa-stop-beta` → 核对运行状态 → `q.tap(text='停止任务')` → 查询持久状态和 `/proc` 进程列表。
- **Comment**: 仅临时 Python 脚本对 `qa-stop` 输入等待 90 秒，为操作提供时间。点击后 UI 变为“已停止”，服务端状态为 `cancelled`，已产生的输出保留；后续核对活动 `qa_echo.py` 进程数量为 0。
- **Screenshot**: [运行中](mobile-native-stop-running-android.png)、[停止后](mobile-native-stop-completed-android.png)。

### Action: 强制结束再打开应用，恢复设备与工作栏 ✅

- **Commands**: `adb.exe -s emulator-5554 shell am force-stop dev.orchester.mobile` → `shell am start -n dev.orchester.mobile/.MainActivity` → 按当前截图点击“任务”导航。
- **Comment**: 两台服务自动重连，两栏对应设备、项目和双栏布局恢复，没有输入新配对码。
- **Screenshot**: [恢复后的两栏](mobile-native-restored-panels-android.png)。

### Action: 六套主题逐屏视觉核对 ✅

- **Commands**: 主题页依次点击“珊瑚、玫瑰、海盐、森林、靛蓝、琥珀”；在每个主题中实际导航、打开选择器、浏览目录和读取任务详情。
- **Comment**: 每个主题保留 16 张原始 1080×2400 截图。页面背景、主次按钮、勾选、连接状态、长路径换行和底部导航逐屏核对；设备配对输入为空。联系图仅缩小并加文件名标签，用于并排复核；原始 PNG 保留。
- **Screenshot**: [六主题逐屏截图索引](mobile-native-themes/README.md)。
