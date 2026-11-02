// The tiling's keyboard shortcuts, and the GNOME shortcuts they share a key combination with.
//
// Mutter keeps one action per key combination, and when two shortcuts name the same combination the one it
// indexed last wins, which is not something anybody chooses. So while the tiling is on, a combination it uses is
// taken away from GNOME's shortcut that has it, that combination only: the other combinations of the same GNOME
// shortcut keep working, and it all comes back when the tiling is turned off.
//
// Most of it never touches the person's settings. The tiling's own shortcuts are registered from a copy of its
// keys kept in memory, and a GNOME shortcut that is Mutter's own hands the shared combination over from its
// handler. The shortcuts that belong to GNOME Shell itself or to the settings daemon's media keys cannot be handed
// over like that. Of those, only the four whose GNOME defaults the tiling's defaults share (TAKE_OUT) have the
// combination taken out of the GNOME key, and exactly what was taken out is written down first (state.js), so that
// it is put back even after a crash. Any other such key keeps what it holds, which is often the person's own
// choice: there the tiling gives the combination up, from its copy in memory, for as long as the key has it.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

// GNOME's schemas of shortcuts the tiling's could collide with
const GNOME_SCHEMAS = [
    'org.gnome.mutter.keybindings',
    'org.gnome.mutter.wayland.keybindings',
    'org.gnome.desktop.wm.keybindings',
    'org.gnome.shell.keybindings',
    'org.gnome.settings-daemon.plugins.media-keys',
];

// Mutter's own shortcuts, whose handler can be replaced and put back in memory
const MUTTER_SCHEMAS = new Set(GNOME_SCHEMAS.slice(0, 3));

// The only GNOME keys a combination is taken out of in dconf: the shell's and the media keys' shortcuts whose GNOME
// default collides with a tiling default (AGENTS.md, the tiling)
const TAKE_OUT = new Set(['focus-active-notification', 'shift-overview-up', 'shift-overview-down',
    'rotate-video-lock-static']);

// The Mutter shortcuts GNOME Shell gives a handler of its own, by the method that handles them: the ones a
// shared combination can be taken from while their other combinations keep doing what GNOME does
const SHELL_HANDLERS = [
    [/^(switch|move)-to-workspace-(left|right|up|down|last|\d+)$/, '_showWorkspaceSwitcher'],
    [/^(switch-(applications|group|windows)|cycle-(windows|group))(-backward)?$/, '_startSwitcher'],
    [/^switch-monitor$/, '_startSwitcher'],
    [/^switch-panels(-backward)?$/, '_startA11ySwitcher'],
];

const MODIFIERS = [
    [/^(primary|control|ctrl|ctl)$/, Clutter.ModifierType.CONTROL_MASK],
    [/^(shift|shft)$/, Clutter.ModifierType.SHIFT_MASK],
    [/^(alt|mod1)$/, Clutter.ModifierType.MOD1_MASK],
    [/^super$/, Clutter.ModifierType.SUPER_MASK],
    [/^meta$/, Clutter.ModifierType.META_MASK],
    [/^hyper$/, Clutter.ModifierType.HYPER_MASK],
    [/^mod4$/, Clutter.ModifierType.SUPER_MASK],
];
// The modifiers a combination is told apart by: Num Lock and the like are not among them, as in Mutter
const RELEVANT = Clutter.ModifierType.SHIFT_MASK | Clutter.ModifierType.CONTROL_MASK | Clutter.ModifierType.MOD1_MASK |
    Clutter.ModifierType.SUPER_MASK | Clutter.ModifierType.META_MASK | Clutter.ModifierType.HYPER_MASK;

// A keysym in lower case, as shortcuts compare them
export function lower(value) {
    return Clutter.keyval_convert_case(value)[0];
}

// The keysym for a key's name, whatever its case, as Mutter reads it
function keysym(name) {
    // Mutter's name for the key above Tab, whatever the layout prints on it
    if (name === 'Above_Tab')
        return Clutter.KEY_grave;
    const plain = name.replace(/^XF86/, '');
    for (const candidate of [plain, plain.toLowerCase(), plain[0].toUpperCase() + plain.slice(1)]) {
        const value = Clutter[`KEY_${candidate}`];
        if (typeof value === 'number')
            return lower(value);
    }
    return 0;
}

