// Is it me or the internet? In the Wi-Fi and Wired menus of quick settings, while the computer is on that connection: a
// drawing of you, the router and the internet, with the line that broke in the theme's error colour, and one sentence
// saying what it means and what to try. A check runs once each time the menu opens, and when the connection changes
// while it is open: one ping to the router and, only while NetworkManager's own connectivity check is on, one lookup of the
// name that check already asks, through systemd-resolved: so it contacts nothing but the router and the name server the
// machine already uses, for a name the machine already looks up. For a connection that is not the one the internet goes
// through only the router is asked. Nothing runs while the menu is closed, and nothing is kept.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import NM from 'gi://NM';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

import {arpAnswers, hostOf, NAME_FLAGS, nameFailure, verdict} from './reachlogic.js';
import {run} from './run.js';

Gio._promisify(Gio.File.prototype, 'load_contents_async');

const PING = '/usr/bin/ping';
const RESOLVE = 'org.freedesktop.resolve1';
const RESOLVE_PATH = '/org/freedesktop/resolve1';
// A router has two seconds to answer, a name server four
const ROUTER_TIMEOUT = 2;
const NAME_TIMEOUT = 4000;

const LINE_STYLES = ['wait', 'ok', 'warn', 'bad', 'off'];

// Whether this is the connection the internet goes through: NetworkManager's check and the name lookup speak for that one alone
function isMain(client, device) {
    return !!client && client.primary_connection === device.active_connection;
}

function connectivityWord(client, device) {
    if (!isMain(client, device))
        return 'unknown';
    switch (client.connectivity) {
    case NM.ConnectivityState.FULL: return 'full';
    case NM.ConnectivityState.LIMITED: return 'limited';
    case NM.ConnectivityState.PORTAL: return 'portal';
    case NM.ConnectivityState.NONE: return 'none';
    default: return 'unknown';
    }
}

function activeDevice(toggle) {
    return [...toggle?._nmDevices ?? []].find(device =>
        device.get_active_connection?.()?.get_state?.() === NM.ActiveConnectionState.ACTIVATED) ?? null;
}

// The gateway as ping takes it, from NetworkManager, an address and nothing else: an argument vector, or null
function pingArguments(device) {
    const four = device.get_ip4_config?.()?.get_gateway?.();
    if (four && Gio.InetAddress.new_from_string(four))
        return [PING, '-4', '-n', '-q', '-c', '1', '-W', `${ROUTER_TIMEOUT}`, '--', four];
    const six = device.get_ip6_config?.()?.get_gateway?.();
    const iface = device.get_iface?.();
    if (six && iface && Gio.InetAddress.new_from_string(six))
        return [PING, '-6', '-n', '-q', '-c', '1', '-W', `${ROUTER_TIMEOUT}`, '-I', iface, '--', six];
    return null;
}

const cancelled = e => !!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED);

// 'ok' when it answers a ping, 'arp' when it does not but answers at the link (some routers never answer pings), 'silent'
// when neither, 'none' without a gateway. A ping that cannot even be started is silence, not a drawing stuck on "Looking…"
async function askRouter(device, cancellable) {
    const argv = pingArguments(device);
    if (!argv)
        return 'none';
    try {
        const {ok} = await run(argv, {timeout: (ROUTER_TIMEOUT + 2) * 1000, cancellable});
        if (ok)
            return 'ok';
    } catch (e) {
        if (cancelled(e))
            throw e;
        return 'silent';
    }
    if (argv[1] === '-4') {
        try {
            const [bytes] = await Gio.File.new_for_path('/proc/net/arp').load_contents_async(cancellable);
            if (arpAnswers(new TextDecoder().decode(bytes), argv.at(-1), device.get_iface?.()))
                return 'arp';
        } catch (e) {
            if (cancelled(e))
                throw e;
        }
    }
    return 'silent';
}

async function askName(host, cancellable) {
    try {
        await Gio.DBus.system.call(RESOLVE, RESOLVE_PATH, `${RESOLVE}.Manager`, 'ResolveHostname',
            new GLib.Variant('(isit)', [0, host, 0, NAME_FLAGS]), new GLib.VariantType('(a(iiay)st)'),
            Gio.DBusCallFlags.NO_AUTO_START, NAME_TIMEOUT, cancellable);
        return 'ok';
    } catch (e) {
        if (cancelled(e))
            throw e;
        return nameFailure(Gio.DBusError.is_remote_error(e) ? Gio.DBusError.get_remote_error(e) : null);
    }
}

