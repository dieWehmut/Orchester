<script setup lang="ts">
import { Maximize2, Minus, Square, X } from '@lucide/vue'
import { onMounted, ref } from 'vue'

import {
  desktopWindow,
  type DesktopWindowController,
} from '../../platform/desktop-window'

const props = withDefaults(
  defineProps<{
    controller?: DesktopWindowController
    title?: string
    minimizeLabel?: string
    maximizeLabel?: string
    restoreLabel?: string
    closeLabel?: string
  }>(),
  {
    title: 'Orchester',
    minimizeLabel: 'Minimize window',
    maximizeLabel: 'Maximize window',
    restoreLabel: 'Restore window',
    closeLabel: 'Close window',
  },
)

const controller = props.controller ?? desktopWindow
const maximized = ref(false)

async function syncMaximized(): Promise<void> {
  if (!controller.enabled) return
  maximized.value = await controller.isMaximized()
}

async function minimize(): Promise<void> {
  await controller.minimize()
}

async function toggleMaximize(): Promise<void> {
  await controller.toggleMaximize()
  await syncMaximized()
}

async function close(): Promise<void> {
  await controller.close()
}

async function startDragging(): Promise<void> {
  await controller.startDragging()
}

onMounted(() => {
  void syncMaximized()
})
</script>

<template>
  <div
    v-if="controller.enabled"
    class="window-chrome"
    data-window-chrome
    @dblclick.self="toggleMaximize"
  >
    <button
      class="window-chrome__drag-region"
      data-tauri-drag-region
      type="button"
      :aria-label="title"
      :title="title"
      @mousedown="startDragging"
      @dblclick="toggleMaximize"
    >
      <span class="window-chrome__mark" aria-hidden="true">O</span>
      <span class="window-chrome__title">{{ title }}</span>
    </button>

    <div class="window-chrome__controls" aria-label="Window controls">
      <button
        class="window-chrome__control"
        data-window-action="minimize"
        type="button"
        :aria-label="minimizeLabel"
        :title="minimizeLabel"
        @click="minimize"
      >
        <Minus :size="15" :stroke-width="1.8" aria-hidden="true" />
      </button>
      <button
        class="window-chrome__control"
        data-window-action="maximize"
        type="button"
        :aria-label="maximized ? restoreLabel : maximizeLabel"
        :title="maximized ? restoreLabel : maximizeLabel"
        @click="toggleMaximize"
      >
        <Square v-if="!maximized" :size="13" :stroke-width="1.8" aria-hidden="true" />
        <Maximize2 v-else :size="14" :stroke-width="1.8" aria-hidden="true" />
      </button>
      <button
        class="window-chrome__control window-chrome__control--close"
        data-window-action="close"
        type="button"
        :aria-label="closeLabel"
        :title="closeLabel"
        @click="close"
      >
        <X :size="15" :stroke-width="1.8" aria-hidden="true" />
      </button>
    </div>
  </div>
</template>

<style scoped>
.window-chrome {
  display: flex;
  block-size: var(--desktop-titlebar-height, 36px);
  align-items: stretch;
  justify-content: space-between;
  border-block-end: 1px solid var(--color-border-base);
  background: var(--color-bg-surface);
  color: var(--color-text-secondary);
  user-select: none;
}

.window-chrome__drag-region {
  display: flex;
  min-inline-size: 0;
  flex: 1;
  align-items: center;
  gap: var(--space-2);
  padding-inline: var(--space-3);
  border: 0;
  background: transparent;
  color: inherit;
  text-align: start;
  cursor: default;
}

.window-chrome__mark {
  display: grid;
  inline-size: 20px;
  block-size: 20px;
  place-items: center;
  border: 1px solid var(--color-accent-border);
  border-radius: var(--radius-xs);
  background: var(--color-accent-muted);
  color: var(--color-accent);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  font-weight: var(--weight-semibold);
}

.window-chrome__title {
  overflow: hidden;
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.window-chrome__controls {
  display: flex;
  align-items: stretch;
}

.window-chrome__control {
  display: grid;
  inline-size: 44px;
  block-size: var(--desktop-titlebar-height, 36px);
  place-items: center;
  border: 0;
  border-inline-start: 1px solid transparent;
  background: transparent;
  color: var(--color-text-secondary);
  cursor: pointer;
}

.window-chrome__control:hover {
  background: var(--color-bg-element);
  color: var(--color-text-primary);
}

.window-chrome__control--close:hover {
  background: var(--color-status-error);
  color: var(--color-text-inverse);
}

@media (max-width: 640px) {
  .window-chrome__title {
    display: none;
  }

  .window-chrome__control {
    inline-size: 40px;
  }
}
</style>
