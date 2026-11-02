// Hold Super: after Super has been held alone for a second (hold-super-delay), a glass card shows the common shortcuts as
// they are set on this machine, and the dock numbers its apps; Control's Keyboard page has the rest. Letting go fades them in a sixth of a second, and does
// not open the overview; a quick tap of Super still does. A shortcut typed quickly never shows them.
//
// GNOME hears Super only when it is let go, and the press goes to the focused app, so nothing in the shell sees it
// being held. The keyboard's map of pressed modifiers does change, though, and says so (Clutter.Keymap
// state-changed): that is all this listens to, one short handler per modifier pressed or let go, and one timer after
// Super alone is pressed. No polling.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Graphene from 'gi://Graphene';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {GLIDE, caps, capRow, ease, glide, label, reduced} from './util.js';

const SUPER = Clutter.ModifierType.SUPER_MASK | Clutter.ModifierType.MOD4_MASK;
const MODIFIERS = Clutter.ModifierType.SHIFT_MASK | Clutter.ModifierType.CONTROL_MASK |
    Clutter.ModifierType.MOD1_MASK | SUPER | Clutter.ModifierType.HYPER_MASK | Clutter.ModifierType.META_MASK;
// After a hold, how long the overview stays deaf to the release of Super if GNOME never asks for it
const SWALLOW = 300;
const CUSTOM = '/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/';
const CLIPBOARD = 'amethystora-clipboard@arsslensoft.github.io';
// The tip about holding Super is offered after Super alone has opened the overview this many times in a session
const TIP_AFTER = 3;

// Settings of a schema that may be missing (an extension not installed): null then
function settingsFor(id, path = null, uuid = null) {
    const source = Gio.SettingsSchemaSource.get_default();
    let schema = source?.lookup(id, true);
    if (!schema && uuid) {
        const dir = Main.extensionManager.lookup(uuid)?.dir?.get_child('schemas');
        if (dir?.query_exists(null)) {
            try {
                schema = Gio.SettingsSchemaSource.new_from_directory(dir.get_path(), source, false).lookup(id, false);
            } catch {}
        }
    }
    if (!schema)
        return null;
    return path ? new Gio.Settings({settings_schema: schema, path}) : new Gio.Settings({settings_schema: schema});
}

// A key's first accelerator, whether the key holds one string or a list
function accelerator(settings, key) {
    if (!settings?.settings_schema.has_key(key))
        return null;
    const value = settings.get_value(key).recursiveUnpack();
    const first = Array.isArray(value) ? value[0] : value;
    return first || null;
}

export class Hold {
    constructor(module) {
        this._module = module;
        this._ext = module.ext;
    }

    enable() {
        this.shownCount = 0;
        this.keymapEvents = 0;
        this._alone = false;
        this._a11y = new Gio.Settings({schema_id: 'org.gnome.desktop.a11y.keyboard'});
        this._keymap = global.stage.context.get_backend().get_default_seat().get_keymap();
        this._keymap.connectObject('state-changed', () => this._stateChanged(), this);
        // What the card reads, so that a key someone changes shows at the next hold
        this._sources = [
            ['search', this._own('org.gnome.shell.extensions.amethystora-ui.search')],
            ['emoji', this._own('org.gnome.shell.extensions.amethystora-ui.emoji')],
            ['terminal', settingsFor('org.gnome.settings-daemon.plugins.media-keys.custom-keybinding', `${CUSTOM}custom20/`)],
            ['manual', settingsFor('org.gnome.settings-daemon.plugins.media-keys.custom-keybinding', `${CUSTOM}custom28/`)],
            ['clipboard', settingsFor('org.gnome.shell.extensions.amethystora-clipboard', null, CLIPBOARD)],
            ['shell', settingsFor('org.gnome.shell.keybindings')],
            ['media', settingsFor('org.gnome.settings-daemon.plugins.media-keys')],
            ['control', settingsFor('org.gnome.settings-daemon.plugins.media-keys.custom-keybinding', `${CUSTOM}custom29/`)],
        ];
        for (const [, settings] of this._sources)
            settings?.connectObject('changed', () => this._stale(), this);
        this._module.settings.connectObject('changed::snap-chooser', () => this._stale(), this);
        // The tip (the hints module): GNOME says when Super alone was let go, which is all that is counted
        if (!this._ext.modules?.get('hints')?.wasShown?.('hold-super')) {
            this._tipOwner = {};
            global.display.connectObject('overlay-key', () => this._tapped(), this._tipOwner);
        }
    }

