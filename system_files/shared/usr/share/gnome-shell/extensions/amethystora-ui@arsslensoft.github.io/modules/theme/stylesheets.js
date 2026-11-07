// The shell's theme made again with every stylesheet it had, less those to drop and with those to add, and put in place in
// one go, as GNOME Shell itself does when its theme changes (Main.loadTheme): loading or unloading a single stylesheet
// restyles the whole stage (some 50 ms each on a laptop's GPU), so this is one restyle whatever the number of files.
// `add` is a list of [name, Gio.File]; gives back those it added.

import St from 'gi://St';

export function replaceStylesheets(drop, add) {
    const context = St.ThemeContext.get_for_stage(global.stage);
    const old = context.get_theme();
    const theme = new St.Theme({
        application_stylesheet: old.application_stylesheet,
        theme_stylesheet: old.theme_stylesheet,
        default_stylesheet: old.default_stylesheet,
    });
    for (const file of old.get_custom_stylesheets()) {
        if (drop.some(each => each.equal(file)))
            continue;
        // Another extension's stylesheet that has gone from the disk since it was loaded is left out, not fatal
        try {
            theme.load_stylesheet(file);
        } catch (e) {
            console.warn(`Amethystora UI: ${file.get_path()} was not loaded again: ${e.message}`);
        }
    }
    const added = [];
    for (const [name, file] of add) {
        try {
            theme.load_stylesheet(file);
            added.push([name, file]);
        } catch (e) {
            console.warn(`Amethystora UI: ${name} was not loaded: ${e.message}`);
        }
    }
    context.set_theme(theme);
    return added;
}
