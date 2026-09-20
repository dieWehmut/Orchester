import { mount } from '@vue/test-utils'
import type { ApprovalView } from '@orchester/ereignis'
import { describe, expect, it } from 'vitest'

import ApprovalsQueue from '../src/components/changes/ApprovalsQueue.vue'

/**
 * The Approvals queue, section 4.7 of the design spec.
 *
 * Each card carries the risk summary, the redacted action and the exact scope,
 * with the three scoped choices the spec names. A stale entry - one whose
 * row version no longer matches what the runtime holds - renders as a distinct
 * "superseded" state rather than as an error, because nothing failed: the
 * decision simply belongs to a row that has moved on.
 */

function approval(overrides: Partial<ApprovalView> = {}): ApprovalView {
  return {
    key: 'approval:approval-1',
    approvalId: 'approval-1' as ApprovalView['approvalId'],
    runId: 'run-1' as ApprovalView['runId'],
    rowVersion: 3,
    risk: 'high',
    action: 'write_file path=src/main.rs',
    reason: 'workspace write',
    expiresAt: null,
    state: 'pending',
    requestedSequence: 12,
    resolvedSequence: null,
    ...overrides,
  }
}

describe('ApprovalsQueue', () => {
  it('renders a card per pending approval with its risk and action', () => {
    const wrapper = mount(ApprovalsQueue, { props: { approvals: [approval()] } })

    const card = wrapper.get('[data-approval-id="approval-1"]')
    expect(card.text()).toContain('high')
    expect(card.text()).toContain('write_file path=src/main.rs')
    expect(card.attributes('data-approval-state')).toBe('pending')
  })

  it('offers the three scoped choices the spec names', () => {
    const wrapper = mount(ApprovalsQueue, { props: { approvals: [approval()] } })

    const choices = wrapper.findAll('[data-approval-choice]')
    expect(choices).toHaveLength(3)
    expect(choices.map((choice) => choice.text())).toEqual([
      'Allow once',
      'Allow for run',
      'Deny',
    ])
  })

  it('emits the decision with the row version it was made against', async () => {
    const wrapper = mount(ApprovalsQueue, { props: { approvals: [approval()] } })

    await wrapper.get('[data-approval-choice="allow-once"]').trigger('click')

    expect(wrapper.emitted('decide')).toEqual([
      [{ approvalId: 'approval-1', rowVersion: 3, decision: 'approved' }],
    ])
  })

  it('renders a superseded entry as its own state, not as an error', () => {
    const wrapper = mount(ApprovalsQueue, {
      props: { approvals: [approval({ state: 'stale' })] },
    })

    const card = wrapper.get('[data-approval-id="approval-1"]')
    expect(card.attributes('data-approval-state')).toBe('stale')
    expect(card.get('[data-approval-superseded]').text()).toBeTruthy()
    // Superseded is news about the row, not a failure, so it is not an alert.
    expect(card.find('[role="alert"]').exists()).toBe(false)
  })

  it('does not offer choices for an entry that is already decided', () => {
    const wrapper = mount(ApprovalsQueue, {
      props: { approvals: [approval({ state: 'approved', resolvedSequence: 14 })] },
    })

    expect(wrapper.findAll('[data-approval-choice]')).toHaveLength(0)
  })

  it('says there is nothing to decide rather than showing a bare list', () => {
    const wrapper = mount(ApprovalsQueue, { props: { approvals: [] } })

    expect(wrapper.get('[data-approvals-empty]').text()).toBeTruthy()
  })
})
