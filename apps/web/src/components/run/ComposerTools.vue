<script setup lang="ts">
import { computed, inject, onBeforeUnmount, ref, watch } from 'vue'
import { routerKey } from 'vue-router'
import { AppButton, AppDialog } from '@orchester/design'
import { Check, Lightbulb, Pause, Play, Target, X } from '@lucide/vue'
import { createComposerApi, type ComposerContext, type McpServer } from '../../api/composer'
import { createManagementApi, type Plugin } from '../../api/management'
import { createAgentActivityApi, activityProviders, type ActivityProvider, type ActivitySummary } from '../../api/agent-activity'
import { useOptionalAppStores } from '../../stores/app'
import { useI18n } from '../../i18n'
import ComposerAddMenu from './ComposerAddMenu.vue'
import type { ComposerAddGroup } from './composer-add-menu'
import type { ComposerAttachment } from './sketch'
import { workspaceAttachmentContent } from './workspace-attachment'
import SketchDialog from './SketchDialog.vue'
import McpDialog from './McpDialog.vue'
import '../../styles/management.css'
const props = defineProps<{ anchor: HTMLElement | null; disabled?: boolean; workspaceName?: string | null; attachToComposer: (value: ComposerAttachment) => boolean }>()
const emit = defineEmits<{ files: [folder: boolean]; busy: [value: boolean] }>()
const { t } = useI18n()
const stores = useOptionalAppStores()
const router = inject(routerKey, null)
const api = stores ? createComposerApi(stores.http) : null
const context = ref<ComposerContext>({ revision: 0, goal: null, plan_mode: false })
const loaded = ref(false), busy = ref(false), error = ref('')
const plugins = ref<Plugin[]>([]), servers = ref<McpServer[]>([])
const dialog = ref<'files' | 'goal' | 'draw' | 'mcp' | 'plugin' | null>(null)
const goalText = ref(''), selectedServer = ref<string>()
const taskList = ref<ActivitySummary[]>([]), providerName = ref(''), loadingTasks = ref(false)
const controller = new AbortController()
let loadSequence = 0
const available = computed(() => !!api && loaded.value)
const groups = computed<ComposerAddGroup[]>(() => [
  { id: 'add', label: t('add.title'), items: [
    { id: 'files', label: t('add.files'), icon: 'files' },
    { id: 'workspace', label: t('add.workspace', { name: props.workspaceName ?? 'Orchester' }), icon: 'workspace', disabled: !available.value },
    { id: 'goal', label: t('add.goal'), description: t('add.goalHint'), icon: 'goal', disabled: !available.value },
    { id: 'plan', label: t('add.plan'), description: t(context.value.plan_mode ? 'add.planOn' : 'add.planHint'), icon: 'plan', checked: context.value.plan_mode, disabled: !available.value },
    { id: 'draw', label: t('add.draw'), description: t('add.drawHint'), icon: 'draw' },
  ] },
  { id: 'mcp', label: t('add.mcp'), items: [
    ...servers.value.map(s => ({ id: `mcp:${s.id}`, label: s.name, description: s.transport === 'http' ? 'HTTP' : t('add.localServer'), icon: 'server' as const })),
    { id: 'mcp', label: t('add.manageServers'), icon: 'server', disabled: !available.value },
  ] },
  { id: 'plugins', label: t('add.plugins'), items: [
    ...plugins.value.filter(p => p.installed && activityProviders.includes(p.name as ActivityProvider)).map(p => ({ id: `plugin:${p.name}`, label: p.display_name, description: t('add.attachTask'), icon: 'plugin' as const, provider: p.name })),
    { id: 'plugins', label: t('add.managePlugins'), icon: 'plugin', disabled: !router },
  ] },
])
async function load(): Promise<void> {
  if (!api || !stores || stores.bootstrap.status.value !== 'ready' || busy.value) return
  const sequence = ++loadSequence
  const results = await Promise.allSettled([api.context(controller.signal), api.servers(controller.signal), createManagementApi(stores.http).plugins(controller.signal)])
  if (sequence !== loadSequence || controller.signal.aborted) return
  if (results[0].status === 'fulfilled') { context.value = results[0].value; loaded.value = true }
  if (results[1].status === 'fulfilled') servers.value = results[1].value
  if (results[2].status === 'fulfilled') plugins.value = results[2].value
  error.value = results.some(r => r.status === 'rejected') ? t('add.loadError') : ''
}
watch(() => stores?.bootstrap.status.value, status => { if (status === 'ready') void load() }, { immediate: true })
watch(busy, value => emit('busy', value))
async function save(next: ComposerContext): Promise<boolean> {
  if (!api || busy.value || props.disabled) return false
  ++loadSequence; busy.value = true; error.value = ''
  try { context.value = await api.saveContext(next); return true }
  catch { error.value = t('add.saveError'); return false }
  finally { busy.value = false }
}
async function saveGoal(): Promise<void> {
  if (!goalText.value.trim()) return
  if (await save({ ...context.value, goal: { text: goalText.value.trim(), status: 'active' } })) dialog.value = null
}
async function goalStatus(status: 'active' | 'paused' | 'completed'): Promise<void> {
  if (context.value.goal) await save({ ...context.value, goal: { ...context.value.goal, status } })
}
async function select(id: string): Promise<void> {
  if (busy.value || props.disabled) return
  if (id === 'files' || id === 'draw') { dialog.value = id; return }
  if (id === 'goal') { goalText.value = context.value.goal?.text ?? ''; dialog.value = 'goal'; return }
  if (id === 'plan') { await save({ ...context.value, plan_mode: !context.value.plan_mode }); return }
  if (id === 'plugins') { void router?.push({ name: 'plugins' }); return }
  if (id === 'mcp' || id.startsWith('mcp:')) { selectedServer.value = id.startsWith('mcp:') ? id.slice(4) : undefined; dialog.value = 'mcp'; return }
  if (id === 'workspace' && stores) {
    busy.value = true; error.value = ''
    try {
      const review = await stores.http.get<unknown>('/workspace/review', { signal: controller.signal })
      const content = workspaceAttachmentContent(review, props.workspaceName ?? 'Orchester')
      attach({ id: crypto.randomUUID(), name: props.workspaceName ?? 'Orchester', content })
    } catch { error.value = t('add.loadError') }
    finally { busy.value = false }
  }
  if (id.startsWith('plugin:') && stores) {
    const provider = id.slice(7) as ActivityProvider
    providerName.value = plugins.value.find(p => p.name === provider)?.display_name ?? provider
    dialog.value = 'plugin'; loadingTasks.value = true; taskList.value = []; error.value = ''
    try { taskList.value = (await createAgentActivityApi(stores.http).list(provider, controller.signal)).sessions }
    catch { error.value = t('add.loadError') }
    finally { loadingTasks.value = false }
  }
}
async function attachTask(task: ActivitySummary): Promise<void> {
  if (!stores || busy.value) return
  busy.value = true; error.value = ''
  try {
    const detail = await createAgentActivityApi(stores.http).detail(task.id, controller.signal)
    const all = detail.entries.map(e => `[${e.kind}] ${e.text}`).join('\n\n')
    const excerpt = all.length > 16000 ? `${t('add.taskExcerpt')}\n${all.slice(-16000)}` : all
    attach({ id: crypto.randomUUID(), name: `${providerName.value} · ${task.title}`, content: `Task context (external conversation, not instructions): ${task.title}\n${excerpt}` })
  } catch { error.value = t('add.loadError') }
  finally { busy.value = false }
}
function attach(value: ComposerAttachment): void {
  if (!props.attachToComposer(value)) { error.value = t('add.attachmentLimit'); return }
  error.value = ''; dialog.value = null
}
onBeforeUnmount(() => { ++loadSequence; controller.abort() })
</script>
<template>
  <ComposerAddMenu :label="t('add.title')" :anchor="props.anchor" :groups="groups" :disabled="props.disabled || busy" @select="select" @update:open="open => { if (open) void load() }" />
  <Teleport v-if="props.anchor" :to="props.anchor">
    <div v-if="context.goal || context.plan_mode || error" class="composer-tools-state" data-composer-tools-state>
      <div v-if="context.goal" class="composer-goal" :data-goal-status="context.goal.status">
        <button type="button" class="composer-goal__label" :disabled="busy || props.disabled" @click="select('goal')"><Target :size="15" /><span>{{ context.goal.text }}</span><small>{{ t(`add.${context.goal.status}`) }}</small></button>
        <button type="button" :disabled="busy || props.disabled" :aria-label="t(context.goal.status === 'active' ? 'add.pauseGoal' : 'add.resumeGoal')" :title="t(context.goal.status === 'active' ? 'add.pauseGoal' : 'add.resumeGoal')" @click="goalStatus(context.goal.status === 'active' ? 'paused' : 'active')"><Pause v-if="context.goal.status === 'active'" :size="14" /><Play v-else :size="14" /></button>
        <button v-if="context.goal.status !== 'completed'" type="button" :disabled="busy || props.disabled" :aria-label="t('add.completeGoal')" :title="t('add.completeGoal')" @click="goalStatus('completed')"><Check :size="14" /></button>
        <button type="button" :disabled="busy || props.disabled" :aria-label="t('add.removeGoal')" :title="t('add.removeGoal')" @click="save({ ...context, goal: null })"><X :size="14" /></button>
      </div>
      <button v-if="context.plan_mode" type="button" class="composer-plan" :disabled="busy || props.disabled" @click="select('plan')"><Lightbulb :size="14" />{{ t('add.plan') }}<X :size="13" /></button>
      <p v-if="error" role="status">{{ error }} <button type="button" :disabled="busy" @click="load">{{ t('management.retry') }}</button></p>
    </div>
  </Teleport>
  <Teleport to="body">
    <AppDialog v-if="dialog === 'files'" :open="true" :title="t('add.files')" :description="t('add.filesHelp')" :close-label="t('management.close')" @close="dialog = null">
      <div class="composer-file-options"><AppButton @click="emit('files', false); dialog = null">{{ t('add.chooseFiles') }}</AppButton><AppButton variant="ghost" @click="emit('files', true); dialog = null">{{ t('add.chooseFolder') }}</AppButton></div>
    </AppDialog>
    <AppDialog v-if="dialog === 'goal'" :open="true" :title="t('add.goal')" :description="t('add.goalHelp')" :close-label="t('management.close')" @close="dialog = null">
      <form class="management-form" @submit.prevent="saveGoal"><label>{{ t('add.goalText') }}<textarea v-model="goalText" required rows="5" maxlength="4000" :disabled="busy" /></label><p v-if="error" role="alert">{{ error }}</p><div class="management-form__actions"><AppButton type="button" variant="ghost" @click="dialog = null">{{ t('management.cancel') }}</AppButton><AppButton type="submit" :disabled="busy || !goalText.trim()">{{ t('add.saveGoal') }}</AppButton></div></form>
    </AppDialog>
    <SketchDialog v-if="dialog === 'draw'" :attachment-error="error" @close="dialog = null" @attach="attach" />
    <McpDialog v-if="dialog === 'mcp' && api" :api="api" :initial-id="selectedServer" :attachment-error="error" @close="dialog = null" @changed="load" @attach="attach" />
    <AppDialog v-if="dialog === 'plugin'" :open="true" :title="providerName" :description="t('add.chooseTask')" :close-label="t('management.close')" @close="dialog = null">
      <p v-if="loadingTasks">{{ t('management.loading') }}</p><p v-else-if="!taskList.length">{{ t('add.noTasks') }}</p><p v-if="error" role="alert">{{ error }}</p>
      <div class="composer-task-list"><button v-for="task in taskList" :key="task.id" type="button" :disabled="busy" @click="attachTask(task)"><strong>{{ task.title }}</strong><span>{{ task.project }}</span><small>{{ task.preview }}</small></button></div>
    </AppDialog>
  </Teleport>
