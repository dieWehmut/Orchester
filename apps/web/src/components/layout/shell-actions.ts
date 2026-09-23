/**
 * What the app chrome can do, and where it is available.
 *
 * The title row sits above the routed view: it is drawn by the shell, and the
 * panes its menus act on belong to the view underneath. So the row does not
 * assume a pane exists - the mounted surface *registers* the actions it can
 * answer, and the row asks. Availability is then a fact about what is mounted
 * rather than a list of route names the chrome would have to keep in step with
 * the router, which is the same rule the shortcut registry already follows for
 * keys.
 *
 * Registering happens on the view's own lifecycle, so a row can only be offered
 * while something would answer it, and a route with no rail draws the toggle
 * disabled with its reason rather than moving a boolean nothing can see.
 */

import { onUnmounted, ref } from 'vue'

export type ShellActionId =
  | 'rail.toggle'
  | 'inspector.toggle'
  | 'prompt.focus'
  | 'prompt.clear'
  | 'companion.toggle'
  | 'close-tab'

export type ShellAction = () => void

/**
 * What a toggled surface says about itself.
 *
 * The chrome draws the control; the surface the control toggles is what knows
 * whether it is showing. The rail is the case that needs it: at one viewport
 * width it is a column, at another a drawer, and only the shell that drew it
 * can say which of those the reader is looking at.
 */
export type ShellActionExpanded = () => boolean

const actions = new Map<ShellActionId, ShellAction>()
const expanded = new Map<ShellActionId, ShellActionExpanded>()

/**
 * A counter rather than a reactive map.
 *
 * Vue cannot track a `Map` that the module owns, and the row has to re-render
 * when a route mounts or unmounts a pane. The counter is the same device the
 * shortcut registry uses: the surface watches it, and the data lives here.
 */
const version = ref(0)

function notify(): void {
  version.value += 1
}

export function registerShellAction(id: ShellActionId, action: ShellAction): () => void {
  actions.set(id, action)
  notify()
  return () => {
    if (actions.get(id) === action) {
      actions.delete(id)
      notify()
    }
  }
}
/**
 * Registers what the action's surface reports as its state.
 *
 * Optional rather than implied by the action: the prompt actions and a close
 * toggle nothing, so they register no reading and the control that reaches
 * them is drawn as a plain button rather than as a switch.
 */
export function registerShellActionExpanded(
  id: ShellActionId,
  read: ShellActionExpanded,
): () => void {
  expanded.set(id, read)
  notify()
  return () => {
    if (expanded.get(id) === read) {
      expanded.delete(id)
      notify()
    }
  }
}

/** Whether the surface that answered the action is showing, when it says. */
export function shellActionExpanded(id: ShellActionId): boolean | undefined {
  return expanded.get(id)?.()
}

export function hasShellAction(id: ShellActionId): boolean {
  return actions.has(id)
}

/** Runs the action, and reports whether anything was there to run. */
export function runShellAction(id: ShellActionId): boolean {
  const action = actions.get(id)
  if (!action) return false
  action()
  return true
}

/** The subscription that makes `hasShellAction` usable from a render. */
export function shellActionsVersion(): typeof version {
  return version
}

export function clearShellActionsForTests(): void {
  actions.clear()
  expanded.clear()
  notify()
}

/**
 * Registers an action for the lifetime of the calling component.
 *
 * The mirror of `useShortcut`: the surface that owns the pane says what can be
 * done to it, and the registration is undone when that surface goes away.
 */
export function useShellAction(id: ShellActionId, action: ShellAction): void {
  const unregister = registerShellAction(id, action)
  onUnmounted(unregister)
}

/** Registers the state reading for the lifetime of the calling component. */
export function useShellActionState(id: ShellActionId, read: ShellActionExpanded): void {
  const unregister = registerShellActionExpanded(id, read)
  onUnmounted(unregister)
}
