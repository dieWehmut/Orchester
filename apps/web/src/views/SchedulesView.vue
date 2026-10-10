<script setup lang="ts">
import { computed, onUnmounted, reactive, ref, watch } from 'vue'
import { AppButton, AppDialog, Spinner } from '@orchester/design'
import { Clock3, Plus, Play, Pause, Pencil, Trash2, RefreshCw } from '@lucide/vue'
import { createManagementApi, scheduleInput, type Schedule } from '../api/management'
import { HttpResponseError } from '../api/http'
import { useAppStores } from '../stores/app'
import { useI18n, type MessageKey } from '../i18n'
import '../styles/management.css'

const { t, locale } = useI18n()
const stores = useAppStores()
const api = createManagementApi(stores.http)
const tasks = ref<Schedule[]>([])
const loading = ref(true)
const ready = ref(false)
const busy = ref('')
const error = ref<MessageKey | null>(null)
const editorOpen = ref(false)
const editing = ref<string | undefined>()
const deleting = ref<Schedule | null>(null)
const draft = reactive({ name: '', prompt: '', start: '', repeat: 86400, enabled: true })
const expanded = ref<string | null>(null)
const controller = new AbortController()
let polling: ReturnType<typeof setInterval> | undefined
let fetching = false
const configured = computed(() => stores.models.catalog?.active.state === 'configured')

function errorKey(reason: unknown): MessageKey {
  return reason instanceof HttpResponseError && reason.response.status === 409 ? 'management.conflict' : 'management.error'
}
async function load(quiet = false): Promise<void> {
  if (fetching || controller.signal.aborted || stores.bootstrap.status.value !== 'ready') return
  fetching = true
  if (!quiet) loading.value = true
  try { tasks.value = await api.schedules(controller.signal); ready.value = true; if (!busy.value) error.value = null }
  catch (reason) { if (!controller.signal.aborted) error.value = errorKey(reason) }
  finally { loading.value = false; fetching = false }
}
watch(() => stores.bootstrap.status.value, status => {
  if (status === 'ready') void load()
  if (status === 'error') { loading.value = false; error.value = 'management.error' }
}, { immediate: true })
polling = setInterval(() => { if (!editorOpen.value && !busy.value) void load(true) }, 5000)
onUnmounted(() => { controller.abort(); clearInterval(polling) })

