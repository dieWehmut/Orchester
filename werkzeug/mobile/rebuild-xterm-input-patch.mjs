// Rebuild the patched xterm source in a pnpm patch edit directory. The patch
// retains readable TypeScript, ESM/UMD runtimes and their matching source maps.
import { readFile, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';

if (!process.argv[2]) throw new Error('Pass the pnpm patch edit directory');
const directory = resolve(process.argv[2]);
const requireMobile = createRequire(new URL('../../apps/mobile/package.json', import.meta.url));
const requireVite = createRequire(requireMobile.resolve('vite'));
const { build, version } = requireVite('esbuild');
if (version !== '0.28.2') throw new Error(`Expected esbuild 0.28.2, received ${version}`);
const metadata = JSON.parse(await readFile(`${directory}/package.json`, 'utf8'));
if (metadata.name !== '@xterm/xterm' || metadata.version !== '6.0.0') {
  throw new Error('Expected the pinned @xterm/xterm 6.0.0 package');
}
const original = await readFile(`${directory}/lib/xterm.mjs`, 'utf8');
const license = original.slice(0, original.indexOf('var ')).trim();
const options = {
  absWorkingDir: directory,
  entryPoints: ['src/browser/public/Terminal.ts'],
  bundle: true,
  target: 'es2021',
  sourcemap: true,
  treeShaking: true,
  minify: true,
  legalComments: 'none',
  tsconfigRaw: {
    compilerOptions: {
      target: 'ES2021',
      experimentalDecorators: true,
      useDefineForClassFields: false,
      baseUrl: '.',
      paths: {
        'common/*': ['src/common/*'],
        'browser/*': ['src/browser/*'],
        'vs/*': ['src/vs/*'],
      },
    },
  },
};
await build({ ...options, format: 'esm', outfile: 'lib/xterm.mjs', banner: { js: license } });
// Preserve the package's CommonJS, AMD and browser-global entry point contract.
const umd = `(function(root, factory) {
  if (typeof exports === 'object' && typeof module === 'object') module.exports = factory();
  else if (typeof define === 'function' && define.amd) define([], factory);
  else Object.assign(typeof exports === 'object' ? exports : root, factory());
})(globalThis, function() {
  var module = { exports: {} };
  var exports = module.exports;`;
await build({
  ...options,
  format: 'cjs',
  outfile: 'lib/xterm.js',
  banner: { js: `${license}\n${umd}` },
  footer: { js: 'return module.exports;\n});' },
});
// The original UMD artifact has no final newline, and pnpm preserves that
// unchanged sourceMappingURL line while applying the patch.
const umdFile = `${directory}/lib/xterm.js`;
await writeFile(umdFile, (await readFile(umdFile, 'utf8')).replace(/\n$/, ''));
