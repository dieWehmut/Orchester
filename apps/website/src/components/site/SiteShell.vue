<script setup lang="ts">
import { ref } from 'vue'

import MobileNav from './MobileNav.vue'
import GiscusComments from './GiscusComments.vue'
import SiteFooter from './SiteFooter.vue'
import SiteHeader from './SiteHeader.vue'

const mobileOpen = ref(false)
</script>

<template>
  <div class="site-shell">
    <a class="site-skip-link" href="#site-content">Skip to content</a>
    <SiteHeader :mobile-open="mobileOpen" @toggle-mobile="mobileOpen = !mobileOpen" />
    <MobileNav v-model:open="mobileOpen" />
    <div id="site-content" class="site-shell__content">
      <slot />
      <GiscusComments />
    </div>
    <SiteFooter />
  </div>
</template>
