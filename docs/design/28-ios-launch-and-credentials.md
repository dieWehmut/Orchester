# iOS 启动与凭据修补（2026-11-09）

本轮继续核对 [远程控制验收记录](27-remote-acceptance.md) 中尚未完成的 iOS 安装路径。已修复一个会阻止配对凭据插件工作的启动错误，并实际同步原生工程；以下结构检查不等同于 Xcode 编译或 iPhone 运行通过。

## 已修复的启动错误

`Main.storyboard` 的初始控制器为 `RemoteBridgeViewController`，该子类在 `capacitorDidLoad()` 注册 `CredentialsPlugin`。但是启用 scene 的 `SceneDelegate.willConnectTo` 随后创建新 window，并把 root controller 覆盖为普通 `CAPBridgeViewController`。新 scene 路径因此不会执行自定义注册，手机端第一次访问 `Capacitor.Plugins.Credentials` 时会缺失原生实现，无法正常配对或读取保存的令牌。

已将 [SceneDelegate.swift](../../apps/mobile/ios/App/App/SceneDelegate.swift) 中的 root controller 改为 `RemoteBridgeViewController()`。Storyboard 和 scene 两种入口现在选择同一个子类。核对本机安装的 Capacitor 8.6.0 源码可见：`CAPBridgeViewController.loadView()` 创建 bridge 后调用 `capacitorDidLoad()`，随后 `viewDidLoad()` 才加载打包页面，因此该注册位置能在客户端开始使用桥接之前执行。

## 原生工程同步结果

原先仅有初始 iOS 模板，`public/`、`capacitor.config.json` 和 `config.xml` 尚未生成；SPM 依赖也仍为模板中的 `from: "8.0.0"`。本轮实际运行：

```sh
pnpm --filter @orchester/mobile exec cap sync ios
```

命令返回 0，完成：

- 复制当前 `dist` 中的 3 个 Web 文件；逐字节核对与 `ios/App/App/public` 一致。
- 生成 bundle app ID 为 `dev.orchester.mobile` 的 Capacitor 配置及 Cordova 配置。
- 由 Capacitor CLI 重写 [Package.swift](../../apps/mobile/ios/App/CapApp-SPM/Package.swift)，固定原生依赖为 `exact: "8.6.0"`，与 npm 原生依赖版本一致。
- 同步没有覆写 `SceneDelegate` 中的自定义控制器修补。

`public/` 和上述配置文件属于 `.gitignore` 中的构建产物。新 checkout 或前端修改后必须先构建前端并重新 `cap sync ios`，不能只打开尚未同步的 Xcode 工程。

## 当前结构检查范围

已通过标准库解析和文件核对确认：

- `Info.plist` 可解析，scene delegate 指向 `App.SceneDelegate`；Storyboard 指向 `App.RemoteBridgeViewController`。
- `AppDelegate.swift` 与 `SceneDelegate.swift` 均由 Xcode target 的 Sources phase 引用，自定义控制器和插件声明在已编译文件中。
- `CredentialsPlugin` 的 `jsName` 是 `Credentials`，声明 `get` / `set` / `remove` 三个 Promise 方法，与前端约定一致。
- Keychain 查询按固定 service 与设备地址 key 定位条目；保存使用 `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`；未找到条目返回 `null`，删除缺失条目仍成功。此处仅核对 API 与源码，尚未在 iOS Keychain 中执行。
- 局域网说明 `NSLocalNetworkUsageDescription` 和允许当前用户指定 HTTP 电脑地址的 ATS 设置仍保留。运行时是否授予局域网权限、拒绝后界面如何恢复仍需真机验收。
- App icon 为有效 PNG，1024×1024 RGB，没有透明通道。

当前 Linux 环境没有 Swift / Xcode / iOS SDK，未运行 Swift 类型检查、链接、签名、模拟器或真机安装。不能据此声称已经生成可安装 IPA。

## 在 macOS 完成验证

在装有 Xcode 的 macOS 仓库中运行：

```sh
pnpm install --frozen-lockfile
pnpm --filter @orchester/mobile build
pnpm --filter @orchester/mobile exec cap sync ios
pnpm --filter @orchester/mobile exec cap open ios
```

先使用 Xcode 的 App scheme 构建运行到 iOS Simulator，核对冷启动没有 `Credentials` 未实现错误。也可以执行不需要设备签名的模拟器编译：

```sh
xcodebuild -project apps/mobile/ios/App/App.xcodeproj \
  -scheme App -configuration Debug -sdk iphonesimulator \
  -destination 'generic/platform=iOS Simulator' \
  -derivedDataPath /tmp/orchester-ios-build \
  CODE_SIGNING_ALLOWED=NO build
```

随后在 App target 的 Signing & Capabilities 中选择有效开发团队，将应用运行到 iPhone。实际设备验收需要：

1. 冷启动 App，输入可达电脑 IP 和电脑新生成的配对码，允许局域网权限；配对后加载 Agent 与任务历史。
2. 发送真实任务、收到完整输出并继续原会话，核对返回的会话与电脑记录一致。
3. 强制关闭并重启 App，仅点击已保存电脑即可读取 Keychain 凭据重连。
4. 在电脑撤销设备，确认 App 显示需要配对；新码重新配对后能覆盖旧凭据。
5. 从本机移除电脑，确认对应 Keychain 条目删除；断线期间电脑任务继续，重新连接后恢复输出游标。
6. 验证局域网权限拒绝和随后在系统设置启用的恢复流程。

这些命令与操作仍待在 macOS / iPhone 执行，后续须记录真实构建结果、设备版本及运行证据。
