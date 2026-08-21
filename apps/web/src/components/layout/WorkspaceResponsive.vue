<script setup lang="ts">
import { AppButton, AppDrawer } from '@orchester/design'
import { ref } from 'vue'

const props = withDefaults(
  defineProps<{
    sessionsTitle: string
    inspectorTitle: string
    controlsLabel?: string
  }>(),
  { controlsLabel: 'Workspace panels' },
)

const sessionsOpen = ref(false)
const inspectorOpen = ref(false)
</script>

<template>
  <div class="workspace-responsive">
    <nav
      class="workspace-responsive__mobile-controls"
      data-mobile-controls
      :aria-label="props.controlsLabel"
    >
      <AppButton
        variant="ghost"
        size="sm"
        data-mobile-sessions
        :aria-label="props.sessionsTitle"
        @click="sessionsOpen = true"
      >
        {{ props.sessionsTitle }}
      </AppButton>
      <AppButton
        variant="ghost"
        size="sm"
        data-mobile-inspector
        :aria-label="props.inspectorTitle"
        @click="inspectorOpen = true"
      >
        {{ props.inspectorTitle }}
      </AppButton>
    </nav>

    <div class="workspace-responsive__grid">
      <aside
        class="workspace-responsive__desktop-sessions"
        data-pane="sessions"
        aria-label="Sessions"
      >
        <slot name="sessions" />
      </aside>
      <section
        class="workspace-responsive__transcript"
        data-pane="transcript"
        aria-label="Run transcript"
      >
        <slot />
      </section>
      <aside
        class="workspace-responsive__desktop-inspector"
        data-pane="inspector"
        aria-label="Inspector"
      >
        <slot name="inspector" />
      </aside>
    </div>

    <AppDrawer v-model:open="sessionsOpen" :title="props.sessionsTitle" side="left">
      <slot name="sessions" />
    </AppDrawer>
    <AppDrawer v-model:open="inspectorOpen" :title="props.inspectorTitle" side="right">
      <slot name="inspector" />
    </AppDrawer>
  </div>
</template>

<style scoped>
.workspace-responsive {
  min-block-size: calc(100vh - var(--app-top-chrome-height, var(--header-height)));
}

.workspace-responsive__mobile-controls {
  display: none;
}

.workspace-responsive__grid {
  display: grid;
  grid-template-columns:
    minmax(var(--sidebar-min-width), var(--sidebar-width))
    minmax(0, 1fr)
    minmax(var(--inspector-min-width), var(--inspector-width));
  min-block-size: calc(100vh - var(--app-top-chrome-height, var(--header-height)));
  overflow: hidden;
}

.workspace-responsive__desktop-sessions,
.workspace-responsive__transcript,
.workspace-responsive__desktop-inspector {
  min-inline-size: 0;
  min-block-size: 0;
  overflow: auto;
}

.workspace-responsive__desktop-sessions,
.workspace-responsive__desktop-inspector {
  background: var(--color-bg-surface);
}

.workspace-responsive__desktop-sessions {
  border-inline-end: 1px solid var(--color-border-base);
}

.workspace-responsive__desktop-inspector {
  border-inline-start: 1px solid var(--color-border-base);
}

.workspace-responsive__transcript {
  background: var(--color-bg-base);
}

@media (max-width: 1279px) {
  .workspace-responsive__mobile-controls {
    display: flex;
    min-block-size: var(--control-height-lg);
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    border-block-end: 1px solid var(--color-border-base);
    background: var(--color-bg-surface);
  }

  .workspace-responsive__grid {
    grid-template-columns: minmax(0, 1fr);
    min-block-size: calc(
      100vh - var(--app-top-chrome-height, var(--header-height)) - var(--control-height-lg)
    );
  }

  .workspace-responsive__desktop-sessions,
  .workspace-responsive__desktop-inspector {
    display: none;
  }
}
</style>
