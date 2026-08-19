<script setup lang="ts">
import { StatusDot, ThemeToggle } from '@orchester/design'

import { useI18n } from './i18n'

const { t } = useI18n()
</script>

<template>
  <div class="app-shell">
    <header class="app-header">
      <div class="brand-lockup">
        <div class="brand-mark" aria-hidden="true">O</div>
        <div>
          <strong data-testid="product-name">{{ t('app.name') }}</strong>
          <span>{{ t('app.workspace') }}</span>
        </div>
      </div>

      <div class="header-actions">
        <div class="connection-state" aria-label="Runtime connection pending">
          <StatusDot status="idle" label="Runtime connection pending" :pulse="false" />
          <span>{{ t('app.runtimePending') }}</span>
        </div>
        <ThemeToggle />
      </div>
    </header>

    <main aria-label="Agent workspace">
      <RouterView />
    </main>
  </div>
</template>
