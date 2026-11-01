// What the shield reads, each when it is shown and never on a timer: the security report as the watcher last wrote it
// (for the administrators who may read it), the network's firewall zone, encrypted DNS, USB protection, the Private
// folder, and GNOME's camera and microphone switches. Every value comes from what the image already writes or a
// service already answers; nothing here works out a security state of its own, and nothing is changed.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

export const REPORT = '/var/lib/amethystora/security/report.json';
const HARDENING_ON = '/etc/amethystora/hardening-on';
const TIMEOUT_MS = 3000;
// The report's checks the desktop never shows (20-tests.sh holds these two)
const HIDDEN = /^duress-/;
const HIDDEN_TEXT = /duress/i;

function call(bus, name, path, iface, method, params, reply, cancellable) {
    return new Promise((resolve, reject) => {
        bus.call(name, path, iface, method, params, reply ? new GLib.VariantType(reply) : null,
            Gio.DBusCallFlags.NO_AUTO_START, TIMEOUT_MS, cancellable, (connection, result) => {
                try {
                    resolve(connection.call_finish(result));
                } catch (e) {
                    reject(e);
                }
            });
    });
}

async function property(bus, name, path, iface, key, cancellable) {
    const [value] = (await call(bus, name, path, 'org.freedesktop.DBus.Properties', 'Get',
        new GLib.Variant('(ss)', [iface, key]), '(v)', cancellable)).deepUnpack();
    return value.deepUnpack();
}

function readFile(path, cancellable) {
    return new Promise(resolve => {
        Gio.File.new_for_path(path).load_contents_async(cancellable, (file, result) => {
            try {
                const [, bytes] = file.load_contents_finish(result);
                resolve(new TextDecoder().decode(bytes));
            } catch {
                resolve(null);
            }
        });
    });
}

// The report: null when it cannot be read (not an administrator, no run yet, report alerts off); else the checks that
// need a look (their texts), how many are fine, and when it was written
export async function report(cancellable) {
    const text = await readFile(REPORT, cancellable);
    if (text === null)
        return null;
    let checks;
    try {
        checks = JSON.parse(text);
    } catch {
        return null;
    }
    if (!Array.isArray(checks))
        return null;
    // What the security report keeps to itself is never on the desktop: such a check is neither said nor counted,
    // so that nothing glanced at changes because of it
    checks = checks.filter(c => c && !HIDDEN.test(String(c.id ?? '')) && !HIDDEN_TEXT.test(String(c.text ?? '')));
    const failing = checks.filter(c => c.state === 'check' || c.state === 'off')
        .map(c => typeof c.text === 'string' ? c.text : '');
    const fine = checks.filter(c => c.state === 'ok').length;
    let written = 0;
    try {
        const info = Gio.File.new_for_path(REPORT).query_info('time::modified', Gio.FileQueryInfoFlags.NONE, null);
        written = info.get_attribute_uint64('time::modified');
    } catch {}
    return {failing, fine, written};
}

// The network the machine is on: its name, and its firewall zone as firewalld says (the same read-only question the
// phone link asks); null offline
export async function network(cancellable) {
    const system = Gio.DBus.system;
    const NM = 'org.freedesktop.NetworkManager';
    const primary = await property(system, NM, '/org/freedesktop/NetworkManager', NM, 'PrimaryConnection', cancellable);
    if (!primary || primary === '/')
        return null;
    const active = `${NM}.Connection.Active`;
    const [id, type, devices] = await Promise.all([
        property(system, NM, primary, active, 'Id', cancellable),
        property(system, NM, primary, active, 'Type', cancellable),
        property(system, NM, primary, active, 'Devices', cancellable),
    ]);
    let zone = '';
    try {
        const iface = devices?.[0]
            ? await property(system, NM, devices[0], `${NM}.Device`, 'IpInterface', cancellable) ||
              await property(system, NM, devices[0], `${NM}.Device`, 'Interface', cancellable)
            : '';
        const FW = 'org.fedoraproject.FirewallD1';
        if (iface)
            [zone] = (await call(system, FW, '/org/fedoraproject/FirewallD1', `${FW}.zone`, 'getZoneOfInterface',
                new GLib.Variant('(s)', [iface]), '(s)', cancellable)).deepUnpack();
        if (!zone)
            [zone] = (await call(system, FW, '/org/fedoraproject/FirewallD1', `${FW}.zone`, 'getDefaultZone',
                null, '(s)', cancellable)).deepUnpack();
    } catch (e) {
        if (e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
            throw e;
    }
    return {name: typeof id === 'string' ? id : '', wireless: type === '802-11-wireless', zone};
}

// Encrypted DNS as systemd-resolved has it: its mode (yes, opportunistic, no) and the TLS name of the server in use
export async function dns(cancellable) {
    const R = 'org.freedesktop.resolve1';
    const path = '/org/freedesktop/resolve1';
    const mode = await property(Gio.DBus.system, R, path, `${R}.Manager`, 'DNSOverTLS', cancellable);
    let server = '';
    try {
        const current = await property(Gio.DBus.system, R, path, `${R}.Manager`, 'CurrentDNSServerEx', cancellable);
        server = typeof current?.[4] === 'string' ? current[4] : '';
    } catch {}
    return {mode: typeof mode === 'string' ? mode : 'no', server};
}

// USB protection as the report names it: off, on (every new device waits), or locked (new devices wait while the
// screen is locked: usbguard on, its D-Bus service not masked)
export async function usb(cancellable) {
    const S = 'org.freedesktop.systemd1';
    const state = async unit => {
        try {
            const [value] = (await call(Gio.DBus.system, S, '/org/freedesktop/systemd1', `${S}.Manager`,
                'GetUnitFileState', new GLib.Variant('(s)', [unit]), '(s)', cancellable)).deepUnpack();
            return value;
        } catch {
            return '';
        }
    };
    const [guard, bus] = await Promise.all([state('usbguard.service'), state('usbguard-dbus.service')]);
    if (guard !== 'enabled')
        return 'off';
    // The report's own reading: GNOME's mode needs the D-Bus service, which the other mode masks
    return bus === '' || bus.startsWith('masked') ? 'on' : 'locked';
}

// The Private folder: whether it is set up, and whether it is open (mounted at ~/Private)
export async function privateFolder(cancellable) {
    const home = GLib.get_home_dir();
    // Set up as the report says it: gocryptfs has written its configuration
    const store = GLib.build_filenamev([home, '.local', 'share', 'amethystora', 'private', 'gocryptfs.conf']);
    const setUp = GLib.file_test(store, GLib.FileTest.IS_REGULAR);
    if (!setUp)
        return {setUp, open: false};
    const mounts = await readFile('/proc/self/mountinfo', cancellable) ?? '';
    // The mount point is the fifth field, with spaces and the like written as octal escapes
    const target = GLib.build_filenamev([home, 'Private']);
    const open = mounts.split('\n').some(line => {
        const point = line.split(' ')[4];
        return point && point.replace(/\\([0-7]{3})/g, (m, o) => String.fromCharCode(parseInt(o, 8))) === target;
    });
    return {setUp, open};
}

// Whether the webcam is blocked by the stricter setting block-webcam
export async function webcamBlocked(cancellable) {
    const text = await readFile(HARDENING_ON, cancellable) ?? '';
    return text.split('\n').some(line => line.trim().split(/\s+/)[0] === 'block-webcam');
}
