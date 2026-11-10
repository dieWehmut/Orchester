# 终端关闭后的最后输出回放验证

2026-11-10，在独立 worktree、分支 `fix/mobile-terminal-final-replay` 验证 main `7ba9065` 的关闭竞态。测试使用 `FakeSocket` 与 HTTP fixture，没有连接用户后台、启动模型、浏览器、Android 工具或 PTY。

## 复现与修复

- HTTP DELETE 或关闭轮询 GET 先返回 `closed`，WS 最后输出随后到达：旧实现释放连接，丢失输出。
- HTTP 确认关闭后 WS 断开：旧实现认为已收到退出消息，不再补读。
- WS 已收到 `exit` 后，迟到的 HTTP `running` 快照：旧实现覆盖退出状态。
- 重新打开 `closed` 与 `exited` 会话，服务器连续发送 `ready`、历史输出、`exit`、close：修复前后均可回放。此测试未复现原生设备连接超时，也不证明该超时已解决。

客户端现在把 HTTP 进程终态与 WS 输出排空分别处理，保留连接或按渲染序号重放到 `exit`；WS 退出后忽略迟到 HTTP 回调。

## 命令与结果

以下命令在 `apps/mobile` 执行：

```sh
pnpm exec vitest run --maxWorkers=2 test/terminal.test.ts
pnpm exec vitest run --maxWorkers=2
pnpm run typecheck
pnpm run build
```

- [修复前终端测试](terminal-red.log)：23 项中新增竞态的 4 项失败，19 项通过。
- [修复后终端测试](terminal-green.log)：23 项通过。
- [移动端全部测试](mobile-tests.log)：8 个文件，119 项通过。
- [类型检查](typecheck.log) 与 [生产构建](build.log) 通过。
- `git diff --check` 通过。

原生 closed 会话在重载系统上的持续 connecting/reconnecting 尚需独立设备复测；此改动仅修复已由测试确认的关闭与输出排空竞态。
