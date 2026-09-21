<script setup lang="ts">
/**
 * The shell's two live regions.
 *
 * Section 7 splits announcements by politeness, and a single region can only
 * carry one: failures go to the assertive region so they interrupt, everything
 * else to the polite one so it waits for the reader to finish their sentence.
 * Both regions exist from the moment the panel mounts, empty, because a live
 * region that appears along with its first message is a region assistive
 * technology has no chance to start watching.
 *
 * The cursor starts at the head of the journal the panel mounted with: a
 * retained window is history the reader has already been shown, so replaying it
 * here would narrate the transcript instead of the news.
 */
import { VisuallyHidden } from '@orchester/design'
import type { UiEventEnvelope } from '@orchester/protokoll'
import { ref, watch } from 'vue'

import { useI18n } from '../../i18n'
import { advanceAnnouncements, cursorAtHead, type AnnouncementCursor } from './announcements'

const props = defineProps<{
  events: readonly UiEventEnvelope[]
}>()

const { t } = useI18n()

const polite = ref('')
const assertive = ref('')

let cursor: AnnouncementCursor = cursorAtHead(props.events)

watch(
  () => props.events,
  (events) => {
    const next = advanceAnnouncements(cursor, events)
    cursor = next.cursor
    for (const announcement of next.announcements) {
      const text = t(announcement.key, announcement.params)
      if (announcement.politeness === 'assertive') assertive.value = text
      else polite.value = text
    }
  },
)
</script>

<template>
  <VisuallyHidden>
    <span data-run-announcer="polite" role="status" aria-live="polite">{{ polite }}</span>
    <span data-run-announcer="assertive" role="alert" aria-live="assertive">{{ assertive }}</span>
  </VisuallyHidden>
</template>
