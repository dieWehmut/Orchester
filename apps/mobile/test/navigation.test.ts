// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { createApp, nextTick, type App } from "vue";
import { credentials } from "../src/credentials";
import type { TerminalSession } from "../src/terminal";
let MobileApp: typeof import("../src/App.vue").default;

const address = "http://navigation-fixture:7337";
const project = { id: "web", name: "Web", path: "/work/web" };
const run = { id: "run-1", project_id: project.id, agent: "codex", prompt: "Navigation fixture", state: "completed" };
let apps: App[];
let root: HTMLElement;
let entries: unknown[];
let index: number;
let traversals: (() => void)[];
let exits: number;
let requests: { path: string; method: string }[];
let held: Map<string, Promise<void>>;
let sockets: FixtureSocket[];
const terminal: TerminalSession = { id: "term-1", project_id: project.id, workspace: project.path, shell: "/bin/bash", cols: 80, rows: 24, state: "running", created_at: 0, exit_code: null };
class FixtureSocket {
  readyState = 1;
  sent: string[] = [];
  onopen: ((event: Event) => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  onclose: ((event: CloseEvent) => void) | null = null;
  constructor(readonly url: string) { sockets.push(this); }
  send(value: string) { this.sent.push(value); }
  close() { this.readyState = 3; }
  ready() { this.onmessage?.(new MessageEvent("message", { data: JSON.stringify({ type: "ready", terminal }) })); }
}

// Only browser history, network and absent jsdom layout APIs are fixtures.
// App.vue, every child page, the fleet and its RemoteClient run unchanged.
beforeEach(async () => {
  apps = [];
  entries = [{ external: "preserved" }];
  index = 0;
  traversals = [];
  exits = 0;
  requests = [];
  held = new Map();
  sockets = [];
  localStorage.clear();
  await credentials.set({ key: `orchester.token:${address}`, value: "fixture-token" });
  vi.spyOn(window.history, "state", "get").mockImplementation(() => entries[index]);
  vi.spyOn(window.history, "pushState").mockImplementation((state) => {
    entries.splice(index + 1);
    entries.push(state);
    index++;
  });
  vi.spyOn(window.history, "replaceState").mockImplementation((state) => { entries[index] = state; });
  vi.spyOn(window.history, "back").mockImplementation(() => {
    traversals.push(() => {
      if (index === 0) { exits++; return; }
      index--;
      window.dispatchEvent(new PopStateEvent("popstate", { state: entries[index] }));
    });
  });
  vi.spyOn(window, "scrollTo").mockImplementation(() => {});
  HTMLDialogElement.prototype.showModal = function () { this.open = true; };
  HTMLDialogElement.prototype.close = function () { this.open = false; this.dispatchEvent(new Event("close")); };
  HTMLElement.prototype.scrollIntoView = vi.fn();
  vi.stubGlobal("ResizeObserver", class { observe() {} disconnect() {} unobserve() {} });
  vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: false, addListener() {}, removeListener() {}, addEventListener() {}, removeEventListener() {} })));
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue({ measureText: () => ({ width: 8 }) } as unknown as CanvasRenderingContext2D);
  vi.stubGlobal("WebSocket", FixtureSocket);
  MobileApp = (await import("../src/App.vue")).default;
  vi.stubGlobal("fetch", vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = new URL(String(input));
    const path = url.pathname.replace(/^\/remote\/v1/, "");
    const method = init?.method ?? "GET";
    requests.push({ path, method });
    expect(url.origin).toBe(address);
    expect(new Headers(init?.headers).get("Authorization")).toBe("Bearer fixture-token");
    const response = held.get(`${method} ${path}`) ?? held.get(path);
    if (response) await response;
    let data: unknown;
    if (path === "/info") data = { name: "Fixture computer", default_project_id: project.id };
    else if (path === "/projects") data = method === "POST" ? project : { projects: [project] };
    else if (path === "/agents") data = { agents: [{ name: "codex", available: true, supports_resume: true }] };
    else if (path === "/runs") data = { runs: [run] };
    else if (path === `/runs/${run.id}/events`) data = { events: [], next_cursor: 0, state: run.state };
    else if (path === "/folders") data = { path: "/work", parent_path: "/", roots: ["/work"], entries: [{ name: "web", path: project.path, type: "directory" }], next_offset: null };
    else if (path === "/terminals") data = { terminals: [terminal] };
    else if (path === `/terminals/${terminal.id}/ticket`) data = { ticket: "fixture-ticket", expires_at: Date.now() + 10_000, ws_path: `/remote/v1/terminals/${terminal.id}/ws?ticket=fixture-ticket` };
    else throw new Error(`Unexpected fixture request: ${method} ${path}`);
    return new Response(JSON.stringify(data), { status: 200 });
  }));
});

