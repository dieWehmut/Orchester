# Orchester 手机端

Vue + Capacitor 客户端。手机输入电脑 IP（省略端口时默认 7337）与电脑显示的配对码，即可连接多台电脑，选择电脑上的真实项目目录，在独立工作栏中管理 Agent 与任务。电脑需先启动远程服务；部署参见 [远程控制设计](../../docs/design/26-remote-control.md)，协议见 [OpenAPI](../../docs/remote.openapi.yaml)。

## 使用

1. 在电脑启动远程服务，让手机能通过局域网或 VPN 访问监听地址。
2. 手机输入 IP、手机名称和配对码。配对码一次性有效，有效期 10 分钟；每次配对新手机需在电脑生成新码。
3. 在“项目”页选择设备和项目，或浏览电脑目录后点击“使用此文件夹”添加项目；再选择可用 Agent，打开工作栏。目录必须已经存在并位于电脑服务允许浏览的根目录内；服务的 `--allow-root` 可重复指定额外根目录。
4. 在“任务”页输入任务并查看该栏回复。默认双栏，可在任务页或设置页切换单栏；每栏固定对应一个设备、项目和 Agent，草稿、附件、续接和输出独立，最多打开 12 栏。
5. 点击工作栏的“文件”或“选文件”，浏览这台电脑上的文件并附加到任务。提交时将文件路径列入 prompt，由电脑上的 Agent 使用；手机不上传文件内容。
6. 已结束任务若具有会话 ID、项目匹配且 Agent 可用并声明 `supports_resume`，可点击“继续会话”。运行中任务可停止，终态任务可删除。关闭工作栏只停止手机上的轮询，电脑上的任务继续执行；断网时输出轮询会重试。
7. 可以保存多台电脑并同时连接；启动时尝试重连已保存设备，恢复可用的项目与 Agent 工作栏。访问令牌有效期 30 天，过期或被撤销后需重新配对。
8. 从项目页打开交互终端，在电脑上的真实 shell 中输入命令。返回项目页会断开页面连接，重新进入可补读仍存活会话的输出；点击“关闭会话”并确认后结束电脑进程。

底部导航为“控制台 / 任务 / 项目 / 设置”：控制台按设备显示连接状态、项目数、运行中数量和任务数；任务页同时展示工作栏与按设备分组的任务记录；项目页浏览与选择真实目录；设置页管理设备、单栏/双栏和主题。六套暗色主题为珊瑚、玫瑰、海盐、森林、靛蓝和琥珀，选择后立即应用并保存偏好。

界面复用桌面设计包的字体：默认中英文使用随安装包附带的霞鹜文楷，代码与终端使用同一等宽字体栈。字体偏好和常规/中等字重沿用桌面的 `orchester:ui-font`、`orchester:content-font`、`orchester:ui-font-weight`、`orchester:content-font-weight`；偏好分别保存在当前客户端。终端在字体加载完成后初始化网格，加载失败仍可使用系统回退字体。等宽栈中的系统字体由各平台提供，因此不同系统的终端字形可能不同。

Android 凭据插件通过 Android Keystore 密钥加密保存令牌；iOS 工程包含 Keychain 实现。浏览器预览仅在页面会话内存中保存令牌，刷新页面后需重新配对。设备地址和名称存于 `orchester.devices`；工作栏仅保存 `{hostAddress, projectId, agent}` 到 `orchester.panels`，单栏/双栏保存到 `orchester.panel-columns`，主题保存到 `orchester.theme`。这些偏好不保存令牌、任务草稿、附件列表、会话 ID 或输出；恢复工作栏时重新读取电脑项目和任务。“忘记”删除该设备本机凭据及工作栏偏好；服务端令牌撤销由电脑设备管理或 `DELETE /remote/v1/device` 完成。

HTTP 用于可信局域网或 VPN。不要直接把 HTTP 服务暴露到公网。手机只控制电脑，模型登录与 CLI 安装在电脑完成。

## 本地开发与安装工程

在仓库根目录运行：

```sh
pnpm install
pnpm --filter @orchester/mobile dev
pnpm --filter @orchester/mobile test
pnpm --filter @orchester/mobile typecheck
pnpm --filter @orchester/mobile build
pnpm --filter @orchester/mobile sync
pnpm --filter @orchester/mobile android
# macOS + Xcode:
pnpm --filter @orchester/mobile ios
```

Android 工程需要 Android SDK/JDK；iOS 工程需要 macOS/Xcode 与相应签名配置。上述命令构建 Web 资源并打开原生工程，不能等同于已经生成、签名或真机验证安装包。

## 请求与恢复

输出通过游标分页，每页最多 200 条。即使状态已终止，也必须继续请求直到返回空页。当前页面保留游标，短暂断网重试从上次已收到的位置恢复；重新打开任务从头读取。

客户端为发送请求附加幂等键；请求失败后，同一连接、同一内容再次提交会复用该键。更改任务或重启页面会生成新键，因此响应丢失后应先查看任务记录。后端在任务记录保留期间记住幂等键，相同键及内容返回已有任务，不同内容返回 409。删除终态任务同时结束其幂等保留窗口。

字体对齐版 `6278e3b` 的源码、原生资源和交付门禁见[桌面与手机交付记录](../../docs/design/38-desktop-mobile-final-delivery.md)。手机测试覆盖地址校验、认证请求、幂等重试、分页尾页、断线游标、过期重配、多设备/多项目/多 Agent 工作栏隔离、附件与续接、晚到响应、Android 输入、子页返回、终端关闭回放和字体加载。HTTP/凭据 fixture 与 mock xterm 的通过结果只证明对应客户端逻辑；原生桥接、真实 PTY 和安装另有独立证据。

Android 16 旧参考版的真实模型首轮、续接和强退重连见[安装版验收记录](../../docs/design/27-remote-acceptance.md)。API 37 四导航版本的双设备、目录/附件、任务操作与六主题验收见[历史原生记录](../../docs/design/31-mobile-native-branding.md)；后续真实终端输出补读、明确关闭、六主题和 Android 返回验收见[终端及返回记录](../../docs/design/36-mobile-native-back-acceptance.md)。共享字体 APK 的安装哈希、实际 WebView 字体加载和原生桥接结果见[新版交付记录](../../docs/design/38-desktop-mobile-final-delivery.md)。界面说明见[手机端参考 UI](../../docs/design/29-mobile-reference-ui.md)。iOS 工程尚未在 macOS/Xcode 编译或安装，物理 Android 手机网络与安装验收也未完成。
