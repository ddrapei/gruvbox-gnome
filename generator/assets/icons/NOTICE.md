# Adwaita-Red

GNOME's Adwaita folder icons in GNOME's red accent, as installed by the
"Accent Icons" GNOME Shell extension (accent-directories@taiwbi.com, version
2.2, https://github.com/taiwbi/gnome-accent-directories) into
`~/.local/share/icons/Adwaita-Red`.

Except `scalable/places/folder-projects.svg`, which that version lacks (the
Projects folder, `XDG_PROJECTS_DIR`, is newer): it is Adwaita's own
(adwaita-icon-theme 51.0, Fedora 45) with its blue swapped for the same red,
colour by colour: `#438de6`→`#e62d42`, `#62a0ea`→`#e94a5c`,
`#a4caee`→`#f2939e`, `#afd4ff` and `#c0d5ea`→`#f4a2ab`.

They are the *input* of `gruvbox-shell-theme --folders`, which recolours them
into the `Adwaita-Brick` icon theme. They are built into the binary
(see `build.rs`), so the extension is not needed.

The Adwaita icon theme is licensed LGPL-3.0-only OR CC-BY-SA-3.0.