// Whether resolved sends names over TLS (ame security dns): asked only when a lookup failed, to say so in the words
async function askEncrypted(cancellable) {
    try {
        const reply = await Gio.DBus.system.call(RESOLVE, RESOLVE_PATH, 'org.freedesktop.DBus.Properties', 'Get',
            new GLib.Variant('(ss)', [`${RESOLVE}.Manager`, 'DNSOverTLS']), new GLib.VariantType('(v)'),
            Gio.DBusCallFlags.NO_AUTO_START, 2000, cancellable);
        return reply.get_child_value(0).get_variant().get_string()[0] === 'yes';
    } catch (e) {
        if (cancelled(e))
            throw e;
        return false;
    }
}

// The drawing and its sentence, as an item of the menu
function makeNode(icon, text) {
    const box = new St.BoxLayout({style_class: 'amethystora-tools-reach-node', orientation: Clutter.Orientation.VERTICAL});
    box.add_child(new St.Icon({icon_name: icon, icon_size: 22, x_align: Clutter.ActorAlign.CENTER}));
    box.add_child(new St.Label({style_class: 'amethystora-tools-reach-name', text, x_align: Clutter.ActorAlign.CENTER}));
    return box;
}

// One menu: the Wi-Fi's or the wired connection's
class ReachMenu {
    constructor(name, wireless) {
        this._name = name;
        this._wireless = wireless;
    }

    _toggle() {
        return Main.panel.statusArea.quickSettings?._network?.[this._name] ?? null;
    }

