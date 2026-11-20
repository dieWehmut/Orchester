import { createRenderer } from "vue";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { useRemoteFleet } from "../src/composables/use-remote-fleet";
import { credentials } from "../src/credentials";
import type { Project, Run } from "../src/client";

type Fleet = ReturnType<typeof useRemoteFleet>;
type RenderNode = { parent?: RenderNode };
type HostFixture = {
  address: string;
  name: string;
  token: string;
  projects: Project[];
  runs: Run[];
  offline: boolean;
};
type HttpCall = {
  origin: string;
  path: string;
  query: URLSearchParams;
  method: string;
  authorization: string | null;
  body: Record<string, unknown>;
};

// Mount Vue lifecycle hooks without a browser DOM. The fleet, its controllers,
// and RemoteClient all run unchanged; only HTTP and credential storage are fixtures.
const renderer = createRenderer<RenderNode, RenderNode>({
  insert: (node, parent) => {
    node.parent = parent;
  },
  remove: (node) => {
    delete node.parent;
  },
  createElement: () => ({}),
  createText: () => ({}),
  createComment: () => ({}),
  setText: () => {},
  setElementText: () => {},
  parentNode: (node) => node.parent ?? null,
  nextSibling: () => null,
  patchProp: () => {},
});

let storage: Map<string, string>;
let tokens: Map<string, string>;
let httpCalls: HttpCall[];
let fixtures: Map<string, HostFixture>;
let heldResponses: Map<string, Promise<void>>;
let mounted: { fleet: Fleet; unmount: () => void }[];

function hostFixture(address: string, name: string): HostFixture {
  const projects = [
    { id: "shared project", name: "Web", path: `/work/${name}/web` },
    { id: "service", name: "Service", path: `/work/${name}/service` },
  ];
  const runs = projects.flatMap((project) =>
    ["codex", "claude"].map((agent) => ({
      id: `${project.id}-${agent}`,
      project_id: project.id,
      agent,
      prompt: `${name} ${agent} history`,
      state: "completed",
      session_id: `${name}-${project.id}-${agent}-session`,
    })),
  );
  const fixture = {
    address,
    name,
    token: `${name}-private-token`,
    projects,
    runs,
    offline: false,
  };
  fixtures.set(address, fixture);
  tokens.set(`orchester.token:${address}`, fixture.token);
  return fixture;
}

function mountFleet() {
  let fleet!: Fleet;
  const app = renderer.createApp({
    setup() {
      fleet = useRemoteFleet();
      return () => null;
    },
  });
  app.mount({});
  const instance = { fleet, unmount: () => app.unmount() };
  mounted.push(instance);
  return instance;
}

function savedPanels(): {
  hostAddress: string;
  projectId: string;
  agent: string;
}[] {
  return JSON.parse(storage.get("orchester.panels") ?? "[]");
}

function savedDevices(): { address: string; name: string }[] {
  return JSON.parse(storage.get("orchester.devices") ?? "[]");
}

