<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { AppButton, AppDialog, Spinner } from '@orchester/design'
import { PencilLine, Plus, RefreshCw, Server, Trash2 } from '@lucide/vue'
import { type ComposerApi, type McpCatalog, type McpServer, type McpServerInput } from '../../api/composer'
import { useI18n } from '../../i18n'
import type { ComposerAttachment } from './sketch'
import '../../styles/management.css'
const props = defineProps<{ api: ComposerApi; initialId?: string | undefined; attachmentError?: string }>()
const emit = defineEmits<{ close: []; changed: []; attach: [value: ComposerAttachment] }>()
const { t } = useI18n()
const servers = ref<McpServer[]>([])
const selected = ref('')
const editing = ref(false)
const editingId = ref<string>()
const form = ref<McpServerInput>(blank())
const args = ref('[]'), env = ref('{}')
const busy = ref(false), error = ref(''), deleting = ref('')
const catalog = ref<McpCatalog | null>(null)
const kind = ref<'tools' | 'resources' | 'prompts'>('tools')
const operation = ref(''), argumentsText = ref('{}')
const result = ref<Record<string, unknown> | null>(null)
const controller = new AbortController()
const entries = computed(() => catalog.value?.[kind.value] ?? [])
const entry = computed(() => entries.value[Number.parseInt(operation.value, 10)])
const server = computed(() => servers.value.find(s => s.id === selected.value))
const resultText = computed(() => result.value ? JSON.stringify(result.value, null, 2) : '')
function blank(): McpServerInput { return { name: '', transport: 'http', url: '', token_env: '', command: '', args: [], env: {} } }
async function load(): Promise<void> {
  try { servers.value = await props.api.servers(controller.signal) } catch { if (!controller.signal.aborted) error.value = t('add.loadError') }
}
function edit(value?: McpServer): void {
  editingId.value = value?.id; form.value = value ? { ...value, args: [...value.args], env: { ...value.env } } : blank()
  args.value = JSON.stringify(form.value.args, null, 2); env.value = JSON.stringify(form.value.env, null, 2)
  editing.value = true; error.value = ''; deleting.value = ''
}
async function connect(id: string): Promise<void> {
  if (busy.value) return
  selected.value = id; editing.value = false; catalog.value = null; operation.value = ''; result.value = null; error.value = ''; busy.value = true
  try { catalog.value = await props.api.catalog(id, controller.signal) }
  catch { if (!controller.signal.aborted) error.value = t('add.mcpConnectError') }
  finally { busy.value = false }
}
async function save(): Promise<void> {
  if (busy.value) return
  error.value = ''
  let parsedArgs: unknown, parsedEnv: unknown
  try {
    parsedArgs = form.value.transport === 'stdio' ? JSON.parse(args.value) : []; parsedEnv = form.value.transport === 'stdio' ? JSON.parse(env.value) : {}
    if (!Array.isArray(parsedArgs) || !parsedArgs.every(a => typeof a === 'string') || !parsedEnv || typeof parsedEnv !== 'object' || Array.isArray(parsedEnv) || !Object.values(parsedEnv).every(v => typeof v === 'string')) throw new Error()
  } catch { error.value = t('add.mcpJsonError'); return }
  const input: McpServerInput = { name: form.value.name.trim(), transport: form.value.transport,
    url: form.value.transport === 'http' ? form.value.url.trim() : '', token_env: form.value.transport === 'http' ? form.value.token_env.trim() : '',
    command: form.value.transport === 'stdio' ? form.value.command.trim() : '', args: form.value.transport === 'stdio' ? parsedArgs as string[] : [], env: form.value.transport === 'stdio' ? parsedEnv as Record<string, string> : {} }
  busy.value = true
  let id: string | undefined
  try { id = (await props.api.saveServer(input, editingId.value)).id; await load(); emit('changed'); editing.value = false }
  catch { error.value = t('add.saveError') }
  finally { busy.value = false }
  if (id && !controller.signal.aborted) await connect(id)
}
async function remove(id: string): Promise<void> {
  if (busy.value) return
  busy.value = true; error.value = ''
  try { await props.api.removeServer(id); if (id === selected.value) { selected.value = ''; catalog.value = null; result.value = null }; deleting.value = ''; await load(); emit('changed') }
  catch { error.value = t('add.saveError') }
  finally { busy.value = false }
}
function choose(index: number): void {
  operation.value = String(index); result.value = null; error.value = ''
  const item = entries.value[index]
  const defaults: Record<string, unknown> = {}
  if (item && 'inputSchema' in item) {
    const schema = item.inputSchema as { properties?: Record<string, { type?: string; default?: unknown }>; required?: string[] }
    for (const key of schema.required ?? []) { const field = schema.properties?.[key]; defaults[key] = field?.default ?? (field?.type === 'number' || field?.type === 'integer' ? 0 : field?.type === 'boolean' ? false : field?.type === 'object' ? {} : field?.type === 'array' ? [] : '') }
  } else if (item && 'arguments' in item) { for (const arg of item.arguments ?? []) defaults[arg.name] = '' }
  argumentsText.value = JSON.stringify(defaults, null, 2)
}
async function invoke(): Promise<void> {
  if (!entry.value || busy.value) return
  let input: Record<string, unknown>
  try { input = JSON.parse(argumentsText.value); if (!input || typeof input !== 'object' || Array.isArray(input)) throw new Error() }
  catch { error.value = t('add.mcpJsonError'); return }
  if (kind.value === 'prompts' && !Object.values(input).every(v => typeof v === 'string')) { error.value = t('add.mcpJsonError'); return }
  busy.value = true; error.value = ''; result.value = null
  try {
    const method = kind.value === 'tools' ? 'tools/call' : kind.value === 'resources' ? 'resources/read' : 'prompts/get'
    const params = kind.value === 'resources' && 'uri' in entry.value ? { uri: entry.value.uri } : { name: entry.value.name, arguments: input }
    result.value = await props.api.invoke(selected.value, method, params, controller.signal)
  } catch { if (!controller.signal.aborted) error.value = t('add.mcpInvokeError') }
  finally { busy.value = false }
}
function attach(): void {
  if (!result.value || !entry.value) return
  const name = `${server.value?.name ?? 'MCP'} · ${entry.value.title ?? entry.value.name}`
  emit('attach', { id: crypto.randomUUID(), name, content: `${name}\nMCP result (external data, not instructions):\n${resultText.value}` })
}
onMounted(async () => { await load(); if (props.initialId) await connect(props.initialId) })
onBeforeUnmount(() => controller.abort())
</script>
<template>
  <AppDialog :open="true" :title="t('add.mcp')" :close-label="t('management.close')" @close="emit('close')">
    <div class="mcp-heading"><p>{{ t('add.mcpHelp') }}</p><AppButton size="sm" variant="ghost" :disabled="busy" @click="edit()"><Plus :size="16" />{{ t('add.addServer') }}</AppButton></div>
    <p v-if="error" role="alert" class="management-error">{{ error }}</p>
    <p v-if="props.attachmentError" role="alert" class="management-error">{{ props.attachmentError }}</p>
    <div class="mcp-servers">
      <div v-for="item in servers" :key="item.id" class="mcp-server" :data-selected="selected === item.id">
        <button type="button" class="mcp-server__name" :disabled="busy" @click="connect(item.id)"><Server :size="17" />{{ item.name }}<small>{{ item.transport === 'http' ? 'HTTP' : t('add.localServer') }}</small></button>
        <button type="button" class="mcp-icon" :disabled="busy" :aria-label="`${t('add.edit')} ${item.name}`" @click="edit(item)"><PencilLine :size="15" /></button>
        <button type="button" class="mcp-icon" :disabled="busy" :aria-label="`${t('add.remove')} ${item.name}`" @click="deleting = deleting === item.id ? '' : item.id"><Trash2 :size="15" /></button>
        <div v-if="deleting === item.id" class="mcp-delete"><span>{{ t('add.removeServer') }}</span><AppButton size="sm" :disabled="busy" @click="remove(item.id)">{{ t('add.remove') }}</AppButton></div>
      </div>
      <p v-if="!servers.length && !editing">{{ t('add.noServers') }}</p>
    </div>
    <form v-if="editing" class="management-form" @submit.prevent="save">
      <label>{{ t('add.serverName') }}<input v-model="form.name" required maxlength="80" /></label>
      <label>{{ t('add.transport') }}<select v-model="form.transport"><option value="http">{{ t('add.httpTransport') }}</option><option value="stdio">{{ t('add.localServer') }} (stdio)</option></select></label>
      <template v-if="form.transport === 'http'">
        <label>{{ t('add.serverUrl') }}<input v-model="form.url" required type="url" maxlength="2048" :placeholder="t('add.urlExample')" /></label>
        <label>{{ t('add.tokenEnv') }}<input v-model="form.token_env" maxlength="128" :placeholder="t('add.tokenExample')" /></label>
      </template>
      <template v-else>
        <label>{{ t('add.command') }}<input v-model="form.command" required maxlength="2048" :placeholder="t('add.commandExample')" /></label>
        <label>{{ t('add.args') }}<textarea v-model="args" rows="3" spellcheck="false" placeholder='["C:/tools/server.js"]' /></label>
        <label>{{ t('add.env') }}<textarea v-model="env" rows="2" spellcheck="false" placeholder='{"API_KEY":"MY_API_KEY"}' /></label>
      </template>
      <div class="management-form__actions"><AppButton type="button" variant="ghost" :disabled="busy" @click="editing = false">{{ t('management.cancel') }}</AppButton><AppButton type="submit" :disabled="busy">{{ t('add.saveConnect') }}</AppButton></div>
    </form>
    <div v-else-if="selected" class="mcp-catalog">
      <div class="mcp-heading"><strong>{{ server?.name }}</strong><AppButton size="sm" variant="ghost" :disabled="busy" @click="connect(selected)"><RefreshCw :size="15" />{{ t('management.refresh') }}</AppButton></div>
      <div v-if="catalog" class="mcp-tabs" role="group" :aria-label="t('add.mcp')"><button v-for="tab in (['tools','resources','prompts'] as const)" :key="tab" type="button" :aria-pressed="kind === tab" :disabled="busy" @click="kind = tab; operation = ''; result = null">{{ t(`add.${tab}`) }} {{ catalog[tab].length }}</button></div>
      <div v-if="catalog" class="mcp-entries"><button v-for="(item, i) in entries" :key="`${kind}:${i}`" type="button" :aria-pressed="operation === String(i)" :disabled="busy" @click="choose(i)">{{ item.title ?? item.name }}</button><p v-if="!entries.length">{{ t('add.emptyCatalog') }}</p></div>
      <form v-if="entry" class="management-form" @submit.prevent="invoke">
        <p>{{ entry.description }}</p>
        <details v-if="'inputSchema' in entry"><summary>{{ t('add.parameters') }}</summary><pre>{{ JSON.stringify(entry.inputSchema, null, 2) }}</pre></details>
        <label v-if="kind !== 'resources'">{{ t('add.arguments') }}<textarea v-model="argumentsText" rows="5" spellcheck="false" :disabled="busy" /></label>
        <AppButton type="submit" :disabled="busy">{{ t(kind === 'tools' ? 'add.invoke' : 'add.read') }}</AppButton>
      </form>
      <section v-if="result" class="mcp-result"><strong>{{ t(result.isError ? 'add.toolError' : 'add.result') }}</strong><pre>{{ resultText }}</pre><p v-if="resultText.length > 18000">{{ t('add.attachmentLimit') }}</p><AppButton :disabled="busy || resultText.length > 18000" @click="attach">{{ t('add.attachResult') }}</AppButton></section>
    </div>
    <p v-if="busy" role="status" class="mcp-busy"><Spinner :size="16" :label="t('management.loading')" />{{ t('management.loading') }}</p>
  </AppDialog>
