<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import type { RemoteHost } from "../composables/use-remote-fleet";
import type { AppearanceTheme } from "../composables/use-appearance";
import type { Project } from "../client";
import { useRemoteTerminal } from "../composables/use-remote-terminal";
import {
  TERMINAL_KEYS,
  terminalState,
  terminalTheme,
  type TerminalSession,
} from "../terminal";
import AppIcon from "./AppIcon.vue";
import BottomPicker, { type PickerOption } from "./BottomPicker.vue";

const props = defineProps<{
  host: RemoteHost;
  project: Project;
  theme: AppearanceTheme;
  previousId?: string;
}>();
const emit = defineEmits<{ selected: [id: string] }>();
const surface = ref<HTMLElement>();
const compact = ref(false);
const accessible = ref(false);
const sessions = ref<TerminalSession[]>([]);
const busy = ref(false),
  loading = ref(true),
  pageError = ref(""),
  confirmClose = ref(false);
let xterm: Terminal | undefined,
  fit: FitAddon | undefined,
  observer: ResizeObserver | undefined;
let resizeFrame = 0,
  revision = 0,
  disposed = false;
let disposeInput: { dispose(): void } | undefined;
const remote = useRemoteTerminal({
  address: props.host.address,
  request: props.host.control.remoteRequest,
  write: (data) =>
    new Promise((resolve) => {
      if (xterm) xterm.write(data, resolve);
      else resolve();
    }),
  reset: () => xterm?.reset(),
});
const { terminal, connection, error, closing, canInput } = remote;
const connectionLabel = computed(
  () =>
    ({
      idle: "未打开",
      connecting: "正在连接",
      connected: "已连接",
      reconnecting: "正在重连",
      disconnected: "连接中断",
      finished: "会话已结束",
    })[connection.value],
);
const options = computed<PickerOption[]>(() =>
  sessions.value.map((row) => ({
    value: row.id,
    label: row.shell.split(/[\\/]/).pop() || "终端",
    description: row.id.slice(0, 8),
    status: terminalState(row),
    tone: row.state === "running" ? "secondary" : "tertiary",
  })),
);
const hostConnected = computed(
  () => props.host.control.connectionStatus.value === "connected",
);

function scheduleFit() {
  cancelAnimationFrame(resizeFrame);
  resizeFrame = requestAnimationFrame(() => {
    if (disposed || !xterm || !fit) return;
    const dims = fit.proposeDimensions();
    if (dims && Number.isFinite(dims.cols) && Number.isFinite(dims.rows)) {
      const cols = Math.max(2, Math.min(400, dims.cols)),
        rows = Math.max(2, Math.min(200, dims.rows));
      xterm.resize(cols, rows);
      remote.resize(cols, rows);
    }
  });
}
function viewportChanged() {
  const height = window.visualViewport?.height ?? window.innerHeight;
  compact.value = height <= 500;
  document.documentElement.style.setProperty(
    "--terminal-viewport",
    `${height}px`,
  );
  scheduleFit();
}
async function refresh(choose = false) {
  const current = ++revision;
  loading.value = true;
  pageError.value = "";
  try {
    const result = await props.host.control.remoteRequest<{
      terminals: TerminalSession[];
    }>("/terminals");
    if (disposed || current !== revision) return;
    sessions.value = result.terminals.filter(
      (row) => row.project_id === props.project.id,
    );
    if (choose) {
      const existing =
        sessions.value.find((row) => row.id === props.previousId) ??
        sessions.value.find((row) => row.state === "running") ??
        sessions.value[0];
      if (existing) select(existing.id);
    }
  } catch (reason) {
    if (!disposed && current === revision)
      pageError.value =
        reason instanceof Error ? reason.message : "无法读取终端会话";
  } finally {
    if (!disposed && current === revision) loading.value = false;
  }
}
function select(id: string) {
  const row = sessions.value.find((row) => row.id === id);
  if (!row) return;
  confirmClose.value = false;
  remote.attach(row);
  emit("selected", id);
  scheduleFit();
}
async function create() {
  if (busy.value || !hostConnected.value) return;
  busy.value = true;
  pageError.value = "";
  try {
    const row = await props.host.control.remoteRequest<TerminalSession>(
      "/terminals",
      "POST",
      {
        project_id: props.project.id,
        cols: xterm?.cols ?? 80,
        rows: xterm?.rows ?? 24,
      },
    );
    if (disposed) return;
    sessions.value = [row, ...sessions.value];
    select(row.id);
  } catch (reason) {
    if (!disposed)
      pageError.value =
        reason instanceof Error ? reason.message : "无法创建终端";
  } finally {
    if (!disposed) busy.value = false;
  }
}
function key(data: string) {
  xterm?.focus();
  remote.input(data);
}
watch(terminal, (row) => {
  if (row)
    sessions.value = sessions.value.map((item) =>
      item.id === row.id ? row : item,
    );
});
watch(canInput, (enabled) => {
  if (xterm) xterm.options.disableStdin = !enabled;
});
watch(accessible, (enabled) => {
  if (xterm) xterm.options.screenReaderMode = enabled;
});
watch(
  () => props.theme,
  (theme) => {
    if (xterm) xterm.options.theme = terminalTheme(theme);
  },
);
onMounted(async () => {
  xterm = new Terminal({
    theme: terminalTheme(props.theme),
    fontFamily: '"SFMono-Regular", Consolas, "Liberation Mono", monospace',
    fontSize: 13,
    cursorBlink: true,
    cursorStyle: "bar",
    scrollback: 3000,
    screenReaderMode: accessible.value,
    disableStdin: true,
    allowProposedApi: false,
  });
  fit = new FitAddon();
  xterm.loadAddon(fit);
  xterm.open(surface.value!);
  xterm.textarea?.setAttribute("aria-label", "终端输入");
  disposeInput = xterm.onData((data) => remote.input(data));
  observer = new ResizeObserver(scheduleFit);
  observer.observe(surface.value!);
  window.visualViewport?.addEventListener("resize", viewportChanged);
  window.addEventListener("resize", viewportChanged);
  viewportChanged();
  await nextTick();
  await refresh(true);
});
onBeforeUnmount(() => {
  disposed = true;
  revision++;
  cancelAnimationFrame(resizeFrame);
  observer?.disconnect();
  window.visualViewport?.removeEventListener("resize", viewportChanged);
  window.removeEventListener("resize", viewportChanged);
  document.documentElement.style.removeProperty("--terminal-viewport");
  disposeInput?.dispose();
  xterm?.dispose();
  xterm = undefined;
});
</script>

