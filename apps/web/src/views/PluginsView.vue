<script setup lang="ts">
import { inject, onUnmounted, ref, watch } from 'vue'
import { routerKey } from 'vue-router'
import { AppButton, AppDialog, Spinner } from '@orchester/design'
import { Puzzle, Download, Trash2, RefreshCw, Check } from '@lucide/vue'
import { createManagementApi, type Plugin } from '../api/management'
import { useAppStores } from '../stores/app'
import { useI18n } from '../i18n'
import '../styles/management.css'

const { t } = useI18n()
const router = inject(routerKey, null)
const stores = useAppStores()
const api = createManagementApi(stores.http)
const plugins = ref<Plugin[]>([])
const loading = ref(true)
const busy = ref('')
const error = ref(false)
const removing = ref<Plugin | null>(null)
const controller = new AbortController()
let fetching = false
async function load(): Promise<void> {
  if (fetching || stores.bootstrap.status.value !== 'ready') return
  fetching = true; loading.value = !plugins.value.length; error.value = false
  try { plugins.value = await api.plugins(controller.signal) }
  catch { if (!controller.signal.aborted) error.value = true }
  finally { loading.value = false; fetching = false }
}
watch(() => stores.bootstrap.status.value, status => {
  if (status === 'ready') void load()
  if (status === 'error') { loading.value = false; error.value = true }
}, { immediate: true })
onUnmounted(() => controller.abort())
async function mutate(plugin: Plugin, remove = false): Promise<void> {
  if (busy.value) return
  busy.value = plugin.name; error.value = false
  try {
    if (remove) await api.removePlugin(plugin.name)
    else await api.installPlugin(plugin.name)
    removing.value = null
    await load()
    await stores.agents.load()
  } catch { error.value = true }
  finally { busy.value = '' }
}
</script>

<template>
  <main class="management-view" data-plugins-view>
    <header class="management-header"><div><span class="management-eyebrow">{{ t('app.name') }}</span><h1>{{ t('navigation.plugins') }}</h1><p>{{ t('plugins.subtitle') }}</p></div><AppButton variant="ghost" :disabled="loading || !!busy" @click="load()"><RefreshCw :size="16" />{{ t('management.refresh') }}</AppButton></header>
    <p class="management-note"><Puzzle :size="16" />{{ t('plugins.note') }}</p>
    <div v-if="error" class="management-error" role="alert">{{ t('plugins.error') }}<AppButton size="sm" variant="ghost" :disabled="!!busy" @click="load()">{{ t('management.retry') }}</AppButton></div>
    <div v-if="loading" class="management-empty"><Spinner :label="t('management.loading')" /></div>
    <section v-else class="plugin-grid" :aria-label="t('navigation.plugins')">
      <article v-for="plugin in plugins" :key="plugin.name" class="management-card plugin-card" :data-plugin-name="plugin.name"><div class="plugin-card__icon"><Puzzle :size="24" /></div><h2>{{ plugin.display_name }}<span v-if="plugin.installed" class="management-badge" data-active="true"><Check :size="12" />{{ t('plugins.installed') }}</span></h2><p class="management-meta">{{ plugin.package_name }} · {{ plugin.version }}</p><p>{{ t('plugins.adapterDescription', { name: plugin.display_name }) }}</p><div class="management-card__bottom"><span class="management-meta">{{ t(plugin.origin === 'project' ? 'plugins.project' : plugin.installed ? 'plugins.managed' : 'plugins.available') }}</span><AppButton v-if="!plugin.installed" size="sm" :disabled="!!busy" @click="mutate(plugin)"><Spinner v-if="busy === plugin.name" :size="14" :label="t('plugins.installing')" /><Download v-else :size="14" />{{ t(busy === plugin.name ? 'plugins.installing' : 'plugins.install') }}</AppButton><AppButton v-else-if="plugin.removable" size="sm" variant="ghost" :disabled="!!busy" @click="removing = plugin"><Trash2 :size="14" />{{ t('plugins.remove') }}</AppButton><span v-else class="management-meta">{{ t('plugins.readOnly') }}</span></div><AppButton size="sm" variant="ghost" data-plugin-activity @click="router?.push({ name: 'workspace', query: { agent: plugin.name } })">{{ t('activity.view') }}</AppButton></article>
    </section>
    <AppDialog :open="!!removing" :title="t('plugins.removeTitle')" :description="removing?.display_name ?? ''" :close-label="t('management.close')" @close="removing = null"><p>{{ t('plugins.removeDescription') }}</p><p v-if="error" class="management-error" role="alert">{{ t('plugins.error') }}</p><div class="management-form__actions"><AppButton variant="ghost" :disabled="!!busy" @click="removing = null">{{ t('management.cancel') }}</AppButton><AppButton variant="danger" :disabled="!!busy" @click="removing && mutate(removing, true)">{{ t(busy ? 'plugins.removing' : 'plugins.remove') }}</AppButton></div></AppDialog>
  </main>
</template>
