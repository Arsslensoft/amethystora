// Which desk the computer is at: the screens connected to it other than its own, each by the maker, the model and the
// serial number its EDID gives (as Mutter's own memory of monitor layouts keys them), and the Thunderbolt devices
// plugged in. A screen without a serial number, or two alike, are told apart by the connector they are on as well.
// Virtual screens (a remote session's, a screen share's) come and go with what made them and are not part of a desk.
//
// Read when the screens change, never on a timer: a list of monitors, and a handful of small files under
// /sys/bus/thunderbolt.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';

const THUNDERBOLT = '/sys/bus/thunderbolt/devices';
// A device of a Thunderbolt domain (0-1, 1-301): the host's own router is N-0
const DEVICE = /^\d+-[1-9]\d*$/;
const MOST = 64;

const text = value => String(value ?? '').replace(/[\p{Cc}\p{Cf}]/gu, '').slice(0, MOST);

// The Thunderbolt devices plugged in, by the unique id each says
function thunderbolt() {
    const ids = [];
    try {
        const folder = Gio.File.new_for_path(THUNDERBOLT);
        const children = folder.enumerate_children('standard::name', Gio.FileQueryInfoFlags.NONE, null);
        let info;
        while ((info = children.next_file(null))) {
            const name = info.get_name();
            if (!DEVICE.test(name))
                continue;
            try {
                const [, bytes] = GLib.file_get_contents(`${THUNDERBOLT}/${name}/unique_id`);
                const id = text(new TextDecoder().decode(bytes).trim());
                if (id)
                    ids.push(id);
            } catch {}
        }
        children.close(null);
    } catch {}
    return ids.sort();
}

// The desk the computer is at now: {id, base, screens, thunderbolt, builtin, lidShut}. The screens make the desk
// (base); the Thunderbolt devices only tell apart two desks with the same screens (id), so that a Thunderbolt drive
// plugged in at a desk does not make it another one (DesksState.find)
export function readDesk() {
    const manager = global.backend.get_monitor_manager();
    const monitors = manager.get_monitors().filter(m => !m.is_virtual());
    const own = monitors.filter(m => m.is_builtin());
    const others = monitors.filter(m => !m.is_builtin()).map(m => ({
        vendor: text(m.get_vendor()),
        product: text(m.get_product()),
        serial: text(m.get_serial()),
        connector: text(m.get_connector()),
        name: text(m.get_display_name()),
    }));
    const base = s => `${s.vendor}|${s.product}|${s.serial}`;
    const keys = others.map(s => {
        const twins = others.filter(o => base(o) === base(s)).length;
        return !s.serial || twins > 1 ? `${base(s)}|${s.connector}` : base(s);
    });
    const devices = thunderbolt();
    const hash = key => GLib.compute_checksum_for_string(GLib.ChecksumType.SHA256, `amethystora-desk\n${key}`, -1)
        .slice(0, 12);
    keys.sort();
    others.sort((a, b) => a.connector.localeCompare(b.connector));
    return {
        id: hash([...keys, ...devices.map(id => `tb|${id}`)].join('\n')),
        base: hash(keys.join('\n')),
        screens: others,
        thunderbolt: devices.length,
        builtin: own.length > 0,
        lidShut: own.length > 0 && own.every(m => !m.is_active()),
    };
}

// A desk's name until it is renamed: what it has
export function defaultName(desk) {
    const count = desk.screens?.length ?? 0;
    if (count === 0)
        return desk.builtin === false ? _('This computer') : _('The laptop alone');
    if (count === 1)
        return desk.screens[0].name || desk.screens[0].product || _('One screen');
    return ngettext('%d screen', '%d screens', count).format(count);
}

// What a desk has, said under its name on the card
export function hasWords(desk) {
    const count = desk.screens?.length ?? 0;
    if (count === 0)
        return _('No other screen');
    if (desk.lidShut)
        return ngettext('%d screen, the lid shut', '%d screens, the lid shut', count).format(count);
    return ngettext('%d screen', '%d screens', count).format(count);
}
