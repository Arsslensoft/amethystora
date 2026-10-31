// The search box, the Card. Super+Space opens GNOME's own search in a card near the top of the main screen, over
// whatever is open, without going to the overview. The card holds the overview's own search field and results, lent to
// it while it is open and given back when it closes, so that every search provider (apps, files, settings, the
// calculator, the software centre...) is there, beside Amethystora's own (search/providers.js: the clipboard with
// clip:, sums and units, Control's settings, open windows, the manual and the agent row), which the overview shows too.
// It starts as a single line and grows downwards as results come, its field never moving; Enter opens the first result,
// Ctrl+1 to Ctrl+9 the one with that number, and Space on a file found by Files shows it without opening it (Glance,
// search/glance.js). Escape, the same key again, a click anywhere else, opening a result, a window going full screen or
// the screens changing closes the card, and the Super key brings the typing back to the field instead of opening the
// overview. While lent, GNOME's own sections wear the card's look (one column, a title over the results) and get it
// back with the search. Its glass is the glass module's, its colours the theme's (shell-search.css).

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import Graphene from 'gi://Graphene';
import Meta from 'gi://Meta';
import Pango from 'gi://Pango';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as AppDisplay from 'resource:///org/gnome/shell/ui/appDisplay.js';
import * as BoxPointer from 'resource:///org/gnome/shell/ui/boxpointer.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as Overview from 'resource:///org/gnome/shell/ui/overview.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import * as GnomeSearch from 'resource:///org/gnome/shell/ui/search.js';
import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {Providers, resultsOf} from './search/providers.js';
import {Section} from './search/section.js';
import {Glance, fileOf} from './search/glance.js';
import {cancelPastes, giveBackTextInput} from './emoji/paste.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.search';

// The card: its width, its top edge as a share of the work area's height, the most of the work area's height it takes,
// and the room it leaves at the sides of a narrow screen (logical pixels)
const WIDTH = 640;
const TOP = 0.11;
const TALLEST = 0.70;
const MARGIN = 16;
// Milliseconds for the card to appear or go, and to step aside for Glance; GNOME's animation settings, and the
// tweaks, shorten them
const DURATION = 100;
// The size it grows from and shrinks to
const SMALL = 0.9;
// Results that have a Ctrl key
const NUMBERED = 9;
// The size of a result's picture in the card
const PICTURE = 28;

// The keys that open the result in focus, and the Super keys
const OPENING_KEYS = [Clutter.KEY_Return, Clutter.KEY_KP_Enter, Clutter.KEY_ISO_Enter, Clutter.KEY_space,
    Clutter.KEY_KP_Space];
const SUPER_KEYS = [Clutter.KEY_Super_L, Clutter.KEY_Super_R];
// Moving between the results and opening one: St's focus manager and the results take these after the box has seen
// them, so the box lets them through
const RESULT_KEYS = [...OPENING_KEYS, Clutter.KEY_Up, Clutter.KEY_Down, Clutter.KEY_Left, Clutter.KEY_Right,
    Clutter.KEY_Tab, Clutter.KEY_ISO_Left_Tab, Clutter.KEY_Page_Up, Clutter.KEY_Page_Down, Clutter.KEY_Home,
    Clutter.KEY_End, Clutter.KEY_Menu];
const KEY_EVENTS = [Clutter.EventType.KEY_PRESS, Clutter.EventType.KEY_RELEASE];
const POINTER_EVENTS = [Clutter.EventType.BUTTON_PRESS, Clutter.EventType.BUTTON_RELEASE,
    Clutter.EventType.TOUCH_BEGIN, Clutter.EventType.TOUCH_END];
// The keys 1 to 9 by where they are on the keyboard (evdev's codes plus 8), for layouts that need Shift for digits
const DIGIT_CODES = [10, 11, 12, 13, 14, 15, 16, 17, 18];
const MODIFIERS = Clutter.ModifierType.CONTROL_MASK | Clutter.ModifierType.MOD1_MASK |
    Clutter.ModifierType.SUPER_MASK | Clutter.ModifierType.MOD4_MASK;

