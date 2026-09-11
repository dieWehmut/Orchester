export interface GiscusConfig {
  readonly repo: string
  readonly repoId: string
  readonly category: string
  readonly categoryId: string
  readonly strict: '0' | '1'
  readonly reactionsEnabled: '0' | '1'
  readonly inputPosition: 'top' | 'bottom'
  readonly lang: string
  readonly loading: 'lazy' | 'eager'
}

export type GiscusEnvironment = Readonly<Record<string, string | undefined>>

function readRequired(environment: GiscusEnvironment, key: string): string | null {
  const value = environment[key]?.trim()
  return value ? value : null
}

function readBinary(
  environment: GiscusEnvironment,
  key: string,
  fallback: '0' | '1',
): '0' | '1' {
  return environment[key]?.trim() === '0' ? '0' : environment[key]?.trim() === '1' ? '1' : fallback
}

export function parseGiscusConfig(environment: GiscusEnvironment): GiscusConfig | null {
  const repo = readRequired(environment, 'VITE_GISCUS_REPO')
  const repoId = readRequired(environment, 'VITE_GISCUS_REPO_ID')
  const category = readRequired(environment, 'VITE_GISCUS_CATEGORY')
  const categoryId = readRequired(environment, 'VITE_GISCUS_CATEGORY_ID')
  if (!repo || !repoId || !category || !categoryId) return null

  const inputPosition = environment.VITE_GISCUS_INPUT_POSITION?.trim()
  const lang = readRequired(environment, 'VITE_GISCUS_LANG') ?? 'en'
  return {
    repo,
    repoId,
    category,
    categoryId,
    strict: readBinary(environment, 'VITE_GISCUS_STRICT', '1'),
    reactionsEnabled: readBinary(environment, 'VITE_GISCUS_REACTIONS_ENABLED', '1'),
    inputPosition: inputPosition === 'bottom' ? 'bottom' : 'top',
    lang,
    loading: 'lazy',
  }
}

export function giscusTermForRoute(path: string): string {
  const withoutQuery = path.split(/[?#]/, 1)[0] ?? '/'
  const normalized = withoutQuery.replace(/^\/+|\/+$/g, '')
  return `orchester:${normalized || 'home'}`
}
