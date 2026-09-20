import { clearStored, readStored, writeStored } from '@orchester/design/storage'

import type { ApprovalPreset } from './ApprovalPresetControl.vue'

/**
 * Run-scoped composer settings, task U4-03 of the implementation plan.
 *
 * Section 4.6 puts the model, the effort and the approval preset on the task
 * rather than on the user: a resumed run keeps the settings it was started
 * with, and a new task does not inherit the last one's full access. The
 * settings are therefore keyed by task, which is also what makes "changing
 * them affects the next run" true rather than aspirational.
 */

export interface RunSettings {
  model: string | null
  effort: string | null
  approvalPreset: ApprovalPreset
}

export const DEFAULT_RUN_SETTINGS: RunSettings = {
  model: null,
  effort: null,
  approvalPreset: 'ask',
}

const PRESETS: readonly ApprovalPreset[] = ['ask', 'governed', 'full-access']

function storageKey(taskId: string): string {
  return `orchester:run-settings:${taskId}`
}

function isPreset(value: unknown): value is ApprovalPreset {
  return typeof value === 'string' && (PRESETS as readonly string[]).includes(value)
}

/**
 * The settings the given task was last run with, or the defaults. A payload
 * this module did not write opens the defaults rather than a half-read object,
 * because a composer showing an approval preset that does not exist is worse
 * than one showing the safe default.
 */
export function readRunSettings(taskId: string): RunSettings {
  const stored = readStored(storageKey(taskId))
  if (stored === null) return DEFAULT_RUN_SETTINGS

  let parsed: unknown
  try {
    parsed = JSON.parse(stored)
  } catch {
    return DEFAULT_RUN_SETTINGS
  }
  if (typeof parsed !== 'object' || parsed === null) return DEFAULT_RUN_SETTINGS

  const record = parsed as Record<string, unknown>
  return {
    model: typeof record.model === 'string' ? record.model : null,
    effort: typeof record.effort === 'string' ? record.effort : null,
    approvalPreset: isPreset(record.approvalPreset)
      ? record.approvalPreset
      : DEFAULT_RUN_SETTINGS.approvalPreset,
  }
}

/** Remember what the task is being run with; `null` forgets it. */
export function writeRunSettings(taskId: string, settings: RunSettings | null): void {
  const key = storageKey(taskId)
  if (settings === null) {
    clearStored(key)
    return
  }
  writeStored(key, JSON.stringify(settings))
}
