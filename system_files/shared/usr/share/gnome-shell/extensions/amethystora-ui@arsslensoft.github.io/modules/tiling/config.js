// The tiling's settings, as the rest of it reads them: one object of plain values, read from the schema when the
// tiling starts and again whenever a key changes, so that a gap or a speed changed in the preferences window
// applies at once. A value out of shape (a list of steps written by hand) falls back to the image's.

import * as Layout from './layout.js';

// What the strip is laid out and moved with; the image's values until the schema has been read
export const config = {
    animation: 250,
    border: 8, radiusTop: 12, radiusBottom: 0,
    fillShare: 1, maximizeFills: true,
    defaultMode: 0, openPosition: 0, openOffered: [0, 1, 4],
    driftSpeed: 2, dragDriftSpeed: 2,
    mapScale: 0.15, mapShade: 160,
    edgePreview: true, edgeScale: 0.15, edgeClick: true, edgeHover: false, edgeHoverTime: 800, edgeHoverRepeat: false,
    gestures: true, stripFingers: 3, pileFingers: 3, swipeSpeed: [2, 2], swipeFriction: [0.3, 0.1],
    modeIcon: true, openIcon: true, topBarDefault: true, barScroll: false, indicator: false, positionBar: false,
    switcherShare: 0.15, overviewFloating: 'both', overviewExit: 1,
    workspaceColors: [], workspaceBackgrounds: [], defaultBackground: '', gnomeBackground: true,
    workspaceNames: [], workspaceFolders: [],
    rules: [],
};

// The open positions in the order the key goes through them, by the key that offers each
const OFFERED = [['right', 0], ['left', 1], ['start', 2], ['end', 3], ['down', 4], ['up', 5]];

const numbers = (list, fallback, length = 0) =>
    list.length >= Math.max(1, length) && list.every(n => Number.isFinite(n) && n > 0) ? list : fallback;

export class Config {
    constructor(settings) {
        this._settings = settings;
    }

    // `changed(key)` is told of each key changed after the first reading
    enable(changed) {
        this._read();
        this._settings.connectObject('changed', (s, key) => {
            this._read();
            changed(key);
        }, this);
    }

    disable() {
        this._settings.disconnectObject(this);
    }

