#!/usr/bin/env bash
# Build and install the Gruvbox desktop look. Needs no root.
#
#   ./install.sh            build, install and (re)generate everything; run it
#                           after changing colours or assets
#   ./install.sh --setup    the same, plus the one-time setup of a fresh
#                           system: the Gruvbox-Dark GTK 3 theme and the
#                           desktop settings (see README.md)
#
# Installs:
#   extension -> ~/.local/share/gnome-shell/extensions/gruvbox-shell@ex_gatito/
#   generator -> ~/.local/bin/gruvbox-shell-theme
#   service   -> ~/.config/systemd/user/gruvbox-shell-theme.service (runs the
#                generator at every login, before GNOME Shell starts)
#   generated -> shell theme, Adwaita-Brick folder icons, GTK 3/4 colours,
#                Ptyxis palette, Firefox userChrome.css + user.js
#
# Other arguments go to the shell-theme step, e.g. ./install.sh --force --reload
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
uuid=gruvbox-shell@ex_gatito        # shared with extension/metadata.json and generator/src/main.rs
unit=gruvbox-shell-theme.service
# The GTK 3 theme, pinned to a reviewed commit (name shared with
# generator/src/settings.rs: GTK_THEME).
gtk_theme=Gruvbox-Dark
gtk_theme_repo=https://github.com/Fausto-Korpsvart/Gruvbox-GTK-Theme.git
gtk_theme_commit=578cd220b5ff6e86b078a6111d26bb20ec8c733f

setup=false
theme_args=()
for arg in "$@"; do
    case "$arg" in
        --setup) setup=true ;;
        *) theme_args+=("$arg") ;;
    esac
done

# XDG base directory spec (same rule as generator/src/paths.rs): only an
# absolute XDG_*_HOME counts.
case "${XDG_DATA_HOME:-}" in /*) data_home=$XDG_DATA_HOME ;; *) data_home=$HOME/.local/share ;; esac
case "${XDG_CONFIG_HOME:-}" in /*) config_home=$XDG_CONFIG_HOME ;; *) config_home=$HOME/.config ;; esac
ext_dir="$data_home/gnome-shell/extensions/$uuid"
unit_dir="$config_home/systemd/user"
bin_dir="$HOME/.local/bin"
generator="$bin_dir/gruvbox-shell-theme"

die() { echo "install.sh: $*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "'$1' not found; install it with: sudo dnf install $2"; }
need npm nodejs-npm
need cargo cargo
need pkg-config pkgconf-pkg-config
need curl curl
pkg-config --exists gio-2.0 || die "GIO development files missing; install them with: sudo dnf install glib2-devel"
if $setup; then
    need git git
    need sassc sassc
fi

echo "==> Building the extension"
cd "$here/extension"
# Exact versions from package-lock.json; no dependency needs an install
# script, so none may run.
npm ci --ignore-scripts --no-audit --no-fund --silent
# The project's own tsc. (Never `npx tsc`: without node_modules it would
# download and run an unrelated npm package that happens to be named "tsc".)
npm run --silent build
install -d "$ext_dir"
install -m 644 metadata.json dist/extension.js "$ext_dir/"

echo "==> Building the generator"
cd "$here/generator"
cargo build --release --locked --quiet
install -d "$bin_dir"
# Copy then rename: the login service never sees a half-written binary.
install -m 755 target/release/gruvbox-shell-theme "$bin_dir/.gruvbox-shell-theme.new"
mv -f "$bin_dir/.gruvbox-shell-theme.new" "$generator"

echo "==> Installing the login service"
install -d "$unit_dir"
install -m 644 "$here/systemd/$unit" "$unit_dir/$unit"
systemctl --user daemon-reload
systemctl --user enable --quiet "$unit"

echo "==> Generating the shell theme"
"$generator" ${theme_args[@]+"${theme_args[@]}"}

echo "==> Building the folder icons"
"$generator" --folders

install_gtk_theme() {
    if [[ -f "$HOME/.themes/$gtk_theme/gtk-3.0/gtk.css" ]]; then
        echo "    $gtk_theme is already installed"
        return
    fi
    local src
    src="$(mktemp -d)"
    # Clean up when this function returns (then remove the trap again).
    # shellcheck disable=SC2064  # expand now: $src is local
    trap "rm -rf -- '$src'; trap - RETURN" RETURN
    git -C "$src" init --quiet
    git -C "$src" fetch --quiet --depth 1 "$gtk_theme_repo" "$gtk_theme_commit"
    git -C "$src" checkout --quiet FETCH_HEAD
    [[ "$(git -C "$src" rev-parse HEAD)" == "$gtk_theme_commit" ]] || die "fetched the wrong commit of the GTK theme"
    # Not -l/--libadwaita: that would replace ~/.config/gtk-4.0/gtk.css (our
    # colours) with a link to the theme's own.
    (cd "$src/themes" && ./install.sh --color dark >/dev/null)
    [[ -f "$HOME/.themes/$gtk_theme/gtk-3.0/gtk.css" ]] || die "the GTK theme installer did not produce $gtk_theme"
    echo "    installed $gtk_theme (commit ${gtk_theme_commit:0:12})"
}

if $setup; then
    echo "==> Installing the $gtk_theme GTK 3 theme"
    install_gtk_theme
    echo "==> Applying the desktop settings"
    "$generator" --settings
fi

echo "==> Writing the app colours (GTK, Ptyxis, Firefox)"
"$generator" --apps

echo
if $setup; then
    echo "Done. Log out and back in to finish: GNOME Shell only loads a newly"
    echo "installed extension at login. Then check with: gruvbox-shell-theme --check"
else
    echo "Done. Status: gruvbox-shell-theme --check"
fi