// The session's language as a BCP 47 tag, for upper-casing titles in its own way
function sessionLocale() {
    const name = GLib.get_language_names_with_category('LC_MESSAGES')[0] ?? 'C';
    const tag = name.split(/[.@]/)[0].replace('_', '-');
    return tag === 'C' || tag === 'POSIX' ? 'en' : tag;
}

export class Search {
    constructor(ext) {
        this._ext = ext;
    }

    // For the tiling, which leaves the keyboard here while the pointer crosses the screens
    get open() {
        return !!this._open;
    }

    // Whether the providers could not be shown in sections of their own (the suite fails on it)
    get degraded() {
        return this._providers?.degraded ?? false;
    }

    // The window being typed in when the card opened, where a pasted item goes; the focused one when it is closed
    get typingWindow() {
        return this._open || this._lent ? this._typingWindow : global.display.focus_window;
    }

    enable() {
        this._open = false;
        this._glancing = false;
        // The signals that last only while the box is open are kept under this, and those that outlast it a little
        // under the other
        this._whileOpen = {};
        this._afterClose = {};
        this._dressed = new Map();
        this._icons = new Map();
        this._stretched = new Map();
        this._stretchedIds = new Map();

        this._boxGone = false;
        this._box = new St.BoxLayout({
            style_class: 'amethystora-search-box',
            orientation: Clutter.Orientation.VERTICAL,
            // A click on the box's own edge stays in the box rather than reaching the window under it
            reactive: true,
            visible: false,
            pivot_point: new Graphene.Point({x: 0.5, y: 0}),
        });
        this._box.connectObject('key-press-event', (actor, event) => this._onKeyPress(event),
            // GNOME Shell ending destroys its chrome without turning the extensions off: the glass is not given it after
            'destroy', () => (this._boxGone = true), this);
        // Ctrl+1 to Ctrl+9 and Glance's Space are seen before the field and the results see them
        this._keys = new Clutter.KeyController();
        this._keys.connectObject('key-press', controller => {
            const [, keyval, keycode] = controller.get_key();
            const [, state] = controller.get_state();
            return this._onCapturedKey(keyval, keycode, state);
        }, this);
        this._box.add_action_full('amethystora-search-keys', Clutter.EventPhase.CAPTURE, this._keys);
        // Over the windows, under their menus, the on-screen keyboard and the screenshot tool
        Main.layoutManager.addChrome(this._box);

        // The overview gets its search back before it lays it out
        Main.overview.connectObject('showing', () => this._giveBack(), this);

        // While the box is open, opening a result asks the overview to go (it is not there); the Super key asks it to
        // come. Both are answered here instead. A result opened by its Ctrl key on another workspace has closed the box
        // by the time GNOME's result asks, and still brings no overview
        const proto = Overview.Overview.prototype;
        const {toggle, hide} = proto;
        this._overviewMethods = {toggle, hide};
        const search = this;
        proto.toggle = function (...args) {
            if ((search._open || search._activating) && search._answer(true))
                return undefined;
            return toggle.apply(this, args);
        };
        proto.hide = function (...args) {
            if (search._open)
                search._answer(false);
            return hide.apply(this, args);
        };

        this._settings = this._ext.getSettings(SCHEMA);
        const action = Main.wm.addKeybinding('open', this._settings, Meta.KeyBindingFlags.IGNORE_AUTOREPEAT,
            Shell.ActionMode.NORMAL | Shell.ActionMode.POPUP, () => this._toggle());
        if (action === Meta.KeyBindingAction.NONE)
            console.warn('Amethystora UI: the search box key could not be set: another one has its name');

        // Amethystora's own providers, in the overview's search and so in the card
        this._providers = new Providers(this._ext, this);
        this._providers.enable(this._settings);
        this._settings.connectObject('changed::glance', () => {
            if (!this._settings.get_boolean('glance')) {
                this._glance?.destroy();
                this._glance = null;
            }
            this._syncFocusHint();
        }, this);

        // GNOME's view says when the results of a provider are in; the card numbers them then
        const view = Main.overview.searchController?._searchResults;
        if (view && typeof view._maybeSetInitialSelection === 'function') {
            this._view = view;
            const select = view._maybeSetInitialSelection;
            view._maybeSetInitialSelection = function (...args) {
                const result = select.apply(this, args);
                search._queueRefresh();
                return result;
            };
        }
    }

