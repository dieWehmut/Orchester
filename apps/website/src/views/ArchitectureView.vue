<script setup lang="ts">
import {
  ArrowDown,
  Braces,
  CircleDot,
  Layers3,
  Route,
  Server,
  ShieldCheck,
} from '@lucide/vue'

import SiteShell from '../components/site/SiteShell.vue'
import {
  architectureBoundary,
  architectureStages,
  type ArchitectureStage,
} from '../content/architecture'

const stageIcons: Record<ArchitectureStage['id'], typeof Braces> = {
  browser: Layers3,
  service: Route,
  runtime: Server,
  provider: Braces,
}
</script>

<template>
  <SiteShell>
    <main class="site-page architecture-page" data-page="architecture">
      <header class="page-intro">
        <p class="site-eyebrow">System map</p>
        <h1>Architecture that keeps authority in one place.</h1>
        <p>
          Orchester separates rendering from execution. Each boundary has a typed contract, a
          narrow responsibility, and a failure mode that can be shown to the operator.
        </p>
      </header>

      <section class="architecture-flow" data-architecture-flow aria-labelledby="flow-title">
        <div class="section-kicker">
          <CircleDot :size="16" :stroke-width="1.8" aria-hidden="true" />
          <h2 id="flow-title">From surface to adapter</h2>
        </div>

        <div class="architecture-flow__stages">
          <template v-for="stage in architectureStages" :key="stage.id">
            <article class="architecture-stage" :data-architecture-stage="stage.id">
              <div class="architecture-stage__topline">
                <span class="architecture-stage__index">{{ stage.index }}</span>
                <span class="architecture-stage__label">{{ stage.label }}</span>
              </div>
              <component :is="stageIcons[stage.id]" class="architecture-stage__icon" :size="22" :stroke-width="1.8" aria-hidden="true" />
              <h3>{{ stage.title }}</h3>
              <p>{{ stage.description }}</p>
              <div class="architecture-stage__contract">
                <span>Contract</span>
                <code>{{ stage.contract }}</code>
              </div>
              <ul>
                <li v-for="detail in stage.details" :key="detail">{{ detail }}</li>
              </ul>
            </article>
            <ArrowDown
              v-if="stage.id !== 'provider'"
              class="architecture-flow__arrow"
              :size="18"
              :stroke-width="1.8"
              aria-hidden="true"
            />
          </template>
        </div>
      </section>

      <section class="architecture-boundary" data-architecture-boundary aria-labelledby="boundary-title">
        <div class="architecture-boundary__icon">
          <ShieldCheck :size="24" :stroke-width="1.8" aria-hidden="true" />
        </div>
        <div>
          <p class="site-eyebrow">Threat model in plain language</p>
          <h2 id="boundary-title">{{ architectureBoundary.title }}</h2>
          <p>{{ architectureBoundary.description }}</p>
          <ul>
            <li v-for="bullet in architectureBoundary.bullets" :key="bullet">{{ bullet }}</li>
          </ul>
        </div>
      </section>

      <section class="architecture-notes" aria-labelledby="notes-title">
        <div class="section-kicker">
          <Braces :size="16" :stroke-width="1.8" aria-hidden="true" />
          <h2 id="notes-title">What the browser receives</h2>
        </div>
        <p>
          A versioned event envelope: run id, sequence, stable event id, and redacted details that
          are sufficient to render the state. The browser never receives a raw harness event or a
          workspace absolute path.
        </p>
        <RouterLink class="text-link" to="/install">Set up the local surface <ArrowDown :size="15" :stroke-width="1.8" aria-hidden="true" /></RouterLink>
      </section>
    </main>
  </SiteShell>
</template>