<template>
  <section
    class="terminal-page"
    :class="{ 'terminal-compact': compact }"
    aria-label="远程交互终端"
  >
    <div class="terminal-context">
      <div class="terminal-context-copy">
        <strong>{{ host.name }} · {{ project.name }}</strong
        ><small>{{ project.path }}</small>
      </div>
      <span
        class="terminal-connection"
        :data-connected="connection === 'connected'"
        role="status"
        >{{ connectionLabel }}</span
      >
    </div>
    <div class="terminal-toolbar">
      <BottomPicker
        v-if="sessions.length"
        :model-value="terminal?.id || ''"
        label="会话"
        title="选择终端会话"
        :description="project.name"
        icon="terminal"
        :options="options"
        :disabled="busy || closing"
        @update:model-value="select"
      />
      <button
        class="tonal-button terminal-new"
        :disabled="busy || loading || !hostConnected"
        @click="create"
      >
        <AppIcon name="task" />{{ busy ? "正在创建…" : "新建终端" }}
      </button>
      <button
        v-if="compact && terminal && !canInput && !closing && connection !== 'finished'"
        class="tonal-button terminal-compact-close"
        aria-label="重连"
        @click="remote.reconnect()"
      >
        <AppIcon name="refresh" />
      </button>
      <button
        v-else-if="compact && terminal?.state === 'running'"
        class="tonal-button terminal-compact-close"
        :disabled="closing"
        aria-label="关闭进程"
        @click="confirmClose = true"
      >
        <AppIcon name="stop" />
      </button>
      <span
        v-else-if="compact && terminal && terminal.state !== 'running'"
        class="terminal-exit"
        role="status"
      >{{ terminalState(terminal) }}</span>
    </div>
    <p v-if="pageError || error" class="terminal-error" role="alert">
      {{ pageError || error }}
    </p>
    <div
      v-if="confirmClose && terminal?.state === 'running'"
      class="terminal-confirm"
      role="alert"
    >
      <p>关闭电脑上的终端进程？其中运行的命令也会停止。</p>
      <div>
        <button class="text-button" @click="confirmClose = false">
          保留终端</button
        ><button
          class="tonal-button"
          @click="
            confirmClose = false;
            remote.closeProcess();
          "
        >
          关闭进程
        </button>
      </div>
    </div>
    <div class="terminal-surface-wrap">
      <div ref="surface" class="terminal-surface" aria-label="电脑终端" />
      <div v-if="!terminal" class="terminal-empty">
        <AppIcon name="terminal" />
        <p>{{ loading ? "正在读取终端会话…" : "打开这个项目的电脑终端" }}</p>
        <small>可以运行命令、使用交互工具；返回后可重新连接。</small>
      </div>
    </div>
    <div class="terminal-shortcuts" aria-label="终端快捷键">
      <button
        v-for="item in TERMINAL_KEYS"
        :key="item.label"
        :aria-label="'aria' in item ? item.aria : item.label"
        :disabled="!canInput"
        @click="key(item.data)"
      >
        {{ item.label }}
      </button>
    </div>
    <div class="terminal-footer">
      <button v-if="canInput" class="text-button" @click="xterm?.focus()">
        显示键盘</button
      ><button
        v-else-if="terminal && connection !== 'finished'"
        class="text-button"
        :disabled="closing"
        @click="remote.reconnect()"
      >
        <AppIcon name="refresh" />重连</button
      ><span
        v-if="terminal && terminal.state !== 'running'"
        class="terminal-exit"
        role="status"
        >{{ terminalState(terminal) }}</span
      ><button
        class="text-button"
        :aria-pressed="accessible"
        title="为屏幕阅读器提供终端输出"
        @click="accessible = !accessible"
      >
        辅助阅读</button
      ><button
        v-if="terminal?.state === 'running'"
        class="text-button terminal-close-process"
        :disabled="closing"
        @click="confirmClose = true"
      >
        <AppIcon name="stop" />{{ closing ? "正在关闭…" : "关闭进程" }}
      </button>
    </div>
    <p class="terminal-return-note">
      返回会断开手机连接，电脑进程保留；断开 5 分钟后自动关闭。
    </p>
  </section>
