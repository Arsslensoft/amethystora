// A plugin's shortcut (plans/done/PLAN-plugins.md, "What everything reaches": keybindings): one combination a plugin declared and was granted
// (`key`), which runs one of its actions as a click. Never one that is bound already: Mutter refuses only a combination grabbed the same
// way, so every shortcut GNOME, Amethystora's extensions and the person keep in GSettings is read first (each time one is about to be
// grabbed, never while idle), and a combination found there is said once and left. Grabbed only while the plugin is on and not paused,
// and given back with it.

// GNOME's own keybinding schemas; Amethystora's are every schema of its extension (schemaDir), and the person's custom ones are
// media-keys' custom-keybindings
const GNOME_SCHEMAS = ['org.gnome.desktop.wm.keybindings', 'org.gnome.shell.keybindings', 'org.gnome.mutter.keybindings',
    'org.gnome.mutter.wayland.keybindings', 'org.gnome.settings-daemon.plugins.media-keys'];
const CUSTOM = 'org.gnome.settings-daemon.plugins.media-keys.custom-keybinding';
const MODS = {super: '<Super>', mod4: '<Super>', control: '<Control>', primary: '<Control>', ctrl: '<Control>', alt: '<Alt>', mod1: '<Alt>', shift: '<Shift>'};

// One form for a shortcut however it is written: the modifiers in one order, the key in lower case
export function normal(accel) {
    const mods = new Set();
    const key = accel.replace(/<([A-Za-z0-9]+)>/g, (m, name) => {
        mods.add(MODS[name.toLowerCase()] ?? `<${name}>`);
        return '';
    });
    return ['<Super>', '<Control>', '<Alt>', '<Shift>'].filter(m => mods.delete(m)).join('') + [...mods].sort().join('') + key.toLowerCase();
}

// Every combination bound in GSettings now: each schema's string lists and strings that look like shortcuts. Each Settings object made here
// is disposed before this returns
export function bound(schemaDir) {
    const found = new Set();
    const read = (settings, schema) => {
        for (const key of schema.list_keys()) {
            const type = schema.get_key(key).get_value_type().dup_string();
            const values = type === 'as' ? settings.get_strv(key) : type === 's' && key === 'binding' ? [settings.get_string(key)] : [];
            for (const v of values) {
                if (v.includes('<') || /^[A-Za-z0-9]+$/.test(v))
                    found.add(normal(v));
            }
        }
    };
    const use = (source, id, path = null) => {
        const schema = source?.lookup(id, true);
        if (!schema)
            return;
        const settings = path ? new Gio.Settings({settings_schema: schema, path}) : new Gio.Settings({settings_schema: schema});
        try {
            read(settings, schema);
            if (id === 'org.gnome.settings-daemon.plugins.media-keys') {
                for (const p of settings.get_strv('custom-keybindings'))
                    use(source, CUSTOM, p);
            }
        } finally {
            settings.run_dispose();
        }
    };
    const system = Gio.SettingsSchemaSource.get_default();
    for (const id of GNOME_SCHEMAS)
        use(system, id);
    try {
        const ours = Gio.SettingsSchemaSource.new_from_directory(schemaDir, system, false);
        for (const id of ours.list_schemas(true)[0])
            use(ours, id);
    } catch (e) {
        console.warn(`Amethystora UI: the extension's shortcuts could not be read: ${e.message}`);
    }
    return found;
}

import Gio from 'gi://Gio';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

export class PluginKeys {
    // run(id, action): a press, a person's click. refused(plugin, accel): said once when the combination is taken. schemaDir: the
    // extension's compiled schemas, whose shortcuts are Amethystora's
    constructor(run, refused, schemaDir) {
        this._run = run;
        this._refused = refused;
        this._schemaDir = schemaDir;
        this._held = new Map(); // id -> {accel, action, grabbed}
        this._said = new Set();
        global.display.connectObject('accelerator-activated', (display, grabbed) => {
            for (const [id, held] of this._held) {
                if (held.grabbed === grabbed)
                    this._run(id, held.action);
            }
        }, this);
    }

    sync(plugins) {
        const wanted = new Map(plugins.filter(p => p.places.key && p.granted.includes('key') && !p.paused).map(p => [p.id, p]));
        for (const [id, held] of [...this._held]) {
            const plugin = wanted.get(id);
            if (!plugin || plugin.places.key.accel !== held.accel || plugin.places.key.action !== held.action)
                this._release(id);
        }
        let taken = null;
        for (const [id, plugin] of wanted) {
            if (this._held.has(id))
                continue;
            const {accel, action} = plugin.places.key;
            taken ??= bound(this._schemaDir);
            const grabbed = taken.has(normal(accel)) ? Meta.KeyBindingAction.NONE : global.display.grab_accelerator(accel, Meta.KeyBindingFlags.NONE);
            if (grabbed === Meta.KeyBindingAction.NONE) {
                if (!this._said.has(`${id}|${accel}`)) {
                    this._said.add(`${id}|${accel}`);
                    this._refused(plugin, accel);
                }
                continue;
            }
            Main.wm.allowKeybinding(Meta.external_binding_name_for_action(grabbed), Shell.ActionMode.NORMAL | Shell.ActionMode.OVERVIEW);
            this._held.set(id, {accel, action, grabbed});
        }
    }

    _release(id) {
        const held = this._held.get(id);
        if (!held)
            return;
        global.display.ungrab_accelerator(held.grabbed);
        Main.wm.allowKeybinding(Meta.external_binding_name_for_action(held.grabbed), Shell.ActionMode.NONE);
        this._held.delete(id);
    }

    held() {
        return [...this._held.entries()].map(([id, h]) => `${id} ${h.accel}`);
    }

    destroy() {
        for (const id of [...this._held.keys()])
            this._release(id);
        global.display.disconnectObject(this);
    }
}
