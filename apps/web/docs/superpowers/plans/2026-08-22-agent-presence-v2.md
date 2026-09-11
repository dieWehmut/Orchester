# Agent Presence v2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the existing agent fleet UI into a bounded Agent Presence feature with provider identity, grouped runtime states, and accessible window/run metrics while preserving the empty-conversation mark contract.

**Architecture:** Keep the existing runtime DTOs, Pinia store, socket transport, workspace layout, and inspector behavior unchanged. Move only the presentational components and mappings behind `src/features/agent-presence`, then add pure provider/group presenters and small Vue components that the existing sidebar and inspector consume.

**Tech Stack:** Vue 3 SFCs, TypeScript, Pinia-provided DTO state, `@orchester/design`, Lucide Vue icons, Vitest, Vue Test Utils.

---

### Task 1: Establish the Agent Presence feature boundary

**Files:**
- Create: `test/agent-presence-feature.test.ts`
- Create: `src/features/agent-presence/index.ts`
- Move: `src/components/agents/AgentDetails.vue` to `src/features/agent-presence/components/AgentDetails.vue`
- Move: `src/components/agents/AgentFleetPanel.vue` to `src/features/agent-presence/components/AgentFleetPanel.vue`
- Move: `src/components/agents/AgentFleetRow.vue` to `src/features/agent-presence/components/AgentFleetRow.vue`
- Move: `src/components/agents/AgentIcon.vue` to `src/features/agent-presence/components/AgentIcon.vue`
- Move: `src/components/agents/agent-presenter.ts` to `src/features/agent-presence/agent-presenter.ts`
- Modify: `src/components/layout/WorkspaceSidebar.vue`
- Modify: `src/views/WorkspaceView.vue`
- Modify: `test/agent-details.test.ts`
- Modify: `test/agent-fleet-panel.test.ts`
- Modify: `test/agent-fleet-row.test.ts`
- Modify: `test/agent-presenter.test.ts`

- [ ] **Step 1: Write the failing public-boundary test**

```ts
import { describe, expect, it } from 'vitest'

import {
  AgentDetails,
  AgentFleetPanel,
  AgentFleetRow,
  AgentIcon,
  agentActivityMessageKey,
} from '../src/features/agent-presence'

describe('agent presence feature boundary', () => {
  it('exports the components and presentation helpers used by the workspace', () => {
    expect(AgentDetails).toBeTruthy()
    expect(AgentFleetPanel).toBeTruthy()
    expect(AgentFleetRow).toBeTruthy()
    expect(AgentIcon).toBeTruthy()
    expect(agentActivityMessageKey).toBeTypeOf('function')
  })
})
```

- [ ] **Step 2: Run the test and confirm the missing feature module failure**

Run: `pnpm --dir apps --filter @orchester/web test -- agent-presence-feature.test.ts`

Expected: FAIL because `src/features/agent-presence` does not exist.

- [ ] **Step 3: Move the existing components and expose the feature entry point**

```ts
export { default as AgentDetails } from './components/AgentDetails.vue'
export { default as AgentFleetPanel } from './components/AgentFleetPanel.vue'
export { default as AgentFleetRow } from './components/AgentFleetRow.vue'
export { default as AgentIcon } from './components/AgentIcon.vue'
export * from './agent-presenter'
```

Update workspace imports to `../features/agent-presence` or `../../features/agent-presence`, and update moved component-relative imports to `../../../i18n`, `../../../stores/agent-fleet`, and `../../../transport/agent-status-socket` as appropriate.

- [ ] **Step 4: Run the feature and existing agent tests**

Run: `pnpm --dir apps --filter @orchester/web test -- agent-presence-feature.test.ts agent-presenter.test.ts agent-fleet-row.test.ts agent-fleet-panel.test.ts agent-details.test.ts workspace-view.test.ts`

Expected: PASS with no failed tests.

- [ ] **Step 5: Commit the boundary**

```powershell
git add apps/web/src apps/web/test
git commit -m "refactor(web): bound agent presence feature"
```

### Task 2: Map provider identity, icon, and color

**Files:**
- Create: `src/features/agent-presence/provider-presentation.ts`
- Create: `test/agent-provider-presentation.test.ts`
- Modify: `src/features/agent-presence/index.ts`
- Modify: `src/features/agent-presence/components/AgentIcon.vue`
- Modify: `src/features/agent-presence/components/AgentFleetRow.vue`
- Modify: `src/features/agent-presence/components/AgentDetails.vue`
- Modify: `test/agent-fleet-row.test.ts`
- Modify: `test/agent-details.test.ts`

