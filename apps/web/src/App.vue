<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue'

import WorkspaceHeader from './components/layout/WorkspaceHeader.vue'
import type { RuntimeConnection } from './components/layout/WorkspaceHeader.vue'
import { useAppStores } from './stores/app'

const stores = useAppStores()
const connection = computed<RuntimeConnection>(() => {
  if (stores.bootstrap.status.value === 'ready') return 'ready'
  if (stores.bootstrap.status.value === 'error') return 'error'
  return 'pending'
})
const workspaceName = computed(() => stores.bootstrap.context.value?.workspace.name ?? null)

onMounted(() => {
  void stores.start()
})

onUnmounted(() => {
  stores.stop()
})

</script>

<template>
  <div class="app-shell">
    <WorkspaceHeader :connection="connection" :workspace-name="workspaceName" />

    <main aria-label="Agent workspace">
      <RouterView />
    </main>
  </div>
</template>
