// The tiling's settings window: the gaps, the sizes the keys step through, where new windows open, how the strip
// moves, the screen edges and the touchpad, every keyboard shortcut, and the window rules. Each change is written to
// the tiling's schema at once, and the tiling, which watches it, applies it at once (modules/tiling/config.js).

import Adw from 'gi://Adw';
import Gdk from 'gi://Gdk';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Gtk from 'gi://Gtk';

import {ExtensionPreferences, gettext as _} from 'resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js';

const TILING = 'org.gnome.shell.extensions.amethystora-ui.tiling';
const ORDINALS = ['second', 'third', 'fourth', 'fifth', 'sixth', 'seventh', 'eighth', 'ninth', 'tenth', 'eleventh'];

export default class AmethystoraUiPreferences extends ExtensionPreferences {
    fillPreferencesWindow(window) {
        const settings = this.getSettings(TILING);
        const keys = this.getSettings(`${TILING}.keybindings`);
        window.set_default_size(900, 820);
        window.search_enabled = true;
        window.title = _('Window tiling');
        const pages = [windowsPage(settings), motionPage(settings), workspacesPage(settings), keyboardPage(window, keys),
            rulesPage(settings)];
        for (const page of pages)
            window.add(page);
        // The windows of the pages keep the settings for as long as the window is open
        window._tilingSettings = [settings, keys];
    }
}

// ---- Rows ----

function group(page, title, description = null) {
    const g = new Adw.PreferencesGroup({title});
    if (description)
        g.description = description;
    page.add(g);
    return g;
}

function switchRow(settings, key, title, subtitle = null) {
    const row = new Adw.SwitchRow({title});
    if (subtitle)
        row.subtitle = subtitle;
    settings.bind(key, row, 'active', Gio.SettingsBindFlags.DEFAULT);
    return row;
}

// A number, in the key's own unit
function spinRow(settings, key, title, min, max, step, digits = 0, subtitle = null) {
    const row = Adw.SpinRow.new_with_range(min, max, step);
    row.title = title;
    row.digits = digits;
    if (subtitle)
        row.subtitle = subtitle;
    settings.bind(key, row, 'value', Gio.SettingsBindFlags.DEFAULT);
    return row;
}

// A share kept as 0 to 1 and shown in percent
function percentRow(settings, key, title, min, max, subtitle = null) {
    const row = Adw.SpinRow.new_with_range(min, max, 1);
    row.title = title;
    if (subtitle)
        row.subtitle = subtitle;
    const read = () => {
        const value = Math.round(settings.get_double(key) * 100);
        if (Math.round(row.value) !== value)
            row.value = value;
    };
    read();
    row.connect('notify::value', () => {
        if (Math.round(settings.get_double(key) * 100) !== Math.round(row.value))
            settings.set_double(key, Math.round(row.value) / 100);
    });
    settings.connect(`changed::${key}`, read);
    return row;
}

// A choice of named values: [[value, label], ...]
function comboRow(settings, key, title, choices, subtitle = null) {
    const row = new Adw.ComboRow({title, model: Gtk.StringList.new(choices.map(([, label]) => label))});
    if (subtitle)
        row.subtitle = subtitle;
    const read = () => {
        const index = choices.findIndex(([value]) => value === settings.get_int(key));
        if (index >= 0 && row.selected !== index)
            row.selected = index;
    };
    read();
    row.connect('notify::selected', () => {
        const value = choices[row.selected]?.[0];
        if (value !== undefined && settings.get_int(key) !== value)
            settings.set_int(key, value);
    });
    settings.connect(`changed::${key}`, read);
    return row;
}

// A button that puts a key back to the image's value
function resetButton(settings, keyNames) {
    const button = new Gtk.Button({
        icon_name: 'edit-undo-symbolic', valign: Gtk.Align.CENTER, tooltip_text: _('Back to the default'),
        css_classes: ['flat'],
    });
    button.connect('clicked', () => keyNames.forEach(key => settings.reset(key)));
    return button;
}

// ---- Windows ----