    disable() {
        if (this._settings)
            Main.wm.removeKeybinding('open');
        this._settings?.disconnectObject(this);

        this._close('now');
        if (this._afterClose)
            global.stage.disconnectObject(this._afterClose);
        this._glance?.destroy();
        this._glance = null;
        cancelPastes();

        if (this._view)
            delete this._view._maybeSetInitialSelection;
        this._view = null;
        this._providers?.disable();
        this._providers = null;
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;

        if (this._overviewMethods)
            Object.assign(Overview.Overview.prototype, this._overviewMethods);
        this._overviewMethods = null;
        Main.overview.disconnectObject(this);

        this._removeLater('_refreshLater');
        this._removeLater('_focusLater');
        this._removeLater('_giveBackLater');
        this._keys?.disconnectObject(this);
        this._keys = null;
        this._spaceHint?.destroy();
        this._spaceHint = null;
        this._box?.destroy();
        this._box = null;
    }

    // ---- For the glass ----

    glassSurfaces() {
        const surfaces = [];
        if (this._box && !this._boxGone)
            surfaces.push({actor: this._box, material: 'glass', radius: null});
        if (this._glance?.pill)
            surfaces.push({actor: this._glance.pill, material: 'glass', radius: null});
        return surfaces;
    }

    glassChanged() {
        this._ext.modules?.get('glass')?.surfacesChanged?.();
    }

    // ---- Opening and closing ----

    _toggle() {
        if (this._open)
            this._close('animate');
        else
            this._show();
    }

    get _reduceMotion() {
        return this._ext.modules?.get('comfort')?.reduceMotion ?? false;
    }

    _show() {
        // The overview has its own search
        if (Main.overview.visible || Main.overview.animationInProgress)
            return;
        // A menu of the top bar open (the key works over one too): it gives way, with the keyboard it holds
        Main.panel.menuManager.activeMenu?.close(BoxPointer.PopupAnimation.NONE);
        // Any other grab (the screenshot tool, a dock's menu) would keep the keys from the box, which is not modal
        if (Main.modalCount > 0)
            return;
        // Still fading from the last time
        this._giveBack();
        global.stage.disconnectObject(this._afterClose);

        // The overview's field sits in a holder of its own; the holder and the results are side by side in the
        // overview's controls
        const entry = Main.overview.searchEntry;
        const results = Main.overview.searchController;
        const bin = entry?.get_parent();
        const controls = bin?.get_parent();
        const monitor = Main.layoutManager.primaryMonitor;
        if (!monitor || !(entry instanceof St.Entry) || !(bin instanceof St.Bin) || !controls ||
            results?.get_parent() !== controls) {
            console.warn('Amethystora UI: the overview\'s search was not found, the search box stays closed');
            return;
        }

        this._typingWindow = global.display.focus_window;
        const children = controls.get_children();
        this._lent = {
            controls, bin, results, entry,
            places: [[bin, children.indexOf(bin)], [results, children.indexOf(results)]].sort((a, b) => a[1] - b[1]),
            align: bin.x_align,
            expand: entry.x_expand,
        };
        controls.remove_child(bin);
        controls.remove_child(results);
        // The field takes the box's whole width (the overview centres it at its own)
        bin.x_align = Clutter.ActorAlign.FILL;
        entry.x_expand = true;
        this._box.add_child(bin);
        this._box.add_child(results);

        // Worked out again at every opening, on the screen the pointer is on, as it is now: its top edge never moves, and
        // it grows downwards as far as 70% of the work area, where the results scroll
        const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        const area = Main.layoutManager.getWorkAreaForMonitor(global.display.get_current_monitor());
        const width = Math.min(WIDTH * scale, area.width - 2 * MARGIN * scale);
        this._box.set_style(`max-height: ${Math.floor(TALLEST * area.height / scale)}px;`);
        this._box.set_size(width, -1);
        this._box.set_position(area.x + Math.round((area.width - width) / 2), area.y + Math.round(TOP * area.height));

        this._open = true;
        // Drawn over a full-screen game or video too
        global.compositor.disable_unredirect();
        this._providers.prepare();

        global.stage.connectObject('notify::key-focus', () => {
            this._queueFocusCheck();
            this._syncFocusHint();
        }, this._whileOpen);
        // Not for the previewer of a Glance: the first window it opens in a session makes this fire, and took the search
        // and the preview with it. The box has stepped aside, and comes back as the preview closes
        global.display.connectObject('in-fullscreen-changed', () => {
            if (!this._glancing)
                this._close('animate');
        }, this._whileOpen);
        Main.layoutManager.connectObject('monitors-changed', () => this._close('now'), this._whileOpen);
        // An app dragged from the results (to the dock, say) keeps the box open until it is dropped
        Main.overview.connectObject(
            'item-drag-begin', () => (this._dragging = true),
            'item-drag-end', () => {
                this._dragging = false;
                this._queueFocusCheck();
            }, this._whileOpen);
        this._dress();

        this._box.remove_all_transitions();
        this._box.opacity = 0;
        const small = this._reduceMotion ? 1 : SMALL;
        this._box.set_scale(small, small);
        this._box.show();
        this._box.ease({
            opacity: 255,
            scale_x: 1,
            scale_y: 1,
            duration: DURATION,
            mode: Clutter.AnimationMode.EASE_OUT_QUAD,
        });
        entry.grab_key_focus();
    }