// A shortcut's key combination as {mods, key}, the same for every way of writing it, or null for one that is
// empty or that the tiling cannot know
export function parse(accelerator) {
    let rest = accelerator.trim();
    if (!rest || rest === 'disabled')
        return null;
    let mods = 0;
    for (let match; (match = /^<([^>]+)>/.exec(rest));) {
        const name = match[1].toLowerCase();
        const modifier = MODIFIERS.find(([pattern]) => pattern.test(name));
        if (!modifier)
            return null;
        mods |= modifier[1];
        rest = rest.slice(match[0].length);
    }
    const key = rest ? keysym(rest) : 0;
    return key ? {mods, key} : null;
}

const same = (a, b) => a && b && a.mods === b.mods && a.key === b.key;

// The combination a key press makes: its key without the case Shift gives it, and the modifiers held
export function ofEvent(event) {
    let mods = event.get_state();
    if (mods & Clutter.ModifierType.MOD4_MASK)
        mods |= Clutter.ModifierType.SUPER_MASK;
    return {mods: mods & RELEVANT, key: lower(event.get_key_symbol())};
}

// The combination written for people: "Super+Shift+C"
export function label(accelerator) {
    const combo = parse(accelerator ?? '');
    if (!combo)
        return '';
    const names = [
        [Clutter.ModifierType.SUPER_MASK, 'Super'], [Clutter.ModifierType.CONTROL_MASK, 'Ctrl'],
        [Clutter.ModifierType.MOD1_MASK, 'Alt'], [Clutter.ModifierType.SHIFT_MASK, 'Shift'],
    ].filter(([bit]) => combo.mods & bit).map(([, name]) => name);
    const key = accelerator.replace(/<[^>]+>/g, '');
    names.push(key.length === 1 ? key.toUpperCase() : key);
    return names.join('+');
}

export class Keys {
    // settings: the tiling's keybindings schema; actions: name -> {flags, run(context)}; state: the state file
    constructor(settings, actions, state) {
        this._settings = settings;
        this._actions = actions;
        this._state = state;
    }

    enable() {
        // The copy Mutter reads the tiling's shortcuts from
        this._memory = Gio.Settings.new_full(this._settings.settings_schema, Gio.memory_settings_backend_new(), null);
        this._byAction = new Map();
        this._wrapped = new Map();
        this._gnome = new Map();
        for (const id of GNOME_SCHEMAS) {
            if (!Gio.SettingsSchemaSource.get_default().lookup(id, true))
                continue;
            const settings = new Gio.Settings({schema_id: id});
            this._gnome.set(id, settings);
            settings.connectObject('changed', (s, key) => this._gnomeChanged(id, key), this);
        }
        this._settings.connectObject('changed', () => this._queue(), this);
        this._update();
        for (const [name, action] of this._actions) {
            const id = Main.wm.addKeybinding(name, this._memory, action.flags ?? Meta.KeyBindingFlags.NONE,
                Shell.ActionMode.NORMAL, (display, window, event, binding) =>
                    this._run(name, {window, event, binding}));
            if (id !== Meta.KeyBindingAction.NONE)
                this._byAction.set(id, name);
        }
        // Mutter skips a shortcut that acts on a window while no window has the keyboard; one handed over by
        // such a shortcut is caught here instead
        global.stage.connectObject('key-press-event', (actor, event) => this._unfocused(event), this);
    }

    // suspend: the screen is locking, and the tiling comes back at unlock as it was. What was taken out of the
    // person's settings stays out until then rather than being written twice at every lock.
    disable(suspend = false) {
        if (this._idle)
            GLib.source_remove(this._idle);
        this._idle = 0;
        global.stage.disconnectObject(this);
        this._settings.disconnectObject(this);
        for (const settings of this._gnome?.values() ?? [])
            settings.disconnectObject(this);
        for (const name of this._actions.keys())
            Main.wm.removeKeybinding(name);
        for (const name of [...this._wrapped?.keys() ?? []])
            this._unwrap(name);
        if (!suspend)
            this._giveBackAll();
        // Disposed now rather than by the collector (dock.js says why)
        for (const settings of this._gnome?.values() ?? [])
            settings.run_dispose();
        this._gnome = null;
        this._memory?.run_dispose();
        this._memory = null;
        this._byAction = null;
        this._wrapped = null;
    }