function windowsPage(settings) {
    const page = new Adw.PreferencesPage({title: _('Windows'), icon_name: 'view-grid-symbolic'});

    const tiling = group(page, _('Tiling'));
    tiling.add(switchRow(settings, 'enabled', _('Tile windows'),
        _('Off is the classic layout: windows float and overlap, as on Windows and macOS')));

    const gaps = group(page, _('Gaps'), _('In pixels.'));
    gaps.header_suffix = resetButton(settings,
        ['window-gap', 'horizontal-margin', 'vertical-margin', 'vertical-margin-bottom']);
    gaps.add(spinRow(settings, 'window-gap', _('Between windows'), 0, 200, 1));
    gaps.add(spinRow(settings, 'horizontal-margin', _('At the left and right of the screen'), 0, 400, 1,
        0, _('How far from the edge a window stops when the strip scrolls to it')));
    gaps.add(spinRow(settings, 'vertical-margin', _('Below the top bar'), 0, 400, 1));
    gaps.add(spinRow(settings, 'vertical-margin-bottom', _('At the bottom of the screen'), 0, 400, 1));

    const frame = group(page, _('Focus frame'), _('The frame around the window you are on.'));
    frame.header_suffix = resetButton(settings,
        ['selection-border-size', 'selection-border-radius-top', 'selection-border-radius-bottom']);
    frame.add(spinRow(settings, 'selection-border-size', _('How far it stands out'), 0, 100, 1, 0,
        _('Never further than the gap between windows; 0 draws no frame')));
    frame.add(spinRow(settings, 'selection-border-radius-top', _('Roundness of the upper corners'), 0, 100, 1));
    frame.add(spinRow(settings, 'selection-border-radius-bottom', _('Roundness of the lower corners'), 0, 100, 1));

    const opening = group(page, _('New windows'));
    const positions = [
        [0, _('To the right of the window you are on')], [1, _('To its left')], [2, _('At the start of the strip')],
        [3, _('At the end of the strip')], [4, _('Below it, in its column')], [5, _('Above it, in its column')],
    ];
    opening.add(comboRow(settings, 'open-window-position', _('Open new windows'), positions));
    const offered = new Adw.ExpanderRow({
        title: _('Places the key goes through'),
        subtitle: _('Super+Shift+W and the arrow in the top bar change where new windows open, among these'),
    });
    const names = ['right', 'left', 'start', 'end', 'down', 'up'];
    positions.forEach(([value, label]) =>
        offered.add_row(switchRow(settings, `open-window-position-option-${names[value]}`, label)));
    opening.add(offered);
    opening.add(comboRow(settings, 'default-focus-mode', _('Scroll to the window you go to'), [
        [0, _('Just into view')], [1, _('To the centre of the screen')], [2, _('To the nearest edge')],
    ], _('Super+Shift+C changes it for one workspace')));

    const sizes = group(page, _('Sizes'));
    sizes.add(stepsRow(settings, 'cycle-width-steps', _('Widths Super+R steps through')));
    sizes.add(stepsRow(settings, 'cycle-height-steps', _('Heights Super+Shift+R steps through'),
        _('For a window that shares its column')));
    sizes.add(percentRow(settings, 'maximize-width-percent', _('Width Super+F gives a window'), 50, 100,
        _('In percent of the screen, less its margins')));
    sizes.add(switchRow(settings, 'maximize-within-tiling', _('Maximizing fills the width'),
        _('Off, a maximized window covers the whole screen until it is restored')));
    return page;
}

// The steps as people write them: "38%, 50%, 62%", or "800px"
function stepsText(steps) {
    return steps.map(step => step <= 1 ? `${Math.round(step * 1000) / 10}%` : `${Math.round(step)}px`).join(', ');
}

function parseSteps(text) {
    const parts = text.split(/[,;\s]+/).filter(Boolean);
    const steps = [];
    for (const part of parts) {
        const match = /^(\d+(?:[.,]\d+)?)\s*(%|px)?$/.exec(part);
        if (!match)
            return null;
        const number = Number(match[1].replace(',', '.'));
        const step = match[2] === '%' ? number / 100 : number;
        if (!(step > 0) || (match[2] === '%' && step > 1))
            return null;
        steps.push(step);
    }
    return steps.length ? steps : null;
}

function stepsRow(settings, key, title, subtitle = null) {
    const row = new Adw.EntryRow({title, show_apply_button: true});
    if (subtitle)
        row.tooltip_text = subtitle;
    const read = () => {
        row.text = stepsText(settings.get_value(key).deep_unpack());
        row.remove_css_class('error');
    };
    read();
    row.connect('apply', () => {
        const steps = parseSteps(row.text);
        if (!steps) {
            row.add_css_class('error');
            return;
        }
        settings.set_value(key, new GLib.Variant('ad', steps));
        read();
    });
    row.connect('changed', () => row.remove_css_class('error'));
    row.add_suffix(resetButton(settings, [key]));
    settings.connect(`changed::${key}`, read);
    return row;
}

// ---- Motion ----

