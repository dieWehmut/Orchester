import type { MessageKey } from '../../i18n'

export interface ComposerCommand {
  readonly id: string
  readonly name: string
  readonly descriptionKey: MessageKey
}

/**
 * The composer's `/` vocabulary.
 *
 * Deliberately the same set the TUI answers to: a reader who learned the
 * commands in the terminal should not find a different list in the workspace,
 * and two lists would drift apart the first time only one of them grew. The
 * one-line explanation is a locale key here rather than a sentence, so the
 * vocabulary stays one list in three languages.
 */
export const COMPOSER_COMMANDS: readonly ComposerCommand[] = [
  { id: 'agent', name: '/agent', descriptionKey: 'commands.agent' },
  { id: 'model', name: '/model', descriptionKey: 'commands.model' },
  { id: 'theme', name: '/theme', descriptionKey: 'commands.theme' },
  { id: 'config', name: '/config', descriptionKey: 'commands.config' },
  { id: 'resume', name: '/resume', descriptionKey: 'commands.resume' },
  { id: 'permissions', name: '/permissions', descriptionKey: 'commands.permissions' },
  { id: 'status', name: '/status', descriptionKey: 'commands.status' },
  { id: 'plugins', name: '/plugins', descriptionKey: 'commands.plugins' },
  { id: 'help', name: '/help', descriptionKey: 'commands.help' },
]

