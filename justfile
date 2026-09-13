# Name of the application's binary.
name := 'clocks'
# The unique ID of the application.
appid := 'dev.th3jk.clocks'

# Path to root file system, which defaults to `/`.
rootdir := ''
# The prefix for the `/usr` directory.
prefix := '/usr'
# The location of the cargo target directory.
cargo-target-dir := env('CARGO_TARGET_DIR', 'target')

# Installed names. These are derived from the app ID because the desktop entry
# has to be named after the Wayland app_id for the compositor to resolve the
# window's icon, and the icon has to be named after the entry's `Icon=` key.
appdata := appid + '.metainfo.xml'
# Application's desktop entry
desktop := appid + '.desktop'
# Application's icon.
icon-svg := appid + '.svg'

# Source names in `resources/`, which differ from the installed names above.
# Installing from the installed name silently breaks every install recipe.
appdata-src := 'resources' / 'app.metainfo.xml'
desktop-src := 'resources' / 'app.desktop'
icon-svg-src := 'resources' / 'icons' / 'hicolor' / 'scalable' / 'apps' / 'icon.svg'
audio-src := 'resources' / 'audio'
daemon-name := name + '-daemon'
dbus-service-src := 'resources' / 'daemon.service'
autostart-src := 'resources' / 'daemon-autostart.desktop'

# Install destinations
base-dir := absolute_path(clean(rootdir / prefix))
appdata-dst := base-dir / 'share' / 'appdata' / appdata
bin-dst := base-dir / 'bin' / name
desktop-dst := base-dir / 'share' / 'applications' / desktop
icons-dst := base-dir / 'share' / 'icons' / 'hicolor'
icon-svg-dst := icons-dst / 'scalable' / 'apps' / icon-svg
# Notification sounds. `resolve_sound_path` derives this from the binary's own
# location, so it follows whatever prefix is installed to.
audio-dst := base-dir / 'share' / name / 'audio'
daemon-bin-dst := base-dir / 'bin' / daemon-name
# D-Bus activation works on any distro with a session bus -- no systemd needed.
dbus-service-dst := base-dir / 'share' / 'dbus-1' / 'services' / (appid + '.Daemon.service')
# Starts the daemon at login so alarms fire with the app closed.
autostart-dst := base-dir / 'etc' / 'xdg' / 'autostart' / (appid + '.Daemon.desktop')

# Local user data directory for development installs
local-data-dir := env('XDG_DATA_HOME', env('HOME') + '/.local/share')

# Default recipe which runs `just build-release`
default: build-release

# Runs `cargo clean`
clean:
    cargo clean

# Removes vendored dependencies
clean-vendor:
    rm -rf .cargo vendor vendor.tar

# `cargo clean` and removes vendored dependencies
clean-dist: clean clean-vendor

# Compiles with debug profile
build-debug *args:
    cargo build {{args}}

# Compiles with release profile
build-release *args: (build-debug '--release' args)

# Compiles release profile with vendored dependencies
build-vendored *args: vendor-extract (build-release '--frozen --offline' args)

# Runs a clippy check
check *args:
    cargo clippy --all-features {{args}} -- -W clippy::pedantic

# Runs a clippy check with JSON message format
check-json: (check '--message-format=json')

# Install icon and desktop entry to user's local data dir so the system
# can resolve the app icon when running via `cargo run` or `just run`.
install-local:
    install -Dm0644 {{desktop-src}} {{ local-data-dir / 'applications' / desktop }}
    install -Dm0644 {{icon-svg-src}} {{ local-data-dir / 'icons' / 'hicolor' / 'scalable' / 'apps' / icon-svg }}
    -update-desktop-database {{ local-data-dir / 'applications' }}
    -gtk-update-icon-cache -f {{ local-data-dir / 'icons' / 'hicolor' }}

# Register the daemon for the current user: D-Bus activation plus autostart at
# login, both pointing at the built binary rather than an installed prefix.
# Defaults to the debug binary, since that is what `cargo run` produces while
# iterating. Pass `release` once you want to test what actually ships.
install-daemon-local profile='debug':
    cargo build {{ if profile == 'release' { '--release' } else { '' } }} --bin {{daemon-name}}
    mkdir -p {{ local-data-dir / 'dbus-1' / 'services' }}
    printf '[D-BUS Service]\nName=dev.th3jk.clocks.Daemon\nExec=%s\n' \
        {{ cargo-target-dir / profile / daemon-name }} \
        > {{ local-data-dir / 'dbus-1' / 'services' / (appid + '.Daemon.service') }}
    mkdir -p {{ env('XDG_CONFIG_HOME', env('HOME') + '/.config') / 'autostart' }}
    printf '[Desktop Entry]\nType=Application\nName=Clocks alarms\nExec=%s\nTerminal=false\nNoDisplay=true\n' \
        {{ cargo-target-dir / profile / daemon-name }} \
        > {{ env('XDG_CONFIG_HOME', env('HOME') + '/.config') / 'autostart' / (appid + '.Daemon.desktop') }}
    @echo "Registered. D-Bus will now start {{ cargo-target-dir / profile / daemon-name }} on demand."

# Run the application for testing purposes
run *args: install-local
    env RUST_BACKTRACE=full cargo run --release {{args}}

# Installs files
install:
    install -Dm0755 {{ cargo-target-dir / 'release' / name }} {{bin-dst}}
    install -Dm0644 {{desktop-src}} {{desktop-dst}}
    install -Dm0644 {{appdata-src}} {{appdata-dst}}
    install -Dm0644 {{icon-svg-src}} {{icon-svg-dst}}
    install -Dm0644 -t {{audio-dst}} {{audio-src}}/*.wav
    install -Dm0755 {{ cargo-target-dir / 'release' / daemon-name }} {{daemon-bin-dst}}
    install -Dm0644 {{dbus-service-src}} {{dbus-service-dst}}
    install -Dm0644 {{autostart-src}} {{autostart-dst}}

# Uninstalls installed files
uninstall:
    rm {{bin-dst}} {{desktop-dst}} {{appdata-dst}} {{icon-svg-dst}}
    rm {{daemon-bin-dst}} {{dbus-service-dst}} {{autostart-dst}}
    rm -rf {{audio-dst}}

# Vendor dependencies locally
vendor:
    mkdir -p .cargo
    cargo vendor | head -n -1 > .cargo/config.toml
    echo 'directory = "vendor"' >> .cargo/config.toml
    tar pcf vendor.tar vendor
    rm -rf vendor

# Extracts vendored dependencies
vendor-extract:
    rm -rf vendor
    tar pxf vendor.tar

# Bump cargo version, create git commit, and create tag
tag version:
    find -type f -name Cargo.toml -exec sed -i '0,/^version/s/^version.*/version = "{{version}}"/' '{}' \; -exec git add '{}' \;
    cargo check
    cargo clean
    git add Cargo.lock
    git commit -m 'release: {{version}}'
    git commit --amend
    git tag -a {{version}} -m ''

