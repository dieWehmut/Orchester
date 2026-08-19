<script setup lang="ts">
import InspectorDock from '../components/layout/InspectorDock.vue'
import WorkspaceResponsive from '../components/layout/WorkspaceResponsive.vue'
import SessionRail from '../components/sessions/SessionRail.vue'
import SessionTranscript from '../components/sessions/SessionTranscript.vue'
import { useI18n } from '../i18n'
import { useAppStores } from '../stores/app'

const { t } = useI18n()
const { sessions } = useAppStores()
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

    <SessionTranscript :status="detailStatus" :session="selected" :error="detailError" />

    <template #inspector>
      <InspectorDock />
    </template>
  </WorkspaceResponsive>
</template>
