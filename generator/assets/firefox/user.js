// Firefox preferences for the Gruvbox look.
// Firefox applies this file at every start, overriding the same settings made
// in its own Settings page. To manage one of them in Firefox again, delete its
// line here, run install.sh, then change it in Settings.

// Load chrome/userChrome.css (the tab tweaks).
user_pref("toolkit.legacyUserProfileCustomizations.stylesheets", true);
// Dark toolbars and menus (0 = dark).
user_pref("browser.theme.toolbar-theme", 0);
// Websites in dark mode where they support it (0 = dark; Settings > General >
// Website appearance).
user_pref("layout.css.prefers-color-scheme.content-override", 0);
