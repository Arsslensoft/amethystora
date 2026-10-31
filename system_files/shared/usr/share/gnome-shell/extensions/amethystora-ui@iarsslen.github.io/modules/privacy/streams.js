// Whether a share shows only windows, asked of Mutter itself. The handle Mutter gives the shell for a share does not
// say what is shared, but its screen cast service lists every stream on the session bus, under
// /org/gnome/Mutter/ScreenCast/Stream, each with its parameters: a window's stream gives its size and nothing else (and
// may give the mapping id the portal uses), where a monitor's or a virtual monitor's gives more. An area's gives the same
// keys as a window's with the size (0, 0) (GNOME 50), so a size of nothing is no window. The shell asks its own service
// through the bus, so every call here is asynchronous.
//
// Only a positive answer counts: a stream is a window's when its parameters are exactly that, and every other case (a
// stream described any other way, a stream that went between the two questions, no streams at all, an error) is a screen.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

const BUS = 'org.gnome.Mutter.ScreenCast';
const STREAMS = '/org/gnome/Mutter/ScreenCast/Stream';
const STREAM = 'org.gnome.Mutter.ScreenCast.Stream';
const TIMEOUT_MS = 2000;
// What a window's stream says of itself, and all it says
const WINDOW_PARAMETERS = new Set(['size', 'mapping-id']);

function call(path, iface, method, params, reply, cancellable) {
    return new Promise((resolve, reject) => {
        Gio.DBus.session.call(BUS, path, iface, method, params, new GLib.VariantType(reply),
            Gio.DBusCallFlags.NO_AUTO_START, TIMEOUT_MS, cancellable, (bus, result) => {
                try {
                    resolve(bus.call_finish(result));
                } catch (e) {
                    reject(e);
                }
            });
    });
}

// The streams there are now, by their object path
async function streams(cancellable) {
    const [xml] = (await call(STREAMS, 'org.freedesktop.DBus.Introspectable', 'Introspect', null, '(s)',
        cancellable)).deepUnpack();
    const node = Gio.DBusNodeInfo.new_for_xml(xml);
    return node.nodes.map(child => child.path).filter(name => /^[A-Za-z0-9_]+$/.test(name ?? ''))
        .map(name => `${STREAMS}/${name}`);
}

// True only when there is at least one stream and every stream is a window's; anything else is false
export async function windowsOnly(cancellable) {
    try {
        const paths = await streams(cancellable);
        if (paths.length === 0)
            return false;
        for (const path of paths) {
            // eslint-disable-next-line no-await-in-loop
            const [parameters] = (await call(path, 'org.freedesktop.DBus.Properties', 'Get',
                new GLib.Variant('(ss)', [STREAM, 'Parameters']), '(v)', cancellable)).deepUnpack();
            const values = parameters.recursiveUnpack() ?? {};
            const keys = Object.keys(values);
            if (!keys.includes('size') || keys.some(key => !WINDOW_PARAMETERS.has(key)) || !values.size[0] || !values.size[1])
                return false;
        }
        return true;
    } catch (e) {
        if (e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
            throw e;
        return false;
    }
}
