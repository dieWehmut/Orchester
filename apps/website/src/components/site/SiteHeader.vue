<script setup lang="ts">
import { Menu, X } from '@lucide/vue'

import { SITE_NAV_ITEMS } from '../../content/navigation'

withDefaults(defineProps<{ mobileOpen?: boolean }>(), { mobileOpen: false })

defineEmits<{
  'toggle-mobile': []
}>()
</script>

<template>
  <header class="site-header" data-site-header>
    <div class="site-header__inner">
      <RouterLink class="site-brand" to="/" aria-label="Orchester home">
        <span class="site-brand__mark" aria-hidden="true">O/</span>
        <span class="site-brand__name">Orchester</span>
      </RouterLink>

      <nav class="site-header__nav" aria-label="Primary navigation">
        <RouterLink
          v-for="item in SITE_NAV_ITEMS"
          :key="item.to"
          class="site-nav-link"
          :class="{ 'site-nav-link--home': item.to === '/' }"
          :to="item.to"
          exact-active-class="site-nav-link--active"
          :data-site-link="item.to"
        >
          {{ item.label }}
        </RouterLink>
      </nav>

      <button
        class="site-header__menu-button"
        data-mobile-nav-trigger
        type="button"
        aria-controls="site-mobile-nav"
        :aria-expanded="mobileOpen"
        :aria-label="mobileOpen ? 'Close navigation' : 'Open navigation'"
        :title="mobileOpen ? 'Close navigation' : 'Open navigation'"
        @click="$emit('toggle-mobile')"
      >
        <X v-if="mobileOpen" :size="20" :stroke-width="1.8" aria-hidden="true" />
        <Menu v-else :size="20" :stroke-width="1.8" aria-hidden="true" />
      </button>
    </div>
  </header>
</template>
