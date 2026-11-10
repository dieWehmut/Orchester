<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  ref,
  shallowRef,
  watch,
} from "vue";
import { Capacitor } from "@capacitor/core";
import {
  useRemoteFleet,
  type RemoteHost,
  type SavedDevice,
} from "./composables/use-remote-fleet";
import { useAppearance } from "./composables/use-appearance";
import { usePageNavigation, type Tab, type Screen } from "./composables/use-page-navigation";
import type { FolderPage, Project, Run } from "./client";
import AppIcon from "./components/AppIcon.vue";
import AppearancePage from "./components/AppearancePage.vue";
import BottomPicker, { type PickerOption } from "./components/BottomPicker.vue";
import FolderBrowser from "./components/FolderBrowser.vue";
import AgentWorkbench from "./components/AgentWorkbench.vue";
import RunList from "./components/RunList.vue";
import RunOutput from "./components/RunOutput.vue";
import DevicePage from "./views/DevicePage.vue";
import TerminalPage from "./components/TerminalPage.vue";
const fleet = useRemoteFleet();
const { hosts, panels, error } = fleet;
const { theme, themes, setTheme } = useAppearance();
const { tab, screen, navigate, openPage, back } = usePageNavigation();
const heading = ref<HTMLElement>();
const navigation: { id: Tab; name: string; icon: string }[] = [
  { id: "dashboard", name: "控制台", icon: "dashboard" },
  { id: "tasks", name: "任务", icon: "agent" },
  { id: "projects", name: "项目", icon: "folder" },
  { id: "settings", name: "设置", icon: "settings" },
];
const titles: Record<Screen, string> = {
  dashboard: "控制台",
  tasks: "任务",
  projects: "项目",
  settings: "设置",
  devices: "电脑设备",
  appearance: "主题",
  about: "关于",
  folders: "选择电脑目录",
  agents: "选择 Agent",
  output: "任务详情",
  terminal: "终端",
};
const status: Record<string, string> = {
  disconnected: "未连接",
  connecting: "正在连接",
  connected: "已连接",
  reconnecting: "正在重连",
  needs_pairing: "需要配对",
};
const address = ref(""),
  code = ref(""),
  name = ref("我的手机"),
  pairing = ref(false);
const hostFilter = ref("all");
const hostOptions = computed<PickerOption[]>(() => [
  {
    value: "all",
    label: "全部设备",
    description: `${hosts.value.length} 台电脑`,
  },
  ...hosts.value.map((host) => ({
    value: host.address,
    label: host.name,
    description: host.address,
    status: status[host.control.connectionStatus.value] || "未连接",
    ...(host.control.connectionStatus.value === "connected"
      ? { tone: "secondary" as const }
      : {}),
  })),
]);
watch(hosts, (current) => {
  if (
    hostFilter.value !== "all" &&
    !current.some((host) => host.address === hostFilter.value)
  )
    hostFilter.value = "all";
});
const shownHosts = computed(() =>
  hostFilter.value === "all"
    ? hosts.value
    : hosts.value.filter((host) => host.address === hostFilter.value),
);
const selectedHost = shallowRef<RemoteHost | null>(null),
  selectedProject = ref<Project | null>(null);
const recentTerminals = new Map<string, string>();
const terminalKey = computed(
  () => `${selectedHost.value?.address}\0${selectedProject.value?.id}`,
);
function openTerminal(host: RemoteHost, project: Project) {
  selectedHost.value = host;
  selectedProject.value = project;
  openPage("terminal");
}
const outputControl = shallowRef<RemoteHost["control"] | null>(null);
const folder = ref<FolderPage | null>(null),
  folderPath = ref(""),
  folderBusy = ref(false),
  folderError = ref("");
const availableAgents = ref<
    { name: string; available: boolean; supports_resume: boolean }[]
  >([]),
  agentsBusy = ref(false),
  agentsError = ref("");
let folderRevision = 0,
  agentRevision = 0;