beforeEach(() => {
  vi.useFakeTimers();
  storage = new Map();
  tokens = new Map();
  httpCalls = [];
  fixtures = new Map();
  heldResponses = new Map();
  mounted = [];
  vi.stubGlobal("localStorage", {
    getItem: (key: string) => storage.get(key) ?? null,
    setItem: (key: string, value: string) => storage.set(key, value),
    removeItem: (key: string) => storage.delete(key),
  });
  vi.spyOn(credentials, "get").mockImplementation(async ({ key }) => ({
    value: tokens.get(key) ?? null,
  }));
  vi.spyOn(credentials, "set").mockImplementation(async ({ key, value }) => {
    tokens.set(key, value);
  });
  vi.spyOn(credentials, "remove").mockImplementation(async ({ key }) => {
    tokens.delete(key);
  });
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = new URL(
        typeof input === "string"
          ? input
          : input instanceof URL
            ? input.href
            : input.url,
      );
      const path = url.pathname.replace(/^\/remote\/v1/, "");
      const method = init?.method ?? "GET";
      const body: Record<string, unknown> = init?.body
        ? JSON.parse(String(init.body))
        : {};
      const authorization = new Headers(init?.headers).get("Authorization");
      httpCalls.push({
        origin: url.origin,
        path,
        query: url.searchParams,
        method,
        authorization,
        body,
      });
      const host = fixtures.get(url.origin);
      if (!host) throw new Error(`Unknown fixture host: ${url.origin}`);
      if (host.offline) throw new Error(`${host.name} is offline`);
      const respond = (data: unknown, status = 200) =>
        new Response(JSON.stringify(data), { status });
      if (path === "/pair" && method === "POST") {
        return respond({
          token: host.token,
          device_id: "test-phone",
          expires_at: 100,
        });
      }
      if (authorization !== `Bearer ${host.token}`)
        return respond({ error: "unauthorized" }, 401);
      const held = heldResponses.get(`${url.origin}${path}`);
      if (held) await held;
      if (path === "/info")
        return respond({
          name: host.name,
          default_project_id: host.projects[0]!.id,
        });
      if (path === "/projects") return respond({ projects: host.projects });
      if (path === "/agents")
        return respond({
          agents: ["codex", "claude"].map((name) => ({
            name,
            available: true,
            supports_resume: true,
          })),
        });
      if (path === "/runs" && method === "GET") {
        const projectId = url.searchParams.get("project_id");
        return respond({
          runs: host.runs.filter(
            (run) => !projectId || run.project_id === projectId,
          ),
        });
      }
      if (path === "/runs" && method === "POST") {
        const run: Run = {
          id: `submitted-${httpCalls.filter((call) => call.path === "/runs" && call.method === "POST").length}`,
          project_id: String(body.project_id),
          agent: String(body.agent),
          prompt: String(body.prompt),
          state: "running",
          session_id: `${host.name}-submitted-session`,
        };
        host.runs.unshift(run);
        return respond(run);
      }
      const events = /^\/runs\/([^/]+)\/events$/.exec(path);
      if (events) {
        const run = host.runs.find(
          (task) => task.id === decodeURIComponent(events[1]!),
        );
        if (!run) return respond({ error: "not found" }, 404);
        const after = Number(url.searchParams.get("after"));
        return respond({
          events:
            after === 0
              ? [
                  {
                    seq: 1,
                    event: { type: "message", text: `${host.name}:${run.id}` },
                  },
                ]
              : [],
          next_cursor: 1,
          state: run.state,
        });
      }
      throw new Error(`Unexpected fixture request: ${method} ${url.href}`);
    }),
  );
});

