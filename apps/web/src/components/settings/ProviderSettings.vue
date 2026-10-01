<script setup lang="ts">
import { AppButton, AppInput, AppSelect, Spinner } from '@orchester/design'
import { computed, onMounted, ref } from 'vue'
import { useI18n } from '../../i18n'
import { useAppStores } from '../../stores/app'
import type { ModelProviderRequestDto } from '@orchester/protokoll'

const { t } = useI18n()
const { models } = useAppStores()
const editing = ref(false)
const saving = ref(false)
const saved = ref(false)
const saveAttempted = ref(false)
const name = ref('')
const endpoint = ref('')
const model = ref('')
const wireApi = ref<'responses' | 'anthropic'>('responses')
const apiKey = ref('')
let providerId = `provider-${crypto.randomUUID()}`

const options = [
  { value: 'responses', label: 'Responses API' },
  { value: 'anthropic', label: 'Anthropic Messages API' },
] as const
const providers = computed(() => models.catalog?.providers ?? [])
const pending = computed(() => ['idle', 'loading', 'refreshing'].includes(models.status))
const errorMessage = computed(() => {
  if (!models.error) return ''
  if (models.error.code === 'conflict') return t('settings.providers.conflict')
  if (models.error.code === 'validation_failed') return t('settings.providers.validationFailed')
  return t(saveAttempted.value ? 'settings.providers.saveFailed' : 'settings.providers.loadFailed')
})

onMounted(() => { if (models.status === 'idle') void models.load() })

function openForm(): void {
  saved.value = false
  editing.value = true
}

function refresh(): void {
  saveAttempted.value = false
  void models.load()
}

function cancel(): void {
  apiKey.value = ''
  editing.value = false
}