    // How it closes: 'animate' fades it out; 'quick' makes it clear at once and gives the search back before the next
    // frame, once the result that was opened (or its menu) has finished: hiding the box would empty the results
    // under it, since GNOME empties its search when it stops being shown; 'now' gives it back at once
    _close(how) {
        this._stop();
        if (!this._lent)
            return;

        if (how === 'animate') {
            const small = this._reduceMotion ? 1 : SMALL;
            this._box.ease({
                opacity: 0,
                scale_x: small,
                scale_y: small,
                duration: DURATION,
                mode: Clutter.AnimationMode.EASE_OUT_QUAD,
                onComplete: () => {
                    if (!this._open)
                        this._giveBack();
                },
            });
        } else if (how === 'quick') {
            this._box.remove_all_transitions();
            this._box.opacity = 0;
            this._box.reactive = false;
            this._queueLater('_giveBackLater', () => this._giveBack());
        } else {
            this._giveBack();
        }
    }

    // The box stops listening and lets go of the keyboard; what it shows stays until it is given back
    _stop() {
        if (!this._open)
            return;
        this._open = false;
        this._dragging = false;
        if (this._glancing)
            this._glance?.destroy();
        this._glancing = false;
        for (const source of [global.stage, global.display, Main.layoutManager, Main.overview])
            source?.disconnectObject(this._whileOpen);
        this._removeLater('_focusLater');
        global.compositor.enable_unredirect();

        // The keyboard goes back to the windows before anything fades: nothing that cannot be seen holds it
        const focus = global.stage.get_key_focus();
        if (focus && focus !== global.stage && this._box.contains(focus)) {
            global.stage.set_key_focus(null);
            giveBackTextInput();
        }
    }

    // The field and the results go back to the overview, where they were, in the overview's own look
    _giveBack() {
        this._removeLater('_giveBackLater');
        this._stop();
        const lent = this._lent;
        if (!lent)
            return;
        this._lent = null;
        this._glance?.destroy();
        this._glance = null;
        this._removeLater('_refreshLater');

        this._box.remove_all_transitions();
        this._keepingFocus(() => {
            this._box.hide();
            this._box.remove_child(lent.bin);
            this._box.remove_child(lent.results);
        });
        this._box.opacity = 255;
        this._box.reactive = true;
        this._box.set_scale(1, 1);
        lent.bin.x_align = lent.align;
        lent.entry.x_expand = lent.expand;
        this._undress();
        for (const [actor, index] of lent.places)
            lent.controls.insert_child_at_index(actor, index);
        // GNOME fades its results out when the search empties and hides them once the fade ends, which taken out of
        // the overview it never does: left half seen, its "No results" stood over the app grid
        if (lent.results.searchActive === false) {
            lent.results.remove_all_transitions();
            lent.results.opacity = 0;
            lent.results.hide();
        }
        this._guardReturn(lent);
    }

