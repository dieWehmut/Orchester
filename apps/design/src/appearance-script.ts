import {
  COLOR_SCHEMES,
  COLOR_SCHEME_ATTRIBUTE,
  COLOR_SCHEME_STORAGE_KEY,
  DEFAULT_COLOR_SCHEME,
  DEFAULT_THEME,
  THEME_ATTRIBUTE,
  THEME_MODES,
  THEME_STORAGE_KEY,
} from './theme'

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
