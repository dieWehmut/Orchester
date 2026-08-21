# Frontend Tooling Launch Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Provide one machine-verifiable contract for starting the local WebUI, project website, Tauri desktop shell, and GitHub Pages build, with an honest Windows linker preflight.

**Architecture:** `apps/stack.manifest.json` is the declarative catalog of surfaces, ports, URLs, package names, and Pages metadata. Focused Node modules under `werkzeug/frontend/` validate that catalog against checked-in package/Vite/Tauri/workflow configuration, launch a selected surface from the repository root, and diagnose host prerequisites before desktop startup. Production UI code and Rust runtime code remain untouched.

**Tech Stack:** Node.js ESM, Node test runner, pnpm 10, Vite configuration contracts, Tauri 2 configuration, GitHub Actions.

---

## File map

- `apps/stack.manifest.json`: versioned declarative surface and toolchain contract.
- `apps/package.json`: stable developer-facing commands and tooling test entrypoint.
- `werkzeug/frontend/stack-manifest.mjs`: manifest loading and repository drift validation.
- `werkzeug/frontend/launch.mjs`: surface selection, preflight, dry-run, and child process handoff.
- `werkzeug/frontend/doctor.mjs`: CLI rendering and exit-code policy for environment checks.
- `werkzeug/frontend/environment.mjs`: pure diagnostic model and injectable command probes.
- `werkzeug/frontend/test/*.test.mjs`: Node tests for validation, launch, and diagnostics.
- `docs/FRONTENDS-OPERATIONS.md`: canonical commands, ports, Pages behavior, and failure semantics.
- `docs/BUILD-TOOLCHAIN.md`: Windows ARM64/MSVC setup and actionable linker repair.
- `.github/workflows/pages.yml`: verify the launch/Pages contract before deployment.

### Task 1: Version the stack manifest

- [ ] Write a failing Node test for surface IDs, package names, strict ports, URLs, Pages base path, artifact directory, and workflow path.
- [ ] Run the focused test and confirm failure because the manifest/validator is absent.
- [ ] Add the minimal manifest loader and repository drift validator.
- [ ] Run the focused test and `pnpm --dir apps stack:verify` green.
- [ ] Commit the manifest contract.

### Task 2: Standardize launch commands

- [ ] Write failing tests for WebUI, website, and desktop launch resolution plus dry-run behavior.
- [ ] Confirm the tests fail because the launcher and scripts are absent.
- [ ] Implement the launcher with repository-root `cwd`, inherited stdio, strict port arguments, and propagated child exit codes.
- [ ] Add `dev:webui`, `dev:website`, and `dev:desktop` scripts to `apps/package.json`.
- [ ] Run focused tests and commit.

### Task 3: Diagnose desktop prerequisites honestly

- [ ] Write failing tests for a healthy MSVC environment, missing Rust tools, ARM64 Rust with MSYS2 `link.exe`, missing `cl.exe`, and JSON/exit-code behavior.
- [ ] Confirm expected failures before implementation.
- [ ] Implement pure checks with injected command results and a thin real-command adapter.
- [ ] Block desktop launch on diagnostic errors while leaving WebUI and website startup independent.
- [ ] Run focused tests against fixtures and run the real desktop doctor, expecting a non-zero result on the current host.
- [ ] Commit diagnostics.

### Task 4: Document and continuously verify the contract

- [ ] Update frontend operations with exact commands, ports, Pages URL/base path, and Tauri prerequisites.
- [ ] Update the toolchain guide with doctor commands and the current ARM64/MSYS2 failure codes.
- [ ] Add the tooling tests to the apps test entrypoint.
- [ ] Add a Pages workflow validation step and relevant path filters.
- [ ] Run tooling tests, manifest verification, relevant website/Desktop tests, and `git diff --check`.
- [ ] Commit documentation and CI changes separately.

### Task 5: Publish the feature branch

- [ ] Rebase or merge only if `origin/main` moved and the changes remain conflict-free.
- [ ] Run fresh full scoped verification from the final branch state.
- [ ] Push `chore/tooling-launch-contract` to `origin` without merging `main`.
- [ ] Report commits, verification evidence, real doctor failure details, and files likely to conflict.