afterEach(() => {
  for (const instance of mounted) instance.unmount();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

it("keeps host, project, agent, draft and output independent across panels", async () => {
  const alpha = hostFixture("http://alpha:7337", "Alpha");
  const beta = hostFixture("http://beta:7337", "Beta");
  const { fleet } = mountFleet();
  const [a, b] = await Promise.all([
    fleet.connectSaved({ address: alpha.address, name: alpha.name }),
    fleet.connectSaved({ address: beta.address, name: beta.name }),
  ]);
  const panels = (
    await Promise.all([
      fleet.openPanel(a!, alpha.projects[0]!, "codex"),
      fleet.openPanel(a!, alpha.projects[0]!, "claude"),
      fleet.openPanel(a!, alpha.projects[1]!, "codex"),
      fleet.openPanel(b!, beta.projects[0]!, "codex"),
    ])
  ).map((panel) => panel!);
  expect(fleet.hosts.value).toHaveLength(2);
  expect(fleet.panels.value).toHaveLength(4);
  expect(new Set(panels.map((panel) => panel.control)).size).toBe(4);
  const snapshots = panels.map((panel, index) => {
    panel.control.prompt.value = `draft ${index}`;
    return {
      run: panel.control.selected.value?.id,
      events: JSON.parse(JSON.stringify(panel.control.events.value)),
    };
  });
  for (const panel of panels) {
    expect(panel.control.project.value?.id).toBe(panel.project.id);
    expect(
      panel.control.runs.value.every(
        (run) => run.project_id === panel.project.id,
      ),
    ).toBe(true);
    expect(panel.control.selected.value?.agent).toBe(panel.agent);
  }
  expect(panels[0]!.control.events.value[0]?.event).toEqual({
    type: "message",
    text: "Alpha:shared project-codex",
  });
  expect(panels[3]!.control.events.value[0]?.event).toEqual({
    type: "message",
    text: "Beta:shared project-codex",
  });
  const first = panels[0]!;
  first.control.attachments.value = [
    { name: "notes.md", path: "/work/Alpha/web/notes.md", type: "file" },
  ];
  first.control.continueRun();
  expect(first.control.resume.value).toBe("Alpha-shared project-codex-session");
  expect(
    panels
      .slice(1)
      .every(
        (panel) =>
          panel.control.resume.value === "" &&
          panel.control.attachments.value.length === 0,
      ),
  ).toBe(true);
  await first.control.submit();
  const post = httpCalls.find(
    (call) => call.path === "/runs" && call.method === "POST",
  )!;
  expect(post.origin).toBe(alpha.address);
  expect(post.body).toMatchObject({
    agent: "codex",
    project_id: "shared project",
    resume: "Alpha-shared project-codex-session",
    prompt: "draft 0\n\n参考文件（在这台电脑上）：\n- /work/Alpha/web/notes.md",
  });
  expect(first.control.selected.value?.id).toBe("submitted-1");
  expect(first.control.prompt.value).toBe("");
  for (let index = 1; index < panels.length; index++) {
    expect(panels[index]!.control.prompt.value).toBe(`draft ${index}`);
    expect(panels[index]!.control.selected.value?.id).toBe(
      snapshots[index]!.run,
    );
    expect(panels[index]!.control.events.value).toEqual(
      snapshots[index]!.events,
    );
  }
  expect(
    httpCalls.every(
      (call) =>
        call.authorization === `Bearer ${fixtures.get(call.origin)!.token}`,
    ),
  ).toBe(true);
  expect(
    httpCalls
      .filter((call) => call.path === "/runs" && call.query.has("project_id"))
      .map((call) => [call.origin, call.query.get("project_id")]),
  ).toEqual(
    expect.arrayContaining([
      [alpha.address, "shared project"],
      [alpha.address, "service"],
      [beta.address, "shared project"],
    ]),
  );
});

it("stops only a closed panel's polling without cancelling remote tasks", async () => {
  const host = hostFixture("http://alpha:7337", "Alpha");
  host.runs = host.runs.map((run) => ({ ...run, state: "running" }));
  const instance = mountFleet();
  const connected = (await instance.fleet.connectSaved({
    address: host.address,
    name: host.name,
  }))!;
  const closing = (await instance.fleet.openPanel(
    connected,
    host.projects[0]!,
    "codex",
  ))!;
  const surviving = (await instance.fleet.openPanel(
    connected,
    host.projects[1]!,
    "codex",
  ))!;
  await vi.advanceTimersByTimeAsync(0);
  const closingPath = "/runs/shared%20project-codex/events";
  const survivingPath = "/runs/service-codex/events";
  const closedReads = httpCalls.filter(
    (call) => call.path === closingPath,
  ).length;
  const survivingReads = httpCalls.filter(
    (call) => call.path === survivingPath,
  ).length;
  instance.fleet.closePanel(closing);
  expect(closing.scope.active).toBe(false);
  expect(closing.control.selected.value).toBeNull();
  expect(surviving.scope.active).toBe(true);
  await vi.advanceTimersByTimeAsync(1000);
  expect(httpCalls.filter((call) => call.path === closingPath)).toHaveLength(
    closedReads,
  );
  expect(httpCalls.filter((call) => call.path === survivingPath)).toHaveLength(
    survivingReads + 1,
  );
  expect(surviving.control.selected.value?.id).toBe("service-codex");
  expect(savedPanels()).toEqual([
    { hostAddress: host.address, projectId: "service", agent: "codex" },
  ]);
  expect(host.runs.every((run) => run.state === "running")).toBe(true);
  const callsBeforeUnmount = httpCalls.length;
  instance.unmount();
  await vi.advanceTimersByTimeAsync(6000);
  expect(httpCalls).toHaveLength(callsBeforeUnmount);
  expect(vi.getTimerCount()).toBe(0);
  expect(httpCalls.every((call) => call.method === "GET")).toBe(true);
});

it("merges device saves across controllers and does not revive a forgotten host", async () => {
  const alpha = hostFixture("http://alpha:7337", "Alpha");
  const beta = hostFixture("http://beta:7337", "Beta");
  const { fleet } = mountFleet();
  const [a, b] = await Promise.all([
    fleet.connectSaved({ address: alpha.address, name: alpha.name }),
    fleet.connectSaved({ address: beta.address, name: beta.name }),
  ]);
  expect(savedDevices()).toEqual(
    expect.arrayContaining([
      { address: alpha.address, name: alpha.name },
      { address: beta.address, name: beta.name },
    ]),
  );
  expect(savedDevices()).toHaveLength(2);
  const panel = (await fleet.openPanel(a!, alpha.projects[0]!, "codex"))!;
  await fleet.forgetHost(a!);
  expect(panel.scope.active).toBe(false);
  expect(tokens.has(`orchester.token:${alpha.address}`)).toBe(false);
  expect(savedDevices()).toEqual([{ address: beta.address, name: beta.name }]);
  await b!.control.connect({ address: beta.address, name: beta.name });
  expect(savedDevices()).toEqual([{ address: beta.address, name: beta.name }]);
  expect(fleet.hosts.value.map((host) => host.address)).toEqual([beta.address]);
  expect(httpCalls.every((call) => call.method === "GET")).toBe(true);
});

it("restores panel metadata against fresh host history while preserving offline preferences", async () => {
  const alpha = hostFixture("http://alpha:7337", "Alpha");
  const beta = hostFixture("http://beta:7337", "Beta");
  beta.offline = true;
  storage.set(
    "orchester.devices",
    JSON.stringify(
      [alpha, beta].map(({ address, name }) => ({ address, name })),
    ),
  );
  const metadata = [
    { hostAddress: alpha.address, projectId: "shared project", agent: "codex" },
    { hostAddress: beta.address, projectId: "shared project", agent: "claude" },
  ];
  storage.set("orchester.panels", JSON.stringify(metadata));
  const first = mountFleet();
  await vi.waitFor(() => {
    expect(first.fleet.panels.value[0]?.control.selected.value?.id).toBe(
      "shared project-codex",
    );
    expect(
      first.fleet.hosts.value.find((host) => host.address === beta.address)
        ?.control.connectionStatus.value,
    ).toBe("disconnected");
  });
  expect(savedPanels()).toEqual(expect.arrayContaining(metadata));
  expect(savedPanels()).toHaveLength(2);
  first.fleet.panels.value[0]!.control.prompt.value = "private unsent draft";
  first.fleet.panels.value[0]!.control.attachments.value = [
    { name: "secret.md", path: "/work/private/secret.md", type: "file" },
  ];
  first.unmount();
  beta.offline = false;
  alpha.runs = alpha.runs.map((run) => ({
    ...run,
    id: `fresh-${run.id}`,
    session_id: "fresh-session",
  }));
  const second = mountFleet();
  await vi.waitFor(() => {
    expect(second.fleet.panels.value).toHaveLength(2);
    expect(
      second.fleet.panels.value.find(
        (panel) => panel.hostAddress === alpha.address,
      )?.control.selected.value?.id,
    ).toBe("fresh-shared project-codex");
    expect(
      second.fleet.panels.value.find(
        (panel) => panel.hostAddress === beta.address,
      )?.control.selected.value?.agent,
    ).toBe("claude");
  });
  for (const panel of second.fleet.panels.value) {
    expect(panel.control.prompt.value).toBe("");
    expect(panel.control.attachments.value).toEqual([]);
    expect(panel.control.resume.value).toBe("");
  }
  expect(savedPanels()).toEqual(expect.arrayContaining(metadata));
  for (const item of savedPanels())
    expect(Object.keys(item).sort()).toEqual([
      "agent",
      "hostAddress",
      "projectId",
    ]);
  const saved = storage.get("orchester.panels")!;
  for (const privateValue of [
    "private unsent draft",
    "secret.md",
    "fresh-session",
    "private-token",
    "history",
    "session_id",
    "/work/",
  ]) {
    expect(saved).not.toContain(privateValue);
  }
  expect(httpCalls.every((call) => call.method === "GET")).toBe(true);
});

it("discards an opening panel's delayed output and metadata after it is closed", async () => {
  const host = hostFixture("http://alpha:7337", "Alpha");
  const { fleet } = mountFleet();
  const connected = (await fleet.connectSaved({
    address: host.address,
    name: host.name,
  }))!;
  let release!: () => void;
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  const path = "/runs/shared%20project-codex/events";
  heldResponses.set(`${host.address}${path}`, held);
  const opening = fleet.openPanel(connected, host.projects[0]!, "codex");
  await vi.waitFor(() =>
    expect(httpCalls.some((call) => call.path === path)).toBe(true),
  );
  const panel = fleet.panels.value[0]!;
  fleet.closePanel(panel);
  release();
  await expect(opening).resolves.toBeNull();
  expect(panel.control.events.value).toEqual([]);
  expect(panel.scope.active).toBe(false);
  expect(fleet.panels.value).toEqual([]);
  expect(savedPanels()).toEqual([]);
  expect(httpCalls.every((call) => call.method === "GET")).toBe(true);
});

it("reconnects a saved host after an offline cold start and restores its panels", async () => {
  const host = hostFixture("http://alpha:7337", "Alpha");
  host.offline = true;
  storage.set("orchester.devices", JSON.stringify([{ address: host.address, name: host.name }]));
  storage.set("orchester.panels", JSON.stringify([
    { hostAddress: host.address, projectId: "service", agent: "claude" },
  ]));
  const { fleet } = mountFleet();
  await vi.waitFor(() => {
    expect(fleet.hosts.value[0]?.control.connectionStatus.value).toBe("disconnected");
    expect(httpCalls.some((call) => call.path === "/info")).toBe(true);
  });
  host.offline = false;
  await vi.advanceTimersByTimeAsync(5000);
  expect(fleet.hosts.value[0]?.control.connectionStatus.value).toBe("connected");
  expect(fleet.panels.value).toHaveLength(1);
  expect(fleet.panels.value[0]?.control.project.value?.id).toBe("service");
  expect(fleet.panels.value[0]?.control.agent.value).toBe("claude");
  expect(fleet.panels.value[0]?.control.selected.value?.id).toBe("service-claude");
});

it("retries an existing panel that failed to connect without replacing its draft", async () => {
  const host = hostFixture("http://alpha:7337", "Alpha");
  const { fleet } = mountFleet();
  const connected = (await fleet.connectSaved(host))!;
  host.offline = true;
  const failed = (await fleet.openPanel(connected, host.projects[1]!, "claude"))!;
  expect(failed.control.connectionStatus.value).toBe("disconnected");
  failed.control.prompt.value = "keep this draft";
  host.offline = false;
  const recovered = await fleet.openPanel(connected, host.projects[1]!, "claude");
  expect(recovered).toBe(failed);
  expect(fleet.panels.value).toHaveLength(1);
  expect(recovered?.control.connectionStatus.value).toBe("connected");
  expect(recovered?.control.project.value?.id).toBe("service");
  expect(recovered?.control.agent.value).toBe("claude");
  expect(recovered?.control.prompt.value).toBe("keep this draft");
});

it("allows immediate reopen while an old closed panel request is still pending", async () => {
  const host = hostFixture("http://alpha:7337", "Alpha");
  const { fleet } = mountFleet();
  const connected = (await fleet.connectSaved(host))!;
  let releaseFirst!: () => void;
  let releaseSecond!: () => void;
  const path = "/runs/service-claude/events";
  heldResponses.set(`${host.address}${path}`, new Promise<void>((resolve) => { releaseFirst = resolve; }));
  const first = fleet.openPanel(connected, host.projects[1]!, "claude");
  await vi.waitFor(() => expect(httpCalls.filter((call) => call.path === path)).toHaveLength(1));
  const closed = fleet.panels.value[0]!;
  fleet.closePanel(closed);
  heldResponses.set(`${host.address}${path}`, new Promise<void>((resolve) => { releaseSecond = resolve; }));
  const second = fleet.openPanel(connected, host.projects[1]!, "claude");
  expect(second).not.toBe(first);
  fleet.closePanel(closed);
  await vi.waitFor(() => expect(httpCalls.filter((call) => call.path === path)).toHaveLength(2));
  releaseFirst();
  await expect(first).resolves.toBeNull();
  // Completion of the old request must not erase the new in-flight entry.
  expect(fleet.openPanel(connected, host.projects[1]!, "claude")).toBe(second);
  releaseSecond();
  const reopened = await second;
  expect(reopened).not.toBeNull();
  expect(reopened).not.toBe(closed);
  expect(fleet.panels.value).toEqual([reopened]);
  expect(reopened?.control.selected.value?.id).toBe("service-claude");
});

it("never falls back to another project when re-pairing a deleted panel project", async () => {
  const host = hostFixture("http://alpha:7337", "Alpha");
  const { fleet } = mountFleet();
  const connected = (await fleet.connectSaved(host))!;
  const panel = (await fleet.openPanel(connected, host.projects[1]!, "claude"))!;
  panel.control.prompt.value = "only run in service";
  host.projects = host.projects.filter((project) => project.id !== "service");
  host.token = "replacement-private-token";
  await fleet.pairDevice({ address: host.address, code: "new-code", name: "Phone" });
  expect(fleet.panels.value).toEqual([panel]);
  expect(panel.project.id).toBe("service");
  expect(panel.control.project.value).toBeNull();
  expect(panel.control.agent.value).toBe("");
  expect(panel.control.runs.value).toEqual([]);
  expect(panel.control.selected.value).toBeNull();
  expect(panel.control.prompt.value).toBe("only run in service");
  expect(fleet.error.value).toContain("项目");
  expect(httpCalls.some((call) => call.path === "/runs" && call.method === "POST")).toBe(false);
});

it("does not automatically retry a saved host that needs fresh pairing", async () => {
  const host = hostFixture("http://alpha:7337", "Alpha");
  storage.set("orchester.devices", JSON.stringify([{ address: host.address, name: host.name }]));
  tokens.set(`orchester.token:${host.address}`, "expired-token");
  const { fleet } = mountFleet();
  await vi.waitFor(() => expect(fleet.hosts.value[0]?.control.connectionStatus.value).toBe("needs_pairing"));
  const calls = httpCalls.length;
  await vi.advanceTimersByTimeAsync(15000);
  expect(httpCalls).toHaveLength(calls);
  expect(fleet.panels.value).toEqual([]);
});

it("recovers an initially failed panel during a healthy host's next heartbeat", async () => {
  const host = hostFixture("http://alpha:7337", "Alpha");
  const { fleet } = mountFleet();
  const connected = (await fleet.connectSaved(host))!;
  host.offline = true;
  const panel = (await fleet.openPanel(connected, host.projects[1]!, "claude"))!;
  panel.control.prompt.value = "retry this project";
  host.offline = false;
  await vi.advanceTimersByTimeAsync(5000);
  expect(fleet.panels.value).toEqual([panel]);
  expect(panel.control.connectionStatus.value).toBe("connected");
  expect(panel.control.project.value?.id).toBe("service");
  expect(panel.control.agent.value).toBe("claude");
  expect(panel.control.prompt.value).toBe("retry this project");
});

it("does not reopen a panel closed while another panel is being re-paired", async () => {
  const host = hostFixture("http://alpha:7337", "Alpha");
  const { fleet } = mountFleet();
  const connected = (await fleet.connectSaved(host))!;
  const first = (await fleet.openPanel(connected, host.projects[0]!, "codex"))!;
  const closing = (await fleet.openPanel(connected, host.projects[1]!, "claude"))!;
  await vi.advanceTimersByTimeAsync(0);
  let release!: () => void;
  const path = "/runs/shared%20project-codex/events";
  const before = httpCalls.filter((call) => call.path === path).length;
  heldResponses.set(`${host.address}${path}`, new Promise<void>((resolve) => { release = resolve; }));
  host.token = "replacement-private-token";
  const pairing = fleet.pairDevice({ address: host.address, code: "new-code", name: "Phone" });
  await vi.waitFor(() => expect(httpCalls.filter((call) => call.path === path)).toHaveLength(before + 1));
  fleet.closePanel(closing);
  release();
  await pairing;
  expect(fleet.panels.value).toEqual([first]);
  expect(savedPanels()).toEqual([
    { hostAddress: host.address, projectId: "shared project", agent: "codex" },
  ]);
  expect(closing.scope.active).toBe(false);
});

it("does not restore a failed panel closed during another panel's recovery", async () => {
  const host = hostFixture("http://alpha:7337", "Alpha");
  const { fleet } = mountFleet();
  const connected = (await fleet.connectSaved(host))!;
  host.offline = true;
  const first = (await fleet.openPanel(connected, host.projects[0]!, "codex"))!;
  const closing = (await fleet.openPanel(connected, host.projects[1]!, "claude"))!;
  host.offline = false;
  let release!: () => void;
  const path = "/runs/shared%20project-codex/events";
  heldResponses.set(`${host.address}${path}`, new Promise<void>((resolve) => { release = resolve; }));
  const recovering = fleet.refreshHosts();
  await vi.waitFor(() => expect(httpCalls.some((call) => call.path === path)).toBe(true));
  fleet.closePanel(closing);
  release();
  await recovering;
  expect(fleet.panels.value).toHaveLength(1);
  expect(fleet.panels.value[0]).toBe(first);
  expect(savedPanels()).toEqual([
    { hostAddress: host.address, projectId: "shared project", agent: "codex" },
  ]);
  expect(closing.scope.active).toBe(false);
});