    // Hiding GNOME's results empties the search, and GNOME then lets go of the keyboard: whatever had it outside the
    // box (a menu of the top bar, say) gets it back
    _keepingFocus(change) {
        const focus = global.stage.get_key_focus();
        const keep = focus && focus !== global.stage && !this._box.contains(focus) ? focus : null;
        change();
        if (keep?.get_stage() && global.stage.get_key_focus() !== keep)
            keep.grab_key_focus();
    }

    // A menu that opened while the field had the keyboard gives it back to the field when it closes, which may be
    // after the field is back in the hidden overview: the keyboard goes back to the windows instead, now or when
    // the menu closes
    _guardReturn(lent) {
        global.stage.disconnectObject(this._afterClose);
        const home = [lent.bin, lent.results];
        const away = () => {
            const focus = global.stage.get_key_focus();
            if (focus && !this._open && !Main.overview.visible && home.some(actor => actor.contains(focus)))
                global.stage.set_key_focus(null);
        };
        away();
        if (Main.modalCount === 0)
            return;
        global.stage.connectObject('notify::key-focus', () => {
            away();
            if (Main.modalCount === 0)
                global.stage.disconnectObject(this._afterClose);
        }, this._afterClose);
    }

    // The overview was asked to come or to go while the box is open. Returns whether the call is answered here.
    _answer(toggling) {
        const event = Clutter.get_current_event();
        const isKey = event ? KEY_EVENTS.includes(event.type()) : false;

        // The Super key: the typing goes back to the field, and the box stays
        if (toggling && isKey && !this._activating && SUPER_KEYS.includes(event.get_key_symbol())) {
            this._lent.entry.grab_key_focus();
            return true;
        }

        // A result was opened from the box (or from its menu, or by its Ctrl key): the box is gone at once, so that
        // the app appears without it, and no overview comes. Anything else that wants the overview gets it, without
        // the box, which gives the search back as the overview starts to show. The search itself is given back only
        // once the result has finished, since giving it back empties the results.
        const source = event ? global.stage.get_event_actor(event) : null;
        const fromBox = this._activating || (source !== null && this._box.contains(source) &&
            (isKey ? OPENING_KEYS.includes(event.get_key_symbol()) : POINTER_EVENTS.includes(event.type())));
        this._close('quick');
        return toggling && fromBox;
    }

    // ---- Keys ----

    _onKeyPress(event) {
        if (!this._open)
            return Clutter.EVENT_PROPAGATE;

        const symbol = event.get_key_symbol();
        // The field handles Escape itself (it empties and lets go, which closes the box); here a result has the focus
        if (symbol === Clutter.KEY_Escape) {
            this._close('animate');
            return Clutter.EVENT_STOP;
        }

        // Any other key that a result had no use for (a letter, a modifier, a shortcut) sends the keyboard back to the
        // field, and goes no further
        const text = this._lent.entry.clutter_text;
        if (global.stage.get_key_focus() === text || RESULT_KEYS.includes(symbol))
            return Clutter.EVENT_PROPAGATE;
        text.grab_key_focus();
        return Clutter.EVENT_STOP;
    }

