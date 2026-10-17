import type { ModelCatalogDto } from '@orchester/protokoll'

export interface ModelMenuChoice {
  id: string
  label: string
  description?: string | undefined
  checked: boolean
  disabled?: boolean
}

export interface ModelMenuGroup {
  id: string
  label: string
  choices: ModelMenuChoice[]
}

export const REASONING_EFFORTS = ['minimal', 'low', 'medium', 'high', 'xhigh', 'max', 'ultra'] as const

export function effortName(value: string): string {
  return REASONING_EFFORTS.some(name => name === value)
    ? value.charAt(0).toUpperCase() + value.slice(1)
    : value
}

/** Group by provider identity, never by its display name or a profile name. */
export function modelMenuGroups(catalog: ModelCatalogDto | null): ModelMenuGroup[] {
  if (!catalog) return []
  const active = catalog.active.state === 'configured' ? catalog.active.choice : null
  const groups = new Map<string, ModelMenuGroup>()
  const group = (id: string, label: string) => {
    if (!groups.has(id)) groups.set(id, { id, label, choices: [] })
    return groups.get(id)!
  }
  for (const provider of catalog.providers) {
    group(provider.id, provider.name).choices.push({
      id: `provider:${provider.id}`,
      label: provider.model || provider.name,
      description: provider.state === 'unavailable' ? provider.reason || provider.field || '' : undefined,
      disabled: provider.state !== 'selectable',
      checked: catalog.selected_provider
        ? catalog.selected_provider === provider.id
        : !active?.profile && active?.provider === provider.id,
    })
  }
  for (const profile of catalog.profiles) {
    group(profile.provider, profile.provider_name).choices.push({
      id: `profile:${profile.profile}`,
      label: profile.model,
      description: profile.profile !== profile.model ? profile.profile : undefined,
      checked: !catalog.selected_provider && active?.profile === profile.profile,
    })
  }
  if (active && !Array.from(groups.values()).some(item => item.choices.some(choice => choice.checked))) {
    group(active.provider, active.provider_name).choices.unshift({
      id: active.profile ? `profile:${active.profile}` : 'configured:',
      label: active.model,
      description: active.profile || undefined,
      checked: true,
    })
  }
  return Array.from(groups.values())
}
