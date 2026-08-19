<script setup lang="ts">
import { StatusDot, ThemeToggle } from '@orchester/design'
</script>

<template>
  <div class="app-shell">
    <header class="app-header">
      <div class="brand-lockup">
        <div class="brand-mark" aria-hidden="true">O</div>
        <div>
          <strong data-testid="product-name">Orchester</strong>
          <span>Agent workspace</span>
        </div>
      </div>

      <div class="header-actions">
        <div class="connection-state" aria-label="Runtime connection pending">
          <StatusDot status="idle" label="Runtime connection pending" :pulse="false" />
          <span>Runtime pending</span>
        </div>
        <ThemeToggle />
      </div>
    </header>

    <main aria-label="Agent workspace">
      <RouterView />
    </main>
  </div>
</template>
