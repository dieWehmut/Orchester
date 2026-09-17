# @orchester/cli

Cross-platform launcher for the Orchester native CLI.

```sh
npm install --global @orchester/cli
orchester --version
```

The package selects the native executable for the current operating system and
CPU architecture through optional platform packages. It supports Linux x64 and
arm64, macOS x64 and arm64, and Windows x64 and arm64. Native packages are
published before this package so npm can resolve the matching optional
dependency during installation.

Orchester reads its user configuration from `%USERPROFILE%/.orchester/orchester.jsonc`
on Windows and `$HOME/.orchester/orchester.jsonc` on Unix. Provider settings
belong in `model_providers`; a configured `api_key` enables Bearer
authentication by default. Keep literal keys only in a protected user file.

Repository: <https://github.com/dieWehmut/Orchester>
