<script setup lang="ts">
/**
 * The light/dark switch.
 *
 * Takes its label from a prop rather than reaching for an i18n instance: this
 * package is used by an app with three locales and by an app with none, so it
 * owns no copy.
 */
import { computed } from 'vue'
import { Moon, Sun } from '@lucide/vue'

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
    <Sun v-if="isDark" :size="16" :stroke-width="1.8" aria-hidden="true" />
    <Moon v-else :size="16" :stroke-width="1.8" aria-hidden="true" />
  </IconButton>
</template>
