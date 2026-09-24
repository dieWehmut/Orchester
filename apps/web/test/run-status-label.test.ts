import { describe, expect, it } from 'vitest'

import {
  RUN_STATUS_MESSAGE_KEYS,
  RUN_STATUS_MODEL_KEYS,
  RUN_STATUS_VALUES,
  runStatusMessageKey,
} from '../src/components/run/run-status-label'
import { createI18n, supportedLocales } from '../src/i18n'

/**
 * What a run's state is called.
 *
 * The model names the keys and leaves localization to this application, so the
 * two maps have to agree and every key has to exist in every catalogue - a state
 * whose label is missing would render as its own key, which is how a reader
 * learns the product's internal vocabulary instead of the state.
 */
describe('run status labels', () => {
  it('agrees with the model on every state', () => {
    for (const status of RUN_STATUS_VALUES) {
      expect(RUN_STATUS_MESSAGE_KEYS[status], status).toBe(RUN_STATUS_MODEL_KEYS[status])
      expect(runStatusMessageKey(status), status).toBe(RUN_STATUS_MESSAGE_KEYS[status])
    }
  })

  it('has a label in every catalogue for every state', () => {
    const missing: string[] = []
    for (const locale of supportedLocales) {
      // Resolved through the translation the app itself reads, so a catalogue
      // that fell behind is caught here rather than only in the browser.
      const i18n = createI18n(locale)
      for (const status of RUN_STATUS_VALUES) {
        const key = RUN_STATUS_MESSAGE_KEYS[status]
        // A missing key resolves to the key itself, which is how a reader ends
        // up learning the product's internal vocabulary instead of the state.
        if (i18n.t(key) === key) missing.push(`${locale}:${key}`)
      }
    }
    expect(missing).toEqual([])
  })
})