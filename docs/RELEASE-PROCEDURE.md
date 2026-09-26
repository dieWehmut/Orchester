# Cutting a release

Two workflows publish this repository, and neither can be run from a developer
machine without credentials. This is what they validate, and the order they have
to run in.

## What the remote carries

| the remote | what it is | workflow |
| --- | --- | --- |
| GitHub Release `desktop-v<version>` | `Orchester_<version>_x64-setup.exe`, `Orchester_<version>_arm64-setup.exe`, `SHA256SUMS` | [`desktop-release.yml`](../.github/workflows/desktop-release.yml) |
| npm `@orchester/cli@<version>` and its six platform packages | the CLI, per platform | [`npm-release.yml`](../.github/workflows/npm-release.yml) |
| npm `@orchester/{claude,codex,opencode}@<version>` | the agent plugins | `npm-release.yml` |
| GitHub Release `v<version>` | the six native archives and ten `.tgz` files | `npm-release.yml` |

The CLI's own updater asks `https://registry.npmjs.org/@orchester/cli/latest`
(`kisten/konsole/src/update.rs`), so **publishing the npm packages is what makes
a CLI "latest"**. Nothing else does.

## What a version cut has to touch

Both workflows refuse to run unless the version agrees everywhere, and each
checks a different set:

- `desktop-release.yml` checks `apps/desktop/package.json`,
  `apps/desktop/src-tauri/tauri.conf.json` and
  `apps/desktop/src-tauri/Cargo.toml`.
- `npm-release.yml` checks `apps/cli/package.json`, the six
  `@orchester/cli-*` pins inside its `optionalDependencies`, every
  `npm/plugins/*/package.json`, and **every package `cargo metadata` reports** -
  which is the whole `kisten/` workspace plus the two desktop crates.

The same cut also moves, because the repository keeps them in step: the workspace
manifests (`apps/web`, `packages/*`), the plugin manifests
(`npm/plugins/*/orchester-plugin.json`), the three `Cargo.lock` files, the
fixtures that pin the version (`apps/cli/test/target.test.cjs`,
`werkzeug/npm/plugin*.test.mjs`, `werkzeug/npm/release-workflow.test.mjs`), and
the workflows' own `default:` inputs.

## The order

1. **Cut the version** in everything above, and push the commit. Move
   `pnpm-lock.yaml` only as part of step 3.
2. **Dispatch `npm-release`** with the version. It builds the six native
   archives, packs and publishes the ten npm packages, tags `v<version>`, creates
   the GitHub Release, and verifies both halves. It does not run
   `pnpm install`, which is why the stale lockfile below does not stop it.
3. **Refresh `pnpm-lock.yaml`** once the packages exist: the six
   `@orchester/cli-*` entries are resolved from the registry with integrity
   hashes, so the file can only be brought up to the new version *after*
   publication. `pnpm install --lockfile-only` then, and commit it.
4. **Dispatch `desktop-release`** with the version. It builds the two NSIS
   installers, verifies install/shortcut/launch/uninstall, stages and checksums
   them, tags `desktop-v<version>`, creates the GitHub Release, and verifies the
   published assets against `SHA256SUMS`.

Steps 3 and 4 are in this order for one reason: `desktop-release`'s test job runs
`pnpm install --frozen-lockfile`, and that fails while `apps/cli/package.json`
disagrees with `pnpm-lock.yaml`. That is derived from the workflow and the
lockfile's shape rather than observed here - the machine this was written on
cannot reach the registry, so it could not be reproduced.

## Credentials

- GitHub: both workflows use the repository's own token. The npm one declares
  `id-token: write` and publishes with `--provenance` from the `npm-release`
  environment.
- npm: `secrets.NPM_TOKEN` is used when it is set. With no token the workflow
  still succeeds **only if every package already exists on the registry**; a
  brand-new package name needs one publish with a granular token first, and then
  Trusted Publishing can be configured. The workflow says so itself, in the job
  that would otherwise fail.

## Before dispatching

One command asks every question the two workflows ask, plus two they only ask
later, and prints the order below:

```text
node werkzeug/desktop/release-preflight.mjs
```

It checks the version in the three desktop manifests, the CLI manifest and its
six platform pins, every plugin's package **and** manifest, every package each of
the three `cargo metadata --locked` calls reports, and whether a tag for this
version already names a different commit. It touches neither the network nor
`pnpm-lock.yaml`, because the latter can only be refreshed after publication -
a preflight that demanded it would refuse every release forever.

The longer form, if something needs looking at by hand:

```text
node --test werkzeug/desktop/*.test.mjs
node --test werkzeug/npm/*.test.mjs apps/cli/test/*.test.cjs
node werkzeug/desktop/verify-bundle.mjs
cargo metadata --locked --no-deps --format-version 1          # root
cargo metadata --locked --no-deps --format-version 1          # apps/desktop/src-tauri
cargo metadata --locked --no-deps --format-version 1          # apps/desktop/native-chrome
pnpm stack:verify
pnpm typecheck
```

The first three of those are the release tooling's own tests; the `cargo
metadata` calls are what prove the three lockfiles agree with the manifests after
a cut.