const columns = ref<1 | 2>(readColumns());
function readColumns(): 1 | 2 {
  try {
    return localStorage.getItem("orchester.panel-columns") === "1" ? 1 : 2;
  } catch {
    return 2;
  }
}
function setColumns(value: 1 | 2) {
  columns.value = value;
  try {
    localStorage.setItem("orchester.panel-columns", String(value));
  } catch {}
}
let navigationRevision = 0;
watch(screen, () => {
  navigationRevision++;
  folderRevision++;
  agentRevision++;
}, { flush: "sync" });
onBeforeUnmount(() => {
  navigationRevision++;
  folderRevision++;
  agentRevision++;
});
function usable(host: RemoteHost) {
  return host.control.connectionStatus.value === "connected";
}
function hostError(host: RemoteHost) {
  return host.control.error.value || host.syncError;
}
async function connect(device?: SavedDevice) {
  if (device) {
    await fleet.connectSaved(device);
    return;
  }
  pairing.value = true;
  const revision = navigationRevision;
  try {
    const host = await fleet.pairDevice({
      address: address.value,
      code: code.value,
      name: name.value,
    });
    if (host && revision === navigationRevision) {
      code.value = "";
      navigate("dashboard");
    }
  } finally {
    pairing.value = false;
  }
}
async function forget(device: SavedDevice) {
  const host = hosts.value.find((item) => item.address === device.address);
  if (host) await fleet.forgetHost(host);
}
async function browse(path?: string, more = false) {
  const host = selectedHost.value,
    revision = ++folderRevision;
  if (!host) return;
  folderBusy.value = true;
  folderError.value = "";
  try {
    const page = await host.control.browseFolders(
      path,
      more ? (folder.value?.next_offset ?? 0) : 0,
    );
    if (revision !== folderRevision || host !== selectedHost.value) return;
    folder.value =
      more && folder.value?.path === page.path
        ? { ...page, entries: [...folder.value.entries, ...page.entries] }
        : page;
    folderPath.value = page.path;
  } catch (error) {
    if (revision === folderRevision)
      folderError.value =
        error instanceof Error ? error.message : String(error);
  } finally {
    if (revision === folderRevision) folderBusy.value = false;
  }
}
function chooseDirectory(host: RemoteHost) {
  selectedHost.value = host;
  folder.value = null;
  folderPath.value = "";
  openPage("folders");
  void browse();
}
async function addProject(path: string) {
  const host = selectedHost.value, revision = navigationRevision;
  if (!host) return;
  folderBusy.value = true;
  folderError.value = "";
  try {
    const project = await host.control.addProject(path);
    if (revision !== navigationRevision) return;
    await chooseAgent(host, project);
  } catch (error) {
    if (revision === navigationRevision)
      folderError.value = error instanceof Error ? error.message : String(error);
  } finally {
    if (revision === navigationRevision) folderBusy.value = false;
  }
}
async function chooseAgent(host: RemoteHost, project: Project) {
  selectedHost.value = host;
  selectedProject.value = project;
  openPage("agents");
  availableAgents.value = [];
  agentsError.value = "";
  agentsBusy.value = true;
  const revision = ++agentRevision;
  try {
    const agents = await host.control.listAgents(project.id);
    if (revision === agentRevision) availableAgents.value = agents;
  } catch (error) {
    if (revision === agentRevision)
      agentsError.value =
        error instanceof Error ? error.message : String(error);
  } finally {
    if (revision === agentRevision) agentsBusy.value = false;
  }
}
async function openPanel(agent: string) {
  const host = selectedHost.value,
    project = selectedProject.value,
    revision = navigationRevision;
  if (!host || !project) return;
  const panel = await fleet.openPanel(host, project, agent);
  if (panel && revision === navigationRevision) navigate("tasks");
}
async function openRun(host: RemoteHost, run: Run) {
  selectedHost.value = host;
  outputControl.value = null;
  openPage("output");
  const revision = navigationRevision;
  const project = host.control.projects.value.find(
    (project) => project.id === run.project_id,
  );
  const panel = project
    ? await fleet.openPanel(host, project, run.agent)
    : null;
  if (revision !== navigationRevision) return;
  const control = panel?.control ?? host.control;
  outputControl.value = control;
  await control.action(() => control.open(run));
}
async function continueRun() {
  const host = selectedHost.value,
    run = outputControl.value?.selected.value,
    revision = navigationRevision;
  if (!host || !run) return;
  const project = host.control.projects.value.find(
    (project) => project.id === run.project_id,
  );
  if (!project) {
    error.value = "该任务的项目当前不可用";
    return;
  }
  const panel = await fleet.openPanel(host, project, run.agent);
  if (!panel || revision !== navigationRevision) return;
  await panel.control.action(() => panel.control.open(run));
  if (revision !== navigationRevision) return;
  panel.control.continueRun();
  navigate("tasks");
}
watch(screen, async () => {
  await nextTick();
  window.scrollTo(0, 0);
  heading.value?.focus({ preventScroll: true });
});
</script>
<template>
  <div
    class="app-shell"
    :class="{
      'wide-shell': screen === 'tasks' || screen === 'projects',
      'terminal-shell': screen === 'terminal',
    }"
  >
    <main class="page-content">
      <header class="page-header">
        <button
          v-if="!navigation.some((item) => item.id === screen)"
          class="back-button"
          aria-label="返回"
          @click="back"
        >
          <AppIcon name="back" />
        </button>
        <h1 ref="heading" tabindex="-1">{{ titles[screen] }}</h1>
        <button
          v-if="screen === 'dashboard' || screen === 'tasks'"
          class="text-button header-action"
          aria-label="刷新所有设备"
          @click="fleet.refreshHosts"
        >
          <AppIcon name="refresh" />
        </button>
      </header>
      <div v-if="error" class="error-banner" role="alert">
        <p>{{ error }}</p>
        <button class="text-button" aria-label="关闭提示" @click="error = ''">
          ×
        </button>
      </div>
      <template v-if="screen === 'dashboard'">
        <p class="page-intro">你的电脑与 Agent</p>
        <div v-if="!hosts.length" class="panel empty-state">
          <AppIcon name="computer" />
          <p>连接电脑后，在这里查看各设备的项目和任务。</p>
        </div>
        <article
          v-for="host in hosts"
          :key="host.address"
          class="panel device-dashboard"
        >
          <div class="section-line">
            <span class="icon-disc secondary"><AppIcon name="computer" /></span>
            <div class="row-copy">
              <h2>{{ host.name }}</h2>
              <small>{{ host.address }}</small>
            </div>
            <span
              class="connection-badge"
              :data-status="host.control.connectionStatus.value"
              ><i />{{ status[host.control.connectionStatus.value] }}</span
            >
          </div>
          <p v-if="hostError(host)" class="hint error-message" role="alert">
            {{ hostError(host) }}
          </p>
          <div class="device-stats">
            <span
              ><strong>{{
                usable(host) ? host.control.projects.value.length : "—"
              }}</strong
              >项目</span
            ><span
              ><strong>{{
                usable(host)
                  ? host.control.runs.value.filter(
                      (run) => run.state === "running",
                    ).length
                  : "—"
              }}</strong
              >运行中</span
            ><span
              ><strong>{{
                usable(host) ? host.control.runs.value.length : "—"
              }}</strong
              >任务</span
            >
          </div>
          <div class="output-actions">
            <button
              class="tonal-button"
              :disabled="!usable(host)"
              @click="
                hostFilter = host.address;
                navigate('projects');
              "
            >
              <AppIcon name="folder" />项目</button
            ><button
              class="tonal-button"
              :disabled="!usable(host)"
              @click="
                hostFilter = host.address;
                navigate('tasks');
              "
            >
              任务</button
            ><button
              v-if="!usable(host)"
              class="tonal-button"
              @click="
                host.control.connectionStatus.value === 'needs_pairing'
                  ? ((address = host.address), openPage('devices'))
                  : fleet.connectSaved(host)
              "
            >
              {{
                host.control.connectionStatus.value === "needs_pairing"
                  ? "重新配对"
                  : "重连"
              }}
            </button>
          </div>
        </article>
        <button class="primary-button full-button" @click="openPage('devices')">
          <AppIcon name="computer" />连接电脑
        </button>
      </template>
      <template v-else-if="screen === 'tasks'">
        <div class="section-line">
          <h2>Agent 工作栏</h2>
          <div class="filter-chips">
            <button :aria-pressed="columns === 1" @click="setColumns(1)">
              单栏</button
            ><button :aria-pressed="columns === 2" @click="setColumns(2)">
              双栏
            </button>
          </div>
        </div>
        <p class="hint">每栏独立显示一个设备、项目与 Agent。</p>
        <div
          v-if="panels.length"
          class="workbench-grid"
          :data-columns="columns"
        >
          <AgentWorkbench
            v-for="panel in panels"
            :key="panel.id"
            :panel="panel"
            :device-name="
              hosts.find((host) => host.address === panel.hostAddress)?.name ||
              panel.hostAddress
            "
            @close="fleet.closePanel(panel)"
          />
        </div>
        <div v-else class="panel empty-state">
          <AppIcon name="agent" />
          <p>从项目中选择 Agent，打开工作栏。</p>
          <button class="primary-button" @click="navigate('projects')">
            选择项目
          </button>
        </div>
        <h2 class="subheading">各设备的任务</h2>
        <BottomPicker
          v-if="hosts.length"
          v-model="hostFilter"
          class="device-filter"
          label="设备"
          title="选择设备"
          description="选择要查看任务记录的电脑"
          :options="hostOptions"
          inline
        />
        <section v-for="host in shownHosts" :key="host.address">
          <h2 class="subheading">{{ host.name }}</h2>
          <p class="hint project-path">{{ host.address }}</p>
          <p v-if="hostError(host)" class="error-banner" role="alert">
            {{ hostError(host) }}
          </p>
          <RunList
            v-if="host.control.runs.value.length"
            :runs="host.control.runs.value"
            :disabled="host.control.busy.value || !usable(host)"
            @open="openRun(host, $event)"
          />
          <p v-else class="panel hint">
            {{
              usable(host)
                ? "这台电脑还没有任务。"
                : "连接后读取这台电脑的任务。"
            }}
          </p>
        </section>
      </template>
      <template v-else-if="screen === 'projects'">
        <p class="page-intro">选择电脑上的真实项目目录</p>
        <BottomPicker
          v-if="hosts.length"
          v-model="hostFilter"
          class="device-filter"
          label="设备"
          title="选择设备"
          description="选择要浏览项目的电脑"
          :options="hostOptions"
          inline
        />
        <div v-if="!hosts.length" class="panel empty-state">
          <AppIcon name="folder" />
          <p>先连接一台电脑，再选择项目。</p>
          <button class="primary-button" @click="openPage('devices')">
            连接电脑
          </button>
        </div>
        <section
          v-for="host in shownHosts"
          :key="host.address"
          class="project-device"
        >
          <div class="section-line">
            <h2>{{ host.name }}</h2>
            <button
              class="tonal-button"
              :disabled="!usable(host)"
              @click="chooseDirectory(host)"
            >
              <AppIcon name="folder" />选择目录
            </button>
          </div>
          <p class="hint project-path">{{ host.address }}</p>
          <p v-if="hostError(host)" class="error-banner" role="alert">
            {{ hostError(host) }}
          </p>
          <div class="project-grid">
            <article
              v-for="project in host.control.projects.value"
              :key="project.id"
              class="panel"
            >
              <AppIcon name="folder" />
              <h2>{{ project.name }}</h2>
              <p class="project-path">{{ project.path }}</p>
              <button
                class="primary-button full-button"
                :disabled="!usable(host)"
                @click="chooseAgent(host, project)"
              >
                打开 Agent 栏
              </button>
              <button
                class="tonal-button full-button project-terminal"
                :disabled="!usable(host)"
                @click="openTerminal(host, project)"
              >
                <AppIcon name="terminal" />进入终端
              </button>
            </article>
          </div>
          <p v-if="!host.control.projects.value.length" class="panel hint">
            {{ usable(host) ? "选择电脑目录添加项目。" : "连接后读取项目。" }}
          </p>
        </section>
      </template>
      <template v-else-if="screen === 'settings'">
        <p class="page-intro">按你的方式使用 Orchester</p>
        <div class="list-panel">
          <button class="list-row" @click="openPage('devices')">
            <span class="icon-disc secondary"><AppIcon name="computer" /></span
            ><span class="row-copy"
              ><strong>电脑设备</strong
              ><small>连接、配对与管理电脑</small></span
            ><AppIcon name="chevron" /></button
          ><button class="list-row" @click="openPage('appearance')">
            <span class="icon-disc"><AppIcon name="palette" /></span
            ><span class="row-copy"
              ><strong>主题</strong><small>选择你的配色</small></span
            ><AppIcon name="chevron" />
          </button>
        </div>
        <section class="panel">
          <h2>工作栏布局</h2>
          <p class="hint">任务页同时显示多个项目和 Agent</p>
          <div class="filter-chips">
            <button :aria-pressed="columns === 1" @click="setColumns(1)">
              单栏</button
            ><button :aria-pressed="columns === 2" @click="setColumns(2)">
              双栏
            </button>
          </div>
        </section>
        <div class="list-panel">
          <button class="list-row" @click="openPage('about')">
            <span class="icon-disc"><AppIcon name="info" /></span
            ><span class="row-copy"
              ><strong>关于 Orchester</strong
              ><small>远程项目与 Agent</small></span
            ><AppIcon name="chevron" />
          </button>
        </div>
      </template>
      <TerminalPage
        v-else-if="screen === 'terminal' && selectedHost && selectedProject"
        :key="terminalKey"
        :host="selectedHost"
        :project="selectedProject"
        :theme="themes.find((item) => item.id === theme)!"
        v-bind="
          recentTerminals.has(terminalKey)
            ? { previousId: recentTerminals.get(terminalKey)! }
            : {}
        "
        @selected="recentTerminals.set(terminalKey, $event)"
      />
      <DevicePage
        v-else-if="screen === 'devices'"
        v-model:address="address"
        v-model:code="code"
        v-model:name="name"
        :devices="hosts"
        connected=""
        :busy="pairing"
        connection-status="disconnected"
        :native="Capacitor.isNativePlatform()"
        @connect="connect"
        @forget="forget"
      />
      <AppearancePage
        v-else-if="screen === 'appearance'"
        :theme="theme"
        :themes="themes"
        @select="setTheme"
      />
      <template v-else-if="screen === 'folders'"
        ><p class="page-intro">{{ selectedHost?.name }}</p>
        <FolderBrowser
          v-model:path="folderPath"
          :page="folder"
          :busy="folderBusy"
          :error="folderError"
          mode="project"
          @navigate="browse"
          @more="browse(folderPath, true)"
          @choose-directory="addProject"
      /></template>
      <template v-else-if="screen === 'agents'"
        ><p class="page-intro">
          {{ selectedHost?.name }} · {{ selectedProject?.name }}
        </p>
        <p class="project-path">{{ selectedProject?.path }}</p>
        <p v-if="agentsBusy" class="hint" role="status">
          正在读取项目的 Agent…
        </p>
        <p v-if="agentsError" class="error-banner" role="alert">
          {{ agentsError }}
        </p>
        <div class="list-panel">
          <button
            v-for="agent in availableAgents"
            :key="agent.name"
            class="list-row"
            :disabled="!agent.available"
            @click="openPanel(agent.name)"
          >
            <span class="icon-disc"><AppIcon name="agent" /></span
            ><span class="row-copy"
              ><strong>{{ agent.name }}</strong
              ><small>{{
                agent.available
                  ? agent.supports_resume
                    ? "可用 · 支持续接"
                    : "可用"
                  : "电脑尚未安装此 Agent"
              }}</small></span
            ><AppIcon name="chevron" />
          </button>
        </div>
        <p
          v-if="!agentsBusy && !agentsError && !availableAgents.length"
          class="panel hint"
        >
          此项目没有可用的 Agent。
        </p></template
      >
      <template v-else-if="screen === 'output' && selectedHost"
        ><p class="page-intro">{{ selectedHost.name }}</p>
        <p v-if="outputControl?.error.value" class="error-banner" role="alert">
          {{ outputControl.error.value }}
        </p>
        <RunOutput
          v-if="outputControl?.selected.value"
          :run="outputControl.selected.value"
          :events="outputControl.events.value"
          :busy="outputControl.busy.value"
          :disabled="outputControl.connectionStatus.value !== 'connected'"
          :resumable="outputControl.canResume(outputControl.selected.value)"
          @cancel="outputControl.cancel"
          @remove="outputControl.removeRun"
          @continue="continueRun"
        />
        <p v-else class="panel hint">正在读取任务…</p></template
      >
      <section v-else-if="screen === 'about'" class="panel">
        <h2>Orchester</h2>
        <p class="hint">
          连接你自己的电脑，在手机上选择项目目录，管理多个设备的 Agent
          和任务。运行能力与模型来自电脑已安装、配置的 CLI。
        </p>
      </section>
    </main>
    <nav class="bottom-navigation" aria-label="主导航">
      <button
        v-for="item in navigation"
        :key="item.id"
        :aria-current="tab === item.id ? 'page' : false"
        @click="navigate(item.id)"
      >
        <span class="nav-indicator"><AppIcon :name="item.icon" /></span
        ><span>{{ item.name }}</span>
      </button>
    </nav>
  </div>
</template>
