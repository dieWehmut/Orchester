import type { CommandEntry } from './CommandPalette.vue'

/**
 * The composer's `/` vocabulary.
 *
 * Deliberately the same set the TUI answers to, with the same one-line
 * explanations: a reader who learned the commands in the terminal should not
 * find a different list in the workspace, and two lists would drift apart the
 * first time only one of them grew.
 */
export const COMPOSER_COMMANDS: readonly CommandEntry[] = [
  { id: 'agent', name: '/agent', description: 'choose a delegate' },
  { id: 'model', name: '/model', description: 'choose a model or provider' },
  { id: 'theme', name: '/theme', description: 'preview the workspace theme' },
  { id: 'config', name: '/config', description: 'inspect the resolved configuration' },
  { id: 'resume', name: '/resume', description: 'resume a recorded session' },
  { id: 'permissions', name: '/permissions', description: 'inspect the approval policy' },
  { id: 'status', name: '/status', description: 'inspect the runtime status' },
  { id: 'plugins', name: '/plugins', description: 'manage the installed plugins' },
  { id: 'help', name: '/help', description: 'show this list' },
]