- [ ] **Step 1: Write failing provider-presentation tests**

```ts
import { AGENT_FLEET_FIXTURE, type AgentRuntimeSummaryDto } from '@orchester/protokoll'
import { describe, expect, it } from 'vitest'

import { agentProviderPresentation } from '../src/features/agent-presence'

describe('agent provider presentation', () => {
  it('maps known runtimes to stable provider identities', () => {
    expect(agentProviderPresentation(AGENT_FLEET_FIXTURE.agents[0]!)).toEqual({
      key: 'codex',
      label: 'OpenAI',
      iconKey: 'codex',
    })
    expect(agentProviderPresentation(AGENT_FLEET_FIXTURE.agents[1]!)).toEqual({
      key: 'claude',
      label: 'Anthropic',
      iconKey: 'claude',
    })
  })

  it('humanizes unknown providers and uses the generic identity', () => {
    const custom: AgentRuntimeSummaryDto = {
      ...AGENT_FLEET_FIXTURE.agents[0]!,
      provider: 'local-bridge',
      icon_key: 'local-bridge',
    }
    expect(agentProviderPresentation(custom)).toEqual({
      key: 'generic',
      label: 'Local bridge',
      iconKey: 'generic',
    })
  })
})
```

- [ ] **Step 2: Run the provider test and confirm the missing export failure**

Run: `pnpm --dir apps --filter @orchester/web test -- agent-provider-presentation.test.ts`

Expected: FAIL because `agentProviderPresentation` is not exported.

- [ ] **Step 3: Implement the pure provider mapping**

```ts
import type { AgentRuntimeSummaryDto } from '@orchester/protokoll'

export type AgentProviderKey = 'codex' | 'claude' | 'deepseek' | 'opencode' | 'generic'

export interface AgentProviderPresentation {
  readonly key: AgentProviderKey
  readonly label: string
  readonly iconKey: AgentProviderKey
}

const PROVIDERS: Record<Exclude<AgentProviderKey, 'generic'>, AgentProviderPresentation> = {
  codex: { key: 'codex', label: 'OpenAI', iconKey: 'codex' },
  claude: { key: 'claude', label: 'Anthropic', iconKey: 'claude' },
  deepseek: { key: 'deepseek', label: 'DeepSeek', iconKey: 'deepseek' },
  opencode: { key: 'opencode', label: 'OpenCode', iconKey: 'opencode' },
}

function knownProvider(agent: AgentRuntimeSummaryDto): Exclude<AgentProviderKey, 'generic'> | null {
  const candidates = [agent.provider, agent.icon_key].map((value) => value.trim().toLowerCase())
  if (candidates.some((value) => value === 'codex' || value === 'openai')) return 'codex'
  if (candidates.some((value) => value === 'claude' || value === 'anthropic')) return 'claude'
  if (candidates.includes('deepseek')) return 'deepseek'
  if (candidates.includes('opencode')) return 'opencode'
  return null
}

function humanizeProvider(provider: string): string {
  const text = provider.trim().replace(/[_-]+/g, ' ')
  return text.length === 0 ? 'Custom provider' : text.charAt(0).toUpperCase() + text.slice(1)
}

export function agentProviderPresentation(agent: AgentRuntimeSummaryDto): AgentProviderPresentation {
  const key = knownProvider(agent)
  return key ? PROVIDERS[key] : { key: 'generic', label: humanizeProvider(agent.provider), iconKey: 'generic' }
}
```

Use `data-agent-provider` to select subdued semantic colors in `AgentIcon.vue`: Codex uses success, Claude warning, DeepSeek info, OpenCode accent, and generic neutral. Keep provider text visible so color is never the only identifier.

- [ ] **Step 4: Run provider and component tests**

Run: `pnpm --dir apps --filter @orchester/web test -- agent-provider-presentation.test.ts agent-fleet-row.test.ts agent-details.test.ts`

Expected: PASS with known provider labels, provider data attributes, and generic fallback coverage.

- [ ] **Step 5: Commit provider presentation**

```powershell
git add apps/web/src/features/agent-presence apps/web/test
git commit -m "feat(web): map agent provider presence"
```