function motionPage(settings) {
    const page = new Adw.PreferencesPage({title: _('Motion'), icon_name: 'input-touchpad-symbolic'});

    const motion = group(page, _('Animation'));
    motion.header_suffix = resetButton(settings, ['animation-time', 'drift-speed', 'drag-drift-speed']);
    motion.add(spinRow(settings, 'animation-time', _('Seconds a window takes to glide'), 0, 2, 0.05, 2,
        _('Before GNOME’s own speed, which the desktop sets a little quicker')));
    motion.add(spinRow(settings, 'drift-speed', _('Speed of Super+[ and Super+]'), 1, 20, 1, 0,
        _('Pixels a millisecond the strip slides while the key is held')));
    motion.add(spinRow(settings, 'drag-drift-speed', _('Speed of a window dragged against a side'), 1, 20, 1, 0,
        _('Pixels a millisecond the strip slides under it')));

    const map = group(page, _('Moving around'), _('While Super is held after a key that moves.'));
    map.add(percentRow(settings, 'minimap-scale', _('Size of the map of the strip'), 0, 50,
        _('In percent of the windows’ size; 0 shows no map')));
    map.add(spinRow(settings, 'minimap-shade-opacity', _('Darkening of the other windows'), 0, 255, 5, 0,
        _('0 leaves them as they are')));

    const edges = group(page, _('Screen edges'), _('The pointer pushed against the left or right side of the screen.'));
    edges.add(switchRow(settings, 'edge-preview-enable', _('Show the window beyond the edge')));
    edges.add(percentRow(settings, 'edge-preview-scale', _('Its size'), 5, 50, _('In percent of the window')));
    edges.add(switchRow(settings, 'edge-preview-click-enable', _('A click goes to it')));
    edges.add(switchRow(settings, 'edge-preview-timeout-enable', _('Staying there goes to it')));
    edges.add(spinRow(settings, 'edge-preview-timeout', _('After this many milliseconds'), 100, 5000, 50));
    edges.add(switchRow(settings, 'edge-preview-timeout-continual', _('And on to the next, while the pointer stays')));

    const touchpad = group(page, _('Touchpad'));
    touchpad.add(switchRow(settings, 'gesture-enabled', _('Swipes move the strip'),
        _('Sideways slides the strip, down opens the workspaces you were on, up opens the overview')));
    const fingers = [[0, _('None')], [3, _('Three fingers')], [4, _('Four fingers')]];
    touchpad.add(comboRow(settings, 'gesture-horizontal-fingers', _('Sliding the strip'), fingers));
    touchpad.add(comboRow(settings, 'gesture-workspace-fingers', _('Opening the workspaces'), fingers));
    touchpad.add(pairRow(settings, 'swipe-sensitivity', 0, _('How far the strip goes for the fingers’ travel'),
        0.5, 6, 0.1));
    touchpad.add(pairRow(settings, 'swipe-sensitivity', 1, _('How far the workspaces go for it'), 0.5, 6, 0.1));
    touchpad.add(pairRow(settings, 'swipe-friction', 0, _('How soon the strip stops after the fingers let go'),
        0.05, 3, 0.05, _('More stops sooner')));
    touchpad.add(pairRow(settings, 'swipe-friction', 1, _('How soon the workspaces stop after the fingers let go'),
        0.05, 3, 0.05, _('More stops sooner')));

    const bar = group(page, _('Top bar'));
    bar.add(switchRow(settings, 'show-focus-mode-icon', _('Show how the workspace scrolls to a window')));
    bar.add(switchRow(settings, 'show-open-position-icon', _('Show where new windows open')));
    bar.add(switchRow(settings, 'show-workspace-indicator', _('Show the name of the workspace'),
        _('A click opens the overview, the other button renames it, the wheel goes to the next. Each other screen shows its own at its top.')));
    bar.add(switchRow(settings, 'show-window-position-bar', _('Show where the window is in the strip'),
        _('A line along the bottom of the top bar, and at the top of each other screen')));
    bar.add(switchRow(settings, 'default-show-top-bar', _('Show the top bar on every workspace'),
        _('Super+Ctrl+B hides or shows it on one workspace')));
    bar.add(switchRow(settings, 'topbar-mouse-scroll-enable', _('The mouse wheel over it moves between windows'),
        _('The workspaces in the top bar use the wheel otherwise')));
    bar.add(percentRow(settings, 'window-switcher-preview-scale', _('Size of the windows in Alt+Tab'), 5, 50,
        _('In percent of the screen’s height')));

    const overview = group(page, _('Overview'));
    overview.add(switchRow(settings, 'only-scratch-in-overview', _('Show only the floating windows')));
    overview.add(switchRow(settings, 'disable-scratch-in-overview', _('Leave the floating windows out')));
    overview.add(comboRow(settings, 'overview-ensure-viewport-animation', _('Coming back to the focused window'), [
        [0, _('At once')], [1, _('Gliding')], [2, _('Fading in')],
    ], _('How the strip shows the window when the overview closes')));
    return page;
}

