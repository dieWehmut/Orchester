// WebView history traversals are asynchronous. Page navigation and a picker
// being unmounted must share the same traversal instead of both calling back().
let returning: Promise<void> | undefined;

declare global {
  interface Window {
    orchesterHandleBack?: () => boolean;
  }
}

export function isHistoryReturning() {
  return returning !== undefined;
}

export function returnThroughHistory(): Promise<void> {
  if (returning) return returning;
  returning = new Promise<void>((resolve) => {
    window.addEventListener("popstate", () => {
      returning = undefined;
      resolve();
    }, { once: true });
  });
  window.history.back();
  return returning;
}