    // GNOME's handler has run: the overview is opening, unless this was a hold, whose release is still swallowed now.
    // The tip waits for the overview to close, since none shows over it.
    _tapped() {
        if (this._swallowed || !Main.overview.visible)
            return;
        this._module.superTaps = (this._module.superTaps ?? 0) + 1;
        const hints = this._ext.modules?.get('hints');
        if (hints?.wasShown('hold-super')) {
            this._stopTip();
            return;
        }
        if (this._module.superTaps >= TIP_AFTER) {
            const text = _('Tip: hold Super for a second to see the dock\'s numbers and the shortcuts.');
            hints?.offerWhenClosed('hold-super', {text});
        }
    }

    _stopTip() {
        if (!this._tipOwner)
            return;
        global.display.disconnectObject(this._tipOwner);
        this._tipOwner = null;
    }

    // A schema of this extension, which lives in its own folder
    _own(id) {
        try {
            return this._ext.getSettings(id);
        } catch {
            return null;
        }
    }

    disable() {
        this._stopTip();
        this._keymap?.disconnectObject(this);
        this._keymap = null;
        // Disposed now rather than by the collector (dock.js says why)
        for (const [, settings] of this._sources ?? []) {
            settings?.disconnectObject(this);
            settings?.run_dispose();
        }
        this._sources = [];
        this._module.settings.disconnectObject(this);
        this._cancel();
        this._unswallow();
        this._hide(true);
        this._card?.destroy();
        this._card = null;
        this._veil?.destroy();
        this._veil = null;
        this._module.surfacesChanged();
        this._a11y?.run_dispose();
        this._a11y = null;
    }

    get cardVisible() {
        return !!this._showing;
    }

    glassSurfaces() {
        return this._card ? [{actor: this._card, material: 'glass', radius: 24}] : [];
    }

    // --- the detector ---------------------------------------------------------------------------------------------

    _superAlone() {
        const [, , mods] = global.get_pointer();
        const held = mods & MODIFIERS;
        return (held & SUPER) !== 0 && (held & ~SUPER) === 0;
    }

    _stateChanged() {
        this.keymapEvents++;
        const alone = this._superAlone();
        if (alone === this._alone)
            return;
        this._alone = alone;
        if (alone) {
            // A new hold: whatever an earlier one left swallowed is given back first
            this._unswallow();
            this._start();
        } else {
            this._cancel();
            if (this._showing)
                this._hide();
            // Whatever hid the card before Super was let go, nothing may stay swallowed: GNOME sends no overlay-key
            // when another key was pressed meanwhile
            this._swallowTimeout();
        }
    }

    _ready() {
        return Main.actionMode === Shell.ActionMode.NORMAL && global.stage.get_grab_actor() === null &&
            !Main.overview.visible && !this._a11y.get_boolean('stickykeys-enable');
    }

    _snapshot() {
        const focus = global.display.focus_window;
        const rect = focus?.get_frame_rect();
        return {
            focus, rect: rect ? [rect.x, rect.y, rect.width, rect.height].join() : '',
            workspace: global.workspace_manager.get_active_workspace_index(),
            windows: global.get_window_actors().length,
        };
    }