    attach() {
        const toggle = this._toggle();
        if (this._menu)
            return true;
        if (!toggle?.menu)
            return false;
        this._menu = toggle.menu;
        this._item = new PopupMenu.PopupBaseMenuItem({reactive: false, can_focus: false});
        this._item.add_style_class_name('amethystora-tools-reach');
        this._item.visible = false;
        const box = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_expand: true});
        const row = new St.BoxLayout({style_class: 'amethystora-tools-reach-drawing', x_expand: true});
        this._first = new St.Widget({style_class: 'amethystora-tools-reach-line', x_expand: true, y_align: Clutter.ActorAlign.CENTER});
        this._second = new St.Widget({style_class: 'amethystora-tools-reach-line', x_expand: true, y_align: Clutter.ActorAlign.CENTER});
        row.add_child(makeNode('computer-symbolic', _('You')));
        row.add_child(this._first);
        row.add_child(makeNode('go-home-symbolic', _('Router')));
        row.add_child(this._second);
        row.add_child(makeNode('web-browser-symbolic', _('Internet')));
        box.add_child(row);
        this._sentence = new St.Label({style_class: 'amethystora-tools-reach-sentence', x_expand: true});
        this._sentence.clutter_text.line_wrap = true;
        box.add_child(this._sentence);
        this._item.add_child(box);
        // First in the menu, above the networks
        this._menu.addMenuItem(this._item, 0);
        this._menu.connectObject('open-state-changed', (menu, open) => {
            if (open)
                this._opened();
            else
                this._closed();
        }, this);
        return true;
    }

    detach() {
        this._closed();
        this._menu?.disconnectObject(this);
        this._menu = null;
        this._item?.destroy();
        this._item = null;
    }

    _opened() {
        const toggle = this._toggle();
        this._client = toggle?._client ?? null;
        this._client?.connectObject(
            'notify::primary-connection', () => this._check(),
            'notify::connectivity', () => this._check(),
            this);
        this._check();
    }

    _closed() {
        this._client?.disconnectObject(this);
        this._client = null;
        this._cancellable?.cancel();
        this._cancellable = null;
        this._reading = null;
    }

    _check() {
        const device = activeDevice(this._toggle());
        this._cancellable?.cancel();
        this._cancellable = null;
        if (!device) {
            this._item.visible = false;
            this._reading = null;
            return;
        }
        const cancellable = new Gio.Cancellable();
        this._cancellable = cancellable;
        this._device = device;
        // The name is the one NetworkManager's check asks, and is asked only while that check is on and only for the connection
        // the internet goes through: for anything else the lookup would be the machine's, not this link's
        const host = isMain(this._client, device) && this._client.connectivity_check_enabled
            ? hostOf(this._client.connectivity_check_uri) : null;
        this._reading = {router: 'wait', name: host ? 'wait' : 'skip'};
        this._item.visible = true;
        this._show();
        const fill = (key, ask) => ask.then(value => {
            if (cancellable.is_cancelled() || !this._reading)
                return;
            this._reading[key] = value;
            this._show();
        }).catch(e => {
            if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                console.warn(`Amethystora UI: the network's check: ${e.message}`);
        });
        fill('router', askRouter(device, cancellable));
        if (host) {
            fill('name', askName(host, cancellable).then(async name => {
                if (name === 'fail' && !cancellable.is_cancelled() && this._reading)
                    this._reading.tls = await askEncrypted(cancellable);
                return name;
            }));
        }
    }

    _show() {
        this._draw(verdict({...this._reading, connectivity: connectivityWord(this._client, this._device),
            main: isMain(this._client, this._device)}), this._reading?.tls === true);
    }

    _draw(seen, encrypted) {
        this._paint(this._first, seen.first);
        this._paint(this._second, seen.second);
        this._sentence.text = this._words(seen.say, encrypted);
        this._seen = seen;
    }

    _paint(line, state) {
        for (const style of LINE_STYLES)
            line.remove_style_class_name(`amethystora-tools-reach-${style}`);
        line.add_style_class_name(`amethystora-tools-reach-${state}`);
    }

    _words(say, encrypted = false) {
        const wifi = this._wireless;
        switch (say) {
        case 'checking':
            return _('Looking…');
        case 'fine':
            return wifi ? _('Your computer and Wi-Fi are fine, and the internet answers.')
                : _('Your computer and its cable are fine, and the internet answers.');
        case 'beyond':
            return wifi ? _('Your computer and Wi-Fi are fine. Your router may not be reaching the internet, so try restarting it.')
                : _('Your computer and its cable are fine. Your router may not be reaching the internet, so try restarting it.');
        case 'router':
            return wifi ? _('Your router does not seem to answer, and the internet cannot be reached. It may be off or out of range: move closer to it, or restart it.')
                : _('Your router does not seem to answer, and the internet cannot be reached. Check the cable, or restart the router.');
        case 'dns':
            return encrypted ? _('The internet may be reachable, but your name server did not answer. Encrypted DNS is on, and this network may be blocking it.')
                : _('The internet may be reachable, but your name server did not answer, so names may not work for a while.');
        case 'no-gateway':
            return _('This connection has no router to reach the internet through.');
        case 'silent':
            return _('Your router did not answer a ping. Some routers never do, so this cannot say whether the internet is reachable.');
        case 'side-ok':
            return _('Your router answers. This is not the connection your computer uses for the internet, so only the router was asked.');
        case 'side-silent':
            return _('Your router did not answer a ping. This is not the connection your computer uses for the internet, so only the router was asked.');
        case 'side-none':
            return _('This connection has no router, and it is not the one your computer uses for the internet.');
        case 'portal':
            return _('Your router answers, but this network wants you to sign in first.');
        case 'limited':
            return _('Your router and name server answer, but the internet check did not get through. The internet may be partly down.');
        default:
            return _('Your router answers. Whether the internet does could not be asked.');
        }
    }

    get state() {
        return {
            attached: !!this._menu, shown: !!this._item?.visible, checking: !!this._reading,
            first: this._seen?.first ?? null, second: this._seen?.second ?? null, say: this._seen?.say ?? null,
            sentence: this._sentence?.text ?? null,
        };
    }

    // For the harness: what the drawing shows for a reading, with no network asked
    showForTest(reading, connectivity, main = true) {
        this._item.visible = true;
        this._draw(verdict({...reading, connectivity, main}), reading.tls === true);
    }
}

export class Reach {
    enable() {
        this._menus = [new ReachMenu('_wirelessToggle', true), new ReachMenu('_wiredToggle', false)];
        // GNOME makes its quick settings indicators while it starts: a menu not there yet is joined when it comes
        if (!this._attach())
            Main.panel.statusArea.quickSettings?._indicators?.connectObject('child-added', () => this._attach(), this);
    }

    _attach() {
        const all = this._menus.map(menu => menu.attach()).every(Boolean);
        if (all)
            Main.panel.statusArea.quickSettings?._indicators?.disconnectObject(this);
        return all;
    }

    disable() {
        Main.panel.statusArea.quickSettings?._indicators?.disconnectObject(this);
        for (const menu of this._menus ?? [])
            menu.detach();
        this._menus = null;
    }

    // For the suites and the harness
    get menus() {
        return this._menus;
    }

    get state() {
        return (this._menus ?? []).map(menu => menu.state);
    }
}