    // The first combination of a tiling shortcut, as the person has it now
    first(name) {
        return this._settings.get_strv(name)[0] ?? '';
    }

    // The tiling action a key press means inside a navigation session, or null. The combinations GNOME's handlers
    // hand over are not in Mutter's table under the tiling's names, so those are matched here.
    actionFor(event) {
        const id = global.display.get_keybinding_action(event.get_key_code(), event.get_state());
        const name = this._byAction?.get(id);
        if (name)
            return name;
        const combo = ofEvent(event);
        for (const action of this._actions.keys()) {
            if (this._settings.get_strv(action).some(accelerator => same(parse(accelerator), combo)))
                return action;
        }
        return null;
    }

    _run(name, context) {
        try {
            this._actions.get(name).run(context);
        } catch (e) {
            console.error(`Amethystora UI: the tiling's ${name}: ${e.message}\n${e.stack}`);
        }
    }

    _unfocused(event) {
        if (global.display.focus_window || Main.actionMode !== Shell.ActionMode.NORMAL || Main.modalCount > 0)
            return Clutter.EVENT_PROPAGATE;
        const combo = ofEvent(event);
        for (const [name, entries] of this._wrapped) {
            const entry = entries.find(([taken]) => same(taken, combo));
            if (entry && this._gnomeFlags(name) & Meta.KeyBindingFlags.PER_WINDOW) {
                this._run(entry[1], {window: null, event, binding: null});
                return Clutter.EVENT_STOP;
            }
        }
        return Clutter.EVENT_PROPAGATE;
    }

    // Mutter's own window shortcuts only act while a window has the keyboard
    _gnomeFlags(name) {
        return /^(maximize|unmaximize|toggle-tiled-|move-to-monitor-|move-to-workspace-)/.test(name)
            ? Meta.KeyBindingFlags.PER_WINDOW : 0;
    }

