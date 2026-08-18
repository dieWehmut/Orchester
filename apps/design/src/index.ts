export { default as AppBadge } from './components/AppBadge.vue'
export { default as AppButton } from './components/AppButton.vue'
export { default as AppCard } from './components/AppCard.vue'
export { default as ColorSchemePicker } from './components/ColorSchemePicker.vue'
export { default as IconButton } from './components/IconButton.vue'
export { default as Spinner } from './components/Spinner.vue'
export { default as StatusDot } from './components/StatusDot.vue'
export { default as ThemeToggle } from './components/ThemeToggle.vue'

export { APPEARANCE_BOOTSTRAP_SCRIPT } from './appearance-script'

export {
  initAppearance,
  resetAppearanceForTests,
  useAppearance,
  type AppearanceApi,
} from './composables/useAppearance'

export {
  COLOR_SCHEMES,
  COLOR_SCHEME_ATTRIBUTE,
  COLOR_SCHEME_OPTIONS,
  COLOR_SCHEME_STORAGE_KEY,
  DEFAULT_COLOR_SCHEME,
  DEFAULT_THEME,
  THEME_ATTRIBUTE,
  THEME_MODES,
  THEME_STORAGE_KEY,
  applyColorSchemeToDocument,
  applyThemeToDocument,
  isColorScheme,
  isThemeMode,
  readSystemTheme,
  type ColorScheme,
  type ColorSchemeOption,
  type ThemeMode,
} from './theme'