    // Before the field or a result sees it: Ctrl and a digit opens that result, Space on a file shows it
    _onCapturedKey(keyval, keycode, state) {
        if (!this._open || this._glancing)
            return Clutter.EVENT_PROPAGATE;
        const modifiers = state & MODIFIERS;
        if (modifiers === Clutter.ModifierType.CONTROL_MASK) {
            let digit = keyval >= Clutter.KEY_1 && keyval <= Clutter.KEY_9 ? keyval - Clutter.KEY_1 + 1 : 0;
            if (!digit && keyval >= Clutter.KEY_KP_1 && keyval <= Clutter.KEY_KP_9)
                digit = keyval - Clutter.KEY_KP_1 + 1;
            if (!digit && DIGIT_CODES.includes(keycode))
                digit = DIGIT_CODES.indexOf(keycode) + 1;
            if (digit) {
                this._openNumber(digit);
                return Clutter.EVENT_STOP;
            }
        }
        if (modifiers === 0 && (keyval === Clutter.KEY_space || keyval === Clutter.KEY_KP_Space) &&
            this._settings.get_boolean('glance')) {
            const focus = global.stage.get_key_focus();
            const result = this._resultOf(focus);
            if (result && result === focus && fileOf(result)) {
                this._glance ??= new Glance(this);
                if (this._glance.show(result))
                    return Clutter.EVENT_STOP;
            }
        }
        return Clutter.EVENT_PROPAGATE;
    }

    // The Nth result shown, opened as a click on it would
    _openNumber(number) {
        const result = this.results()[number - 1];
        if (!result)
            return;
        this._activating = true;
        try {
            result.emit('clicked', 1);
        } finally {
            this._activating = false;
        }
    }

    _resultOf(actor) {
        for (let at = actor; at && at !== this._box; at = at.get_parent()) {
            if (at instanceof GnomeSearch.SearchResult)
                return at;
        }
        return null;
    }

    // Every result shown, in the order the sections show them
    results() {
        return (this._providers?.sections() ?? []).flatMap(({display}) => resultsOf(display));
    }

    // The keyboard went somewhere: looked at once the change has settled (a drag takes the keyboard before it says
    // that it started, and the results move it while they are rebuilt)
    _queueFocusCheck() {
        if (!this._focusLater)
            this._queueLater('_focusLater', () => this._checkFocus());
    }

    _checkFocus() {
        if (!this._open || this._dragging || this._glancing)
            return;
        const focus = global.stage.get_key_focus();
        if (focus && focus !== global.stage && (this._box.contains(focus) || this._isResultMenu(focus)))
            return;
        this._close('animate');
    }

    // A result's own menu (right click, or the menu key): it opens over the box and leaves it open
    _isResultMenu(actor) {
        for (let at = actor; at; at = at.get_parent()) {
            const menu = at._delegate;
            if (menu instanceof PopupMenu.PopupMenuBase && menu.sourceActor && this._box.contains(menu.sourceActor))
                return true;
        }
        return false;
    }

    // ---- The look GNOME's sections wear in the card ----

    // A section of GNOME's own while lent: one column, the provider's name as a title over its results, its icon put
    // away; the apps get a title over their tiles
    _dress() {
        for (const {display, provider} of this._providers?.sections(true) ?? [])
            this._dressSection(display, provider);
        this._queueRefresh();
    }

    _dressSection(display, provider) {
        if (display instanceof Section || this._dressed.has(display))
            return;
        const undo = [];
        const info = display.providerInfo;
        if (info instanceof St.Button) {
            const container = info.get_parent();
            if (container instanceof St.BoxLayout && container.orientation !== Clutter.Orientation.VERTICAL) {
                const was = container.orientation;
                container.orientation = Clutter.Orientation.VERTICAL;
                undo.push(() => (container.orientation = was));
            }
            // The title at the start of the column, as the card's own are
            const align = info.x_align;
            info.x_align = Clutter.ActorAlign.START;
            undo.push(() => (info.x_align = align));
            const content = info.get_child();
            const [icon, details] = content?.get_children() ?? [];
            if (icon instanceof St.Icon && icon.visible) {
                icon.hide();
                undo.push(() => icon.show());
            }
            const [name, more] = details?.get_children() ?? [];
            if (name instanceof St.Label) {
                const text = name.text;
                const ellipsize = name.clutter_text.ellipsize;
                name.text = text.toLocaleUpperCase(sessionLocale());
                // Never cut: St adds the letter spacing after Pango has measured the words
                name.clutter_text.ellipsize = Pango.EllipsizeMode.NONE;
                name.add_style_class_name('amethystora-search-section-title');
                undo.push(() => {
                    name.text = text;
                    name.clutter_text.ellipsize = ellipsize;
                    name.remove_style_class_name('amethystora-search-section-title');
                });
            }
            if (more instanceof St.Label) {
                more.add_style_class_name('amethystora-search-more');
                undo.push(() => more.remove_style_class_name('amethystora-search-more'));
            }
        } else if (provider instanceof AppDisplay.AppSearchProvider || provider?.id === 'applications') {
            const header = new St.Label({
                style_class: 'amethystora-search-section-title amethystora-search-apps-title',
                text: _('Apps').toLocaleUpperCase(sessionLocale()),
            });
            header.clutter_text.use_markup = false;
            display.insert_child_at_index(header, 0);
            undo.push(() => header.destroy());
        }
        // A plain handler: one of connectObject's would be taken away by the object's own tracker before it ran
        const gone = display.connect('destroy', () => this._dressed.delete(display));
        undo.push(() => display.disconnect(gone));
        this._dressed.set(display, undo);
    }

