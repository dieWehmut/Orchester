<script setup lang="ts">
import InspectorDock from '../components/layout/InspectorDock.vue'
import WorkspaceResponsive from '../components/layout/WorkspaceResponsive.vue'
import SessionRail from '../components/sessions/SessionRail.vue'
import SessionTranscript from '../components/sessions/SessionTranscript.vue'
import RunPanel from '../components/run/RunPanel.vue'
import { useI18n } from '../i18n'
import { useAppStores } from '../stores/app'
import { computed } from 'vue'

const { t } = useI18n()
const { sessions, run } = useAppStores()
const runView = computed(() => run.view.value)
const runConnectionStatus = computed(() => run.connectionStatus.value)
const runProjectionStatus = computed(() => run.projectionStatus.value)
const runErrorMessage = computed(() => run.error.value?.message ?? null)
const {
  status,
  detailStatus,
  items,
  nextCursor,
  selectedId,
  selected,
  error,
  detailError,
} = sessions

function handleRunSubmit(): void {
  run.setConnectionStatus('error')
  run.setError(new Error('Run service is not connected yet'))
}

function handleRunCancel(): void {
  run.reset()
}
</script>

<template>
  <WorkspaceResponsive
    data-testid="workspace-view"
    :sessions-title="t('sessions.title')"
    :inspector-title="t('inspector.label')"
    :controls-label="t('inspector.label')"
  >
    <template #sessions>
      <SessionRail
        :status="status"
        :items="items"
        :selected-id="selectedId"
        :next-cursor="nextCursor"
        :error="error"
        @select="sessions.select"
        @refresh="sessions.load"
        @load-more="sessions.loadMore"
        @new-session="sessions.select(null)"
      />
    </template>

    <RunPanel
      v-if="!selected"
      :view="runView"
      :connection-status="runConnectionStatus"
      :projection-status="runProjectionStatus"
      :error-message="runErrorMessage"
      @submit="handleRunSubmit"
      @cancel="handleRunCancel"
    />
    <SessionTranscript v-else :status="detailStatus" :session="selected" :error="detailError" />

    <template #inspector>
      <InspectorDock />
    </template>
  </WorkspaceResponsive>
</template>