async function settle() {
  // Response.json() consumes a real stream; drain its event-loop turn as well as Vue updates.
  await new Promise((resolve) => setTimeout(resolve, 0));
  for (let i = 0; i < 35; i++) await nextTick();
}
async function flushHistory() {
  await settle();
  for (let i = 0; i < 8 && traversals.length; i++) {
    traversals.shift()!();
    await settle();
  }
  expect(traversals).toHaveLength(0);
}
async function mount(connected = false) {
  if (connected) localStorage.setItem("orchester.devices", JSON.stringify([{ address, name: "Fixture computer" }]));
  root = document.createElement("div");
  document.body.append(root);
  const app = createApp(MobileApp);
  apps.push(app);
  app.mount(root);
  await settle();
  return app;
}
function button(text: string, selector = "button") {
  const match = [...root.querySelectorAll<HTMLButtonElement>(selector)].find((item) => item.textContent?.includes(text));
  expect(match, `button ${text}`).toBeDefined();
  return match!;
}
async function click(text: string, selector = "button") { button(text, selector).click(); await settle(); }
function title() { return root.querySelector("h1")?.textContent; }
function pressNativeBack() {
  const handled = window.orchesterHandleBack?.();
  if (handled === false) exits++;
  else if (handled === undefined) window.history.back();
}
async function nativeBack() { pressNativeBack(); await flushHistory(); }
async function headerBack() { root.querySelector<HTMLButtonElement>(".back-button")!.click(); await settle(); await flushHistory(); }
function unmount(app: App) { apps.splice(apps.indexOf(app), 1); app.unmount(); }
function hold(path: string) {
  let release!: () => void;
  held.set(path, new Promise<void>((resolve) => { release = resolve; }));
  return async () => { held.delete(path); release(); await settle(); };
}
afterEach(async () => {
  apps.forEach((app) => app.unmount());
  await settle();
  await flushHistory();
  document.body.replaceChildren();
  document.body.style.overflow = "";
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  localStorage.clear();
});

it.each(["主题", "电脑设备", "关于 Orchester"])("native Back returns from settings %s to its parent tab", async (page) => {
  await mount();
  await click("设置", ".bottom-navigation button");
  await click(page, ".list-row");
  expect(title()).not.toBe("设置");
  expect(index, "WebView canGoBack must be true on a child page").toBe(1);
  await nativeBack();
  expect(title()).toBe("设置");
  expect(index).toBe(0);
  expect(exits).toBe(0);
  expect(root.querySelector('.bottom-navigation [aria-current="page"]')?.textContent).toContain("设置");
});

it("header Back has the same history behavior and root tabs keep their native exit boundary", async () => {
  await mount();
  await click("设置", ".bottom-navigation button");
  await click("主题", ".list-row");
  await headerBack();
  expect(title()).toBe("设置");
  expect(index).toBe(0);
  for (const tab of ["任务", "项目", "控制台", "设置"]) await click(tab, ".bottom-navigation button");
  expect(index).toBe(0);
  await nativeBack();
  expect(exits).toBe(1);
  expect(title()).toBe("设置");
});

it.each(["控制台", "项目"])("device navigation remembers its %s parent", async (parent) => {
  await mount();
  await click(parent, ".bottom-navigation button");
  await click("连接电脑");
  expect(title()).toBe("电脑设备");
  expect(index).toBe(1);
  await nativeBack();
  expect(title()).toBe(parent);
  expect(index).toBe(0);
});

it("folders and agents each return to projects; adding a folder replaces the child history entry", async () => {
  await mount(true);
  await click("项目", ".bottom-navigation button");
  await click("选择目录");
  expect(title()).toBe("选择电脑目录");
  await nativeBack();
  expect(title()).toBe("项目");
  await click("打开 Agent 栏");
  expect(title()).toBe("选择 Agent");
  await headerBack();
  expect(title()).toBe("项目");
  await click("选择目录");
  await click("使用此文件夹");
  expect(title()).toBe("选择 Agent");
  expect(index).toBe(1);
  await nativeBack();
  expect(title()).toBe("项目");
  expect(index).toBe(0);
});

it("task output returns to tasks and keeps the controller and task available", async () => {
  await mount(true);
  await click("任务", ".bottom-navigation button");
  await click(run.prompt, ".run-row");
  expect(title()).toBe("任务详情");
  expect(root.textContent).toContain(run.prompt);
  expect(index).toBe(1);
  await nativeBack();
  expect(title()).toBe("任务");
  expect(button(run.prompt, ".run-row").disabled).toBe(false);
  expect(requests.filter((request) => request.method === "DELETE")).toHaveLength(0);
});

