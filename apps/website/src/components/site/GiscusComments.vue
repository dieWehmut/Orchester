<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'

import {
  giscusTermForRoute,
  parseGiscusConfig,
  type GiscusConfig,
} from '../../comments/giscus-config'

const props = defineProps<{
  config?: GiscusConfig | null
  term?: string
}>()

const route = useRoute()
const containerRef = ref<HTMLElement | null>(null)
const environmentConfig = parseGiscusConfig(
  import.meta.env as Record<string, string | undefined>,
)
const activeConfig = computed(() => (props.config === undefined ? environmentConfig : props.config))
const discussionTerm = computed(() => {
  const explicit = props.term?.trim()
  return explicit || giscusTermForRoute(route.path)
})

function clearContainer(): void {
  if (containerRef.value) containerRef.value.replaceChildren()
}

async function renderGiscus(): Promise<void> {
  clearContainer()
  if (!activeConfig.value) return
  await nextTick()

  const container = containerRef.value
  if (!container) return
  const config = activeConfig.value
  const script = document.createElement('script')
  script.src = 'https://giscus.app/client.js'
  script.async = true
  script.crossOrigin = 'anonymous'
  script.setAttribute('data-repo', config.repo)
  script.setAttribute('data-repo-id', config.repoId)
  script.setAttribute('data-category', config.category)
  script.setAttribute('data-category-id', config.categoryId)
  script.setAttribute('data-mapping', 'specific')
  script.setAttribute('data-term', discussionTerm.value)
  script.setAttribute('data-strict', config.strict)
  script.setAttribute('data-reactions-enabled', config.reactionsEnabled)
  script.setAttribute('data-emit-metadata', '0')
  script.setAttribute('data-input-position', config.inputPosition)
  script.setAttribute('data-lang', config.lang)
  script.setAttribute('data-loading', config.loading)
  container.appendChild(script)
}

onMounted(() => {
  void renderGiscus()
})

watch(
  [() => route.fullPath, () => props.term, activeConfig],
  () => void renderGiscus(),
)

onBeforeUnmount(clearContainer)
</script>

<template>
  <section class="giscus-comments" data-giscus-comments aria-labelledby="giscus-title">
    <header class="giscus-comments__header">
      <p class="site-eyebrow">Project discussion</p>
      <h2 id="giscus-title">Talk through the boundary.</h2>
      <p>Comments are attached to this page through GitHub Discussions.</p>
    </header>

    <p v-if="!activeConfig" class="giscus-comments__disabled" data-giscus-disabled>
      Comments are not enabled for this deployment.
    </p>
    <div v-else ref="containerRef" class="giscus-comments__container" data-giscus-container />
  </section>
</template>

<style scoped>
.giscus-comments {
  margin-block-start: var(--space-16);
  padding-block: var(--space-8) var(--space-2);
  border-block-start: 1px solid var(--color-border-base);
}

.giscus-comments__header {
  max-inline-size: 48rem;
}

.giscus-comments__header h2 {
  margin: var(--space-2) 0 0;
  color: var(--color-text-primary);
  font-size: var(--text-2xl);
  line-height: var(--leading-tight);
}

.giscus-comments__header p:last-child,
.giscus-comments__disabled {
  margin: var(--space-3) 0 0;
  color: var(--color-text-secondary);
}

.giscus-comments__disabled {
  padding: var(--space-4);
  border: 1px dashed var(--color-border-strong);
  color: var(--color-text-tertiary);
}

.giscus-comments__container {
  min-block-size: 10rem;
  margin-block-start: var(--space-5);
}

.giscus-comments__container :deep(.giscus) {
  max-inline-size: 100%;
}
</style>
