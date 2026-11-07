// Gruvbox Shell: loads the generated Gruvbox recolour of the stock GNOME Shell
// stylesheet *in place of* the stock stylesheet.
//
// The User Themes extension loads a theme *on top of* the stock stylesheet, so
// St matches every widget against both stylesheets: twice the work whenever a
// widget is styled for the first time (first open of a folder, Quick Settings,
// the clipboard, ...). Here the stock stylesheet is swapped out instead, so
// styling costs exactly what it does with the stock theme.
//
// The stylesheet is written by ~/.local/bin/gruvbox-shell-theme. If it is
// missing (the generator removes it whenever anything is off), cannot be
// parsed, or the shell uses its light or high-contrast stylesheet, the stock
// theme is left alone.

import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import St from 'gi://St';

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

// Shared with the generator (generator/src/paths.rs), relative to the user
// data dir (~/.local/share).
const STYLESHEET_PATH = ['themes', 'Gruvbox-Shell', 'gnome-shell', 'gnome-shell.css'];
// The generated stylesheet is a recolour of this stock stylesheet only.
const STOCK_DARK_SUFFIX = '/gnome-shell-dark.css';

function gruvboxStylesheet(): Gio.File {
    return Gio.File.new_for_path(GLib.build_filenamev([GLib.get_user_data_dir(), ...STYLESHEET_PATH]));
}

function usesStockDark(theme: St.Theme): boolean {
    return theme.default_stylesheet?.get_uri().endsWith(STOCK_DARK_SUFFIX) ?? false;
}

// St.Theme only logs a warning when its default stylesheet fails to parse and
// then runs without one (an unstyled shell). Parse it on a throwaway theme
// first, where a failure throws instead.
function assertParses(stylesheet: Gio.File): void {
    new St.Theme().load_stylesheet(stylesheet);
}

// A copy of `current` with `stylesheet` in place of its default stylesheet,
// carrying over the extensions' stylesheets like Main.loadTheme() does.
function themeWith(stylesheet: Gio.File, current: St.Theme): St.Theme {
    // If User Themes is (re-)enabled with this theme, the application
    // stylesheet is this very file: keeping it would load it twice.
    const app = current.application_stylesheet;
    const theme = app && !app.equal(stylesheet)
        ? new St.Theme({default_stylesheet: stylesheet, application_stylesheet: app})
        : new St.Theme({default_stylesheet: stylesheet});
    for (const custom of current.get_custom_stylesheets())
        theme.load_stylesheet(custom);
    return theme;
}

export default class GruvboxShellExtension extends Extension {
    private _theme: St.Theme | null = null;
    private _changedId = 0;

    enable(): void {
        const context = St.ThemeContext.get_for_stage(global.stage);
        // Main.loadTheme() rebuilds the theme from the stock stylesheet when
        // the style variant changes (e.g. Dark Style toggled); swap it again.
        this._changedId = context.connect('changed', () => this._apply());
        this._apply();
    }

    disable(): void {
        const context = St.ThemeContext.get_for_stage(global.stage);
        if (this._changedId) {
            context.disconnect(this._changedId);
            this._changedId = 0;
        }
        const active = this._theme !== null && context.get_theme() === this._theme;
        this._theme = null;
        // Rebuild the stock theme (keeps the extensions' stylesheets).
        if (active)
            Main.loadTheme();
    }

    // Never throws: enable() must not fail half-way (GNOME would then never
    // call disable() to disconnect the signal), and a signal handler has
    // nobody to report to. On any problem the stock theme simply stays.
    private _apply(): void {
        const context = St.ThemeContext.get_for_stage(global.stage);
        const current = context.get_theme();
        // Already ours (this also ends the re-entrant call from set_theme()).
        if (!current || current === this._theme || !usesStockDark(current))
            return;

        const stylesheet = gruvboxStylesheet();
        try {
            if (!stylesheet.query_exists(null)) {
                console.warn(`${this.metadata.name}: ${stylesheet.get_path()} is missing, keeping the stock theme`);
                return;
            }
            assertParses(stylesheet);
            this._theme = themeWith(stylesheet, current);
            context.set_theme(this._theme);
            console.log(`${this.metadata.name}: loaded ${stylesheet.get_path()}`);
        } catch (e) {
            console.error(`${this.metadata.name}: cannot load ${stylesheet.get_path()}, keeping the stock theme:`, e);
        }
    }
}
