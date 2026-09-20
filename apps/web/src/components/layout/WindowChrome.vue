<script setup lang="ts">
import { Maximize2, Minus, Square, X } from '@lucide/vue'

import { useWindowChrome } from '../../composables/use-window-chrome'
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
const platform = controller.platform ?? 'linux'
const { close, maximized, minimize, toggleMaximize } = useWindowChrome(controller)
</script>

<template>
  <div
    v-if="controller.enabled"
    class="window-chrome"
    data-window-chrome
    data-window-material="opaque"
    :data-window-platform="platform"
  >
    <div v-if="platform === 'macos'" class="window-chrome__traffic-lights" data-native-traffic-lights aria-hidden="true" />
    <div
      class="window-chrome__drag-region"
      data-tauri-drag-region="deep"
      :aria-label="title"
      :title="title"
    >
      <span class="window-chrome__mark" aria-hidden="true">O</span>
      <span class="window-chrome__title">{{ title }}</span>
    </div>
    <div v-if="platform !== 'macos'" class="window-chrome__controls" aria-label="Window controls">
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
        :data-native-snap-target="platform === 'windows' ? 'true' : undefined"
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
  justify-content: flex-start;
  border-block-end: 1px solid var(--color-border-base);
  background: var(--color-bg-surface);
  color: var(--color-text-secondary);
  user-select: none;
}

/* CSS blur support does not prove native compositor availability. The shell
   currently supplies opaque windows on every supported OS. */
.window-chrome[data-window-material='opaque'] {
  background: var(--color-surface-base);
  backdrop-filter: none;
}

.window-chrome[data-window-platform='macos'] {
  block-size: var(--window-chrome-height, 38px);
}

.window-chrome__traffic-lights {
  flex: 0 0 80px;
}

.window-chrome[data-window-platform='windows'] {
  block-size: 32px;
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
  flex: 0 0 auto;
  align-items: start;
}

.window-chrome__control {
  position: relative;
  display: grid;
  inline-size: 46px;
  min-inline-size: 46px;
  block-size: 32px;
  min-block-size: 32px;
  padding: 0;
  place-items: center;
  border: 0;
  border-radius: 0;
  background: transparent;
  color: var(--color-text-secondary);
  cursor: default;
}

.window-chrome__control:hover {
  background: var(--color-bg-element);
}

.window-chrome__control--close:hover {
  background: var(--color-intent-danger-solid);
  color: var(--color-text-inverse);
}

@media (max-width: 640px) {
  .window-chrome__title {
    display: none;
  }

}
</style>
