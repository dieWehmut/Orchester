<script setup lang="ts">
import InspectorDock from '../components/layout/InspectorDock.vue'
import WorkspaceShell from '../components/layout/WorkspaceShell.vue'
import SessionRail from '../components/sessions/SessionRail.vue'
import SessionTranscript from '../components/sessions/SessionTranscript.vue'
import { useAppStores } from '../stores/app'

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
  <WorkspaceShell data-testid="workspace-view">
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
  </WorkspaceShell>
</template>