it("native Back closes a root picker before the next Back reaches the native exit boundary", async () => {
  await mount(true);
  await click("项目", ".bottom-navigation button");
  await click("全部设备", ".device-filter .picker-trigger");
  expect(document.querySelector<HTMLDialogElement>("dialog[open]")).not.toBeNull();
  expect(index).toBe(1);
  await nativeBack();
  expect(title()).toBe("项目");
  expect(document.querySelector("dialog[open]")).toBeNull();
  expect(index).toBe(0);
  expect(exits).toBe(0);
  await nativeBack();
  expect(exits).toBe(1);
});

it("rapid tab changes during header Back keep the latest tab and repeated header clicks traverse once", async () => {
  await mount();
  await click("设置", ".bottom-navigation button");
  await click("主题", ".list-row");
  const back = root.querySelector<HTMLButtonElement>(".back-button")!;
  back.click();
  back.click();
  button("任务", ".bottom-navigation button").click();
  button("项目", ".bottom-navigation button").click();
  await settle();
  expect(title()).toBe("项目");
  await flushHistory();
  expect(title()).toBe("项目");
  expect(window.history.back).toHaveBeenCalledTimes(1);
  expect(index).toBe(0);
  expect(exits).toBe(0);
  await nativeBack();
  expect(exits).toBe(1);
});

it("opening a new child while a previous child returns cannot be overwritten by its old popstate", async () => {
  await mount();
  await click("设置", ".bottom-navigation button");
  await click("主题", ".list-row");
  root.querySelector<HTMLButtonElement>(".back-button")!.click();
  await settle();
  await click("关于 Orchester", ".list-row");
  expect(title()).toBe("关于");
  await flushHistory();
  expect(title()).toBe("关于");
  expect(index).toBe(1);
  await nativeBack();
  expect(title()).toBe("设置");
  expect(index).toBe(0);
});

it("switching tabs while a root picker returns shares its traversal and leaves no stale picker entry", async () => {
  await mount(true);
  await click("项目", ".bottom-navigation button");
  await click("全部设备", ".device-filter .picker-trigger");
  document.querySelector<HTMLButtonElement>("dialog[open] .picker-close")!.click();
  await click("任务", ".bottom-navigation button");
  await click("设置", ".bottom-navigation button");
  await flushHistory();
  expect(title()).toBe("设置");
  expect(index).toBe(0);
  expect(window.history.back).toHaveBeenCalledTimes(1);
  expect(document.querySelector("dialog[open]")).toBeNull();
});

it("late folder submission and agent loading cannot restore a page after Back and a tab switch", async () => {
  await mount(true);
  await click("项目", ".bottom-navigation button");
  await click("选择目录");
  const releaseFolder = hold("POST /projects");
  await click("使用此文件夹");
  await headerBack();
  await click("设置", ".bottom-navigation button");
  await releaseFolder();
  expect(title()).toBe("设置");
  expect(index).toBe(0);
  await click("项目", ".bottom-navigation button");
  const releaseAgents = hold("GET /agents");
  await click("打开 Agent 栏");
  await headerBack();
  await click("设置", ".bottom-navigation button");
  await releaseAgents();
  expect(title()).toBe("设置");
  expect(index).toBe(0);
});

it("late panel and output requests cannot navigate to tasks or restore output after leaving", async () => {
  await mount(true);
  await click("项目", ".bottom-navigation button");
  await click("打开 Agent 栏");
  const releasePanel = hold("GET /runs");
  await click("codex", ".list-row");
  await headerBack();
  await click("设置", ".bottom-navigation button");
  await releasePanel();
  expect(title()).toBe("设置");
  expect(index).toBe(0);
  await click("任务", ".bottom-navigation button");
  const releaseOutput = hold(`GET /runs/${run.id}/events`);
  await click(run.prompt, ".run-row");
  await headerBack();
  await click("设置", ".bottom-navigation button");
  await releaseOutput();
  expect(title()).toBe("设置");
  expect(index).toBe(0);
});

