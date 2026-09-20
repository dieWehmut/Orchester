<script setup lang="ts">
/**
 * The application shell.
 *
 * Section 2 of the design spec letters the shell's regions and names each one
 * with a data attribute, so the styles and the tests reach a region by what it
 * is rather than by which class moved recently. This component owns the three
 * columns - the rail, the transcript and the inspector - and the drawer path
 * that replaces them on a narrow viewport.
 *
 * The regions keep the landmark each one already was: the attribute says which
 * region a box is, the element says what a reader should call it. The mobile
 * controls answer to the rail's own landmark because they stand in for it.
 */
import { AppButton, AppDrawer } from '@orchester/design'
import type { RailAppearance } from '@orchester/design'
import { ref } from 'vue'

const props = withDefaults(
  defineProps<{
    sessionsTitle: string
    inspectorTitle: string
    controlsLabel?: string
    inspectorOpen?: boolean
    /** Solid or translucent, as the appearance axis asks. */
    railAppearance?: RailAppearance
    /** Whether the inspector has taken the whole width. */
    inspectorFullWidth?: boolean
    /** The inspector's active tab, for the styles that key off it. */
    inspectorTab?: string
  }>(),
  {
    controlsLabel: 'Workspace panels',
    inspectorOpen: true,
    railAppearance: 'solid',
    inspectorFullWidth: false,
    inspectorTab: 'context',
  },
)

const sessionsOpen = ref(false)
const inspectorDrawerOpen = ref(false)
</script>

<template>
  <div class="app-shell" :class="{ 'app-shell--inspector-closed': !props.inspectorOpen }">
    <nav
      class="app-shell__mobile-controls"
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
        @click="inspectorDrawerOpen = true"
      >
        {{ props.inspectorTitle }}
      </AppButton>
    </nav>

    <div class="app-shell__grid">
      <nav
        class="app-shell__rail"
        data-pane="sessions"
        data-rail
        :data-rail-appearance="props.railAppearance"
        aria-label="Sessions"
      >
        <slot name="sessions" />
      </nav>
      <main class="app-shell__transcript" data-pane="transcript" data-transcript aria-label="Run transcript">
        <slot />
      </main>
      <aside
        class="app-shell__inspector"
        data-pane="inspector"
        data-inspector
        :data-inspector-open="String(props.inspectorOpen)"
        :data-inspector-full-width="String(props.inspectorFullWidth)"
        :data-inspector-tab="props.inspectorTab"
        :hidden="!props.inspectorOpen"
        aria-label="Inspector"
      >
        <slot name="inspector" />
      </aside>
    </div>

    <AppDrawer v-model:open="sessionsOpen" :title="props.sessionsTitle" side="left">
      <slot name="sessions" />
    </AppDrawer>
    <AppDrawer v-model:open="inspectorDrawerOpen" :title="props.inspectorTitle" side="right">
      <slot name="inspector" />
    </AppDrawer>
  </div>
</template>

<style scoped>
.app-shell {
  min-block-size: calc(100vh - var(--app-top-chrome-height, var(--header-height)));
}

.app-shell__mobile-controls {
  display: none;
}

.app-shell__grid {
  display: grid;
  grid-template-columns:
    minmax(var(--sidebar-min-width), var(--sidebar-width))
    minmax(0, 1fr)
    minmax(var(--inspector-min-width), var(--inspector-width));
  min-block-size: calc(100vh - var(--app-top-chrome-height, var(--header-height)));
  overflow: hidden;
}

.app-shell--inspector-closed .app-shell__grid {
  grid-template-columns:
    minmax(var(--sidebar-min-width), var(--sidebar-width))
    minmax(0, 1fr);
}

.app-shell__rail,
.app-shell__transcript,
.app-shell__inspector {
  min-inline-size: 0;
  min-block-size: 0;
  overflow: auto;
}

.app-shell__rail,
.app-shell__inspector {
  background: var(--color-bg-surface);
}

.app-shell__rail {
  border-inline-end: 1px solid var(--color-border-base);
}

.app-shell__inspector {
  border-inline-start: 1px solid var(--color-border-base);
}

.app-shell__transcript {
  background: var(--color-bg-base);
}

@media (max-width: 1279px) {
  .app-shell__mobile-controls {
    display: flex;
    min-block-size: var(--control-height-lg);
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    border-block-end: 1px solid var(--color-border-base);
    background: var(--color-bg-surface);
  }

  .app-shell__grid {
    grid-template-columns: minmax(0, 1fr);
    min-block-size: calc(
      100vh - var(--app-top-chrome-height, var(--header-height)) - var(--control-height-lg)
    );
  }

  .app-shell__rail,
  .app-shell__inspector {
    display: none;
  }
}
</style>
