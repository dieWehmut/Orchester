<script setup lang="ts">
import { useI18n } from '../i18n'

const { t } = useI18n()
</script>

<template>
  <section class="route-placeholder" data-testid="not-found-view">
    <p class="route-placeholder__eyebrow">{{ t('notFound.eyebrow') }}</p>
    <h1>{{ t('routes.notFound') }}</h1>
    <p>{{ t('notFound.description') }}</p>
    <RouterLink class="route-placeholder__link" to="/workspace">
      {{ t('notFound.return') }}
    </RouterLink>
  </section>
</template>

<style scoped>
.route-placeholder {
  display: grid;
  min-block-size: calc(100vh - var(--app-top-chrome-height, var(--header-height)));
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
.route-placeholder p {
  margin: 0;
}

.route-placeholder h1 {
  font-size: var(--text-xl);
}

.route-placeholder p {
  color: var(--color-text-secondary);
}

.route-placeholder__link {
  color: var(--color-accent);
  font-size: var(--text-sm);
}
</style>