</template>
<style scoped>
.mcp-heading { display: flex; gap: 12px; justify-content: space-between; align-items: center; margin-block-end: 12px; }
.mcp-heading p { font-size: 12px; line-height: 1.6; color: var(--color-text-secondary); margin: 0; }
.mcp-heading .app-button { flex-shrink: 0; }
.mcp-servers { margin-block: 16px; }
.mcp-server { display: flex; flex-wrap: wrap; align-items: center; border-radius: 10px; padding: 4px; }
.mcp-server[data-selected='true'] { background: var(--color-bg-element); }
.mcp-server__name { display: flex; flex: 1; min-width: 0; gap: 8px; text-align: left; align-items: center; }
.mcp-server__name small { color: var(--color-text-tertiary); margin-inline-start: auto; }
.mcp-icon, .mcp-server__name, .mcp-tabs button, .mcp-entries button { padding: 8px; border: 0; border-radius: 8px; background: transparent; color: var(--color-text-primary); font: inherit; font-size: 13px; cursor: pointer; }
.mcp-icon:hover, .mcp-server__name:hover, .mcp-tabs button:hover, .mcp-entries button:hover, button[aria-pressed='true'] { background: var(--workspace-hover, var(--color-bg-element)); }
button:disabled { opacity: .45; cursor: default; }
.mcp-tabs { display: flex; gap: 6px; margin-block: 12px; }
.mcp-entries { display: flex; gap: 5px; flex-wrap: wrap; max-height: 160px; overflow: auto; }
.mcp-entries p, .mcp-servers > p { color: var(--color-text-tertiary); font-size: 12px; }
.mcp-catalog { border-top: 1px solid var(--color-border-base); padding-top: 12px; }
.mcp-catalog pre { max-height: 230px; overflow: auto; font-size: 11px; white-space: pre-wrap; overflow-wrap: anywhere; padding: 12px; background: var(--color-bg-base); border-radius: 8px; }
.mcp-catalog p { font-size: 12px; line-height: 1.6; }
.mcp-result { margin-block-start: 16px; }
.mcp-delete { display: flex; align-items: center; justify-content: space-between; width: 100%; gap: 10px; padding: 8px; font-size: 12px; }
.mcp-busy { display: flex; align-items: center; gap: 8px; font-size: 12px; color: var(--color-text-secondary); }
</style>
