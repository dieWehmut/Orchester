import {
  effectScope,
  markRaw,
  onMounted,
  onScopeDispose,
  ref,
  shallowRef,
  type EffectScope,
} from "vue";
import { useRemoteControl } from "./use-remote-control";
import { normalizeAddress, type Project } from "../client";
type Control = ReturnType<typeof useRemoteControl>;
export type SavedDevice = { address: string; name: string };
export type RemoteHost = SavedDevice & {
  control: Control;
  scope: EffectScope;
  syncError: string;
};
export type AgentPanel = {
  id: string;
  hostAddress: string;
  project: Project;
  agent: string;
  control: Control;
  scope: EffectScope;
};
type PanelMetadata = { hostAddress: string; projectId: string; agent: string };
export function useRemoteFleet() {
  const hosts = shallowRef<RemoteHost[]>([]),
    panels = shallowRef<AgentPanel[]>([]),
    error = ref("");
  const connecting = new Map<string, Promise<RemoteHost | null>>(),
    opening = new Map<string, Promise<AgentPanel | null>>();
  let disposed = false;
  const scopes = new Set<EffectScope>();
  const spawn = () => {
    const scope = effectScope(true);
    scopes.add(scope);
    return {
      scope,
      control: markRaw(
        scope.run(() => {
          onScopeDispose(() => scopes.delete(scope));
          return useRemoteControl();
        })!,
      ),
    };
  };
  function savedDevices(): SavedDevice[] {
    try {
      const stored: unknown = JSON.parse(
        localStorage.getItem("orchester.devices") || "[]",
      );
      if (!Array.isArray(stored)) return [];
      return stored.flatMap((device) => {
        if (
          !device ||
          typeof device.address !== "string" ||
          typeof device.name !== "string"
        )
          return [];
        try {
          return [
            { address: normalizeAddress(device.address), name: device.name },
          ];
        } catch {
          return [];
        }
      });
    } catch {
      return [];
    }
  }
  function metadata(): PanelMetadata[] {
    try {
      const value: unknown = JSON.parse(
        localStorage.getItem("orchester.panels") || "[]",
      );
      return Array.isArray(value)
        ? value
            .filter(
              (item): item is PanelMetadata =>
                typeof item?.hostAddress === "string" &&
                typeof item?.projectId === "string" &&
                typeof item?.agent === "string",
            )
            .slice(0, 12)
        : [];
    } catch {
      return [];
    }
  }
  // Keep offline panel preferences while other computers are still reconnecting.
  let remembered = metadata();
  function persist() {
    try {
      localStorage.setItem("orchester.panels", JSON.stringify(remembered));
    } catch {}
  }
  function remember(panel: AgentPanel) {
    const item = {
      hostAddress: panel.hostAddress,
      projectId: panel.project.id,
      agent: panel.agent,
    };
    remembered = remembered.filter(
      (p) =>
        !(
          p.hostAddress === item.hostAddress &&
          p.projectId === item.projectId &&
          p.agent === item.agent
        ),
    );
    remembered.push(item);
    remembered = remembered.slice(-12);
    persist();
  }
  async function restore(host: RemoteHost) {
    if (host.control.connectionStatus.value !== "connected") return;
    const targets = new Map(
      [...remembered, ...panels.value.map((panel) => ({
        hostAddress: panel.hostAddress,
        projectId: panel.project.id,
        agent: panel.agent,
      }))]
        .filter((item) => item.hostAddress === host.address)
        .map((item) => [JSON.stringify([item.projectId, item.agent]), item]),
    );
    for (const item of targets.values()) {
      if (disposed || !hosts.value.includes(host)) return;
      // Connected panels own their polling and selected run. Only retry clients
      // which never connected; an expired token still needs explicit pairing.
      const existing = panels.value.find((panel) =>
        panel.hostAddress === host.address &&
        panel.project.id === item.projectId && panel.agent === item.agent,
      );
      const wanted = existing || remembered.some((panel) =>
        panel.hostAddress === host.address &&
        panel.projectId === item.projectId && panel.agent === item.agent,
      );
      if (!wanted || existing?.control.connected.value) continue;
      const project = host.control.projects.value.find(
        (p) => p.id === item.projectId,
      );
      if (project) await openPanel(host, project, item.agent);
    }
  }
  function connectSaved(device: SavedDevice): Promise<RemoteHost | null> {
    if (disposed) return Promise.resolve(null);
    let address: string;
    try {
      address = normalizeAddress(device.address);
    } catch (e) {
      error.value = String(e);
      return Promise.resolve(null);
    }
    const inFlight = connecting.get(address);
    if (inFlight) return inFlight;
    const work = (async () => {
      let host = hosts.value.find((h) => h.address === address);
      if (!host) {
        host = markRaw({ ...device, address, ...spawn(), syncError: "" });
        hosts.value = [...hosts.value, host];
      }
      if (host.control.connectionStatus.value === "connected") {
        await host.control.action(host.control.refresh);
        await host.control.refreshAgentStatus();
        await restore(host);
        return host;
      }
      await host.control.connect({ address, name: device.name });
      if (disposed || !hosts.value.includes(host)) return null;
      host.name =
        host.control.devices.value.find((d) => d.address === address)?.name ||
        device.name;
      hosts.value = [...hosts.value];
      if (String(host.control.connectionStatus.value) === "connected") {
        await host.control.refreshAgentStatus();
        await restore(host);
      }
      return host;
    })().finally(() => connecting.delete(address));
    connecting.set(address, work);
    return work;
  }
  async function pairDevice(form: {
    address: string;
    code: string;
    name: string;
  }): Promise<RemoteHost | null> {
    if (disposed) return null;
    error.value = "";
    const worker = spawn();
    worker.control.address.value = form.address;
    worker.control.code.value = form.code;
    worker.control.name.value = form.name;
    await worker.control.connect();
    if (disposed || worker.control.connectionStatus.value !== "connected") {
      error.value = worker.control.error.value || "无法连接电脑";
      worker.scope.stop();
      return null;
    }
    const address = worker.control.connected.value,
      name =
        worker.control.devices.value.find((d) => d.address === address)?.name ||
        address;
    const old = hosts.value.find((h) => h.address === address);
    old?.scope.stop();
    const host = markRaw({ address, name, ...worker, syncError: "" });
    hosts.value = [...hosts.value.filter((h) => h.address !== address), host];
    // Rebind each panel to its requested project/agent, never the host default.
    for (const panel of panels.value.filter((p) => p.hostAddress === address)) {
      if (disposed || !hosts.value.includes(host)) return null;
      if (!panels.value.includes(panel)) continue;
      await startPanel(host, panel.project, panel.agent, true);
    }
    await host.control.refreshAgentStatus();
    await restore(host);
    return host;
  }
  function openPanel(
    host: RemoteHost,
    project: Project,
    agent: string,
  ): Promise<AgentPanel | null> {
    return startPanel(host, project, agent);
  }
  function startPanel(
    host: RemoteHost,
    project: Project,
    agent: string,
    reconnect = false,
  ): Promise<AgentPanel | null> {
    if (disposed || !hosts.value.includes(host)) return Promise.resolve(null);
    const id = JSON.stringify([host.address, project.id, agent]),
      existing = panels.value.find((p) => p.id === id);
    const inFlight = opening.get(id);
    if (inFlight && !reconnect) return inFlight;
    if (
      existing && !reconnect &&
      existing.control.connectionStatus.value === "connected" &&
      existing.control.project.value?.id === project.id &&
      existing.control.agent.value === agent
    ) return Promise.resolve(existing);
    if (!existing && panels.value.length >= 12) {
      error.value = "最多打开 12 个 Agent 栏，请先关闭一个栏";
      return Promise.resolve(null);
    }
    const panel = existing ?? markRaw({
      id,
      hostAddress: host.address,
      project,
      agent,
      ...spawn(),
    });
    if (!existing) panels.value = [...panels.value, panel];
    const selected = panel.control.selected.value;
    const active = () =>
      !disposed && hosts.value.includes(host) &&
      panels.value.includes(panel) && opening.get(id) === work;
    const work: Promise<AgentPanel | null> = (async () => {
      await panel.control.connect({ address: host.address, name: host.name });
      if (!active()) return null;
      panel.control.agent.value = "";
      if (panel.control.connectionStatus.value !== "connected") return panel;
      const target = panel.control.projects.value.find((p) => p.id === project.id);
      if (!target) {
        panel.control.project.value = null;
        panel.control.runs.value = [];
        error.value = "该项目已不可用，请重新选择项目";
        return panel;
      }
      await panel.control.selectProject(target);
      if (!active()) return null;
      panel.control.agent.value = "";
      const available = panel.control.agents.value.find(
        (a) => a.name === agent && a.available,
      );
      if (!available) {
        error.value = "该项目的 Agent 不可用，请在电脑安装并登录后重试";
        return panel;
      }
      panel.control.agent.value = agent;
      const matching = panel.control.runs.value.filter((r) => r.agent === agent);
      const run = matching.find((r) => r.id === selected?.id) ?? matching[0];
      if (run) await panel.control.open(run);
      if (!active()) return null;
      remember(panel);
      return panel;
    })().finally(() => {
      if (opening.get(id) === work) opening.delete(id);
    });
    opening.set(id, work);
    return work;
  }
  function closePanel(panel: AgentPanel) {
    if (!panels.value.includes(panel)) return;
    opening.delete(panel.id);
    panel.scope.stop();
    panels.value = panels.value.filter((p) => p !== panel);
    remembered = remembered.filter(
      (p) =>
        !(
          p.hostAddress === panel.hostAddress &&
          p.projectId === panel.project.id &&
          p.agent === panel.agent
        ),
    );
    persist();
  }
  async function forgetHost(host: RemoteHost) {
    await host.control.forget({ address: host.address, name: host.name });
    if (host.control.error.value) {
      error.value = host.control.error.value;
      return;
    }
    for (const panel of [...panels.value].filter(
      (p) => p.hostAddress === host.address,
    ))
      closePanel(panel);
    remembered = remembered.filter((p) => p.hostAddress !== host.address);
    persist();
    host.scope.stop();
    hosts.value = hosts.value.filter((h) => h !== host);
  }
  let heartbeat: ReturnType<typeof setInterval> | undefined,
    refreshing = false;
  async function refreshHosts() {
    if (refreshing || disposed) return;
    refreshing = true;
    try {
      await Promise.allSettled(
        hosts.value
          .filter(
            (h) =>
              h.control.connectionStatus.value !== "needs_pairing",
          )
          .map(async (host) => {
            if (!host.control.connected.value) {
              await connectSaved(host);
              host.syncError = host.control.error.value;
              return;
            }
            await host.control.action(async () => {
              await Promise.all([
                host.control.refresh(),
                host.control.refreshProjects(),
              ]);
            });
            await host.control.refreshAgentStatus();
            await restore(host);
            host.syncError = host.control.error.value;
          }),
      );
      if (!disposed) hosts.value = [...hosts.value];
    } finally {
      refreshing = false;
    }
  }
  onMounted(async () => {
    const queue = savedDevices();
    let index = 0;
    await Promise.allSettled(
      Array.from({ length: Math.min(4, queue.length) }, async () => {
        while (!disposed && index < queue.length) {
          const device = queue[index++];
          if (device) await connectSaved(device);
        }
      }),
    );
  });
  heartbeat = setInterval(() => void refreshHosts(), 5000);
  onScopeDispose(() => {
    disposed = true;
    clearInterval(heartbeat);
    for (const scope of [...scopes]) scope.stop();
    hosts.value = [];
    panels.value = [];
  });
  return {
    hosts,
    panels,
    error,
    pairDevice,
    connectSaved,
    forgetHost,
    openPanel,
    closePanel,
    refreshHosts,
  };
}