function localTime(timestamp: number): string {
  const date = new Date(timestamp * 1000)
  return new Date(date.getTime() - date.getTimezoneOffset() * 60000).toISOString().slice(0, 16)
}
function date(timestamp: number): string { return new Date(timestamp * 1000).toLocaleString(locale.value, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' }) }
function openEditor(task?: Schedule): void {
  editing.value = task?.id
  Object.assign(draft, { name: task?.name ?? '', prompt: task?.prompt ?? '', start: localTime(task?.first_run_at ?? Date.now() / 1000 + 3600), repeat: task?.interval_seconds ?? (task ? 0 : 86400), enabled: task?.enabled ?? true })
  error.value = null
  editorOpen.value = true
}
async function perform(id: string, action: () => Promise<unknown>): Promise<void> {
  if (busy.value) return
  busy.value = id; error.value = null
  try { await action(); await load(true) }
  catch (reason) { error.value = errorKey(reason) }
  finally { busy.value = '' }
}
async function save(): Promise<void> {
  const first = Math.floor(new Date(draft.start).getTime() / 1000)
  if (!Number.isFinite(first) || !draft.name.trim() || !draft.prompt.trim()) return
  await perform('save', async () => {
    await api.save({ name: draft.name.trim(), prompt: draft.prompt, enabled: draft.enabled, first_run_at: first, interval_seconds: draft.repeat || null }, editing.value)
    editorOpen.value = false
  })
}
function active(task: Schedule): boolean { return task.history.some(run => run.status === 'running') }
function statusKey(status: string): MessageKey {
  const keys: Record<string, MessageKey> = { running: 'schedules.running', succeeded: 'schedules.succeeded', failed: 'schedules.failed', cancelled: 'schedules.cancelled', interrupted: 'schedules.interrupted' }
  return keys[status] ?? 'schedules.failed'
}
</script>

<template>
  <main class="management-view" data-schedules-view>
    <header class="management-header"><div><span class="management-eyebrow">{{ t('app.name') }}</span><h1>{{ t('navigation.schedules') }}</h1><p>{{ t('schedules.subtitle') }}</p></div>
      <AppButton :disabled="!ready || !!busy" @click="openEditor()"><Plus :size="16" />{{ t('schedules.create') }}</AppButton>
    </header>
    <p class="management-note"><Clock3 :size="16" />{{ t('schedules.availability') }}</p>
    <p v-if="!configured" class="management-note">{{ t('schedules.modelNote') }} <RouterLink :to="{ name: 'settings', query: { section: 'providers' } }">{{ t('run.modelSettings') }}</RouterLink></p>
    <div v-if="error" class="management-error" role="alert">{{ t(error) }}<AppButton size="sm" variant="ghost" :disabled="!!busy" @click="load()">{{ t('management.retry') }}</AppButton></div>
    <div v-if="loading" class="management-empty"><Spinner :label="t('management.loading')" /></div>
    <div v-else-if="ready && !tasks.length" class="management-empty"><Clock3 :size="32" /><h2>{{ t('schedules.empty') }}</h2><p>{{ t('schedules.emptyDescription') }}</p></div>
    <section v-else class="management-list" :aria-label="t('navigation.schedules')">
      <article v-for="task in tasks" :key="task.id" class="management-card" :data-schedule-id="task.id">
        <div class="management-card__top"><div class="management-card__copy"><h2>{{ task.name }}<span class="management-badge" :data-active="task.enabled">{{ t(task.enabled ? 'schedules.enabled' : 'schedules.paused') }}</span></h2><p class="management-prompt">{{ task.prompt }}</p><p class="management-meta">{{ t(task.enabled ? 'schedules.next' : 'schedules.scheduledFor') }} {{ date(task.next_run_at) }} · {{ task.interval_seconds ? t('schedules.every', { minutes: String(task.interval_seconds / 60) }) : t('schedules.once') }}</p></div>
          <div class="management-actions">
            <AppButton size="sm" variant="ghost" :disabled="!!busy || active(task)" :aria-label="t(task.enabled ? 'schedules.pause' : 'schedules.enable')" @click="perform(task.id, () => api.save({ ...scheduleInput(task), enabled: !task.enabled }, task.id))"><Pause v-if="task.enabled" :size="16" /><Play v-else :size="16" /></AppButton>
            <AppButton size="sm" variant="ghost" :disabled="!!busy || active(task)" :aria-label="t('management.edit')" @click="openEditor(task)"><Pencil :size="16" /></AppButton>
            <AppButton size="sm" variant="ghost" :disabled="!!busy || active(task)" :aria-label="t('management.delete')" @click="deleting = task"><Trash2 :size="16" /></AppButton>
          </div></div>
        <div class="management-card__bottom"><button class="management-link" :aria-expanded="expanded === task.id" @click="expanded = expanded === task.id ? null : task.id">{{ t('schedules.history') }} ({{ task.history.length }})</button><AppButton size="sm" variant="secondary" :disabled="!!busy || active(task)" @click="perform(task.id, () => api.run(task.id))"><Spinner v-if="active(task)" :size="14" :label="t('schedules.running')" /><Play v-else :size="14" />{{ t(active(task) ? 'schedules.running' : 'schedules.runNow') }}</AppButton></div>
        <div v-if="expanded === task.id" class="management-history"><p v-if="!task.history.length">{{ t('schedules.noHistory') }}</p><div v-for="run in task.history" :key="run.id" class="management-history__row"><time>{{ date(run.started_at) }}</time><span class="management-badge" :data-status="run.status">{{ t(statusKey(run.status)) }}</span><span v-if="run.finished_at" class="management-meta">{{ Math.max(0, run.finished_at - run.started_at) }}s</span><AppButton v-if="run.status === 'running' && run.run_id" size="sm" variant="ghost" :disabled="!!busy" @click="perform(task.id, () => api.cancel(run.run_id!))">{{ t('run.stop') }}</AppButton></div></div>
      </article>
    </section>
    <footer v-if="ready" class="management-footer"><button class="management-link" :disabled="loading" @click="load()"><RefreshCw :size="14" />{{ t('management.refresh') }}</button><span>{{ t('schedules.savedLocally') }}</span></footer>
    <AppDialog :open="editorOpen" :title="t(editing ? 'schedules.edit' : 'schedules.create')" :close-label="t('management.close')" @update:open="editorOpen = $event">
      <form class="management-form" @submit.prevent="save"><label>{{ t('schedules.name') }}<input v-model="draft.name" required maxlength="80" :disabled="!!busy" /></label><label>{{ t('schedules.prompt') }}<textarea v-model="draft.prompt" required maxlength="8000" rows="5" :disabled="!!busy" :placeholder="t('schedules.promptPlaceholder')" /></label><div class="management-form__row"><label>{{ t('schedules.firstRun') }}<input v-model="draft.start" type="datetime-local" required :disabled="!!busy" /></label><label>{{ t('schedules.repeat') }}<select v-model.number="draft.repeat" :disabled="!!busy"><option :value="0">{{ t('schedules.once') }}</option><option :value="3600">{{ t('schedules.hourly') }}</option><option :value="86400">{{ t('schedules.daily') }}</option><option :value="604800">{{ t('schedules.weekly') }}</option><option v-if="![0,3600,86400,604800].includes(draft.repeat)" :value="draft.repeat">{{ t('schedules.every', { minutes: String(draft.repeat / 60) }) }}</option></select></label></div><label class="management-checkbox"><input v-model="draft.enabled" type="checkbox" :disabled="!!busy" />{{ t('schedules.enableAfterSave') }}</label><p class="management-meta">{{ t('schedules.selectionNote') }}</p><p v-if="error" role="alert" class="management-error">{{ t(error) }}</p><div class="management-form__actions"><AppButton type="button" variant="ghost" :disabled="!!busy" @click="editorOpen = false">{{ t('management.cancel') }}</AppButton><AppButton type="submit" :disabled="!!busy || !draft.name.trim() || !draft.prompt.trim()">{{ t(busy === 'save' ? 'management.saving' : 'management.save') }}</AppButton></div></form>
    </AppDialog>
    <AppDialog :open="!!deleting" :title="t('schedules.deleteTitle')" :description="deleting?.name ?? ''" :close-label="t('management.close')" @close="deleting = null"><p>{{ t('schedules.deleteDescription') }}</p><div class="management-form__actions"><AppButton variant="ghost" :disabled="!!busy" @click="deleting = null">{{ t('management.cancel') }}</AppButton><AppButton variant="danger" :disabled="!!busy" @click="perform('delete', async () => { if (deleting) await api.removeSchedule(deleting.id); deleting = null })">{{ t('management.delete') }}</AppButton></div></AppDialog>
  </main>
</template>
