// Keep this bootstrap module dependency-free: Vite loads it while evaluating
// an app config in Node, before the source-only package graph is bundled.
// These values intentionally mirror theme.ts; the appearance tests assert the
// public storage keys and defaults so the two axes cannot drift silently.
const THEME_MODES = ['light', 'dark'] as const
const COLOR_SCHEMES = ['amber', 'violet', 'teal', 'rose'] as const
const DEFAULT_THEME = 'dark'
const DEFAULT_COLOR_SCHEME = 'amber'
const THEME_ATTRIBUTE = 'data-theme'
const COLOR_SCHEME_ATTRIBUTE = 'data-color-scheme'
const THEME_STORAGE_KEY = 'orchester:theme'
const COLOR_SCHEME_STORAGE_KEY = 'orchester:color-scheme'

/**
 * Inline this script in `<head>` before loading application styles.
 *
 * It deliberately contains no imports at runtime: both the local WebUI and the
 * project site can execute it before their JavaScript bundles are available.
 */
export const APPEARANCE_BOOTSTRAP_SCRIPT = `(() => {
  const root = document.documentElement;
  const themes = ${JSON.stringify(THEME_MODES)};
  const schemes = ${JSON.stringify(COLOR_SCHEMES)};
  let storedTheme = null;
  let storedScheme = null;
  try {
    storedTheme = globalThis.localStorage?.getItem(${JSON.stringify(THEME_STORAGE_KEY)}) ?? null;
    storedScheme = globalThis.localStorage?.getItem(${JSON.stringify(COLOR_SCHEME_STORAGE_KEY)}) ?? null;
  } catch {}

  const documentTheme = root.getAttribute(${JSON.stringify(THEME_ATTRIBUTE)});
  const documentScheme = root.getAttribute(${JSON.stringify(COLOR_SCHEME_ATTRIBUTE)});
  const systemTheme = typeof globalThis.matchMedia === 'function'
    && globalThis.matchMedia('(prefers-color-scheme: light)').matches
      ? 'light'
      : ${JSON.stringify(DEFAULT_THEME)};
  const theme = themes.includes(storedTheme)
    ? storedTheme
    : themes.includes(documentTheme)
      ? documentTheme
      : systemTheme;
  const scheme = schemes.includes(storedScheme)
    ? storedScheme
    : schemes.includes(documentScheme)
      ? documentScheme
      : ${JSON.stringify(DEFAULT_COLOR_SCHEME)};

  root.setAttribute(${JSON.stringify(THEME_ATTRIBUTE)}, theme);
  root.setAttribute(${JSON.stringify(COLOR_SCHEME_ATTRIBUTE)}, scheme);
  root.style.colorScheme = theme;
})()`
