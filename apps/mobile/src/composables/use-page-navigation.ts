import { onBeforeUnmount, onMounted, ref } from "vue";
import { isHistoryReturning, returnThroughHistory } from "../history";

export type Tab = "dashboard" | "tasks" | "projects" | "settings";
export type ChildPage = "devices" | "appearance" | "about" | "folders" | "agents" | "output" | "terminal";
export type Screen = Tab | ChildPage;
type PageEntry = { owner: string; tab: Tab; screen: Screen };
let previousCleanup: Promise<void> = Promise.resolve();

export function usePageNavigation() {
  const tab = ref<Tab>("dashboard");
  const screen = ref<Screen>("dashboard");
  const owner = crypto.randomUUID();
  let originalState: unknown;
  let pending: PageEntry | undefined;
  let synchronizing: Promise<void> | undefined;
  let disposed = false;
  let ready = false;

  function entry(): PageEntry | undefined {
    const value = window.history.state?.orchesterPage;
    return value?.owner === owner ? value : undefined;
  }
  function rootState(parent: Tab) {
    return { ...(originalState as object), orchesterPage: { owner, tab: parent, screen: parent } };
  }
  function write(target: PageEntry) {
    window.history.replaceState(rootState(target.tab), "");
    if (target.screen !== target.tab)
      window.history.pushState({ ...rootState(target.tab), orchesterPage: target }, "");
  }

  function synchronize(): Promise<void> {
    if (synchronizing) return synchronizing;
    if (!ready) return Promise.resolve();
    synchronizing = Promise.resolve().then(async () => {
      try {
        while (pending || disposed) {
          const current = entry();
          if (current && (current.screen !== current.tab || window.history.state?.orchesterPicker)) {
            await returnThroughHistory();
            continue;
          }
          if (disposed) {
            if (current) window.history.replaceState(originalState, "");
            break;
          }
          // Use the latest target after traversal; rapid tab changes supersede
          // earlier ones, and an old popstate cannot restore a departed page.
          const target = pending!;
          pending = undefined;
          write(target);
        }
      } finally {
        // Release ownership before the loop's promise resolves. A child opened
        // in the following microtask must be able to start a new synchronization.
        synchronizing = undefined;
      }
    });
    return synchronizing;
  }

  function navigate(next: Tab) {
    if (disposed) return;
    tab.value = next;
    screen.value = next;
    pending = { owner, tab: next, screen: next };
    void synchronize();
  }
  function openPage(next: ChildPage) {
    if (disposed) return;
    const target = { owner, tab: tab.value, screen: next };
    screen.value = next;
    if (!ready || synchronizing || isHistoryReturning() || window.history.state?.orchesterPicker) {
      pending = target;
      void synchronize();
    } else if (entry()?.screen !== entry()?.tab) {
      // Folder -> Agent remains one child of the original project tab.
      window.history.replaceState({ ...rootState(tab.value), orchesterPage: target }, "");
    } else {
      write(target);
    }
  }
  function back() {
    if (window.history.state?.orchesterPicker) {
      void returnThroughHistory();
      return;
    }
    if (screen.value !== tab.value) navigate(tab.value);
  }
  function onHistoryBack() {
    if (pending || synchronizing || disposed) return;
    const current = entry();
    if (!current) return;
    tab.value = current.tab;
    screen.value = current.screen;
  }
  function nativeBack(): boolean {
    // Android and header/picker navigation must share the same pending traversal.
    if (!ready || synchronizing || pending || isHistoryReturning()) return true;
    if (window.history.state?.orchesterPicker || screen.value !== tab.value) {
      void returnThroughHistory();
      return true;
    }
    return false;
  }
  onMounted(async () => {
    window.orchesterHandleBack = nativeBack;
    await previousCleanup;
    if (disposed) return;
    originalState = window.history.state;
    ready = true;
    window.history.replaceState(rootState(tab.value), "");
    window.addEventListener("popstate", onHistoryBack);
    void synchronize();
  });
  onBeforeUnmount(() => {
    disposed = true;
    pending = undefined;
    if (window.orchesterHandleBack === nativeBack) delete window.orchesterHandleBack;
    window.removeEventListener("popstate", onHistoryBack);
    if (ready) previousCleanup = synchronize();
  });
  return { tab, screen, navigate, openPage, back };
}