### Task 3: Group the fleet by presence state

**Files:**
- Create: `src/features/agent-presence/fleet-groups.ts`
- Create: `src/features/agent-presence/components/AgentFleetGroup.vue`
- Create: `test/agent-fleet-groups.test.ts`
- Modify: `src/features/agent-presence/index.ts`
- Modify: `src/features/agent-presence/components/AgentFleetPanel.vue`
- Modify: `src/locales/en.json`
- Modify: `src/locales/zh-CN.json`
- Modify: `src/locales/zh-TW.json`
- Modify: `test/agent-fleet-panel.test.ts`

- [ ] **Step 1: Write failing grouping tests**

```ts
import { AGENT_FLEET_FIXTURE } from '@orchester/protokoll'
import { describe, expect, it } from 'vitest'

import { groupAgentFleet } from '../src/features/agent-presence'

describe('agent fleet groups', () => {
  it('orders active agents before attention states and aggregates windows', () => {
    const groups = groupAgentFleet(AGENT_FLEET_FIXTURE.agents)
    expect(groups.map((group) => group.key)).toEqual(['active', 'attention'])
    expect(groups[0]?.agents.map((agent) => agent.agent_id)).toEqual([
      'codex-main',
      'claude-default',
      'deepseek-research',
    ])
    expect(groups[0]?.activeWindows).toBe(4)
    expect(groups[1]?.agents).toHaveLength(2)
  })
})
```

- [ ] **Step 2: Run the grouping test and confirm the missing helper failure**

Run: `pnpm --dir apps --filter @orchester/web test -- agent-fleet-groups.test.ts`

Expected: FAIL because `groupAgentFleet` is not exported.

- [ ] **Step 3: Implement deterministic grouping**

```ts
import type { AgentRuntimeSummaryDto } from '@orchester/protokoll'

export type AgentPresenceGroupKey = 'active' | 'ready' | 'attention'

export interface AgentFleetGroup {
  readonly key: AgentPresenceGroupKey
  readonly agents: readonly AgentRuntimeSummaryDto[]
  readonly activeWindows: number
}

const GROUP_ORDER: readonly AgentPresenceGroupKey[] = ['active', 'ready', 'attention']

export function agentPresenceGroupKey(agent: AgentRuntimeSummaryDto): AgentPresenceGroupKey {
  if (agent.availability !== 'available' || agent.activity === 'offline' || agent.activity === 'error') {
    return 'attention'
  }
  if (agent.activity === 'idle') return 'ready'
  return 'active'
}

export function groupAgentFleet(agents: readonly AgentRuntimeSummaryDto[]): AgentFleetGroup[] {
  return GROUP_ORDER.flatMap((key) => {
    const grouped = agents.filter((agent) => agentPresenceGroupKey(agent) === key)
    return grouped.length === 0
      ? []
      : [{ key, agents: grouped, activeWindows: grouped.reduce((sum, agent) => sum + agent.active_windows, 0) }]
  })
}
```

`AgentFleetGroup.vue` renders a localized heading, agent count, aggregate active-window count, and one `AgentFleetRow` per agent. `AgentFleetPanel.vue` renders `groupAgentFleet(snapshot.agents)` instead of one flat list.

- [ ] **Step 4: Run grouping and panel tests**

Run: `pnpm --dir apps --filter @orchester/web test -- agent-fleet-groups.test.ts agent-fleet-panel.test.ts workspace-view.test.ts`

Expected: PASS with `active`, `ready`, and `attention` groups emitted only when non-empty.

- [ ] **Step 5: Commit fleet grouping**

```powershell
git add apps/web/src/features/agent-presence apps/web/src/locales apps/web/test
git commit -m "feat(web): group agent fleet presence"
```

### Task 4: Extract accessible runtime metrics

**Files:**
- Create: `src/features/agent-presence/components/AgentMetrics.vue`
- Create: `test/agent-metrics.test.ts`
- Modify: `src/features/agent-presence/agent-presenter.ts`
- Modify: `src/features/agent-presence/index.ts`
- Modify: `src/features/agent-presence/components/AgentFleetRow.vue`
- Modify: `src/features/agent-presence/components/AgentDetails.vue`
- Modify: `src/locales/en.json`
- Modify: `src/locales/zh-CN.json`
- Modify: `src/locales/zh-TW.json`
- Modify: `test/agent-presenter.test.ts`
- Modify: `test/agent-fleet-row.test.ts`
- Modify: `test/agent-details.test.ts`

