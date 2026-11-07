# Gruvbox desktop

The whole Gruvbox look of this Fedora GNOME desktop, in one place:

- **GNOME Shell**: a Gruvbox recolour of the installed shell's own stylesheet,
  regenerated at every login (so it never goes stale after a GNOME update) and
  loaded by a small extension *in place of* the stock stylesheet.
- **Folder icons**: `Adwaita-Brick`, GNOME's folder icons in the accent colour.
- **Apps**: GTK 3 and GTK 4/libadwaita colours, the Ptyxis terminal palette,
  Firefox (the Gruvbox theme add-on plus `userChrome.css` and `user.js`).
- **Settings**: dark style, icon and GTK theme, extensions, Flatpak access.

The accent (brick red `#5e2424`) is defined once, in
`generator/src/color.rs`; the shell, the folders and the GTK apps all follow it.

## Install on a fresh Fedora

1. Install the tools:

   ```
   sudo dnf install git cargo nodejs-npm glib2-devel sassc
   ```

2. Start Firefox once and close it (this creates the profile to style).
3. Put this folder at `~/Projects/gruvbox-shell` and run:

   ```
   ~/Projects/gruvbox-shell/install.sh --setup
   ```

4. If Firefox opens and asks to add the Gruvbox theme, click **Add**.
   (Signing in to Firefox Sync also brings it back.)
5. Log out and back in: GNOME Shell only loads a newly installed extension at
   login. Then `gruvbox-shell-theme --check` should say `all good`.

`--setup` downloads two pinned things: the Gruvbox-Dark GTK 3 theme
(github.com/Fausto-Korpsvart/Gruvbox-GTK-Theme, fixed commit, built locally
with its own installer) and the Firefox theme (addons.mozilla.org, checked
against a fixed SHA-256). Everything else is in this folder.

Files it would replace that it did not create itself (e.g. a hand-made
`~/.config/gtk-4.0/gtk.css`) are kept as `<name>.before-gruvbox`.

## Everyday use

| Task | How |
|---|---|
| Is everything in place? | `gruvbox-shell-theme --check` |
| Change the accent or the shell colours | edit `generator/src/color.rs`, run `./install.sh --reload` |
| Change folder shades | edit `Role::offset` in `generator/src/folders.rs`, run `./install.sh` |
| Change GTK, Ptyxis or Firefox styling | edit the file in `generator/assets/`, run `./install.sh` |
| Preview folder colours | `gruvbox-shell-theme --folders --dry-run` |

After `install.sh`, reopen apps to see GTK changes and restart Firefox for its
files. Don't enable User Themes or Accent Icons: `--check` explains why.

## Layout

```
extension/   GNOME Shell extension (TypeScript): loads the generated stylesheet
generator/   gruvbox-shell-theme (Rust): generates, installs and checks everything
  src/         one module per job (see the top of main.rs)
  assets/      GTK templates, Ptyxis palette, Firefox files, Adwaita-Red icons
systemd/     login service that regenerates the shell theme
install.sh   builds and installs it all (no root needed)
```

## Undo

```
gnome-extensions disable gruvbox-shell@ex_gatito
systemctl --user disable gruvbox-shell-theme.service
gsettings reset org.gnome.desktop.interface icon-theme
gsettings reset org.gnome.desktop.interface gtk-theme
rm ~/.config/gtk-3.0/gtk.css ~/.config/gtk-4.0/gtk.css   # or restore the .before-gruvbox copies
```
