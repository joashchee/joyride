# Contributing to JoyrIDE

Thank you for helping. JoyrIDE is small and opinionated: please open an
issue to talk about anything bigger than a fix before you build it.

## The rules a change has to keep

The full list is in `CLAUDE.md`. The ones most often missed:

1. **No telemetry, no accounts of ours.** The only network use is what
   the user asks for or turns on: the update check, the toolchain
   manifest, library downloads and GitHub.
2. **Never mutate the user's source.** Porting works through overlays in
   `.joyride/patches/` and a staged copy at build time.
3. **Credentials stay in the OS keychain**, never a file or a log.
4. **GitHub Actions only where it's free and unlimited**: standard
   GitHub-hosted runners in this public repository.
5. **No GPL, AGPL or LGPL dependencies.** When `Cargo.lock` or
   `package-lock.json` changes, run `scripts/license-scan.py`.
6. **Keyboard alone and screen readers work** for every control.
7. A bullet in `CHANGELOG.md` and a tick in `ROADMAP.md` with each change.

## Building

See the README. Run `cargo test`, `cargo clippy --all-targets` and
`npx tsc --noEmit` before a pull request. Sign off your commits
(`git commit -s`).
