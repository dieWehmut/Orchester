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
