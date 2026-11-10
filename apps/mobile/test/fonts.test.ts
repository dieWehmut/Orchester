// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { createApp, nextTick, ref, type App } from "vue";
import { initFontAppearance } from "@orchester/design/fonts";
import TerminalPage from "../src/components/TerminalPage.vue";
import { THEMES, useAppearance } from "../src/composables/use-appearance";
import type { RemoteHost } from "../src/composables/use-remote-fleet";
import type { Project } from "../src/client";

const xterm = vi.hoisted(() => ({
  options: [] as Record<string, unknown>[],
  open: vi.fn(),
  dispose: vi.fn(),
}));
vi.mock("@xterm/xterm", () => ({
  Terminal: class {
    options: Record<string, unknown>;
    textarea = document.createElement("textarea");
    constructor(options: Record<string, unknown>) {
      this.options = options;
      xterm.options.push(options);
    }
    loadAddon() {}
    open = xterm.open;
    dispose = xterm.dispose;
    onData() { return { dispose() {} }; }
  },
}));
vi.mock("@xterm/addon-fit", () => ({
  FitAddon: class { proposeDimensions() {} },
}));

let app: App | undefined;
const fontAttributes = ["data-ui-font", "data-content-font", "data-ui-font-weight", "data-content-font-weight"];
beforeEach(() => {
  localStorage.clear();
  for (const name of fontAttributes) document.documentElement.removeAttribute(name);
  xterm.options.length = 0;
  vi.stubGlobal("ResizeObserver", class { observe() {} disconnect() {} });
});
afterEach(() => {
  app?.unmount();
  app = undefined;
  document.body.replaceChildren();
  document.documentElement.removeAttribute("style");
  for (const name of fontAttributes) document.documentElement.removeAttribute(name);
  Reflect.deleteProperty(document, "fonts");
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

it("restores desktop font choices without changing the native palette", () => {
  const appearance = useAppearance();
  appearance.setTheme("sea");
  const palette = document.documentElement.style.getPropertyValue("--bg");
  localStorage.setItem("orchester:ui-font", "serif");
  localStorage.setItem("orchester:content-font", "ui");
  localStorage.setItem("orchester:ui-font-weight", "medium");
  localStorage.setItem("orchester:content-font-weight", "medium");
  expect(initFontAppearance()).toEqual({
    uiFont: "serif", contentFont: "ui", uiFontWeight: "medium", contentFontWeight: "medium",
  });
  expect(fontAttributes.map((name) => document.documentElement.getAttribute(name)))
    .toEqual(["serif", "ui", "medium", "medium"]);
  expect(document.documentElement.getAttribute("data-theme")).toBe("sea");
  expect(document.documentElement.style.getPropertyValue("--bg")).toBe(palette);
});

it("uses desktop defaults when stored font choices are invalid or unavailable", () => {
  localStorage.setItem("orchester:ui-font", "unknown");
  expect(initFontAppearance()).toEqual({
    uiFont: "system", contentFont: "mono", uiFontWeight: "regular", contentFontWeight: "regular",
  });
  vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => { throw new Error("blocked"); });
  expect(() => initFontAppearance()).not.toThrow();
});

function mountTerminal(load: Promise<unknown>, ready = Promise.resolve()) {
  const fontLoad = vi.fn(() => load);
  Object.defineProperty(document, "fonts", { configurable: true, value: { load: fontLoad, ready } });
  const family = 'ui-monospace, "Cascadia Code", "JetBrains Mono", "SFMono-Regular", Consolas, "Liberation Mono", monospace';
  vi.stubGlobal("getComputedStyle", () => ({ getPropertyValue: (name: string) =>
    name === "--font-mono" ? family : name === "--weight-content" ? "500" : "",
  }));
  const request = vi.fn(async () => ({ terminals: [] }));
  const host = { name: "字体测试", address: "http://127.0.0.1:7337", control: {
    connectionStatus: ref("connected"), remoteRequest: request,
  } } as unknown as RemoteHost;
  const project = { id: "font-test", name: "字体测试", path: "/font-test" } as Project;
  const root = document.createElement("div");
  document.body.append(root);
  app = createApp(TerminalPage, { host, project, theme: THEMES[0]! });
  app.mount(root);
  return { fontLoad, family, request };
}

async function settle() {
  for (let i = 0; i < 6; i++) await Promise.resolve();
  await nextTick();
}

it("measures xterm only after the shared content face and font set are ready", async () => {
  let loaded!: () => void, ready!: () => void;
  const fixture = mountTerminal(new Promise<void>((resolve) => { loaded = resolve; }),
    new Promise<void>((resolve) => { ready = resolve; }));
  expect(fixture.fontLoad).toHaveBeenCalledWith(`500 13px ${fixture.family}`, "W中文");
  expect(xterm.open).not.toHaveBeenCalled();
  loaded();
  await settle();
  expect(xterm.open).not.toHaveBeenCalled();
  ready();
  await settle();
  expect(xterm.options[0]).toMatchObject({ fontFamily: fixture.family, fontWeight: 500, fontSize: 13 });
  expect(xterm.open).toHaveBeenCalledOnce();
  expect(fixture.request).toHaveBeenCalledWith("/terminals");
});

it("does not open or attach a terminal after leaving during font loading", async () => {
  let loaded!: () => void;
  const fixture = mountTerminal(new Promise<void>((resolve) => { loaded = resolve; }));
  app!.unmount();
  app = undefined;
  loaded();
  await settle();
  expect(xterm.open).not.toHaveBeenCalled();
  expect(fixture.request).not.toHaveBeenCalled();
});

it("still opens the terminal if loading a font fails", async () => {
  mountTerminal(Promise.reject(new Error("missing font")));
  await settle();
  expect(xterm.open).toHaveBeenCalledOnce();
});
