<script setup lang="ts">
/**
 * The light/dark switch.
 *
 * Takes its label from a prop rather than reaching for an i18n instance: this
 * package is used by an app with three locales and by an app with none, so it
 * owns no copy.
 */
import { computed } from 'vue'

import { useAppearance } from '../composables/useAppearance'
import IconButton from './IconButton.vue'

const props = withDefaults(defineProps<{ labelDark?: string; labelLight?: string }>(), {
  labelDark: 'Switch to light theme',
  labelLight: 'Switch to dark theme',
})

const { isDark, toggleTheme } = useAppearance()

const label = computed(() => (isDark.value ? props.labelDark : props.labelLight))
</script>

<template>
  <IconButton :label="label" @click="toggleTheme()">
    <svg
      v-if="isDark"
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="1.8"
      stroke-linecap="round"
      aria-hidden="true"
    >
      <circle cx="12" cy="12" r="4" />
      <path d="M12 3v2M12 19v2M3 12h2M19 12h2M5.6 5.6l1.4 1.4M17 17l1.4 1.4M18.4 5.6L17 7M7 17l-1.4 1.4" />
    </svg>
    <svg
      v-else
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="1.8"
      stroke-linecap="round"
      aria-hidden="true"
    >
      <path d="M20 14.5A8.5 8.5 0 1 1 9.5 4a6.8 6.8 0 0 0 10.5 10.5z" />
    </svg>
  </IconButton>
</template>