it.each(["native", "header"])("%s Back detaches terminal input while preserving its running session and original project tab", async (method) => {
  await mount(true);
  await click("项目", ".bottom-navigation button");
  await click("进入终端");
  expect(title()).toBe("终端");
  expect(sockets).toHaveLength(1);
  const socket = sockets[0]!;
  socket.ready();
  await settle();
  const input = root.querySelector<HTMLTextAreaElement>("textarea[aria-label='终端输入']")!;
  input.dispatchEvent(new KeyboardEvent("keydown", { key: "a", keyCode: 65, bubbles: true }));
  expect(socket.sent.some((data) => JSON.parse(data).type === "input")).toBe(true);
  if (method === "native") await nativeBack(); else await headerBack();
  expect(title()).toBe("项目");
  expect(index).toBe(0);
  expect(socket.readyState).toBe(3);
  const sent = socket.sent.length;
  input.dispatchEvent(new KeyboardEvent("keydown", { key: "b", keyCode: 66, bubbles: true }));
  expect(socket.sent).toHaveLength(sent);
  expect(requests.filter((request) => request.method === "DELETE")).toHaveLength(0);
  await click("进入终端");
  expect(sockets).toHaveLength(2);
  expect(sockets[1]!.url).toContain(terminal.id);
  expect(requests.filter((request) => request.method === "POST" && request.path === "/terminals")).toHaveLength(0);
});

it("terminal picker consumes the first Back and consecutive Back presses return to projects without exiting", async () => {
  await mount(true);
  await click("项目", ".bottom-navigation button");
  await click("进入终端");
  await click("bash", ".terminal-toolbar .picker-trigger");
  expect(index).toBe(2);
  await nativeBack();
  expect(title()).toBe("终端");
  expect(sockets[0]!.readyState).toBe(1);
  expect(index).toBe(1);
  await click("bash", ".terminal-toolbar .picker-trigger");
  await nativeBack();
  await nativeBack();
  expect(title()).toBe("项目");
  expect(index).toBe(0);
  expect(exits).toBe(0);
});

it("changing tabs with a terminal picker open unwinds both entries once and keeps the last chosen tab", async () => {
  await mount(true);
  await click("项目", ".bottom-navigation button");
  await click("进入终端");
  await click("bash", ".terminal-toolbar .picker-trigger");
  await click("任务", ".bottom-navigation button");
  await click("设置", ".bottom-navigation button");
  await flushHistory();
  expect(title()).toBe("设置");
  expect(index).toBe(0);
  expect(exits).toBe(0);
  expect(window.history.back).toHaveBeenCalledTimes(2);
  expect(sockets[0]!.readyState).toBe(3);
});

it("repeated mounting and unmounting removes App listeners and restores the parent history state", async () => {
  const added = vi.spyOn(window, "addEventListener");
  const removed = vi.spyOn(window, "removeEventListener");
  for (let i = 0; i < 3; i++) {
    const app = await mount();
    const pageListener = added.mock.calls.filter(([name]) => name === "popstate").at(-1)![1];
    await click("设置", ".bottom-navigation button");
    await click("主题", ".list-row");
    unmount(app);
    await flushHistory();
    expect(removed.mock.calls.some(([name, listener]) => name === "popstate" && listener === pageListener)).toBe(true);
    expect(window.history.state).toEqual({ external: "preserved" });
    expect(index).toBe(0);
    root.remove();
  }
  expect(exits).toBe(0);
});


it("native Back and a rapid tab switch share the pending traversal instead of exiting the Activity", async () => {
  await mount();
  await click("设置", ".bottom-navigation button");
  await click("主题", ".list-row");
  pressNativeBack();
  pressNativeBack();
  button("任务", ".bottom-navigation button").click();
  button("项目", ".bottom-navigation button").click();
  await settle();
  await flushHistory();
  expect(title()).toBe("项目");
  expect(index).toBe(0);
  expect(exits).toBe(0);
  expect(window.history.back).toHaveBeenCalledTimes(1);
});

it("a new App waits for an old child and picker to finish unmounting before owning its root history", async () => {
  const old = await mount(true);
  await click("项目", ".bottom-navigation button");
  await click("进入终端");
  await click("bash", ".terminal-toolbar .picker-trigger");
  unmount(old);
  await mount();
  await flushHistory();
  await click("设置", ".bottom-navigation button");
  await click("关于 Orchester", ".list-row");
  await nativeBack();
  expect(title()).toBe("设置");
  expect(index).toBe(0);
  expect(exits).toBe(0);
});

it("unmount invalidates a pending folder submission before it starts an agent request", async () => {
  const app = await mount(true);
  await click("项目", ".bottom-navigation button");
  await click("选择目录");
  const release = hold("POST /projects");
  await click("使用此文件夹");
  const before = requests.filter((request) => request.path === "/agents").length;
  unmount(app);
  await flushHistory();
  await release();
  expect(requests.filter((request) => request.path === "/agents")).toHaveLength(before);
});


it("opening a child in the microtask that completes tab navigation still creates its Back entry", async () => {
  await mount();
  button("设置", ".bottom-navigation button").click();
  await Promise.resolve();
  button("主题", ".list-row").click();
  await settle();
  expect(title()).toBe("主题");
  expect(index).toBe(1);
  await nativeBack();
  expect(title()).toBe("设置");
  expect(index).toBe(0);
});