// One number of a pair kept as a list of two
function pairRow(settings, key, index, title, min, max, step, subtitle = null) {
    const row = Adw.SpinRow.new_with_range(min, max, step);
    row.title = title;
    row.digits = 2;
    if (subtitle)
        row.subtitle = subtitle;
    const pair = () => {
        const list = settings.get_value(key).deep_unpack();
        return list.length >= 2 ? [...list] : settings.get_default_value(key).deep_unpack();
    };
    const read = () => {
        if (Math.abs(row.value - pair()[index]) > 1e-6)
            row.value = pair()[index];
    };
    read();
    row.connect('notify::value', () => {
        const list = pair();
        if (Math.abs(list[index] - row.value) < 1e-6)
            return;
        list[index] = row.value;
        settings.set_value(key, new GLib.Variant('ad', list));
    });
    settings.connect(`changed::${key}`, read);
    return row;
}

// ---- Workspaces ----

// GNOME's number of workspaces, at least six (the image has six)
function workspaceCount() {
    const settings = new Gio.Settings({schema_id: 'org.gnome.desktop.wm.preferences'});
    const count = settings.get_int('num-workspaces');
    // Disposed now rather than by the collector (modules/dock.js says why)
    settings.run_dispose();
    return Math.max(6, count);
}

// One entry of a list kept by workspace number, '' for none
function setEntry(settings, key, index, value) {
    const list = settings.get_strv(key);
    while (list.length <= index)
        list.push('');
    list[index] = value;
    while (list.length && !list.at(-1))
        list.pop();
    settings.set_strv(key, list);
}

function hexOf(rgba) {
    const byte = v => Math.round(v * 255).toString(16).padStart(2, '0');
    return `#${byte(rgba.red)}${byte(rgba.green)}${byte(rgba.blue)}`;
}

// A picture chosen in GNOME's file dialog: `done` gets its address
function choosePicture(widget, title, done) {
    const filter = new Gtk.FileFilter({name: _('Pictures')});
    filter.add_mime_type('image/*');
    const filters = new Gio.ListStore({item_type: Gtk.FileFilter});
    filters.append(filter);
    const dialog = new Gtk.FileDialog({title, filters, modal: true});
    dialog.open(widget.get_root(), null, (d, result) => {
        try {
            const file = d.open_finish(result);
            if (file)
                done(file.get_uri());
        } catch {
            // Closed without a choice
        }
    });
}

// A folder chosen in GNOME's file dialog: `done` gets its path
function chooseFolder(widget, title, done) {
    const dialog = new Gtk.FileDialog({title, modal: true});
    dialog.select_folder(widget.get_root(), null, (d, result) => {
        try {
            const file = d.select_folder_finish(result);
            if (file?.get_path())
                done(file.get_path());
        } catch {
            // Closed without a choice
        }
    });
}

// The row of a picture setting (or a folder's): its file's name under the title, a button to choose one and one to
// go back to none
function pictureRow(title, read, write, noneText, choose = choosePicture) {
    const row = new Adw.ActionRow({title});
    const button = new Gtk.Button({label: _('Choose…'), valign: Gtk.Align.CENTER});
    const clear = new Gtk.Button({
        icon_name: 'edit-undo-symbolic', valign: Gtk.Align.CENTER, tooltip_text: _('Back to the default'),
        css_classes: ['flat'],
    });
    row.add_suffix(button);
    row.add_suffix(clear);
    const show = () => {
        // A path or an address, as the setting may hold either
        const uri = read();
        const file = uri && (uri.includes('://') ? Gio.File.new_for_uri(uri) : Gio.File.new_for_path(uri));
        row.subtitle = GLib.markup_escape_text(file ? file.get_basename() ?? uri : noneText, -1);
        clear.sensitive = !!uri;
    };
    button.connect('clicked', () => choose(row, title, uri => {
        write(uri);
        show();
    }));
    clear.connect('clicked', () => {
        write('');
        show();
    });
    show();
    return {row, show};
}

