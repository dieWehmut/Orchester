import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';

export const LOOPBACK_REGISTRY = 'http://127.0.0.1:9/';

function fail(code, message) {
  const error = new Error(message);
  error.code = code;
  throw error;
}

export function assertInside(root, candidate, label) {
  const relative = path.relative(path.resolve(root), path.resolve(candidate));
  if (relative === '' || (!relative.startsWith(`..${path.sep}`)
    && relative !== '..' && !path.isAbsolute(relative))) {
    return;
  }
  fail('ORCHESTER_NPM_PATH_ESCAPE', `${label} escapes the temporary root`);
}

export function mkdirFresh(directory) {
  try {
    fs.mkdirSync(directory, { recursive: false });
  } catch (error) {
    if (error?.code === 'EEXIST') {
      fail('ORCHESTER_NPM_SANDBOX_EXISTS', `temporary sandbox already exists: ${directory}`);
    }
    throw error;
  }
}

export function ensureDirectory(directory) {
  fs.mkdirSync(directory, { recursive: true });
  const status = fs.lstatSync(directory);
  if (!status.isDirectory() || status.isSymbolicLink()) {
    fail('ORCHESTER_NPM_SANDBOX_INVALID', `temporary path is not a directory: ${directory}`);
  }
}

function writeConfig(file, values) {
  const contents = Object.entries(values)
    .map(([key, value]) => {
      const scalar = String(value);
      if (scalar.includes('\n') || scalar.includes('\r')) {
        fail('ORCHESTER_NPM_CONFIG_INVALID', `configuration value for ${key} contains a line break`);
      }
      return `${key}=${scalar}`;
    })
    .join('\n');
  fs.writeFileSync(file, `${contents}\n`, { flag: 'wx', mode: 0o600 });
}

function setEnvironmentValue(env, key, value) {
  const matches = Object.keys(env).filter((candidate) => candidate.toLowerCase() === key.toLowerCase());
  const target = matches.find((candidate) => candidate === 'Path') || matches[0] || key;
  env[target] = value;
  for (const duplicate of matches) {
    if (duplicate !== target) delete env[duplicate];
  }
}

function isolatedBaseEnvironment(baseEnv) {
  const env = {};
  const allowed = [
    'ComSpec',
    'LANG',
    'LC_ALL',
    'Path',
    'PATHEXT',
    'SYSTEMROOT',
    'SystemRoot',
    'TERM',
    'WINDIR',
    'windir',
  ];
  for (const key of allowed) {
    if (baseEnv[key] !== undefined) env[key] = baseEnv[key];
  }
  const inheritedPathKey = Object.keys(baseEnv).find((key) => key.toLowerCase() === 'path');
  if (inheritedPathKey && !Object.keys(env).some((key) => key.toLowerCase() === 'path')) {
    env[inheritedPathKey] = baseEnv[inheritedPathKey];
  }
  return env;
}

export function createIsolatedEnvironment(root, baseEnv = process.env) {
  const resolvedRoot = path.resolve(root);
  if (fs.existsSync(resolvedRoot)) {
    fail('ORCHESTER_NPM_SANDBOX_EXISTS', `temporary sandbox already exists: ${resolvedRoot}`);
  }
  ensureDirectory(path.dirname(resolvedRoot));
  mkdirFresh(resolvedRoot);

  const directories = {
    home: path.join(resolvedRoot, 'home'),
    profile: path.join(resolvedRoot, 'profile'),
    appdata: path.join(resolvedRoot, 'appdata'),
    localappdata: path.join(resolvedRoot, 'localappdata'),
    prefix: path.join(resolvedRoot, 'prefix'),
    globalDirectory: path.join(resolvedRoot, 'global'),
    binDirectory: path.join(resolvedRoot, 'bin'),
    storeDirectory: path.join(resolvedRoot, 'store'),
    cacheDirectory: path.join(resolvedRoot, 'cache'),
    tempDirectory: path.join(resolvedRoot, 'temp'),
    bunInstall: path.join(resolvedRoot, 'bun-install'),
    config: path.join(resolvedRoot, 'config'),
  };
  for (const directory of Object.values(directories)) mkdirFresh(directory);
  directories.bunGlobalDirectory = path.join(directories.bunInstall, 'install', 'global');
  mkdirFresh(path.join(directories.bunInstall, 'install'));
  mkdirFresh(directories.bunGlobalDirectory);

  const env = isolatedBaseEnvironment(baseEnv);
  env.HOME = directories.home;
  env.USERPROFILE = directories.profile;
  env.APPDATA = directories.appdata;
  env.LOCALAPPDATA = directories.localappdata;
  env.XDG_CONFIG_HOME = directories.config;
  env.XDG_CACHE_HOME = directories.cacheDirectory;
  env.PNPM_HOME = directories.binDirectory;
  env.BUN_INSTALL = directories.bunInstall;
  env.BUN_CONFIG_REGISTRY = LOOPBACK_REGISTRY;
  env.YARN_CACHE_FOLDER = directories.cacheDirectory;
  env.YARN_GLOBAL_FOLDER = directories.globalDirectory;
  env.YARN_PREFIX = directories.prefix;
  env.TEMP = directories.tempDirectory;
  env.TMP = directories.tempDirectory;
  env.TMPDIR = directories.tempDirectory;
  const inheritedPathKey = Object.keys(env).find((key) => key.toLowerCase() === 'path');
  const inheritedPath = inheritedPathKey ? env[inheritedPathKey] : '';
  const pathEntries = [
    directories.binDirectory,
    path.dirname(process.execPath),
    inheritedPath,
  ].filter(Boolean).join(path.delimiter);
  setEnvironmentValue(env, 'PATH', pathEntries);

  const userConfig = path.join(directories.config, 'npmrc');
  const globalConfig = path.join(directories.config, 'global.npmrc');
  writeConfig(userConfig, {
    registry: LOOPBACK_REGISTRY,
    cache: directories.cacheDirectory,
    prefix: directories.prefix,
    'global-dir': directories.globalDirectory,
    'global-bin-dir': directories.binDirectory,
    'store-dir': directories.storeDirectory,
    'cache-dir': directories.cacheDirectory,
    'ignore-scripts': 'true',
    audit: 'false',
    fund: 'false',
    'update-notifier': 'false',
  });
  writeConfig(globalConfig, { registry: LOOPBACK_REGISTRY });

  const configValues = {
    npm_config_userconfig: userConfig,
    NPM_CONFIG_USERCONFIG: userConfig,
    npm_config_globalconfig: globalConfig,
    NPM_CONFIG_GLOBALCONFIG: globalConfig,
    npm_config_registry: LOOPBACK_REGISTRY,
    NPM_CONFIG_REGISTRY: LOOPBACK_REGISTRY,
    npm_config_cache: directories.cacheDirectory,
    NPM_CONFIG_CACHE: directories.cacheDirectory,
    npm_config_prefix: directories.prefix,
    NPM_CONFIG_PREFIX: directories.prefix,
    npm_config_global_dir: directories.globalDirectory,
    npm_config_global_bin_dir: directories.binDirectory,
    npm_config_store_dir: directories.storeDirectory,
    npm_config_cache_dir: directories.cacheDirectory,
    npm_config_ignore_scripts: 'true',
    npm_config_audit: 'false',
    npm_config_fund: 'false',
    npm_config_update_notifier: 'false',
    npm_config_offline: 'true',
  };
  for (const [key, value] of Object.entries(configValues)) env[key] = value;

  return {
    root: resolvedRoot,
    env,
    ...directories,
    userConfig,
    globalConfig,
    registry: LOOPBACK_REGISTRY,
  };
}
