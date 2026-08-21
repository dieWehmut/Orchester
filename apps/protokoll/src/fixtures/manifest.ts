import type { StopReason } from '../event'
import type { UiEventEnvelope } from '../ui'
import { approvalPathFixture } from './approval'
import { failurePathFixture } from './failure'
import { happyPathFixture } from './happy'
import { reconnectPathFixture } from './reconnect'

export const FIXTURE_SCENARIO_IDS = [
  'happy',
  'approval',
  'failure',
  'reconnect',
] as const

export type FixtureScenarioId = (typeof FIXTURE_SCENARIO_IDS)[number]

export interface FixtureManifestEntry {
  id: FixtureScenarioId
  title_key: string
  summary_key: string
  event_count: number
  first_sequence: number
  last_sequence: number
  terminal_reason: StopReason
}

type FixtureFactory = () => UiEventEnvelope[]

const fixtureFactories: Record<FixtureScenarioId, FixtureFactory> = {
  happy: happyPathFixture,
  approval: approvalPathFixture,
  failure: failurePathFixture,
  reconnect: () => {
    const fixture = reconnectPathFixture()
    const lastReceived = fixture.before_disconnect.at(-1)?.sequence ?? 0
    return [
      ...fixture.before_disconnect,
      ...fixture.replay_response.events.filter((event) => event.sequence > lastReceived),
    ]
  },
}

const fixtureCopy: Record<
  FixtureScenarioId,
  Pick<FixtureManifestEntry, 'title_key' | 'summary_key'>
> = {
  happy: {
    title_key: 'fixtures.happy.title',
    summary_key: 'fixtures.happy.summary',
  },
  approval: {
    title_key: 'fixtures.approval.title',
    summary_key: 'fixtures.approval.summary',
  },
  failure: {
    title_key: 'fixtures.failure.title',
    summary_key: 'fixtures.failure.summary',
  },
  reconnect: {
    title_key: 'fixtures.reconnect.title',
    summary_key: 'fixtures.reconnect.summary',
  },
}

function terminalReason(events: readonly UiEventEnvelope[]): StopReason {
  for (let index = events.length - 1; index >= 0; index -= 1) {
    const event = events[index]
    if (event?.kind.type === 'run_stopped') return event.kind.reason
  }
  throw new Error('fixture scenario must contain a terminal run_stopped event')
}

function manifestEntry(id: FixtureScenarioId): FixtureManifestEntry {
  const events = fixtureScenarioEvents(id)
  const first = events[0]
  const last = events.at(-1)
  if (first === undefined || last === undefined) {
    throw new Error(`fixture scenario ${id} must not be empty`)
  }
  for (const [index, event] of events.entries()) {
    if (event.sequence !== index + 1) {
      throw new Error(`fixture scenario ${id} must have contiguous sequences`)
    }
  }
  return {
    id,
    ...fixtureCopy[id],
    event_count: events.length,
    first_sequence: first.sequence,
    last_sequence: last.sequence,
    terminal_reason: terminalReason(events),
  }
}

/** Return a new deterministic event graph so consumers cannot share mutation. */
export function fixtureScenarioEvents(id: FixtureScenarioId): UiEventEnvelope[] {
  return fixtureFactories[id]()
}

/** JSON-safe scenario metadata shared by the website, web tests, and E2E. */
export const FIXTURE_MANIFEST: readonly FixtureManifestEntry[] =
  FIXTURE_SCENARIO_IDS.map(manifestEntry)