</template>

<style scoped>
.terminal-page {
  position: relative;
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  gap: 10px;
}
.terminal-context {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-shrink: 0;
}
.terminal-context-copy {
  min-width: 0;
  flex: 1;
}
.terminal-context strong {
  display: block;
  font-size: 13px;
  overflow-wrap: anywhere;
}
.terminal-context small {
  display: block;
  color: var(--muted);
  font-size: 11px;
  line-height: 1.5;
  margin-top: 4px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.terminal-connection {
  flex-shrink: 0;
  color: var(--muted);
  font-size: 11px;
}
.terminal-connection[data-connected="true"] {
  color: var(--secondary);
}
.terminal-toolbar {
  display: flex;
  gap: 8px;
  align-items: flex-end;
  flex-shrink: 0;
}
.terminal-toolbar .bottom-picker {
  flex: 1;
}
.terminal-toolbar :deep(.picker-trigger) {
  padding: 8px 10px;
  min-height: 48px;
}
.terminal-toolbar :deep(.picker-label),
.terminal-toolbar :deep(.picker-detail),
.terminal-toolbar :deep(.picker-status) {
  display: none;
}
.terminal-new {
  font-size: 12px;
  padding: 10px 12px;
  flex-shrink: 0;
}
.terminal-new svg {
  width: 18px;
  height: 18px;
}
.terminal-compact-close {
  display: grid;
  place-items: center;
  flex-shrink: 0;
  width: 48px;
  padding: 0;
}
.terminal-compact {
  gap: 6px;
}
.terminal-compact .terminal-context,
.terminal-compact .terminal-footer,
.terminal-compact .terminal-return-note {
  display: none;
}
.terminal-compact .terminal-surface-wrap {
  min-height: 40px;
}
.terminal-compact .terminal-confirm,
.terminal-compact .terminal-error {
  position: absolute;
  z-index: 3;
  top: 54px;
  left: 0;
  right: 0;
}
.terminal-error {
  padding: 10px 12px;
  border-radius: 12px;
  background: var(--raised);
  color: var(--primary);
  font-size: 12px;
  line-height: 1.5;
  overflow-wrap: anywhere;
  flex-shrink: 0;
}
.terminal-confirm {
  padding: 12px;
  border: 1px solid var(--line);
  border-radius: 14px;
  background: var(--surface);
  font-size: 13px;
  flex-shrink: 0;
}
.terminal-confirm > div {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 8px;
}
.terminal-surface-wrap {
  position: relative;
  flex: 1;
  min-height: 120px;
  border: 1px solid var(--line);
  border-radius: 18px;
  padding: 12px 8px 8px 12px;
  background: var(--bg);
  overflow: clip;
}
.terminal-surface {
  width: 100%;
  height: 100%;
}
.terminal-surface :deep(.xterm) {
  height: 100%;
}
.terminal-surface :deep(.xterm-viewport) {
  background: var(--bg);
  scrollbar-color: var(--line) var(--bg);
}
.terminal-empty {
  position: absolute;
  inset: 12px;
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  gap: 12px;
  text-align: center;
  color: var(--muted);
  pointer-events: none;
}
.terminal-empty svg {
  width: 32px;
  height: 32px;
  color: var(--primary);
}
.terminal-empty p {
  font-size: 14px;
}
.terminal-empty small {
  max-width: 240px;
  font-size: 12px;
  line-height: 1.6;
}
.terminal-shortcuts {
  display: flex;
  gap: 6px;
  overflow-x: auto;
  flex-shrink: 0;
  padding: 2px;
  overscroll-behavior-x: contain;
}
.terminal-shortcuts button {
  min-width: 48px;
  min-height: 48px;
  flex-shrink: 0;
  padding: 8px 10px;
  border-radius: 12px;
  background: var(--raised);
  color: var(--text);
  font-size: 12px;
}
.terminal-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  flex-shrink: 0;
}
.terminal-footer .text-button {
  min-height: 44px;
  font-size: 12px;
}
.terminal-close-process {
  margin-left: auto;
}
.terminal-exit {
  color: var(--muted);
  font-size: 12px;
}
.terminal-return-note {
  color: var(--muted);
  font-size: 10px;
  line-height: 1.5;
  flex-shrink: 0;
}
</style>
