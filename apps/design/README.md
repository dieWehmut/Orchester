# @orchester/design

Shared tokens and Vue 3 primitives for the Orchester WebUI, desktop shell and
project site. The package is source-only and has no router, store, i18n,
transport or fetch layer. It does not own network or business state.

## Install and import

The workspace already links the package. Import components and shared types
from the package root:

```ts
import {
  AppButton,
  AppDialog,
  AppField,
  AppInput,
  AppTabs,
} from '@orchester/design'
```

Load the opt-in reset and tokens from the CSS subpaths in the application entry:

```ts
import '@orchester/design/tokens.css'
import '@orchester/design/index.css'
```

Icons come from the maintained `@lucide/vue` package. Consumers can pass a
Lucide component into `IconButton` or a primitive slot:

```vue
<IconButton label="Open settings">
  <Settings :size="16" aria-hidden="true" />
</IconButton>
```

## Component boundaries

Primitives render accessible structure and emit user intent. They do not call
REST endpoints, open WebSockets, read application stores, or translate
business copy. The host application supplies localized labels, state and
side-effects.

`AppDialog` and `AppDrawer` own modal focus trapping, Escape handling, opener
focus restoration and body scroll locking while they are open. `AppPopover`,
`AppMenu` and `AppTooltip` are non-modal overlays and dismiss from Escape or
outside interaction according to their props.

## Keyboard contracts

| Primitive | Contract |
| --- | --- |
| `AppTabs` | Arrow keys move between enabled tabs; `Home` and `End` jump to the first and last enabled tab. |
| `AppSegmentedControl` | `ArrowLeft` and `ArrowRight` move between enabled options; `Home` and `End` jump to boundaries. |
| `AppMenu` | `ArrowUp` and `ArrowDown` move through enabled items; `Home`, `End` and `Escape` are supported. |
| `AppDialog` / `AppDrawer` | `Tab` and `Shift+Tab` stay inside the focus trap; `Escape` closes and restores focus. |
| `AppPopover` / `AppTooltip` | `Escape` dismisses an open surface; tooltip content appears on pointer or keyboard focus. |
| `AppButton` | A busy button keeps focus, exposes `aria-disabled` and suppresses duplicate activation. |

All icon-only controls require an accessible `label`. Reduced-motion media
preferences are honored by animated feedback primitives.

## Local checks

```sh
pnpm --dir apps --filter @orchester/design typecheck
pnpm --dir apps --filter @orchester/design test
```
