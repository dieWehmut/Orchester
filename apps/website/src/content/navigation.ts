export interface SiteNavItem {
  readonly to: string
  readonly label: string
}

export const SITE_NAV_ITEMS: readonly SiteNavItem[] = Object.freeze([
  { to: '/', label: 'Overview' },
  { to: '/architecture', label: 'Architecture' },
  { to: '/install', label: 'Install' },
])
