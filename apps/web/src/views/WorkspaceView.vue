<template>
  <section class="route-placeholder" data-testid="workspace-view">
    <p class="route-placeholder__eyebrow">Local workspace</p>
    <h1>Workspace</h1>
    <p>Session history, run transcript, and context tools will appear here.</p>
  </section>
</template>

<style scoped>
.route-placeholder {
  display: grid;
  min-block-size: calc(100vh - var(--header-height));
  align-content: center;
  justify-items: center;
  gap: var(--space-2);
  padding: var(--space-8);
  text-align: center;
}

.route-placeholder__eyebrow {
  margin: 0;
  color: var(--color-text-tertiary);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  text-transform: uppercase;
}

.route-placeholder h1,
.route-placeholder p:last-child {
  margin: 0;
}

.route-placeholder h1 {
  font-size: var(--text-xl);
}

.route-placeholder p:last-child {
  max-inline-size: 36rem;
  color: var(--color-text-secondary);
}
</style>
