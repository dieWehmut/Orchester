<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue'

import WindowChrome from './components/layout/WindowChrome.vue'
import WorkspaceHeader from './components/layout/WorkspaceHeader.vue'
import type { RuntimeConnection } from './components/layout/WorkspaceHeader.vue'
import {
  desktopWindow,
  type DesktopWindowController,
} from './platform/desktop-window'
import { useAppStores } from './stores/app'

const props = defineProps<{
  desktopController?: DesktopWindowController
}>()

const stores = useAppStores()
const windowController = props.desktopController ?? desktopWindow
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
  <div class="app-shell" :class="{ 'app-shell--desktop': windowController.enabled }">
    <WindowChrome :controller="windowController" />
    <WorkspaceHeader :connection="connection" :workspace-name="workspaceName" />

    <main aria-label="Agent workspace">
      <RouterView />
    </main>
  </div>
</template>
