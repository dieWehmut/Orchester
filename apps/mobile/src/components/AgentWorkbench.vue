<script setup lang="ts">
import { computed, ref } from "vue";
import type { AgentPanel } from "../composables/use-remote-fleet";
import type { FolderEntry, FolderPage, Run } from "../client";
import { runState } from "../presentation";
import AppIcon from "./AppIcon.vue";
import BottomPicker, { type PickerOption } from "./BottomPicker.vue";
import FolderBrowser from "./FolderBrowser.vue";
import RunOutput from "./RunOutput.vue";
const props = defineProps<{ panel: AgentPanel; deviceName: string }>();
defineEmits<{ close: [] }>();
const control = props.panel.control;
const mode = ref<"agent" | "files">("agent");
const folder = ref<FolderPage | null>(null),
  path = ref(props.panel.project.path),
  folderBusy = ref(false),
  folderError = ref("");
let request = 0;
const disabled = computed(
  () => control.busy.value || control.connectionStatus.value !== "connected",
);
const tasks = computed(() =>
  control.runs.value.filter((run) => run.agent === props.panel.agent),
);
const taskOptions = computed<PickerOption[]>(() =>
  tasks.value.map((run) => ({
    value: run.id,
    label: run.prompt,
    status: runState(run.state),
    tone:
      run.state === "running"
        ? "primary"
        : run.state === "completed"
          ? "secondary"
          : "tertiary",
  })),
);
function chooseTask(id: string) {
  const run = tasks.value.find((task) => task.id === id);
  if (run) void open(run);
}
async function browse(target: string, more = false) {
  const revision = ++request;
  folderBusy.value = true;
  folderError.value = "";
  try {
    const page = await control.browseFolders(
      target,
      more ? (folder.value?.next_offset ?? 0) : 0,
    );
    if (revision !== request) return;
    folder.value =
      more && folder.value?.path === page.path
        ? { ...page, entries: [...folder.value.entries, ...page.entries] }
        : page;
    path.value = page.path;
  } catch (error) {
    if (revision === request)
      folderError.value = String(
        error instanceof Error ? error.message : error,
      );
  } finally {
    if (revision === request) folderBusy.value = false;
  }
}
function files() {
  mode.value = "files";
  if (!folder.value) void browse(path.value);
}
function attach(entry: FolderEntry) {
  if (!control.attachments.value.some((file) => file.path === entry.path))
    control.attachments.value = [...control.attachments.value, entry];
  mode.value = "agent";
}
async function open(run: Run) {
  await control.action(() => control.open(run));
}
</script>
<template>
  <article
    class="workbench panel"
    :aria-label="`${deviceName} ${panel.project.name} ${panel.agent}`"
  >
    <header class="workbench-heading">
      <div>
        <small>{{ deviceName }}</small>
        <h2>{{ panel.project.name }}</h2>
        <span class="agent-label"
          ><AppIcon name="agent" />{{ panel.agent }}</span
        >
      </div>
      <button class="text-button" aria-label="关闭此栏" @click="$emit('close')">
        ×
      </button>
    </header>
    <p class="project-path">{{ panel.project.path }}</p>
    <div class="filter-chips pane-tabs">
      <button :aria-pressed="mode === 'agent'" @click="mode = 'agent'">
        Agent
      </button>
      <button :aria-pressed="mode === 'files'" @click="files">
        <AppIcon name="folder" />文件
      </button>
    </div>
    <div class="workbench-scroll">
      <p v-if="control.error.value" class="error-banner" role="alert">
        {{ control.error.value }}
      </p>
      <FolderBrowser
        v-if="mode === 'files'"
        v-model:path="path"
        :page="folder"
        :busy="folderBusy"
        :error="folderError"
        mode="file"
        @navigate="browse"
        @more="browse(path, true)"
        @choose-file="attach"
      />
      <template v-else>
        <p v-if="control.connectionStatus.value !== 'connected'" class="hint">
          {{
            control.connectionStatus.value === "needs_pairing"
              ? "请在设置中重新配对这台电脑"
              : "正在连接电脑…"
          }}
        </p>
        <form class="pane-composer form-panel" @submit.prevent="control.submit">
          <p v-if="control.resume.value" class="hint">
            继续当前会话
            <button
              type="button"
              class="text-button"
              @click="control.resume.value = ''"
            >
              改为新会话
            </button>
          </p>
          <label
            >任务内容<textarea
              v-model="control.prompt.value"
              placeholder="让这个 Agent 在项目中做什么？"
              rows="3"
              required
              :disabled="disabled"
            />
          </label>
          <div v-if="control.attachments.value.length" class="attachment-list">
            <div v-for="file in control.attachments.value" :key="file.path">
              <span :title="file.path">{{ file.name }}</span
              ><button
                type="button"
                :aria-label="`移除文件 ${file.name}`"
                @click="
                  control.attachments.value = control.attachments.value.filter(
                    (item) => item.path !== file.path,
                  )
                "
              >
                ×
              </button>
            </div>
          </div>
          <div class="output-actions">
            <button
              type="button"
              class="tonal-button"
              :disabled="disabled"
              @click="files"
            >
              <AppIcon name="file" />选文件</button
            ><button
              class="primary-button"
              :disabled="
                disabled || !control.agent.value || !control.prompt.value.trim()
              "
            >
              <AppIcon name="send" />发送
            </button>
          </div>
        </form>
        <BottomPicker
          v-if="tasks.length"
          class="pane-task-picker"
          :model-value="control.selected.value?.id || ''"
          label="查看任务"
          title="选择任务"
          :description="`${deviceName} · ${panel.project.name} · ${panel.agent}`"
          placeholder="选择任务"
          icon="history"
          :options="taskOptions"
          :disabled="disabled"
          @update:model-value="chooseTask"
        />
        <RunOutput
          v-if="control.selected.value"
          :run="control.selected.value"
          :events="control.events.value"
          :busy="control.busy.value"
          :disabled="disabled"
          :resumable="control.canResume(control.selected.value)"
          @cancel="control.cancel"
          @continue="control.continueRun"
          @remove="control.removeRun"
        />
        <div v-else class="empty-state">
          <AppIcon name="agent" />
          <p>发送任务后，回复会显示在此栏。</p>
        </div>
      </template>
    </div>
  </article>
</template>