    // A list result of GNOME's in the card: the card's picture size, and its content the row's whole width so that a
    // hint can sit at its end
    _dressRow(result) {
        if (this._stretched.has(result))
            return;
        const content = result.get_child();
        if (!(content instanceof St.BoxLayout))
            return;
        this._stretched.set(result, content.x_align);
        content.x_align = Clutter.ActorAlign.FILL;
        const title = content.get_first_child();
        const icon = title?.get_first_child();
        if (icon instanceof St.Icon && icon.icon_size !== PICTURE) {
            this._icons.set(icon, icon.icon_size);
            icon.icon_size = PICTURE;
        }
        const gone = result.connect('destroy', () => {
            this._stretched.delete(result);
            this._icons.delete(icon);
        });
        this._stretchedIds.set(result, gone);
    }

    _undress() {
        this._spaceHint?.get_parent()?.remove_child(this._spaceHint);
        for (const [result, align] of this._stretched) {
            result.disconnect(this._stretchedIds.get(result));
            const content = result.get_child();
            if (content)
                content.x_align = align;
        }
        this._stretched.clear();
        this._stretchedIds.clear();
        for (const [icon, size] of this._icons)
            icon.icon_size = size;
        this._icons.clear();
        for (const undo of this._dressed.values()) {
            for (const step of undo.reverse()) {
                try {
                    step();
                } catch {}
            }
        }
        this._dressed.clear();
        for (const row of this._ourRows())
            row.place = 0;
    }

    _ourRows() {
        return (this._providers?.sections(true) ?? []).filter(({display}) => display instanceof Section)
            .flatMap(({display}) => display.rows);
    }

    // After every provider's results: the sections dressed, the rows numbered
    _queueRefresh() {
        if (this._lent && !this._refreshLater)
            this._queueLater('_refreshLater', () => this._refresh());
    }

    _refresh() {
        if (!this._lent)
            return;
        for (const {display, provider} of this._providers?.sections(true) ?? []) {
            this._dressSection(display, provider);
            if (!(display instanceof Section)) {
                for (const result of resultsOf(display)) {
                    if (result instanceof GnomeSearch.ListSearchResult)
                        this._dressRow(result);
                }
            }
        }
        const results = this.results();
        for (const row of this._ourRows()) {
            const index = results.indexOf(row);
            row.place = index >= 0 && index < NUMBERED ? index + 1 : 0;
        }
        this._syncFocusHint();
    }

    // A file found by Files, with the keyboard on it: "Space previews" at its end
    _syncFocusHint() {
        const focus = this._open ? global.stage.get_key_focus() : null;
        const result = this._resultOf(focus);
        const wanted = result && result === focus && fileOf(result) && this._settings?.get_boolean('glance')
            ? result : null;
        if (!wanted) {
            this._spaceHint?.get_parent()?.remove_child(this._spaceHint);
            return;
        }
        if (!this._spaceHint) {
            this._spaceHint = new St.Label({
                style_class: 'amethystora-search-row-hint', x_expand: true, x_align: Clutter.ActorAlign.END,
                y_align: Clutter.ActorAlign.CENTER,
            });
            // It goes with the row it is on, and a label destroyed must not be added to the next one (the shell died so)
            this._spaceHint.connect('destroy', () => {
                this._spaceHint = null;
            });
        }
        this._spaceHint.text = _('Space previews');
        const content = wanted.get_child();
        if (this._spaceHint.get_parent() !== content) {
            this._spaceHint.get_parent()?.remove_child(this._spaceHint);
            content?.add_child(this._spaceHint);
        }
    }

