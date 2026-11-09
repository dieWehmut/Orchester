import { computed, ref, onScopeDispose } from "vue";
import { credentials } from "../credentials";
import {
  RemoteClient,
  RemoteError,
  newIdempotencyKey,
  normalizeAddress,
  type Run,
  type Events,
  type Project,
  type FolderEntry,
  type FolderPage,
} from "../client";
export function useRemoteControl() {
  type Device = {
    address: string;
    name: string;
  };
  const devices = ref<Device[]>([]);
  try {
    devices.value = JSON.parse(
      localStorage.getItem("orchester.devices") || "[]",
    ).filter(
      (d: Device) =>
        typeof d.address === "string" && typeof d.name === "string",
    );
  } catch {}
  const address = ref(""),
    code = ref(""),
    name = ref("我的手机"),
    actionError = ref(""),
    pollError = ref(""),
    busy = ref(false),
    connected = ref("");
  const connectionStatus = ref<
    | "disconnected"
    | "connecting"
    | "connected"
    | "reconnecting"
    | "needs_pairing"
  >("disconnected");
  const error = computed(() => actionError.value || pollError.value);
  const agents = ref<
      {
        name: string;
        available: boolean;
        supports_resume: boolean;
      }[]
    >([]),
    agent = ref(""),
    prompt = ref(""),
    runs = ref<Run[]>([]),
    selected = ref<Run | null>(null),
    events = ref<Events["events"]>([]),
    resume = ref("");
  const projects = ref<Project[]>([]),
    project = ref<Project | null>(null),
    attachments = ref<FolderEntry[]>([]);
  let projectScoped = false,
    projectRevision = 0;
  let client: RemoteClient | null = null,
    cursor = 0,
    generation = 0,
    connection = 0,
    historyRequest = 0,
    operation = 0,
    timer: ReturnType<typeof setTimeout> | undefined;
  let pending: {
    signature: string;
    key: string;
  } | null = null;
  const credentialKey = (host: string) => `orchester.token:${host}`;
  function persist(removeAddress?: string, addedDevice?: Device) {
    let saved: Device[] = [];
    try {
      const value: unknown = JSON.parse(
        localStorage.getItem("orchester.devices") || "[]",
      );
      if (Array.isArray(value))
        saved = value.filter(
          (d) => typeof d?.address === "string" && typeof d?.name === "string",
        );
    } catch {}
    const merged = new Map(saved.map((device) => [device.address, device]));
    if (addedDevice) merged.set(addedDevice.address, addedDevice);
    if (removeAddress) merged.delete(removeAddress);
    devices.value = [...merged.values()];
    try {
      localStorage.setItem("orchester.devices", JSON.stringify(devices.value));
    } catch {}
  }
  async function action(work: () => Promise<void>) {
    const id = ++operation;
    busy.value = true;
    actionError.value = "";
    try {
      await work();
    } catch (e) {
      if (id === operation) {
        actionError.value =
          e instanceof Error ? e.message : "连接失败，请检查网络";
        if (e instanceof RemoteError && e.status === 401)
          connectionStatus.value = "needs_pairing";
        else if (connectionStatus.value === "connecting")
          connectionStatus.value = "disconnected";
      }
    } finally {
      if (id === operation) busy.value = false;
    }
  }
  function clearSelected() {
    generation++;
    clearTimeout(timer);
    timer = undefined;
    selected.value = null;
    events.value = [];
    cursor = 0;
    pollError.value = "";
  }
  function reset() {
    connection++;
    historyRequest++;
    projectRevision++;
    projectScoped = false;
    projects.value = [];
    project.value = null;
    attachments.value = [];
    clearSelected();
    runs.value = [];
    agents.value = [];
    connected.value = "";
    connectionStatus.value = "disconnected";
    client = null;
    resume.value = "";
  }
  async function connect(device?: Device) {
    await action(async () => {
      reset();
      const version = connection,
        host = normalizeAddress(device?.address ?? address.value);
      // A failed saved-device reconnect must still leave the pairing form
      // pointing at that computer, never at the previously connected one.
      address.value = host;
      if (device) code.value = "";
      connectionStatus.value = "connecting";
      const saved = await credentials.get({ key: credentialKey(host) });
      if (version !== connection) return;
      const next = new RemoteClient(host, saved.value ?? "");
      if ((!device && code.value.trim()) || !saved.value) {
        if (device || !code.value.trim()) {
          connectionStatus.value = "needs_pairing";
          throw new Error("请输入电脑显示的配对码");
        }
        const paired = await next.pair(code.value.trim(), name.value);
        if (version !== connection) return;
        await credentials.set({
          key: credentialKey(host),
          value: paired.token,
        });
        if (version !== connection) return;
        code.value = "";
      }
      const info = await next.request<{
        name: string;
        default_project_id?: string;
      }>("/info");
      const catalog = info.default_project_id
        ? await next.request<{ projects: Project[] }>("/projects")
        : { projects: [] };
      const list = await next.request<{
        agents: {
          name: string;
          available: boolean;
          supports_resume: boolean;
        }[];
      }>("/agents");
      if (version !== connection) return;
      client = next;
      connected.value = host;
      connectionStatus.value = "connected";
      address.value = host;
      projects.value = catalog.projects;
      project.value =
        catalog.projects.find((p) => p.id === info.default_project_id) ?? null;
      agents.value = list.agents;
      agent.value = list.agents.find((a) => a.available)?.name ?? "";
      devices.value = devices.value.filter((d) => d.address !== host);
      persist(undefined, { address: host, name: info.name });
      await refresh();
    });
  }
  async function refresh() {
    const current = client;
    if (!current) return;
    const revision = ++historyRequest;
    try {
      const list = await current.request<{ runs: Run[] }>(
        projectScoped && project.value
          ? `/runs?project_id=${encodeURIComponent(project.value.id)}`
          : "/runs",
      );
      if (current !== client || revision !== historyRequest) return;
      runs.value = list.runs;
      const updated = list.runs.find((r) => r.id === selected.value?.id);
      if (updated) selected.value = updated;
      else if (selected.value) clearSelected();
      if (connectionStatus.value === "reconnecting")
        connectionStatus.value = "connected";
    } catch (e) {
      if (current === client && revision === historyRequest) throw e;
    }
  }
  async function open(run: Run) {
    generation++;
    clearTimeout(timer);
    selected.value = run;
    events.value = [];
    cursor = 0;
    pollError.value = "";
    await poll(generation);
  }
  async function poll(version: number) {
    const current = client,
      run = selected.value;
    if (!current || !run) return;
    try {
      const page = await current.request<Events>(
        `/runs/${encodeURIComponent(run.id)}/events?after=${cursor}`,
      );
      if (version !== generation || current !== client) return;
      events.value.push(...page.events);
      cursor = page.next_cursor;
      if (selected.value?.id === run.id) {
        // A history request begun before this lifecycle transition is stale.
        if (selected.value.state !== page.state) historyRequest++;
        selected.value.state = page.state;
        runs.value = runs.value.map((task) =>
          task.id === run.id ? { ...task, state: page.state } : task,
        );
      }
      pollError.value = "";
      if (connectionStatus.value === "reconnecting")
        connectionStatus.value = "connected";
      if (page.events.length || page.state === "running")
        timer = setTimeout(
          () => void poll(version),
          page.events.length ? 0 : 1000,
        );
      else await refresh();
    } catch (e) {
      if (version !== generation) return;
      if (e instanceof RemoteError && [401, 404].includes(e.status)) {
        if (e.status === 401) {
          actionError.value = "";
          connectionStatus.value = "needs_pairing";
        } else {
          historyRequest++;
          runs.value = runs.value.filter((task) => task.id !== run.id);
          clearSelected();
          if (connectionStatus.value === "reconnecting")
            connectionStatus.value = "connected";
        }
        pollError.value = e.message;
        return;
      }
      pollError.value = "连接中断，正在重试；电脑上的任务会继续运行";
      connectionStatus.value = "reconnecting";
      timer = setTimeout(() => void poll(version), 3000);
    }
  }
  async function submit() {
    await action(async () => {
      const current = client,
        revision = projectRevision;
      if (!current || !prompt.value.trim()) return;
      const draft = prompt.value.trim(),
        files = JSON.stringify(attachments.value.map((file) => file.path));
      const data = {
        agent: agent.value,
        ...(project.value ? { project_id: project.value.id } : {}),
        prompt:
          draft +
          (attachments.value.length
            ? "\n\n参考文件（在这台电脑上）：\n" +
              attachments.value.map((file) => "- " + file.path).join("\n")
            : ""),
        ...(resume.value ? { resume: resume.value } : {}),
      };
      const signature = JSON.stringify([current.address, data]);
      if (pending?.signature !== signature)
        pending = { signature, key: newIdempotencyKey() };
      const submission = pending;
      const run = await current.request<Run>("/runs", "POST", {
        ...data,
        idempotency_key: submission.key,
      });
      if (pending === submission) pending = null;
      if (current !== client || revision !== projectRevision) return;
      if (
        prompt.value.trim() === draft &&
        resume.value === (data.resume ?? "")
      ) {
        prompt.value = "";
        resume.value = "";
        if (
          JSON.stringify(attachments.value.map((file) => file.path)) === files
        )
          attachments.value = [];
      }
      // The POST response proves this task exists. A history read may fail or
      // arrive from before submission; neither may hide an accepted task.
      historyRequest++;
      runs.value = [run, ...runs.value.filter((task) => task.id !== run.id)];
      await open(run);
    });
  }
  async function cancel() {
    await action(async () => {
      const current = client,
        run = selected.value;
      if (current && run) {
        await current.request<Run>(
          `/runs/${encodeURIComponent(run.id)}/cancel`,
          "POST",
          {},
        );
        if (current === client) await refresh();
      }
    });
  }
  async function removeRun() {
    await action(async () => {
      const current = client,
        run = selected.value;
      if (!current || !run) return;
      await current.request(`/runs/${encodeURIComponent(run.id)}`, "DELETE");
      if (current !== client) return;
      if (selected.value?.id === run.id) {
        clearSelected();
      }
      await refresh();
    });
  }
  async function forget(d: Device) {
    await action(async () => {
      if (connected.value === d.address || address.value === d.address) reset();
      await credentials.remove({ key: credentialKey(d.address) });
      devices.value = devices.value.filter((x) => x.address !== d.address);
      persist(d.address);
    });
  }
  function canResume(run: Run) {
    return (
      !!run.session_id &&
      run.state !== "running" &&
      (!run.project_id ||
        !project.value ||
        run.project_id === project.value.id) &&
      agents.value.some(
        (a) => a.name === run.agent && a.available && a.supports_resume,
      )
    );
  }
  function continueRun() {
    if (
      selected.value &&
      canResume(selected.value) &&
      selected.value.session_id
    ) {
      resume.value = selected.value.session_id;
      agent.value = selected.value.agent;
    }
  }
  async function refreshProjects() {
    const current = client;
    if (!current) throw new Error("请先连接电脑");
    const catalog = await current.request<{ projects: Project[] }>("/projects");
    if (current === client) projects.value = catalog.projects;
  }
  async function selectProject(target: Project) {
    await action(async () => {
      const current = client,
        revision = ++projectRevision;
      if (!current) throw new Error("请先连接电脑");
      historyRequest++;
      clearSelected();
      projectScoped = true;
      project.value = target;
      resume.value = "";
      attachments.value = [];
      runs.value = [];
      agents.value = [];
      agent.value = "";
      const list = await current.request<{
        agents: {
          name: string;
          available: boolean;
          supports_resume: boolean;
        }[];
      }>(`/agents?project_id=${encodeURIComponent(target.id)}`);
      if (current !== client || revision !== projectRevision) return;
      agents.value = list.agents;
      agent.value = list.agents.find((a) => a.available)?.name ?? "";
      await refresh();
    });
  }
  async function listAgents(projectId: string) {
    if (!client) throw new Error("请先连接电脑");
    return (
      await client.request<{
        agents: {
          name: string;
          available: boolean;
          supports_resume: boolean;
        }[];
      }>(`/agents?project_id=${encodeURIComponent(projectId)}`)
    ).agents;
  }
  async function browseFolders(path?: string, offset = 0): Promise<FolderPage> {
    if (!client) throw new Error("请先连接电脑");
    const query = new URLSearchParams({ offset: String(offset) });
    if (path) query.set("path", path);
    return client.request<FolderPage>(`/folders?${query}`);
  }
  async function addProject(path: string): Promise<Project> {
    if (!client) throw new Error("请先连接电脑");
    const added = await client.request<Project>("/projects", "POST", { path });
    await refreshProjects();
    return added;
  }
  async function remoteRequest<T>(
    path: string,
    method = "GET",
    data?: unknown,
  ): Promise<T> {
    if (!client) throw new Error("请先连接电脑");
    try {
      return await client.request<T>(path, method, data);
    } catch (error) {
      if (error instanceof RemoteError && error.status === 401)
        connectionStatus.value = "needs_pairing";
      throw error;
    }
  }
  onScopeDispose(() => {
    reset();
    operation++;
  });
  return {
    projects,
    project,
    attachments,
    refreshProjects,
    selectProject,
    listAgents,
    browseFolders,
    addProject,
    remoteRequest,
    devices,
    address,
    code,
    name,
    error,
    busy,
    connected,
    connectionStatus,
    agents,
    agent,
    prompt,
    runs,
    selected,
    events,
    resume,
    action,
    connect,
    refresh,
    open,
    submit,
    cancel,
    removeRun,
    forget,
    canResume,
    continueRun,
  };
}
