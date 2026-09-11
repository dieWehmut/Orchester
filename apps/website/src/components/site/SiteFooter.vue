<script setup lang="ts">
import { Code2, Radio } from '@lucide/vue'

import { SITE_NAV_ITEMS } from '../../content/navigation'
</script>

<template>
  <footer class="site-footer" data-site-footer>
    <div class="site-footer__inner">
      <div class="site-footer__identity">
        <span class="site-footer__mark" aria-hidden="true">O/</span>
        <span>Orchester</span>
      </div>

      <p class="site-footer__note">
        Local-first orchestration for agent work you can inspect and govern.
      </p>

      <nav class="site-footer__nav" aria-label="Footer navigation">
        <RouterLink
          v-for="item in SITE_NAV_ITEMS"
          :key="item.to"
          :to="item.to"
          :data-site-link="item.to"
        >
          {{ item.label }}
        </RouterLink>
        <a href="https://github.com/dieWehmut/Orchester" target="_blank" rel="noreferrer">
          <Code2 :size="15" :stroke-width="1.8" aria-hidden="true" />
          <span>Source</span>
        </a>
      </nav>

      <div class="site-footer__status" aria-label="Runtime posture">
        <Radio :size="14" :stroke-width="1.8" aria-hidden="true" />
        <span>Designed for a local runtime</span>
      </div>
    </div>
  </footer>
</template>