- [ ] **Step 1: Write failing accessible-metric tests**

```ts
import { AGENT_FLEET_FIXTURE } from '@orchester/protokoll'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import { AgentMetrics } from '../src/features/agent-presence'

describe('AgentMetrics', () => {
  it('announces counts with singular and plural labels plus the window source', () => {
    const wrapper = mount(AgentMetrics, {
      props: { agent: AGENT_FLEET_FIXTURE.agents[0]!, variant: 'detail' },
    })
    expect(wrapper.get('[data-agent-metrics]').attributes('aria-label')).toContain('2 windows')
    expect(wrapper.get('[data-agent-metrics]').attributes('aria-label')).toContain('1 subagent')
    expect(wrapper.get('[data-agent-window-source]').text()).toContain('Managed sessions')
  })
})
```

- [ ] **Step 2: Run the metric test and confirm the missing component failure**

Run: `pnpm --dir apps --filter @orchester/web test -- agent-metrics.test.ts`

Expected: FAIL because `AgentMetrics` is not exported.

- [ ] **Step 3: Add singular count and window-source mappings**

```ts
export type AgentCountMessageKey =
  | 'agents.counts.window'
  | 'agents.counts.windows'
  | 'agents.counts.run'
  | 'agents.counts.runs'
  | 'agents.counts.subagent'
  | 'agents.counts.subagents'

export function agentCountMessageKey(key: AgentCountKey, count = 2): AgentCountMessageKey {
  if (count === 1) return `agents.counts.${key === 'windows' ? 'window' : key === 'runs' ? 'run' : 'subagent'}`
  return `agents.counts.${key}`
}

export function agentWindowSourceMessageKey(source: AgentRuntimeSummaryDto['window_count_source']):
  | 'agents.windowSource.managedSessions'
  | 'agents.windowSource.desktopWindows' {
  return source === 'tauri_windows'
    ? 'agents.windowSource.desktopWindows'
    : 'agents.windowSource.managedSessions'
}
```

`AgentMetrics.vue` computes the localized summary from `activeAgentCounts`, uses semantic `dl/dt/dd` markup, preserves the existing `data-active-*` hooks, and shows the source only in `detail` mode. `AgentFleetRow.vue` includes the provider, activity, and metric summary in its button label; `AgentDetails.vue` reuses the detail variant.

- [ ] **Step 4: Run all Agent Presence and workspace contract tests**

Run: `pnpm --dir apps --filter @orchester/web test -- agent-metrics.test.ts agent-presenter.test.ts agent-fleet-row.test.ts agent-details.test.ts agent-fleet-panel.test.ts workspace-view.test.ts run-panel.test.ts empty-workspace.test.ts visual-policy.test.ts`

Expected: PASS, including the existing checks that the centered Orchester mark disappears only after `conversationStarted` becomes true.

- [ ] **Step 5: Commit accessible metrics**

```powershell
git add apps/web/src/features/agent-presence apps/web/src/locales apps/web/test
git commit -m "feat(web): announce agent runtime metrics"
```

### Task 5: Verify and publish the feature branch

**Files:**
- Verify all modified files under `apps/web`

- [ ] **Step 1: Run the complete WebUI test suite**

Run: `pnpm --dir apps --filter @orchester/web test`

Expected: all WebUI test files and tests pass.

- [ ] **Step 2: Run WebUI type checking and production build**

Run: `pnpm --dir apps --filter @orchester/web typecheck`

Expected: exit code 0.

Run: `pnpm --dir apps --filter @orchester/web build`

Expected: exit code 0 and a Vite production bundle.

- [ ] **Step 3: Check diff hygiene and scope**

Run: `git diff --check origin/main...HEAD`

Expected: no output and exit code 0.

Run: `git diff --name-only origin/main...HEAD`

Expected: every path starts with `apps/web/`.

- [ ] **Step 4: Request code review and address all critical or important findings**

Review range: `origin/main...HEAD`.

- [ ] **Step 5: Push without merging main**

```powershell
git push -u origin feat/web-agent-presence-v2
```

Expected: remote branch `origin/feat/web-agent-presence-v2` is created or updated.
