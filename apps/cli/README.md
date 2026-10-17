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

Launch `orchester` without arguments for the interactive terminal. `/model`
opens **Model** and **Reasoning level** as separate choices. Use Up/Down and
Enter to choose, and Escape to return to the previous menu. Each picker starts
on the current selection, including in a narrow terminal.

Changing the model keeps the current reasoning level; changing reasoning keeps
the model or profile. The menu includes Minimal through Max and Ultra, plus any
custom value already configured. Support for these values depends on the
provider. Choices apply to subsequent turns in the current session and leave the
configuration file unchanged. `/model <profile>` still opens a profile directly.

Repository: <https://github.com/dieWehmut/Orchester>