    _start() {
        if (!this._ready())
            return;
        this._snap = this._snapshot();
        // Anything that happens meanwhile means Super was the start of something else
        const cancel = () => {
            this._cancel();
            this._hide();
        };
        global.display.connectObject('notify::focus-window', cancel, 'window-created', cancel,
            'grab-op-begin', cancel, 'accelerator-activated', cancel, this._watch = {});
        global.workspace_manager.connectObject('active-workspace-changed', cancel, this._watch);
        Main.overview.connectObject('showing', cancel, this._watch);
        global.stage.connectObject('notify::key-focus', cancel, this._watch);
        this._timer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, this._module.settings.get_int('hold-super-delay'), () => {
            this._timer = 0;
            this._fire();
            return GLib.SOURCE_REMOVE;
        });
    }

    _cancel() {
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = 0;
        if (this._watch && !this._showing) {
            global.display.disconnectObject(this._watch);
            global.workspace_manager.disconnectObject(this._watch);
            Main.overview.disconnectObject(this._watch);
            global.stage.disconnectObject(this._watch);
            this._watch = null;
        }
    }

    _fire() {
        const now = this._snapshot();
        const same = this._snap && now.focus === this._snap.focus && now.rect === this._snap.rect &&
            now.workspace === this._snap.workspace && now.windows === this._snap.windows;
        const tiling = this._ext.modules?.get('tiling');
        if (!same || !this._superAlone() || !this._ready() || tiling?.engine?.session) {
            this._cancel();
            return;
        }
        this.show();
        this._swallow();
    }

    // --- not opening the overview after a hold --------------------------------------------------------------------

    // GNOME toggles the overview when Super is let go; after a hold, that one call does nothing
    _swallow() {
        if (this._swallowed)
            return;
        const overview = Main.overview;
        this._ownToggle = Object.getOwnPropertyDescriptor(overview, 'toggle') ?? null;
        const module = this;
        overview.toggle = function () {
            module._unswallow();
        };
        this._swallowed = true;
        // GNOME's handler runs before this one, so by now the release has been swallowed or never came
        global.display.connectObject('overlay-key', () => this._unswallow(), this._swallowOwner = {});
    }

    _swallowTimeout() {
        if (!this._swallowed || this._swallowTimer)
            return;
        this._swallowTimer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, SWALLOW, () => {
            this._swallowTimer = 0;
            this._unswallow();
            return GLib.SOURCE_REMOVE;
        });
    }

    _unswallow() {
        if (this._swallowTimer)
            GLib.source_remove(this._swallowTimer);
        this._swallowTimer = 0;
        if (!this._swallowed)
            return;
        this._swallowed = false;
        if (this._ownToggle)
            Object.defineProperty(Main.overview, 'toggle', this._ownToggle);
        else
            delete Main.overview.toggle;
        this._ownToggle = null;
        if (this._swallowOwner)
            global.display.disconnectObject(this._swallowOwner);
        this._swallowOwner = null;
    }

    get swallowed() {
        return !!this._swallowed;
    }

    // --- the card -------------------------------------------------------------------------------------------------

    _stale() {
        // Not while it shows: it is made again before it next shows
        if (this._showing) {
            this._staleLater = true;
            return;
        }
        this._card?.destroy();
        this._card = null;
        this._module.surfacesChanged();
    }

    // The rows, read from this machine's settings: [label, words on the caps]
    rows() {
        const get = name => this._sources.find(([n]) => n === name)?.[1];
        const classic = this._module.classic;
        const list = [
            [_('Search'), accelerator(get('search'), 'open')],
            [_('Terminal'), accelerator(get('terminal'), 'binding')],
            [_('Manual'), accelerator(get('manual'), 'binding')],
            [_('Emoji'), accelerator(get('emoji'), 'emoji-picker')],
            [_('Clipboard'), accelerator(get('clipboard'), 'toggle-menu')],
            [_('Window layouts'), classic ? accelerator(this._module.settings, 'snap-chooser') : null],
            [_('Screenshot'), accelerator(get('shell'), 'show-screenshot-ui')],
            [_('Lock the screen'), accelerator(get('media'), 'screensaver')],
            [_('Control'), accelerator(get('control'), 'binding')],
        ];
        return list.map(([name, accel]) => [name, caps(accel)]).filter(([, words]) => words);
    }

    // The footer's caps: the first app's key, and the last one's key alone, when both have the same modifiers
    _footer() {
        const shell = this._sources.find(([n]) => n === 'shell')?.[1];
        const first = caps(accelerator(shell, 'switch-to-application-1'));
        const last = caps(accelerator(shell, 'switch-to-application-9'));
        if (!first || !last || first.slice(0, -1).join() !== last.slice(0, -1).join())
            return null;
        return [first, [last[last.length - 1]]];
    }

    get footerText() {
        const footer = this._footer();
        return footer ? `${footer[0].join('+')} … ${footer[1].join('+')}` : '';
    }

    _build() {
        if (this._card)
            return;
        this._card = new St.BoxLayout({style_class: 'amethystora-windows-card',
            orientation: Clutter.Orientation.VERTICAL, opacity: 0, visible: false,
            pivot_point: new Graphene.Point({x: 0.5, y: 0.5}),
        });
        const lang = (GLib.get_language_names()[0] ?? 'en').split(/[._@]/)[0].replace('_', '-');
        let heading = _('Holding Super');
        try {
            heading = heading.toLocaleUpperCase(lang === 'C' ? 'en' : lang);
        } catch {
            heading = heading.toUpperCase();
        }
        this._card.add_child(label(heading, 'amethystora-windows-card-heading'));

        const grid = new St.Widget({style_class: 'amethystora-windows-card-grid',
            layout_manager: new Clutter.GridLayout({column_homogeneous: true})});
        const s = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        grid.layout_manager.column_spacing = 26 * s;
        this.rows().forEach(([name, words], i) => {
            const row = new St.BoxLayout({style_class: 'amethystora-windows-card-row', x_expand: true});
            const keys = capRow(words, 'amethystora-windows-key');
            keys.add_style_class_name('amethystora-windows-card-keys');
            row.add_child(keys);
            const text = label(name, 'amethystora-windows-card-label');
            text.y_align = Clutter.ActorAlign.CENTER;
            row.add_child(text);
            grid.layout_manager.attach(row, i % 2, Math.floor(i / 2), 1, 1);
        });
        this._card.add_child(grid);

        const footer = this._footer();
        if (footer) {
            const line = new St.BoxLayout({style_class: 'amethystora-windows-card-footer'});
            const sentence = _('{first} to {last} opens the app with that number in the dock');
            for (const part of sentence.split(/(\{first\}|\{last\})/)) {
                if (!part)
                    continue;
                if (part === '{first}' || part === '{last}') {
                    line.add_child(capRow(part === '{first}' ? footer[0] : footer[1], 'amethystora-windows-key'));
                } else {
                    const text = label(part, 'amethystora-windows-card-footer-text');
                    text.y_align = Clutter.ActorAlign.CENTER;
                    line.add_child(text);
                }
            }
            this._card.add_child(line);
        }
        const gone = label(_('Let go and it disappears. Control has every shortcut.'), 'amethystora-windows-card-footer-text');
        gone.add_style_class_name('amethystora-windows-card-footer');
        this._card.add_child(gone);
        Main.layoutManager.uiGroup.add_child(this._card);
        this._module.surfacesChanged();
    }

    // Shows the card and the dock's numbers on the monitor in use; also for tests and screenshots
    show() {
        if (this._staleLater && !this._showing) {
            this._staleLater = false;
            this._card?.destroy();
            this._card = null;
        }
        this._build();
        const monitor = Main.layoutManager.currentMonitor ?? Main.layoutManager.primaryMonitor;
        if (!this._veil) {
            this._veil = new St.Widget({style_class: 'amethystora-windows-card-veil', opacity: 0, visible: false});
            Main.layoutManager.uiGroup.add_child(this._veil);
        }
        this._veil.set_position(monitor.x, monitor.y);
        this._veil.set_size(monitor.width, monitor.height);
        Main.layoutManager.uiGroup.set_child_above_sibling(this._veil, null);
        Main.layoutManager.uiGroup.set_child_above_sibling(this._card, null);
        const [, width] = this._card.get_preferred_width(-1);
        this._card.set_position(Math.round(monitor.x + (monitor.width - width) / 2),
            Math.round(monitor.y + monitor.height * 0.19));
        this._showing = true;
        this.shownCount++;
        this._veil.show();
        this._card.show();
        ease(this._ext, this._veil, {opacity: 255}, {duration: 160});
        this._timeline?.stop();
        if (reduced(this._ext)) {
            this._card.set({translation_y: 0, scale_x: 1, scale_y: 1});
            ease(this._ext, this._card, {opacity: 255}, {duration: 160});
        } else {
            const s = St.ThemeContext.get_for_stage(global.stage).scale_factor;
            this._card.ease({opacity: 255, duration: 160, mode: Clutter.AnimationMode.EASE});
            this._timeline = glide(this._card, 200, GLIDE, p => {
                if (!this._card)
                    return;
                this._card.translation_y = 10 * s * (1 - p);
                this._card.set_scale(0.98 + 0.02 * p, 0.98 + 0.02 * p);
            }, () => (this._timeline = null));
        }
        this._ext.modules?.get('dock')?.showNumbers?.({reveal: true});
        this._module.surfacesChanged();
        return true;
    }

    _hide(now = false) {
        if (!this._showing)
            return;
        this._showing = false;
        this._timeline?.stop();
        this._timeline = null;
        // The signals that would have cancelled it go with it
        const watch = this._watch;
        this._watch = null;
        if (watch) {
            global.display.disconnectObject(watch);
            global.workspace_manager.disconnectObject(watch);
            Main.overview.disconnectObject(watch);
            global.stage.disconnectObject(watch);
        }
        this._ext.modules?.get('dock')?.hideNumbers?.();
        for (const actor of [this._card, this._veil]) {
            if (!actor)
                continue;
            if (now) {
                actor.remove_all_transitions();
                actor.opacity = 0;
                actor.hide();
                continue;
            }
            ease(this._ext, actor, {opacity: 0}, {
                duration: 160,
                onComplete: () => {
                    if (!this._showing)
                        actor.hide();
                },
            });
        }
    }

    // For tests: hides it as letting go of Super does
    hide() {
        this._hide();
        return true;
    }
}