    _queue() {
        if (this._idle)
            return;
        this._idle = GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
            this._idle = 0;
            this._update();
            return GLib.SOURCE_REMOVE;
        });
    }

    _gnomeChanged(id, key) {
        const record = this._record(id, key);
        // Our own write coming back. Never a reset, which takes the account's value away (ours is always one): GNOME's own
        // value is then the person's choice (Control's Reset), even where it reads the same as what was written
        const gnome = this._gnome.get(id);
        if (record && gnome.get_user_value(key) !== null &&
            JSON.stringify(gnome.get_strv(key)) === JSON.stringify(record.wrote))
            return;
        // The person changed a GNOME shortcut we had taken a combination from: what they set is theirs now
        if (record)
            this._forget(id, key);
        this._sayTaken(id, key);
        this._queue();
    }

    // The person set a GNOME shortcut to a combination the tiling has: the tiling keeps it, and says so
    _sayTaken(id, key) {
        const settings = this._gnome.get(id);
        const schemaKey = settings.settings_schema.get_key(key);
        // GNOME's own value, which a reset brings back to every key at once, collides by design and is taken quietly;
        // outside TAKE_OUT the key keeps what the person set and the tiling gives the combination up: nothing to say
        if (schemaKey.get_value_type().dup_string() !== 'as' || settings.get_user_value(key) === null ||
            !(TAKE_OUT.has(key) || MUTTER_SCHEMAS.has(id)))
            return;
        const mine = this._mine();
        const taken = settings.get_strv(key).find(a => mine.some(m => same(m.combo, parse(a))));
        if (!taken)
            return;
        Main.notifyError(_('%s is a shortcut of the tiling').format(label(taken)),
            _('The tiling keeps it, so “%s” does not answer to it until the tiling is turned off.')
                .format(schemaKey.get_summary() || key));
    }

    // Every combination of the tiling's shortcuts, with the action it starts
    _mine() {
        const mine = [];
        for (const name of this._actions.keys()) {
            for (const accelerator of this._settings.get_strv(name)) {
                const combo = parse(accelerator);
                if (combo)
                    mine.push({name, accelerator, combo});
            }
        }
        return mine;
    }

    _taken() {
        return this._state.get('taken') ?? [];
    }

    _record(id, key) {
        return this._taken().find(r => r.schema === id && r.key === key) ?? null;
    }

    _forget(id, key) {
        this._state.set('taken', this._taken().filter(r => r.schema !== id || r.key !== key));
    }

    // A GNOME shortcut's combinations as the person set them: what the key holds and what we took out of it
    _real(id, key) {
        const current = this._gnome.get(id).get_strv(key);
        const record = this._record(id, key);
        if (!record)
            return current;
        const real = [...current];
        for (const {accelerator, index} of record.removed) {
            if (!real.some(a => same(parse(a), parse(accelerator))))
                real.splice(Math.min(index, real.length), 0, accelerator);
        }
        return real;
    }

    _update() {
        const mine = this._mine();
        const handedOver = new Set();
        // The tiling's combinations a GNOME key keeps (not one of TAKE_OUT): the tiling gives them up
        const yielded = new Set();
        const wanted = new Map();
        for (const [id, settings] of this._gnome) {
            const schema = settings.settings_schema;
            for (const key of schema.list_keys()) {
                if (schema.get_key(key).get_value_type().dup_string() !== 'as')
                    continue;
                const real = this._real(id, key);
                const shared = real.map((accelerator, index) => {
                    const combo = parse(accelerator);
                    const owner = combo && mine.find(m => same(m.combo, combo));
                    return owner ? {accelerator, index, combo, owner} : null;
                }).filter(Boolean);
                if (!shared.length) {
                    this._giveBack(id, key);
                    continue;
                }
                if (this._canWrap(id, key, real.length === shared.length)) {
                    this._giveBack(id, key);
                    wanted.set(key, shared);
                    for (const s of shared)
                        handedOver.add(`${s.owner.name}\n${s.owner.accelerator}`);
                } else if (TAKE_OUT.has(key)) {
                    this._takeOut(id, key, real, shared);
                } else {
                    // A combination an older image took out comes back; the GNOME key keeps it, the tiling does not
                    this._giveBack(id, key);
                    for (const s of shared)
                        yielded.add(`${s.owner.name}\n${s.owner.accelerator}`);
                }
            }
        }
        // A combination given up is given up everywhere: a Mutter key that shares it (Super+Left's half tile) leaves it too
        for (const [key, shared] of wanted) {
            const kept = shared.filter(s => !yielded.has(`${s.owner.name}\n${s.owner.accelerator}`));
            if (kept.length)
                wanted.set(key, kept.map(s => [s.combo, s.owner.name]));
            else
                wanted.delete(key);
        }
        for (const name of [...this._wrapped.keys()]) {
            if (!wanted.has(name))
                this._unwrap(name);
        }
        for (const [name, entries] of wanted)
            this._wrap(name, entries);

        // The tiling's own copy, without the combinations GNOME's handlers hand over or a GNOME key keeps
        for (const name of this._actions.keys()) {
            const value = this._settings.get_strv(name).filter(a =>
                !handedOver.has(`${name}\n${a}`) && !yielded.has(`${name}\n${a}`));
            if (JSON.stringify(this._memory.get_strv(name)) !== JSON.stringify(value))
                this._memory.set_strv(name, value);
        }
    }

    _canWrap(id, key, whole) {
        if (!MUTTER_SCHEMAS.has(id))
            return false;
        const handled = SHELL_HANDLERS.some(([pattern, method]) => pattern.test(key) &&
            typeof Main.wm[method] === 'function');
        // A key GNOME Shell has given a handler that is not listed above (the Run dialog's) cannot be put back once
        // wrapped: taking the handler away restores Mutter's own, not the shell's
        if (!handled && key in (Main.wm._allowedKeybindings ?? {}))
            return false;
        return whole || handled;
    }

    _wrap(name, entries) {
        const wrapped = this._wrapped.get(name);
        if (wrapped) {
            // Already handing over: only the combinations change
            wrapped.splice(0, wrapped.length, ...entries);
            return;
        }
        const handler = SHELL_HANDLERS.find(([pattern]) => pattern.test(name));
        const method = handler && typeof Main.wm[handler[1]] === 'function' ? handler[1] : null;
        const modes = Main.wm._allowedKeybindings?.[name];
        const list = [...entries];
        const done = Meta.keybindings_set_custom_handler(name, (display, window, event, binding) => {
            const combo = event ? ofEvent(event) : null;
            const mods = binding?.get_modifiers() ?? -1;
            const mine = list.find(([taken]) => same(taken, combo)) ??
                (list.filter(([taken]) => taken.mods === mods).length === 1
                    ? list.find(([taken]) => taken.mods === mods) : null);
            if (mine && Main.actionMode === Shell.ActionMode.NORMAL)
                this._run(mine[1], {window, event, binding});
            else if (method)
                Main.wm[method](display, window, event, binding);
        });
        if (done)
            this._wrapped.set(name, list);
        Object.assign(list, {method, modes});
    }

    _unwrap(name) {
        const {method, modes} = this._wrapped.get(name);
        this._wrapped.delete(name);
        if (method)
            Main.wm.setCustomKeybindingHandler(name, modes ?? Shell.ActionMode.NORMAL, Main.wm[method].bind(Main.wm));
        else
            Meta.keybindings_set_custom_handler(name, null);
    }

    // Take the shared combinations out of a GNOME key, writing down what is taken before the key is written
    _takeOut(id, key, real, shared) {
        const settings = this._gnome.get(id);
        const removed = shared.map(({accelerator, index}) => ({accelerator, index}));
        const value = real.filter((a, index) => !shared.some(s => s.index === index));
        const others = this._taken().filter(r => r.schema !== id || r.key !== key);
        this._state.set('taken', [...others, {schema: id, key, removed, wrote: value}]);
        if (JSON.stringify(settings.get_strv(key)) !== JSON.stringify(value))
            settings.set_strv(key, value);
    }

    // A record that cannot be given back is dropped, with a word in the log: it would fail the same way every time
    _giveBack(id, key) {
        if (!this._record(id, key))
            return;
        try {
            const value = this._real(id, key);
            this._forget(id, key);
            const settings = this._gnome.get(id);
            if (JSON.stringify(settings.get_strv(key)) !== JSON.stringify(value))
                settings.set_strv(key, value);
        } catch (e) {
            console.warn(`Amethystora UI: ${id} ${key} was not put back: ${e.message}`);
            this._forget(id, key);
        }
    }

    _giveBackAll() {
        for (const record of this._taken()) {
            if (this._gnome?.has(record.schema))
                this._giveBack(record.schema, record.key);
            else
                giveBackWithoutRunning(this._state, record);
        }
    }
}

// What a previous session took out, when the tiling is not running to know it: at the start with the tiling off
export function giveBackLeftovers(state) {
    for (const record of state.get('taken') ?? [])
        giveBackWithoutRunning(state, record);
}

// One record at a time, and the others whatever becomes of it: a key a later GNOME no longer has throws here
function giveBackWithoutRunning(state, record) {
    let settings = null;
    try {
        if (Gio.SettingsSchemaSource.get_default().lookup(record.schema, true)) {
            settings = new Gio.Settings({schema_id: record.schema});
            const value = settings.get_strv(record.key);
            for (const {accelerator, index} of record.removed) {
                if (!value.some(a => same(parse(a), parse(accelerator))))
                    value.splice(Math.min(index, value.length), 0, accelerator);
            }
            settings.set_strv(record.key, value);
        }
    } catch (e) {
        console.warn(`Amethystora UI: ${record.schema} ${record.key} was not put back: ${e.message}`);
    } finally {
        // Disposed now rather than by the collector (dock.js says why)
        settings?.run_dispose();
    }
    state.set('taken', (state.get('taken') ?? []).filter(r => r !== record &&
        (r.schema !== record.schema || r.key !== record.key)));
}
