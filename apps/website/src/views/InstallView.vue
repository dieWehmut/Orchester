<script setup lang="ts">
import { Check, Clipboard, CircleCheck, Terminal } from '@lucide/vue'
import { ref } from 'vue'

import SiteShell from '../components/site/SiteShell.vue'
import { installPrerequisites, installSteps } from '../content/install'

const copiedStep = ref<string | null>(null)

async function copyCommand(index: string, command: string): Promise<void> {
  if (!navigator.clipboard) {
    return
  }

  await navigator.clipboard.writeText(command)
  copiedStep.value = index
  window.setTimeout(() => {
    if (copiedStep.value === index) {
      copiedStep.value = null
    }
  }, 1800)
}
</script>

<template>
  <SiteShell>
    <main class="site-page install-page" data-page="install">
      <header class="page-intro">
        <p class="site-eyebrow">A local setup in three moves</p>
        <h1>Install the surface. Keep the authority local.</h1>
        <p>
          The Pages site is a static guide. The WebUI and desktop shell connect to a runtime on your
          machine, so you can inspect the boundary before you send an action across it.
        </p>
      </header>

      <section class="install-layout" aria-labelledby="steps-title">
        <div class="install-layout__main">
          <div class="section-kicker">
            <Terminal :size="16" :stroke-width="1.8" aria-hidden="true" />
            <h2 id="steps-title">Start from a clean checkout</h2>
          </div>
          <div class="install-steps" data-install-steps>
            <article v-for="step in installSteps" :key="step.index" class="install-step" :data-install-step="step.index">
              <div class="install-step__index">{{ step.index }}</div>
              <div class="install-step__body">
                <h3>{{ step.title }}</h3>
                <p>{{ step.description }}</p>
                <div class="command-block">
                  <code>{{ step.command }}</code>
                  <button
                    class="command-block__copy"
                    type="button"
                    :aria-label="copiedStep === step.index ? `Copied ${step.title} command` : `Copy ${step.title} command`"
                    :title="copiedStep === step.index ? 'Copied' : 'Copy command'"
                    @click="copyCommand(step.index, step.command)"
                  >
                    <Check v-if="copiedStep === step.index" :size="16" :stroke-width="1.8" aria-hidden="true" />
                    <Clipboard v-else :size="16" :stroke-width="1.8" aria-hidden="true" />
                  </button>
                </div>
                <small>{{ step.note }}</small>
              </div>
            </article>
          </div>
        </div>

        <aside class="install-aside" data-install-prerequisites aria-labelledby="prerequisites-title">
          <p class="site-eyebrow">Before you begin</p>
          <h2 id="prerequisites-title">Prerequisites</h2>
          <ul>
            <li v-for="prerequisite in installPrerequisites" :key="prerequisite">
              <CircleCheck :size="16" :stroke-width="1.8" aria-hidden="true" />
              <span>{{ prerequisite }}</span>
            </li>
          </ul>
          <div class="install-aside__note">
            <strong>Windows note</strong>
            <p>
              Rust checks need the MSVC linker in a developer shell. The repository records the
              fallback toolchain details when a local machine does not provide it.
            </p>
          </div>
        </aside>
      </section>

      <section class="install-next" aria-labelledby="next-title">
        <div>
          <p class="site-eyebrow">Next checkpoint</p>
          <h2 id="next-title">Open the WebUI and verify the loopback address.</h2>
          <p>Once the service is running, the browser should show an explicit connection state rather than guessing.</p>
        </div>
        <RouterLink class="site-action site-action--secondary" to="/architecture">
          Revisit the boundary map
        </RouterLink>
      </section>
    </main>
  </SiteShell>
</template>