async function save(): Promise<void> {
  if (saving.value || !name.value.trim() || !endpoint.value.trim() || !model.value.trim() || !apiKey.value.trim()) return
  const request: ModelProviderRequestDto = {
    provider: providerId,
    name: name.value.trim(),
    base_url: endpoint.value.trim(),
    wire_api: wireApi.value,
    model: model.value.trim(),
    api_key: apiKey.value,
  }
  // Credentials stay out of browser persistence, and leave the input as soon
  // as they are handed to the loopback request.
  apiKey.value = ''
  saveAttempted.value = true
  saving.value = true
  try {
    if (await models.saveProvider(request)) {
      editing.value = false
      saved.value = true
      name.value = ''
      endpoint.value = ''
      model.value = ''
      providerId = `provider-${crypto.randomUUID()}`
    }
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <div class="provider-settings" data-provider-settings>
    <div class="provider-settings__toolbar">
      <AppButton size="sm" variant="secondary" :disabled="saving" @click="openForm">
        {{ t('settings.providers.add') }}
      </AppButton>
      <AppButton size="sm" variant="ghost" :disabled="pending || saving" @click="refresh">
        {{ t('settings.providers.refresh') }}
      </AppButton>
    </div>
    <p v-if="pending && !saving" class="provider-settings__note" role="status">
      <Spinner :size="14" :label="t('run.modelLoading')" /> {{ t('run.modelLoading') }}
    </p>
    <p v-if="errorMessage" class="provider-settings__error" role="alert">{{ errorMessage }}</p>
    <p v-if="saved" class="provider-settings__success" role="status">{{ t('settings.providers.saved') }}</p>
    <p v-if="models.status === 'ready' && providers.length === 0 && !editing" class="provider-settings__note" data-provider-empty>
      {{ t('settings.providers.empty') }}
    </p>
    <ul v-if="providers.length" class="provider-settings__list" :aria-label="t('settings.sections.providers')">
      <li v-for="provider in providers" :key="provider.id" class="provider-settings__row">
        <div class="provider-settings__identity">
          <strong>{{ provider.name }}</strong>
          <span>{{ provider.model ?? provider.reason ?? t('settings.providers.unavailable') }}</span>
        </div>
        <span v-if="provider.active" class="provider-settings__active">{{ t('settings.providers.active') }}</span>
        <AppButton v-else-if="provider.state === 'selectable'" size="sm" variant="ghost" :disabled="pending || saving"
          @click="models.select({ provider: provider.id })">{{ t('settings.providers.use') }}</AppButton>
      </li>
    </ul>
    <form v-if="editing" class="provider-settings__form" data-provider-form @submit.prevent="save">
      <h3>{{ t('settings.providers.add') }}</h3>
      <div class="provider-settings__fields">
        <label for="provider-name">{{ t('settings.providers.name') }}
          <AppInput id="provider-name" v-model="name" required :disabled="saving" :maxlength="256"
            :placeholder="t('settings.providers.namePlaceholder')" autocomplete="off" />
        </label>
        <label for="provider-endpoint">{{ t('settings.providers.endpoint') }}
          <AppInput id="provider-endpoint" v-model="endpoint" type="url" required :disabled="saving" :maxlength="256"
            :placeholder="t('settings.providers.endpointPlaceholder')" autocomplete="off" />
        </label>
        <label for="provider-protocol">{{ t('settings.providers.protocol') }}
          <AppSelect id="provider-protocol" v-model="wireApi" :options="options" :disabled="saving" />
        </label>
        <label for="provider-model">{{ t('settings.providers.model') }}
          <AppInput id="provider-model" v-model="model" required :disabled="saving" :maxlength="256"
            :placeholder="t('settings.providers.modelPlaceholder')" autocomplete="off" />
        </label>
        <label for="provider-key" class="provider-settings__key">{{ t('settings.providers.apiKey') }}
          <AppInput id="provider-key" v-model="apiKey" type="password" required :disabled="saving" :maxlength="4096"
            described-by="provider-key-note" autocomplete="off" />
        </label>
      </div>
      <p id="provider-key-note" class="provider-settings__note">{{ t('settings.providers.credentialNote') }}</p>
      <div class="provider-settings__actions">
        <AppButton type="submit" :disabled="saving">
          {{ saving ? t('settings.providers.saving') : t('settings.providers.save') }}
        </AppButton>
        <AppButton type="button" variant="ghost" :disabled="saving" @click="cancel">{{ t('settings.providers.cancel') }}</AppButton>
      </div>
    </form>
  </div>
</template>

<style scoped>
.provider-settings { display: grid; gap: var(--space-4); }
.provider-settings__toolbar, .provider-settings__actions { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-2); }
.provider-settings__note { display: flex; align-items: center; gap: var(--space-2); margin: 0; color: var(--color-text-secondary); font-size: var(--text-sm); }
.provider-settings__error { margin: 0; color: var(--color-status-error); }
.provider-settings__success { margin: 0; color: var(--color-status-success); }
.provider-settings__list { display: grid; gap: var(--space-2); margin: 0; padding: 0; list-style: none; }
.provider-settings__row { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-3); border: 1px solid var(--color-border-base); border-radius: var(--radius-lg); }
.provider-settings__identity { display: grid; min-inline-size: 0; flex: 1; gap: var(--space-1); overflow-wrap: anywhere; }
.provider-settings__identity span { color: var(--color-text-secondary); font-size: var(--text-sm); }
.provider-settings__active { color: var(--color-accent); font-size: var(--text-xs); white-space: nowrap; }
.provider-settings__form { display: grid; gap: var(--space-4); padding: var(--space-5); border: 1px solid var(--color-border-base); border-radius: var(--radius-lg); background: var(--color-bg-surface); }
.provider-settings__form h3 { margin: 0; font-size: var(--text-md); }
.provider-settings__fields { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--space-4); }
.provider-settings__fields label { display: grid; gap: var(--space-2); font-size: var(--text-sm); }
.provider-settings__key { grid-column: 1 / -1; }
@media (max-width: 799px) { .provider-settings__fields { grid-template-columns: minmax(0, 1fr); } .provider-settings__form { padding: var(--space-3); } }
</style>
