<script setup lang="ts">
import { AppDrawer } from '@orchester/design'

import { SITE_NAV_ITEMS } from '../../content/navigation'

const props = withDefaults(defineProps<{ open?: boolean }>(), { open: false })

const emit = defineEmits<{
  'update:open': [value: boolean]
  close: []
}>()

function closeNavigation(): void {
  emit('update:open', false)
  emit('close')
}

function onDrawerUpdate(value: boolean): void {
  emit('update:open', value)
}
</script>

<template>
  <div v-if="props.open" id="site-mobile-nav" data-mobile-nav>
    <AppDrawer
      :open="props.open"
      title="Site navigation"
      description="Move between the Orchester project guides."
      side="right"
      close-label="Close navigation"
      @update:open="onDrawerUpdate"
      @close="emit('close')"
    >
      <nav class="mobile-nav__links" aria-label="Mobile navigation">
        <RouterLink
          v-for="item in SITE_NAV_ITEMS"
          :key="item.to"
          class="mobile-nav__link"
          :to="item.to"
          exact-active-class="mobile-nav__link--active"
          :data-site-link="item.to"
          @click="closeNavigation"
        >
          <span>{{ item.label }}</span>
          <span class="mobile-nav__arrow" aria-hidden="true">-&gt;</span>
        </RouterLink>
      </nav>
    </AppDrawer>
  </div>
</template>