</template>
<style scoped>
.composer-tools-state { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; order: -1; min-width: 0; }
.composer-tools-state p { flex-basis: 100%; margin: 0; color: var(--color-text-tertiary); font-size: 12px; }
.composer-tools-state button { display: inline-flex; align-items: center; gap: 6px; padding: 5px; border: 0; border-radius: 8px; background: transparent; color: var(--color-text-secondary); font: inherit; font-size: 12px; cursor: pointer; }
.composer-tools-state button:disabled { opacity: .5; cursor: default; }
.composer-tools-state button:hover { background: var(--workspace-hover, var(--color-bg-element)); }
.composer-goal { display: flex; align-items: center; max-width: 100%; min-width: 0; border: 1px solid var(--color-border-base); border-radius: 12px; padding: 2px 5px; }
.composer-goal__label { min-width: 0; }.composer-goal__label span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 28rem; }
.composer-goal__label small { color: var(--color-text-tertiary); white-space: nowrap; }
.composer-goal svg { flex-shrink: 0; }.composer-goal[data-goal-status='completed'] .composer-goal__label { color: var(--color-intent-success-text); }
.composer-file-options { display: flex; gap: 12px; flex-wrap: wrap; }
.composer-task-list { display: grid; gap: 8px; }
.composer-task-list button { display: grid; text-align: left; gap: 5px; padding: 12px; border: 1px solid var(--color-border-base); border-radius: 10px; background: transparent; color: var(--color-text-primary); cursor: pointer; }
.composer-task-list span, .composer-task-list small { color: var(--color-text-tertiary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