    _read() {
        const s = this._settings;
        const int = key => s.get_int(key);
        const bool = key => s.get_boolean(key);
        const double = key => s.get_double(key);
        const list = key => s.get_value(key).deep_unpack();
        Layout.configure({
            gap: int('window-gap'),
            margin: int('horizontal-margin'),
            marginTop: int('vertical-margin'),
            marginBottom: int('vertical-margin-bottom'),
            widthSteps: numbers(list('cycle-width-steps'), Layout.DEFAULT_STEPS),
            heightSteps: numbers(list('cycle-height-steps'), Layout.DEFAULT_STEPS),
        });
        Object.assign(config, {
            animation: Math.round(double('animation-time') * 1000),
            border: Math.min(int('selection-border-size'), Layout.MARGIN_TOP, Layout.GAP),
            radiusTop: int('selection-border-radius-top'),
            radiusBottom: int('selection-border-radius-bottom'),
            fillShare: double('maximize-width-percent'),
            maximizeFills: bool('maximize-within-tiling'),
            defaultMode: int('default-focus-mode'),
            openPosition: int('open-window-position'),
            openOffered: OFFERED.filter(([name]) => bool(`open-window-position-option-${name}`)).map(([, v]) => v),
            driftSpeed: int('drift-speed'),
            dragDriftSpeed: int('drag-drift-speed'),
            mapScale: double('minimap-scale'),
            mapShade: int('minimap-shade-opacity'),
            edgePreview: bool('edge-preview-enable'),
            edgeScale: double('edge-preview-scale'),
            edgeClick: bool('edge-preview-click-enable'),
            edgeHover: bool('edge-preview-timeout-enable'),
            edgeHoverTime: int('edge-preview-timeout'),
            edgeHoverRepeat: bool('edge-preview-timeout-continual'),
            gestures: bool('gesture-enabled'),
            stripFingers: int('gesture-horizontal-fingers'),
            pileFingers: int('gesture-workspace-fingers'),
            swipeSpeed: numbers(list('swipe-sensitivity'), [2, 2], 2),
            swipeFriction: numbers(list('swipe-friction'), [0.3, 0.1], 2),
            modeIcon: bool('show-focus-mode-icon'),
            openIcon: bool('show-open-position-icon'),
            topBarDefault: bool('default-show-top-bar'),
            barScroll: bool('topbar-mouse-scroll-enable'),
            indicator: bool('show-workspace-indicator'),
            positionBar: bool('show-window-position-bar'),
            switcherShare: double('window-switcher-preview-scale'),
            // Which windows the overview shows: 'only' the floating ones, 'none' of them, or 'both' kinds
            overviewFloating: bool('only-scratch-in-overview') ? 'only'
                : bool('disable-scratch-in-overview') ? 'none' : 'both',
            // Leaving the overview, the strip comes to the focused window at once (0), gliding (1) or fading in (2)
            overviewExit: int('overview-ensure-viewport-animation'),
            // A colour that is not #rrggbb counts as none, the theme's
            workspaceColors: s.get_strv('workspace-colors').map(c => /^#[0-9a-f]{6}$/i.test(c) ? c : ''),
            workspaceBackgrounds: s.get_strv('workspace-backgrounds'),
            defaultBackground: s.get_string('default-background'),
            gnomeBackground: bool('use-default-background'),
            workspaceNames: s.get_strv('workspace-names'),
            workspaceFolders: s.get_strv('workspace-directories'),
            rules: rules(s.get_strv('winprops')),
        });
    }

    set(key, value) {
        this._settings.set_value(key, value);
    }

    get settings() {
        return this._settings;
    }
}

// ---- Window rules ----

// A rule's test for a class or a title: the exact text, or "/a pattern/flags"; "*" matches what no other rule does
function matcher(text) {
    if (typeof text !== 'string' || !text)
        return null;
    if (text === '*')
        return {any: true, test: () => true};
    const pattern = /^\/(.*)\/([a-z]*)$/s.exec(text);
    if (pattern) {
        try {
            const expression = new RegExp(pattern[1], pattern[2].replace(/[gy]/g, ''));
            return {any: false, test: value => expression.test(value)};
        } catch {
            return null;
        }
    }
    return {any: false, test: value => value === text};
}

// The rules a setting holds, the ones that test a title first; a rule that cannot be read is left out, and said once
function rules(texts) {
    const out = [];
    texts.forEach((text, index) => {
        let rule;
        try {
            rule = JSON.parse(text);
        } catch {
            console.warn(`Amethystora UI: window rule ${index + 1} is not JSON, and is left out`);
            return;
        }
        if (!rule || typeof rule !== 'object' || Array.isArray(rule))
            return;
        const wmClass = matcher(rule.wm_class);
        const title = matcher(rule.title);
        // One that says what it picks but cannot be read picks nothing, rather than everything
        if ((!wmClass && rule.wm_class) || (!title && rule.title) || (!wmClass && !title))
            return;
        out.push({index, text, wmClass, title, rule});
    });
    return out.sort((a, b) => (a.title ? 0 : 1) - (b.title ? 0 : 1) || a.index - b.index);
}

// GNOME's extension preferences and its screenshot tool float, and take the keyboard when they open; so does Notes'
// quick note (amethystora-notes capture), which is Notes' one window that is not its application window: GTK gives only
// an application window an object path (`plain`)
const BUILT_IN = [
    {wmClass: matcher('Gnome-shell-extension-prefs'), title: null, rule: {scratch_layer: true, focus: true}},
    {wmClass: matcher('/gnome-screenshot/i'), title: null, rule: {scratch_layer: true, focus: true}},
    {wmClass: matcher('org.amethystora.Notes'), title: null, plain: true, rule: {scratch_layer: true, focus: true}},
];

// The rule a window opens under: the settings' first, then the built-in ones
export function ruleFor(window) {
    const wmClass = window.get_wm_class() ?? '';
    const title = window.get_title() ?? '';
    const catchAll = entry => entry.wmClass?.any || entry.title?.any;
    const order = [...config.rules.filter(e => !catchAll(e)), ...BUILT_IN, ...config.rules.filter(catchAll)];
    for (const entry of order) {
        if (entry.wmClass && !entry.wmClass.test(wmClass))
            continue;
        if (entry.title && !entry.title.test(title))
            continue;
        if (entry.plain && window.get_gtk_window_object_path?.())
            continue;
        return entry;
    }
    return null;
}

// A rule marked oneshot has done its work: it leaves the setting
export function forget(entry, settings) {
    if (!entry?.rule?.oneshot || entry.index === undefined)
        return;
    const texts = settings.get_strv('winprops');
    if (texts[entry.index] === entry.text)
        settings.set_strv('winprops', texts.filter((t, i) => i !== entry.index));
}

// A preferred width as pixels: "50%" of what two windows share, or "800px"
export function preferredWidth(rule, available) {
    const value = String(rule?.preferredWidth ?? '').trim();
    const match = /^(\d+(?:\.\d+)?)\s*(%|px)$/.exec(value);
    if (!match)
        return 0;
    const number = Number(match[1]);
    return match[2] === '%' ? Math.round(available * Math.min(100, number) / 100) : Math.round(number);
}
