<script setup lang="ts">
import {
  ArrowUpRight,
  Box,
  CircleCheck,
  Eye,
  GitBranch,
  LockKeyhole,
  Server,
  ShieldCheck,
  Workflow,
} from '@lucide/vue'

import SiteShell from '../components/site/SiteShell.vue'
import {
  governancePrinciples,
  homeAdapters,
  homeCapabilities,
  type HomeIcon,
} from '../content/home'

const iconMap: Record<HomeIcon, typeof Eye> = {
  observe: Eye,
  govern: ShieldCheck,
  compose: Workflow,
  model: Box,
  runtime: Server,
  adapter: GitBranch,
}
</script>

<template>
  <SiteShell>
    <main class="site-page site-home" data-page="home">
      <section class="site-hero" data-home-hero aria-labelledby="home-title">
        <div class="site-hero__copy">
          <p class="site-eyebrow">Local-first agent orchestration</p>
          <h1 id="home-title">Keep every run observable, bounded, and yours.</h1>
          <p class="site-hero__lede">
            Orchester coordinates coding agents through a local runtime with a durable timeline,
            explicit approvals, and safe browser-facing contracts.
          </p>
          <div class="site-hero__actions">
            <RouterLink class="site-action site-action--primary" to="/install" data-site-link="/install">
              <span>Install locally</span>
              <ArrowUpRight :size="16" :stroke-width="1.8" aria-hidden="true" />
            </RouterLink>
            <RouterLink class="site-action site-action--secondary" to="/architecture">
              Read the architecture
            </RouterLink>
          </div>
          <ul class="site-hero__facts" aria-label="Project properties">
            <li><CircleCheck :size="15" :stroke-width="1.8" aria-hidden="true" /> Rust authority</li>
            <li><CircleCheck :size="15" :stroke-width="1.8" aria-hidden="true" /> Loopback by default</li>
            <li><CircleCheck :size="15" :stroke-width="1.8" aria-hidden="true" /> Replayable events</li>
          </ul>
        </div>

        <div
          class="workspace-preview"
          role="img"
          aria-label="Preview of the Orchester three-column workspace"
          data-workspace-preview
        >
          <div class="workspace-preview__bar" aria-hidden="true">
            <span class="workspace-preview__brand">O/ workspace</span>
            <span class="workspace-preview__status"><span></span> local</span>
          </div>
          <div class="workspace-preview__columns" aria-hidden="true">
            <aside class="workspace-preview__rail">
              <span class="workspace-preview__label">Sessions</span>
              <span class="workspace-preview__item workspace-preview__item--active">Review auth flow</span>
              <span class="workspace-preview__item">Refactor provider</span>
              <span class="workspace-preview__item">Test workspace</span>
            </aside>
            <div class="workspace-preview__transcript">
              <span class="workspace-preview__label">Run timeline</span>
              <div class="workspace-preview__message workspace-preview__message--user">Inspect the boundary.</div>
              <div class="workspace-preview__message">The service is ready for approval.</div>
              <div class="workspace-preview__tool"><span></span> check_workspace <b>ready</b></div>
            </div>
            <aside class="workspace-preview__inspector">
              <span class="workspace-preview__label">Inspector</span>
              <span class="workspace-preview__metric">3 <small>events</small></span>
              <span class="workspace-preview__metric">0 <small>secrets exposed</small></span>
            </aside>
          </div>
        </div>
      </section>

      <section class="site-section" aria-labelledby="capabilities-title">
        <div class="site-section__heading">
          <p class="site-eyebrow">A runtime you can follow</p>
          <h2 id="capabilities-title">The useful parts stay visible.</h2>
          <p>Orchester turns a long-running agent session into a state you can inspect, pause, and resume.</p>
        </div>
        <div class="site-card-grid" data-capability-grid>
          <article v-for="item in homeCapabilities" :key="item.id" class="site-card">
            <component :is="iconMap[item.icon]" class="site-card__icon" :size="20" :stroke-width="1.8" aria-hidden="true" />
            <h3>{{ item.title }}</h3>
            <p>{{ item.summary }}</p>
            <small>{{ item.detail }}</small>
          </article>
        </div>
      </section>

      <section class="site-section site-section--split" aria-labelledby="adapters-title">
        <div class="site-section__heading">
          <p class="site-eyebrow">One contract, several surfaces</p>
          <h2 id="adapters-title">Keep the core steady as the surface changes.</h2>
          <p>Typed boundaries make the local WebUI, static demo, and desktop shell feel related without sharing secrets.</p>
        </div>
        <div class="site-card-grid site-card-grid--compact" data-adapter-grid>
          <article v-for="item in homeAdapters" :key="item.id" class="site-card site-card--compact">
            <component :is="iconMap[item.icon]" class="site-card__icon" :size="18" :stroke-width="1.8" aria-hidden="true" />
            <div>
              <h3>{{ item.title }}</h3>
              <p>{{ item.summary }}</p>
              <small>{{ item.detail }}</small>
            </div>
          </article>
        </div>
      </section>

      <section class="governance-band" data-governance-section aria-labelledby="governance-title">
        <div class="governance-band__heading">
          <LockKeyhole :size="22" :stroke-width="1.8" aria-hidden="true" />
          <div>
            <p class="site-eyebrow">Governance is part of the product</p>
            <h2 id="governance-title">Boundaries you can explain to a teammate.</h2>
          </div>
        </div>
        <div class="governance-band__items">
          <article v-for="item in governancePrinciples" :key="item.id" class="governance-item">
            <h3>{{ item.title }}</h3>
            <p>{{ item.summary }}</p>
            <small>{{ item.detail }}</small>
          </article>
        </div>
        <RouterLink class="text-link" to="/architecture">
          See the boundary map <ArrowUpRight :size="15" :stroke-width="1.8" aria-hidden="true" />
        </RouterLink>
      </section>
    </main>
  </SiteShell>
</template>
