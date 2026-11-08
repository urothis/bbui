# Repository Guidelines

## Project Structure & Module Organization

This directory does not yet contain source code, tests, assets, or a package manifest. As the project takes shape, keep application code under `src/`, automated tests under `tests/`, and static assets under `assets/` unless the chosen framework requires another layout. Group code by feature or responsibility, and keep generated files out of version control.

## Build, Test, and Development Commands

No build system or local run command is configured yet. Add a short `README.md` with the exact setup, run, build, and test commands when the first toolchain is introduced. Prefer commands defined in a checked-in manifest or script so every contributor runs the same workflow. Do not assume `npm test` or `make build` works until those targets exist.

## Coding Style & Naming Conventions

No language, formatter, or linter has been selected. Follow the conventions of the language and framework adopted for each component, and commit the formatter and linter configuration alongside the code. Use descriptive names, keep related files together, and apply one naming pattern consistently within each directory.

## Testing Guidelines

There is no test framework or coverage target yet. Add tests with new behavior and bug fixes once a framework is chosen. Place tests in `tests/` or beside the code, following the framework's discovery rules; document the test command and any coverage threshold in the `README.md`.

## Commit & Pull Request Guidelines

This directory has no Git history, so no established commit convention can be inferred. Use short, imperative commit subjects that describe the change, such as `Add parser validation`. Pull requests should explain the purpose, summarize the implementation, list the checks run, and link a related issue when one exists. Include screenshots for visible interface changes.
