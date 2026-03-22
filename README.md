# Clocks

A clocks application for the COSMIC™ desktop environment.

## Features

- **World Clocks** — track time across multiple timezones with live offset display
- **Stopwatch** — precision stopwatch with lap tracking, delta comparison, and session history
- **Alarm** — configurable alarms with repeat modes, snooze, custom sounds, and floating dialog notifications
- **Timer** — countdown timers with repeat support and sound alerts
- **Pomodoro** — pomodoro technique timers with work/break session management and progress tracking

## Localization

Supports English (default), Czech, Polish, German, Spanish, and Russian. The app automatically follows system language settings with English as the fallback.

## Project Structure

The codebase follows the COSMIC [MVU (Model-View-Update)][mvu] architecture:

```
src/
├── main.rs                        # Entry point
├── app.rs                         # Application orchestrator (Model + View + Update)
├── audio.rs                       # Audio playback and desktop notifications
├── config.rs                      # Config persistence structs
├── i18n.rs                        # Localization (Fluent)
├── pages/                         # Each page owns its own MVU triad
│   ├── alarm.rs
│   ├── timer.rs
│   ├── pomodoro.rs
│   ├── stopwatch.rs
│   └── world_clocks.rs
└── components/                    # Reusable UI components
    ├── sound_selector.rs
    └── duration.rs
```

## Installation

A [justfile](./justfile) is included for the [casey/just][just] command runner.

- `just` builds the application with the default `just build-release` recipe
- `just run` builds and runs the application (installs icon/desktop entry locally)
- `just install` installs the project into the system
- `just install-local` installs the icon and desktop entry to `~/.local/share/` for development
- `just vendor` creates a vendored tarball
- `just build-vendored` compiles with vendored dependencies from that tarball
- `just check` runs clippy on the project to check for linter warnings
- `just check-json` can be used by IDEs that support LSP

## Translators

[Fluent][fluent] is used for localization. Translation files are in the [i18n directory](./i18n). New translations may copy the [English (en) localization](./i18n/en), rename `en` to the desired [ISO 639-1 language code][iso-codes], and provide translations for each [message identifier][fluent-guide]. If no translation is necessary, the message may be omitted.

## Packaging

If packaging for a Linux distribution, vendor dependencies locally with the `vendor` rule, and build with the vendored sources using the `build-vendored` rule. When installing files, use the `rootdir` and `prefix` variables to change installation paths.

```sh
just vendor
just build-vendored
just rootdir=debian/clocks prefix=/usr install
```

It is recommended to build a source tarball with the vendored dependencies, which can typically be done by running `just vendor` on the host system before it enters the build environment.

## Developers

Developers should install [rustup][rustup] and configure their editor to use [rust-analyzer][rust-analyzer]. To improve compilation times, disable LTO in the release profile, install the [mold][mold] linker, and configure [sccache][sccache] for use with Rust. The [mold][mold] linker will only improve link times if LTO is disabled.

[fluent]: https://projectfluent.org/
[fluent-guide]: https://projectfluent.org/fluent/guide/hello.html
[iso-codes]: https://en.wikipedia.org/wiki/List_of_ISO_639-1_codes
[just]: https://github.com/casey/just
[mvu]: https://pop-os.github.io/libcosmic-book/mvu.html
[rustup]: https://rustup.rs/
[rust-analyzer]: https://rust-analyzer.github.io/
[mold]: https://github.com/rui314/mold
[sccache]: https://github.com/mozilla/sccache
