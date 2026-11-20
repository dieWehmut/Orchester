// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { createApp, effectScope, nextTick, ref, shallowRef, type App, type ComponentPublicInstance } from "vue";
import MobileApp from "../src/App.vue";
import AgentWorkbench from "../src/components/AgentWorkbench.vue";
import type { FolderPage } from "../src/client";
import type { AgentPanel, RemoteHost, useRemoteFleet } from "../src/composables/use-remote-fleet";

type Fleet = ReturnType<typeof useRemoteFleet>;
type Browser = "projects" | "workbench";
let fleet: Fleet;
vi.mock("../src/composables/use-remote-fleet", () => ({ useRemoteFleet: () => fleet }));
vi.mock("../src/components/TerminalPage.vue", () => ({ default: { render: () => null } }));

const project = { id: "web", name: "Web", path: "/work/web" };
const firstPage: FolderPage = {
  path: "/work",
  parent_path: "/",
  roots: ["/work"],
  entries: [{ name: "first", path: "/work/first", type: "directory" }],
  next_offset: 100,
};
const lastPage: FolderPage = {
  ...firstPage,
  entries: [{ name: "last", path: "/work/last", type: "directory" }],
  next_offset: null,
};
let apps: App[];
let scopes: ReturnType<typeof effectScope>[];
let root: HTMLElement;
let folderBrowser: ComponentPublicInstance | undefined;
let browseFolders: ReturnType<typeof vi.fn<RemoteHost["control"]["browseFolders"]>>;

beforeEach(() => {
  apps = [];
  scopes = [];
  folderBrowser = undefined;
  localStorage.clear();
  vi.spyOn(window, "scrollTo").mockImplementation(() => {});
  browseFolders = vi.fn<RemoteHost["control"]["browseFolders"]>()
    .mockResolvedValueOnce(firstPage)
    .mockResolvedValue(lastPage);
  const control = {
    busy: ref(false),
    connectionStatus: ref("connected"),
    error: ref(""),
    agentSnapshot: shallowRef(null),
    agentSnapshotStatus: ref("idle"),
    agentSnapshotError: ref(""),
    projects: ref([project]),
    runs: ref([]),
    attachments: ref([]),
    selected: ref(null),
    resume: ref(""),
    prompt: ref(""),
    agent: ref("codex"),
    browseFolders,
  } as unknown as RemoteHost["control"];
  const scope = effectScope();
  scopes.push(scope);
  const host: RemoteHost = {
    address: "http://folder-fixture:7337",
    name: "Fixture computer",
    control,
    scope,
    syncError: "",
  };
  fleet = {
    hosts: shallowRef([host]),
    panels: shallowRef([]),
    error: ref(""),
    pairDevice: vi.fn(),
    connectSaved: vi.fn(),
    forgetHost: vi.fn(),
    openPanel: vi.fn(),
    closePanel: vi.fn(),
    refreshHosts: vi.fn(),
  };
});

async function settle() {
  for (let i = 0; i < 6; i++) await Promise.resolve();
  await nextTick();
}

afterEach(async () => {
  for (const app of apps) app.unmount();
  await settle();
  for (const scope of scopes) scope.stop();
  document.body.replaceChildren();
  document.documentElement.removeAttribute("style");
  vi.restoreAllMocks();
});

function button(text: string, selector = "button") {
  const match = [...root.querySelectorAll<HTMLButtonElement>(selector)]
    .find((item) => item.textContent?.includes(text));
  expect(match, `button ${text}`).toBeDefined();
  return match!;
}

async function mountBrowser(browser: Browser) {
  root = document.createElement("div");
  document.body.append(root);
  const host = fleet.hosts.value[0]!;
  const panel: AgentPanel = {
    id: "folder-panel",
    hostAddress: host.address,
    project,
    agent: "codex",
    control: host.control,
    scope: host.scope,
  };
  const app = browser === "projects"
    ? createApp(MobileApp)
    : createApp(AgentWorkbench, { panel, deviceName: host.name });
  apps.push(app);
  app.mixin({
    mounted() {
      if (this.$el instanceof HTMLElement && this.$el.matches(".folder-browser"))
        folderBrowser = this;
    },
  });
  app.mount(root);
  await settle();
  if (browser === "projects") {
    button("项目", ".bottom-navigation button").click();
    await settle();
    button("选择目录").click();
  } else {
    button("文件", ".pane-tabs button").click();
  }
  await settle();
  expect(root.querySelector(".current-path")?.textContent).toBe(firstPage.path);
}

it.each<Browser>(["projects", "workbench"])("%s paginates the displayed directory after editing its draft path", async (browser) => {
  await mountBrowser(browser);
  const input = root.querySelector<HTMLInputElement>(".path-controls input")!;
  input.value = "/other";
  input.dispatchEvent(new Event("input", { bubbles: true }));
  await settle();
  button("加载更多").click();
  await settle();
  expect(browseFolders).toHaveBeenLastCalledWith(firstPage.path, firstPage.next_offset);
  expect([...root.querySelectorAll(".directory-row strong")].map((node) => node.textContent))
    .toEqual(["first", "last"]);
});

it.each<Browser>(["projects", "workbench"])("%s ignores a queued more action after the displayed directory has no next page", async (browser) => {
  await mountBrowser(browser);
  const more = button("加载更多");
  more.click();
  await settle();
  expect(browseFolders).toHaveBeenCalledTimes(2);
  expect(root.querySelector(".more-button")).toBeNull();
  // Ignore a late component action even after the more button has disappeared.
  folderBrowser!.$emit("more");
  await settle();
  expect(browseFolders).toHaveBeenCalledTimes(2);
  expect(root.querySelectorAll(".directory-row")).toHaveLength(2);
});
