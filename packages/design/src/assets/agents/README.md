# Agent marks

The WorkBuddy, Qwen Code and OpenClaw marks were retrieved from their official
sites or repositories on 2026-11-11. Each SVG is retained unchanged and used in
both themes to identify the corresponding tool. The assets load locally through
the shared `AgentToolIcon.vue` and `AGENT_IDENTITIES` metadata. Vite bundles the
same source assets for WebUI, desktop, mobile, and the static site, including
each application's configured base path.

| Asset | Official source | SHA-256 |
| --- | --- | --- |
| `workbuddy.svg` | [WorkBuddy site's favicon](https://assets.workbuddy.cn/web/workbuddy/5611ca0b566ae70e862617fecf6f3c8b05e5515a/assets/logo.svg), linked by [WorkBuddy](https://www.workbuddy.cn/) | `10ccbc099e7bc3d5ff12c315c4edd7b21386aa7166c1ec19997b3d55010d5c7d` |
| `qwen.svg` | [Qwen Code desktop mark](https://github.com/QwenLM/qwen-code/blob/e19e1ebe7881960ab2e3319d677c491254600571/packages/desktop/bootstrap/qwen-code-logo.svg) | `24156c849c9124505e13b38c6583a9d5d85a1f57d996c324d967f480a4d88696` |
| `openclaw.svg` | [OpenClaw Control UI favicon](https://github.com/openclaw/openclaw/blob/44a2948a9eaf4345069d4b652ae382d10dd08f62/ui/public/favicon.svg) | `74e1fb7542675f68aafe502576f7c857c3f3620d07095e910515e3c05d62ab2e` |

The Qwen Code repository's Apache 2.0 license is preserved in
[`qwen-LICENSE.txt`](qwen-LICENSE.txt). OpenClaw's MIT license is preserved in
[`openclaw-LICENSE.txt`](openclaw-LICENSE.txt). WorkBuddy's mark identifies Tencent's
WorkBuddy product; the CDN name does not make it the CodeBuddy product.