    // ---- Glance ----

    fileResults() {
        return this.results().filter(result => fileOf(result));
    }

    glanceStarted(result) {
        this._glancing = true;
        this._select(result);
        this._box.reactive = false;
        this._box.remove_all_transitions();
        this._box.ease({opacity: 0, duration: DURATION, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
    }

    glanceSelected(result) {
        this._select(result);
    }

    glanceEnded(last) {
        this._select(null);
        if (!this._glancing)
            return;
        this._glancing = false;
        if (!this._open)
            return;
        this._box.reactive = true;
        this._box.remove_all_transitions();
        this._box.ease({opacity: 255, duration: DURATION, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
        if (last?.get_stage())
            last.grab_key_focus();
        else
            this._lent?.entry.grab_key_focus();
    }

    // The previewer was asked to show the file in Files, or another window took the focus: the card closes
    closeForFiles() {
        this._close('now');
    }

    closeForFocus() {
        this._close('now');
    }

    // The file being previewed, marked selected; forgotten when GNOME destroys its row
    _select(result) {
        const was = this._selected;
        if (was && was !== result) {
            was.disconnect(this._selectedGone);
            was.remove_style_pseudo_class('selected');
        }
        if (was === result)
            return;
        this._selected = result;
        if (result) {
            result.add_style_pseudo_class('selected');
            this._selectedGone = result.connect('destroy', () => (this._selected = null));
        }
    }

    // ---- For the tests (the lab's suite 16) ----

    snapshot() {
        const label = actor => actor instanceof St.Label ? actor.text : '';
        const sections = (this._providers?.sections() ?? []).map(({display, provider}) => {
            let title = '';
            if (display instanceof Section)
                title = label(display.get_first_child()?.get_first_child?.());
            else if (display.providerInfo)
                title = label(display.providerInfo.get_child()?.get_last_child()?.get_first_child());
            else
                title = label(display.get_first_child());
            const rows = resultsOf(display).map(result => {
                const selected = result.has_style_pseudo_class('selected') || result.has_key_focus();
                if (display instanceof Section) {
                    const note = result.get_child().get_children()
                        .find(child => child.has_style_class_name?.('amethystora-search-row-meta'));
                    return {title: result.metaInfo.name, meta: note?.text ?? '', hint: result.hintText, selected};
                }
                const description = result.get_child?.()?.get_children?.()
                    .find(child => child.has_style_class_name?.('list-search-result-description'));
                return {
                    title: result.metaInfo?.name ?? result.app?.get_name?.() ?? '',
                    meta: description?.text ?? '',
                    hint: this._spaceHint?.get_parent() && this._spaceHint.get_parent() === result.get_child?.()
                        ? this._spaceHint.text : '',
                    selected,
                };
            });
            return {id: provider?.id ?? '', title, rows};
        });
        return JSON.stringify({
            open: this.open, degraded: this.degraded, glancing: this._glancing,
            top: this._box?.y ?? 0, width: this._box?.width ?? 0, opacity: this._box?.opacity ?? 0,
            timeouts: ['_focusLater', '_giveBackLater', '_refreshLater'].filter(name => this[name]).length,
            sections,
        });
    }

    // ---- Laters ----

    _queueLater(name, callback) {
        this._removeLater(name);
        this[name] = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this[name] = 0;
            callback();
            return GLib.SOURCE_REMOVE;
        });
    }

    _removeLater(name) {
        if (this[name])
            global.compositor.get_laters().remove(this[name]);
        this[name] = 0;
    }
}