function workspacesPage(settings) {
    const page = new Adw.PreferencesPage({title: _('Workspaces'), icon_name: 'preferences-desktop-wallpaper-symbolic'});
    const all = group(page, _('Every workspace'));
    const fallback = pictureRow(_('Picture of every workspace'), () => settings.get_string('default-background'),
        uri => settings.set_string('default-background', uri), _('GNOME’s own wallpaper, which the theme sets'));
    all.add(fallback.row);
    settings.connect('changed::default-background', fallback.show);
    all.add(switchRow(settings, 'use-default-background', _('Without a picture, show GNOME’s wallpaper'),
        _('Off, a workspace without a picture of its own shows its colour alone')));

    const each = group(page, _('Each workspace'),
        _('A colour frames the workspace while you go through the workspaces with Super+Page Up and Page Down, and fills what its picture leaves. Without its own picture a workspace shows the one above. Apps started while a workspace is shown start in its folder.'));
    const shows = [];
    for (let i = 0; i < workspaceCount(); i++) {
        const title = _('Workspace %d').replace('%d', String(i + 1));
        const workspace = new Adw.ExpanderRow();
        const name = new Adw.EntryRow({title: _('Name'), show_apply_button: true});
        name.connect('apply', () => setEntry(settings, 'workspace-names', i, name.text.trim()));
        const showName = () => {
            const text = settings.get_strv('workspace-names')[i] ?? '';
            workspace.title = GLib.markup_escape_text(text || title, -1);
            if (name.text !== text)
                name.text = text;
        };
        workspace.add_row(name);
        const picture = pictureRow(_('Picture'), () => settings.get_strv('workspace-backgrounds')[i] ?? '',
            uri => setEntry(settings, 'workspace-backgrounds', i, uri), _('The picture of every workspace'));
        const folder = pictureRow(_('Folder'), () => settings.get_strv('workspace-directories')[i] ?? '',
            path => setEntry(settings, 'workspace-directories', i, path), _('None: apps start where they always do'),
            chooseFolder);
        const colour = new Gtk.ColorDialogButton({
            dialog: new Gtk.ColorDialog({with_alpha: false, title}), valign: Gtk.Align.CENTER,
            tooltip_text: _('Its colour'),
        });
        const noColour = new Gtk.Button({
            icon_name: 'edit-clear-symbolic', valign: Gtk.Align.CENTER, tooltip_text: _('The theme’s colour'),
            css_classes: ['flat'],
        });
        let reading = false;
        const showColour = () => {
            const hex = settings.get_strv('workspace-colors')[i] ?? '';
            const rgba = new Gdk.RGBA();
            rgba.parse(/^#[0-9a-f]{6}$/i.test(hex) ? hex : '#808080');
            reading = true;
            colour.rgba = rgba;
            reading = false;
            noColour.sensitive = !!hex;
        };
        colour.connect('notify::rgba', () => {
            if (!reading)
                setEntry(settings, 'workspace-colors', i, hexOf(colour.rgba));
            noColour.sensitive = true;
        });
        noColour.connect('clicked', () => setEntry(settings, 'workspace-colors', i, ''));
        picture.row.add_prefix(colour);
        picture.row.add_suffix(noColour);
        workspace.add_row(picture.row);
        workspace.add_row(folder.row);
        each.add(workspace);
        showColour();
        showName();
        shows.push(picture.show, showColour, folder.show, showName);
    }
    for (const key of ['workspace-colors', 'workspace-backgrounds', 'workspace-names', 'workspace-directories'])
        settings.connect(`changed::${key}`, () => shows.forEach(show => show()));
    return page;
}

// ---- Keyboard ----

// The shortcuts in the groups the page shows them in, each with what it does
function shortcutGroups() {
    const ordinals = [_('Go to the second column'), _('Go to the third column'), _('Go to the fourth column'),
        _('Go to the fifth column'), _('Go to the sixth column'), _('Go to the seventh column'),
        _('Go to the eighth column'), _('Go to the ninth column'), _('Go to the tenth column'),
        _('Go to the eleventh column')];
    return [
        [_('Moving around'), [
            ['switch-left', _('Go to the column on the left')],
            ['switch-right', _('Go to the column on the right')],
            ['switch-up', _('Go to the window above')],
            ['switch-down', _('Go to the window below')],
            ['switch-first', _('Go to the first column')],
            ['switch-last', _('Go to the last column')],
            ...ORDINALS.map((name, i) => [`switch-${name}`, ordinals[i]]),
            ['switch-next', _('Go to the next window, column after column')],
            ['switch-previous', _('Go to the previous window, column after column')],
            ['switch-next-loop', _('Go to the next window, from the last round to the first')],
            ['switch-previous-loop', _('Go to the previous window, from the first round to the last')],
            ['switch-left-loop', _('Go to the column on the left, from the first round to the last')],
            ['switch-right-loop', _('Go to the column on the right, from the last round to the first')],
            ['switch-up-loop', _('Go to the window above, from the top round to the bottom')],
            ['switch-down-loop', _('Go to the window below, from the bottom round to the top')],
            ['drift-left', _('Slide the strip to the left while the key is held')],
            ['drift-right', _('Slide the strip to the right while the key is held')],
            ['activate-window-under-cursor', _('Go to the window under the pointer')],
            ['live-alt-tab', _('Go through the windows used last, the strip showing each in its place')],
            ['live-alt-tab-backward', _('Go through the windows used last, the other way')],
            ['live-alt-tab-scratch', _('Go through the floating windows used last')],
            ['live-alt-tab-scratch-backward', _('Go through the floating windows used last, the other way')],
        ]],
        [_('Arranging'), [
            ['move-left', _('Move the column to the left')],
            ['move-right', _('Move the column to the right')],
            ['move-up', _('Move the window up its column')],
            ['move-down', _('Move the window down its column')],
            ['slurp-in', _('Take the next column’s window into this column')],
            ['barf-out', _('Give the column’s bottom window a column of its own')],
            ['barf-out-active', _('Give this window a column of its own')],
            ['take-window', _('Carry the window while Super is held')],
        ]],
        [_('Sizing'), [
            ['cycle-width', _('Next width')],
            ['cycle-width-backwards', _('Previous width')],
            ['cycle-height', _('Next height in a column')],
            ['cycle-height-backwards', _('Previous height in a column')],
            ['resize-w-inc', _('Wider by a step')],
            ['resize-w-dec', _('Narrower by a step')],
            ['resize-h-inc', _('Taller by a step')],
            ['resize-h-dec', _('Shorter by a step')],
            ['toggle-maximize-width', _('Fill the width of the screen, and back')],
            ['center-horizontally', _('Centre the window from side to side')],
            ['center-vertically', _('Centre a floating window from top to bottom')],
            ['center', _('Centre the window both ways')],
            ['paper-toggle-fullscreen', _('Make the window full screen, and back')],
        ]],
        [_('Floating windows'), [
            ['toggle-scratch', _('Float the window, or put it back in the strip')],
            ['toggle-scratch-layer', _('Hide or show the floating windows')],
            ['toggle-scratch-window', _('Hide or show the floating window used last')],
        ]],
        [_('Workspaces'), [
            ['switch-up-workspace', _('Go to the previous workspace')],
            ['switch-down-workspace', _('Go to the next workspace')],
            ['move-up-workspace', _('Take the window to the previous workspace')],
            ['move-down-workspace', _('Take the window to the next workspace')],
            ['previous-workspace', _('Go back through the workspaces you were on')],
            ['previous-workspace-backward', _('Go forward through the workspaces you were on')],
            ['move-previous-workspace', _('Take the window back through the workspaces you were on')],
            ['move-previous-workspace-backward', _('Take the window forward through the workspaces you were on')],
            ['switch-up-or-else-workspace', _('Go to the window above, or at the top to the previous workspace')],
            ['switch-down-or-else-workspace', _('Go to the window below, or at the bottom to the next workspace')],
            ['switch-up-workspace-from-all-monitors', _('Go to the previous workspace, from among all of them')],
            ['switch-down-workspace-from-all-monitors', _('Go to the next workspace, from among all of them')],
        ]],
        [_('Screens'), [
            ['switch-monitor-left', _('Go to the screen on the left')],
            ['switch-monitor-right', _('Go to the screen on the right')],
            ['switch-monitor-above', _('Go to the screen above')],
            ['switch-monitor-below', _('Go to the screen below')],
            ['switch-global-left', _('Go to the column on the left, or past the strip to the screen on the left')],
            ['switch-global-right', _('Go to the column on the right, or past the strip to the screen on the right')],
            ['switch-global-up', _('Go to the window above, or past the top to the screen above')],
            ['switch-global-down', _('Go to the window below, or past the bottom to the screen below')],
            ['move-monitor-left', _('Move the window to the screen on the left')],
            ['move-monitor-right', _('Move the window to the screen on the right')],
            ['move-monitor-above', _('Move the window to the screen above')],
            ['move-monitor-below', _('Move the window to the screen below')],
            ['swap-monitor-left', _('Swap windows with the screen on the left')],
            ['swap-monitor-right', _('Swap windows with the screen on the right')],
            ['swap-monitor-above', _('Swap windows with the screen above')],
            ['swap-monitor-below', _('Swap windows with the screen below')],
            ['move-space-monitor-left', _('Move this screen’s windows to the screen on the left')],
            ['move-space-monitor-right', _('Move this screen’s windows to the screen on the right')],
            ['move-space-monitor-above', _('Move this screen’s windows to the screen above')],
            ['move-space-monitor-below', _('Move this screen’s windows to the screen below')],
        ]],
        [_('The rest'), [
            ['new-window', _('Open a new window of the same app')],
            ['close-window', _('Close the window')],
            ['switch-focus-mode', _('Change how this workspace scrolls to a window')],
            ['switch-open-window-position', _('Change where new windows open')],
            ['open-window-position-right', _('Open new windows to the right')],
            ['open-window-position-left', _('Open new windows to the left')],
            ['open-window-position-start', _('Open new windows at the start of the strip')],
            ['open-window-position-end', _('Open new windows at the end of the strip')],
            ['open-window-position-down', _('Open new windows below the window you are on')],
            ['open-window-position-up', _('Open new windows above the window you are on')],
            ['toggle-top-and-position-bar', _('Hide or show the top bar on this workspace')],
            ['toggle-top-bar', _('Hide or show the top bar alone on this workspace')],
            ['toggle-position-bar', _('Hide or show where the window is in the strip, on this workspace')],
        ]],
    ];
}

// A combination as one comparable string, whatever way it is written; null for none
function comboOf(accelerator) {
    const [ok, key, mods] = Gtk.accelerator_parse(accelerator);
    if (!ok || !key)
        return null;
    return `${mods & Gtk.accelerator_get_default_mod_mask()}:${Gdk.keyval_to_lower(key)}`;
}

function labelOf(accelerators) {
    const labels = accelerators.map(accelerator => {
        const [ok, key, mods] = Gtk.accelerator_parse(accelerator);
        return ok && key ? Gtk.accelerator_get_label(key, mods) : accelerator;
    });
    return labels.length ? labels.join(', ') : _('None');
}

function keyboardPage(window, keys) {
    const page = new Adw.PreferencesPage({title: _('Keyboard'), icon_name: 'input-keyboard-symbolic'});
    const known = new Set(keys.settings_schema.list_keys());
    const rows = new Map();
    const intro = group(page, _('Shortcuts'),
        _('Click a shortcut to change it. GNOME’s own shortcut on the same keys gives way while the tiling runs, and comes back when it is off.'));
    const resetAll = new Gtk.Button({label: _('Reset all'), valign: Gtk.Align.CENTER, css_classes: ['flat']});
    resetAll.connect('clicked', () => known.forEach(name => keys.reset(name)));
    intro.header_suffix = resetAll;

    for (const [title, list] of shortcutGroups()) {
        const g = group(page, title);
        for (const [name, label] of list) {
            if (!known.has(name))
                continue;
            const row = new Adw.ActionRow({title: label, activatable: true});
            const shown = new Gtk.Label({css_classes: ['dim-label'], valign: Gtk.Align.CENTER});
            row.add_suffix(shown);
            const reset = resetButton(keys, [name]);
            row.add_suffix(reset);
            row.connect('activated', () => captureShortcut(window, keys, name, label));
            g.add(row);
            rows.set(name, {row, shown, reset});
        }
    }

    // What each row shows, and the rows whose keys another shortcut of the tiling uses as well
    const sync = () => {
        const users = new Map();
        for (const name of rows.keys()) {
            for (const accelerator of keys.get_strv(name)) {
                const combo = comboOf(accelerator);
                if (combo)
                    users.set(combo, [...users.get(combo) ?? [], name]);
            }
        }
        for (const [name, {row, shown, reset}] of rows) {
            const accelerators = keys.get_strv(name);
            shown.label = labelOf(accelerators);
            reset.sensitive = keys.get_user_value(name) !== null;
            const clash = accelerators.map(comboOf).filter(Boolean).flatMap(c => users.get(c).filter(n => n !== name));
            if (clash.length) {
                const others = [...new Set(clash)].map(n => rows.get(n).row.title).join(', ');
                row.subtitle = GLib.markup_escape_text(_('Also used by: %s').replace('%s', others), -1);
                row.add_css_class('error');
            } else {
                row.subtitle = '';
                row.remove_css_class('error');
            }
        }
    };
    sync();
    keys.connect('changed', sync);
    return page;
}

// Asks for the new keys of one shortcut: pressed with their modifiers, Backspace for none, Escape to leave it
function captureShortcut(window, keys, name, title) {
    const dialog = new Adw.AlertDialog({
        heading: title,
        body: _('Press the new keys. Backspace leaves the shortcut without any, Escape keeps it as it is.'),
    });
    dialog.add_response('cancel', _('Cancel'));
    dialog.set_close_response('cancel');
    const controller = new Gtk.EventControllerKey();
    controller.connect('key-pressed', (c, keyval, keycode, state) => {
        const mods = state & Gtk.accelerator_get_default_mod_mask();
        const key = Gdk.keyval_to_lower(keyval);
        if (!mods && key === Gdk.KEY_Escape) {
            dialog.close();
            return Gdk.EVENT_STOP;
        }
        if (!mods && key === Gdk.KEY_BackSpace) {
            keys.set_strv(name, []);
            dialog.close();
            return Gdk.EVENT_STOP;
        }
        // A modifier on its own is the start of a combination
        if (Gtk.accelerator_valid(key, mods) && mods) {
            keys.set_strv(name, [Gtk.accelerator_name(key, mods)]);
            dialog.close();
        }
        return Gdk.EVENT_STOP;
    });
    dialog.add_controller(controller);
    // The keys the desktop uses (Super and the arrows, the tiling's own) reach this window while it asks
    const surface = window.get_surface();
    surface?.inhibit_system_shortcuts?.(null);
    dialog.connect('closed', () => surface?.restore_system_shortcuts?.());
    dialog.present(window);
}

// ---- Window rules ----

function rulesPage(settings) {
    const page = new Adw.PreferencesPage({title: _('Window rules'), icon_name: 'view-list-symbolic'});
    const list = group(page, _('Window rules'),
        _('A rule picks windows by their class or their title as they open, and says what to do with them. A pattern is written /like this/, and * is any window no other rule picks.'));
    const add = new Gtk.Button({
        icon_name: 'list-add-symbolic', valign: Gtk.Align.CENTER, tooltip_text: _('Add a rule'), css_classes: ['flat'],
    });
    list.header_suffix = add;
    const rows = [];
    let writing = false;

    const read = () => settings.get_strv('winprops').map(text => {
        try {
            const rule = JSON.parse(text);
            return rule && typeof rule === 'object' && !Array.isArray(rule) ? rule : {};
        } catch {
            return {};
        }
    });
    let rules = read();
    const write = () => {
        writing = true;
        settings.set_strv('winprops', rules.map(rule => JSON.stringify(rule)));
        writing = false;
    };
    const build = () => {
        for (const row of rows.splice(0))
            list.remove(row);
        rules.forEach((rule, index) => {
            const row = ruleRow(rule, () => write(), () => {
                rules.splice(index, 1);
                write();
                build();
            });
            list.add(row);
            rows.push(row);
        });
    };
    add.connect('clicked', () => {
        rules.push({wm_class: ''});
        write();
        build();
        rows.at(-1).expanded = true;
    });
    settings.connect('changed::winprops', () => {
        if (writing)
            return;
        rules = read();
        build();
    });
    build();
    return page;
}

// One rule: what it picks windows by, and what it does with them. Fields the window has no row for are kept.
function ruleRow(rule, changed, remove) {
    const row = new Adw.ExpanderRow();
    // Named by what it picks windows by, its class above its title
    const describe = () => {
        row.title = GLib.markup_escape_text(rule.wm_class || rule.title || _('A new rule'), -1);
        row.subtitle = rule.wm_class && rule.title ? GLib.markup_escape_text(rule.title, -1) : '';
    };
    const entry = (field, title) => {
        const e = new Adw.EntryRow({title, text: typeof rule[field] === 'string' ? rule[field] : ''});
        e.connect('changed', () => {
            if (e.text)
                rule[field] = e.text;
            else
                delete rule[field];
            describe();
            changed();
        });
        return e;
    };
    const toggle = (field, title, subtitle = null) => {
        const s = new Adw.SwitchRow({title, active: !!rule[field]});
        if (subtitle)
            s.subtitle = subtitle;
        s.connect('notify::active', () => {
            if (s.active)
                rule[field] = true;
            else
                delete rule[field];
            describe();
            changed();
        });
        return s;
    };
    row.add_row(entry('wm_class', _('Window class')));
    row.add_row(entry('title', _('Title')));
    row.add_row(toggle('scratch_layer', _('Float above the strip')));
    const width = entry('preferredWidth', _('Width, as 50% or 800px'));
    row.add_row(width);
    const workspace = Adw.SpinRow.new_with_range(0, 36, 1);
    workspace.title = _('Workspace to open on');
    workspace.subtitle = _('0 opens it where you are');
    workspace.value = Number.isInteger(rule.spaceIndex) ? rule.spaceIndex + 1 : 0;
    workspace.connect('notify::value', () => {
        if (workspace.value > 0)
            rule.spaceIndex = Math.round(workspace.value) - 1;
        else
            delete rule.spaceIndex;
        describe();
        changed();
    });
    row.add_row(workspace);
    row.add_row(toggle('focus', _('Go to it when it opens on another workspace')));
    row.add_row(toggle('oneshot', _('Only once'), _('The rule is forgotten once a window has opened under it')));
    const removeRow = new Adw.ButtonRow({title: _('Remove this rule'), css_classes: ['destructive-action']});
    removeRow.connect('activated', remove);
    row.add_row(removeRow);
    describe();
    return row;
}
