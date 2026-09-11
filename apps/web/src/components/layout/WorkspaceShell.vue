<template>
  <div class="workspace-shell">
    <aside class="workspace-shell__sessions" data-pane="sessions" aria-label="Sessions">
      <slot name="sessions" />
    </aside>
    <section class="workspace-shell__transcript" data-pane="transcript" aria-label="Run transcript">
      <slot />
    </section>
    <aside class="workspace-shell__inspector" data-pane="inspector" aria-label="Inspector">
      <slot name="inspector" />
    </aside>
  </div>
</template>

<style scoped>
.workspace-shell {
  display: grid;
  grid-template-columns:
    minmax(var(--sidebar-min-width), var(--sidebar-width))
    minmax(0, 1fr)
    minmax(var(--inspector-min-width), var(--inspector-width));
  min-block-size: calc(100vh - var(--app-top-chrome-height, var(--header-height)));
  overflow: hidden;
  background: var(--color-bg-base);
}

.workspace-shell__sessions,
.workspace-shell__transcript,
.workspace-shell__inspector {
  min-inline-size: 0;
  min-block-size: 0;
  overflow: auto;
}

.workspace-shell__sessions {
  border-inline-end: 1px solid var(--color-border-base);
  background: var(--color-bg-surface);
}

.workspace-shell__transcript {
  background: var(--color-bg-base);
}

.workspace-shell__inspector {
  border-inline-start: 1px solid var(--color-border-base);
  background: var(--color-bg-surface);
}

@media (max-width: 1279px) {
  .workspace-shell {
    grid-template-columns: minmax(0, 1fr);
  }

  .workspace-shell__sessions,
  .workspace-shell__inspector {
    display: none;
  }
}
</style>
