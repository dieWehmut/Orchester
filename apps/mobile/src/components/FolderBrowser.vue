<script setup lang="ts">
import { useId } from "vue";
import AppIcon from "./AppIcon.vue";

type BrowserEntry = {
  name: string;
  path: string;
  type: "directory" | "file";
};

type BrowserPage = {
  path: string;
  parent_path: string | null;
  roots: string[];
  entries: BrowserEntry[];
  next_offset: number | null;
};

const props = defineProps<{
  page: BrowserPage | null;
  busy: boolean;
  error: string;
  mode: "project" | "file";
}>();

const fieldId = useId();
const path = defineModel<string>("path", { required: true });
const emit = defineEmits<{
  navigate: [path: string];
  chooseDirectory: [path: string];
  chooseFile: [entry: BrowserEntry];
  more: [];
}>();

function navigateTo(target: string) {
  if (!props.busy && target.trim()) emit("navigate", target);
}

function openEntry(entry: BrowserEntry) {
  if (props.busy) return;
  if (entry.type === "directory") emit("navigate", entry.path);
  else if (props.mode === "file") emit("chooseFile", entry);
}

function chooseDirectory() {
  if (props.page && !props.busy) emit("chooseDirectory", props.page.path);
}
</script>

<template>
  <section class="folder-browser" :aria-busy="busy" aria-label="电脑目录浏览">
    <form class="panel path-panel" @submit.prevent="navigateTo(path)">
      <label class="path-label" :for="fieldId">电脑上的绝对路径</label>
      <div class="path-controls">
        <input
          :id="fieldId"
          v-model="path"
          type="text"
          name="computer-folder-path"
          autocomplete="off"
          autocapitalize="none"
          autocorrect="off"
          :spellcheck="false"
          :disabled="busy"
          :aria-describedby="`${fieldId}-hint`"
          required
        />
        <button
          class="tonal-button go-button"
          type="submit"
          :disabled="busy || !path.trim()"
        >
          前往
        </button>
      </div>
      <p :id="`${fieldId}-hint`" class="hint">
        {{
          mode === "project"
            ? "打开电脑上的项目目录，再选择此文件夹。"
            : "打开电脑上的目录，选择要使用的文件。"
        }}
      </p>
    </form>

    <div v-if="page?.roots.length" class="roots-panel">
      <p class="hint roots-label">可访问的根目录</p>
      <div class="root-chips">
        <button
          v-for="root in page.roots"
          :key="root"
          class="tonal-button root-chip"
          type="button"
          :disabled="busy"
          :aria-current="page.path === root ? 'location' : false"
          @click="navigateTo(root)"
        >
          {{ root }}
        </button>
      </div>
    </div>

    <div v-if="page" class="directory-header">
      <button
        class="tonal-button parent-button"
        type="button"
        :disabled="busy || !page.parent_path"
        @click="page.parent_path && navigateTo(page.parent_path)"
      >
        <AppIcon name="back" />
        上一级
      </button>
      <p class="current-path">{{ page.path }}</p>
    </div>

    <button
      v-if="mode === 'project' && page"
      class="primary-button choose-button"
      type="button"
      :disabled="busy"
      @click="chooseDirectory"
    >
      使用此文件夹
    </button>

    <p v-if="error" class="panel browser-message error-message" role="alert">
      {{ error }}
    </p>
    <p v-if="busy" class="hint loading-message" role="status">正在读取目录…</p>

    <div v-if="page && page.entries.length" class="list-panel directory-list">
      <component
        :is="entry.type === 'directory' || mode === 'file' ? 'button' : 'div'"
        v-for="entry in page.entries"
        :key="entry.path"
        class="list-row directory-row"
        :class="{ 'static-file': entry.type === 'file' && mode === 'project' }"
        :type="
          entry.type === 'directory' || mode === 'file' ? 'button' : undefined
        "
        :disabled="
          entry.type === 'directory' || mode === 'file' ? busy : undefined
        "
        @click="openEntry(entry)"
      >
        <span
          class="entry-icon"
          :class="{ 'file-icon': entry.type === 'file' }"
        >
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
            focusable="false"
          >
            <path
              v-if="entry.type === 'directory'"
              d="M3 7V5a2 2 0 0 1 2-2h5l2 3h7a2 2 0 0 1 2 2v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7Z M3 8h18"
            />
            <path
              v-else
              d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9l-6-6Z M14 3v6h6 M8 13h8 M8 17h5"
            />
          </svg>
        </span>
        <span class="row-copy">
          <strong>{{ entry.name }}</strong>
          <small>{{ entry.type === "directory" ? "文件夹" : "文件" }}</small>
        </span>
        <AppIcon
          v-if="entry.type === 'directory' || mode === 'file'"
          class="chevron"
          name="chevron"
        />
      </component>
    </div>

    <p v-else-if="page && !busy && !error" class="panel browser-message hint">
      此目录为空
    </p>
    <p v-else-if="!page && !busy && !error" class="panel browser-message hint">
      输入电脑上的绝对路径以浏览目录。
    </p>

    <button
      v-if="page && page.next_offset !== null"
      class="tonal-button more-button"
      type="button"
      :disabled="busy"
      @click="emit('more')"
    >
      加载更多
    </button>


  </section>
