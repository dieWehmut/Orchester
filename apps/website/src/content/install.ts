export interface InstallStep {
  readonly index: string
  readonly title: string
  readonly description: string
  readonly command: string
  readonly note: string
}

export const installPrerequisites: readonly string[] = Object.freeze([
  'Node.js 22 or newer',
  'pnpm 10 or newer',
  'Rust stable with the platform linker configured',
])

export const installSteps: readonly InstallStep[] = Object.freeze([
  {
    index: '01',
    title: 'Clone the repository',
    description: 'Keep the runtime and the frontends in one checkout so workspace links stay deterministic.',
    command: 'git clone https://github.com/dieWehmut/Orchester.git\ncd Orchester',
    note: 'The project is local-first; no hosted account is required for development.',
  },
  {
    index: '02',
    title: 'Install frontend dependencies',
    description: 'Use the checked-in lockfile to install the WebUI, design system, and Pages packages.',
    command: 'pnpm --dir apps install --frozen-lockfile',
    note: 'The frozen install makes CI and local package resolution agree.',
  },
  {
    index: '03',
    title: 'Start a surface',
    description: 'Open the local WebUI first, then use the Pages demo or desktop shell when you need them.',
    command: 'pnpm --dir apps --filter @orchester/web dev',
    note: 'The default WebUI address is http://127.0.0.1:4173/.',
  },
])
