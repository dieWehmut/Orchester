<script setup lang="ts">
/**
 * The application shell.
 *
 * The shell owns three things the routed views do not: the window the product
 * is drawn in, the title row above every route, and the one keydown listener
 * that makes a shortcut a shortcut. What is inside the window belongs to the
 * view the router resolved.
 */
import { computed, onMounted, onUnmounted, provide } from 'vue'

import TitleRow from './components/layout/TitleRow.vue'
import type { RuntimeConnection } from './components/layout/TitleRow.vue'
import {
  DESKTOP_WINDOW_KEY,
  desktopWindow,
  type DesktopWindowController,
} from './platform/desktop-window'
import { useAppStores } from './stores/app'
import { useShortcutListener } from './shortcuts'

const props = defineProps<{
  desktopController?: DesktopWindowController
}>()

const stores = useAppStores()
const windowController = props.desktopController ?? desktopWindow

// The routed views act on the window the shell owns - closing a tab can be a
// window close in the desktop runtime - so the controller is handed down
// rather than each view reaching for the module singleton.
provide(DESKTOP_WINDOW_KEY, windowController)
const connection = computed<RuntimeConnection>(() => {
  if (stores.bootstrap.status.value === 'ready') return 'ready'
  if (stores.bootstrap.status.value === 'error') return 'error'
  return 'pending'
})

onMounted(() => {
  void stores.start()
})

onUnmounted(() => {
  stores.stop()
})

// One listener for the whole app: a shortcut works from wherever focus is,
// which is what makes it a shortcut rather than a key binding on a pane.
useShortcutListener()
</script>

<template>
  <div class="app-shell" :class="{ 'app-shell--desktop': windowController.enabled }">
    <TitleRow :controller="windowController" :connection="connection" />

    <div class="app-shell__outlet">
      <RouterView />
    </div>
  </div>
</template>