</template>

<style scoped>
.folder-browser {
  min-width: 0;
  color: var(--text);
}

.path-panel {
  background: var(--surface);
}

.path-label {
  display: block;
  margin-bottom: 10px;
  font-size: 14px;
  font-weight: 600;
}

.path-controls {
  display: flex;
  align-items: stretch;
  gap: 10px;
}

.path-controls input {
  width: 100%;
  min-width: 0;
  min-height: 48px;
  padding: 10px 12px;
  border: 1px solid var(--line);
  border-radius: 14px;
  background: var(--bg);
  color: var(--text);
  font-size: 16px;
}

.path-panel .hint {
  margin-top: 12px;
}

.go-button {
  flex-shrink: 0;
  padding-inline: 18px;
}

.roots-panel {
  margin: 18px 0;
}

.roots-label {
  margin-bottom: 8px;
}

.root-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.root-chip {
  max-width: 100%;
  min-height: 44px;
  padding: 10px 14px;
  border: 1px solid var(--line);
  border-radius: 22px;
  background: var(--raised);
  text-align: left;
  overflow-wrap: anywhere;
}

.root-chip[aria-current="location"] {
  border-color: var(--primary);
  color: var(--primary);
}

.directory-header {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 12px;
  margin: 18px 0 12px;
}

.parent-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  flex-shrink: 0;
  padding-inline: 14px;
}

.parent-button svg {
  width: 18px;
  height: 18px;
}

.current-path {
  flex: 1 1 140px;
  min-width: 0;
  color: var(--muted);
  font-size: 13px;
  line-height: 1.6;
  overflow-wrap: anywhere;
}

.directory-list {
  padding: 4px 0;
}

.directory-row {
  display: flex;
  align-items: center;
  gap: 14px;
  width: 100%;
  min-height: 76px;
  padding: 16px;
  color: var(--text);
  text-align: left;
}

.directory-row + .directory-row {
  border-top: 1px solid var(--line);
}

.directory-row .row-copy strong {
  overflow-wrap: anywhere;
}

.entry-icon {
  display: grid;
  place-items: center;
  flex-shrink: 0;
  width: 44px;
  height: 44px;
  border-radius: 14px;
  background: var(--raised);
  color: var(--primary);
}

.file-icon {
  color: var(--muted);
}

.static-file {
  cursor: default;
}

.browser-message {
  padding: 20px;
  line-height: 1.6;
  overflow-wrap: anywhere;
}

.error-message {
  border: 1px solid var(--primary);
  color: var(--text);
}

.loading-message {
  margin: 12px 0;
}

.more-button,
.choose-button {
  display: block;
  width: 100%;
  margin-top: 12px;
  min-height: 48px;
}

.tonal-button {
  min-height: 44px;
  border-radius: 16px;
  background: var(--raised);
  color: var(--text);
}

.root-chip {
  border-radius: 22px;
}

.choose-button {
  background: var(--primary);
  color: var(--on-primary);
}

button:focus-visible,
input:focus-visible {
  outline: 3px solid var(--primary);
  outline-offset: 3px;
}
</style>
