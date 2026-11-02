// Hello, headphones (R7-19, the key hello): a device arriving grows the pill for two seconds, with its icon, its name and
// its battery. Sound devices come from the mixer GNOME already has open, sound devices with a battery and game controllers
// from UPower's own signal, a phone from the phone link and a dock from boltd's. A glance only: it asks nothing and holds
// no place. Nothing is said during a call, a recording, Focus or quiet hours, or while the bar cannot be seen, and a mouse
// or keyboard that wakes all day is not a device arriving. No timer but the two seconds, and a moment to let the two
// reports of one pair of headphones become one. The names are in memory only.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Gvc from 'gi://Gvc';
import Pango from 'gi://Pango';
import St from 'gi://St';
import UPower from 'gi://UPowerGlib';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import {getMixerControl} from 'resource:///org/gnome/shell/ui/status/volume.js';

import {retire} from './clock.js';
import {plain} from './text.js';
import {reading} from '../power/devices.js';

const UPOWER = 'org.freedesktop.UPower';
const UPOWER_PATH = '/org/freedesktop/UPower';
const BOLT = 'org.freedesktop.bolt';
const BOLT_PATH = '/org/freedesktop/bolt';
const BUS_TIMEOUT = 2000;
// How long it shows; how long two reports of one device take to become one; what is there as this starts is not arriving
const SHOW_MS = 2000;
const SETTLE_MS = 700;
const SEED_MS = 10000;
// Sound outputs that come with a person, as the mixer calls them
const WORN = ['headphone', 'headset', 'hands-free', 'handset', 'portable', 'car', 'hifi'];
// The kinds UPower says that are worth a hello
const SOUND = () => [UPower.DeviceKind.HEADSET, UPower.DeviceKind.HEADPHONES, UPower.DeviceKind.SPEAKERS, UPower.DeviceKind.OTHER_AUDIO];
const HELLO = () => [...SOUND(), UPower.DeviceKind.GAMING_INPUT];

// The phones last known to be here, across a lock (which stops the module and the link with it)
const phonesHere = new Set();

export class Hello {
    // quiet: whether nothing may be said now; mapped: whether the bar can be seen
    constructor(activities, {connect, quiet}) {
        this._activities = activities;
        this._connect = connect;
        this._quiet = quiet;
        this.id = 'hello';
        this.plate = 'hello';
        this.beside = false;
    }

    enable() {
        this._cancellable = new Gio.Cancellable();
        this._since = GLib.get_monotonic_time();
        this._mixer = getMixerControl();
        this._mixer?.connectObject('stream-added', (c, id) => this._streamAdded(id), this);
        const bus = Gio.DBus.system;
        this._upower = bus.signal_subscribe(UPOWER, UPOWER, 'DeviceAdded', UPOWER_PATH, null, Gio.DBusSignalFlags.NONE,
            (c, s, p, i, n, params) => this._deviceAdded(params.deepUnpack()[0]));
        this._bolt = bus.signal_subscribe(BOLT, `${BOLT}1.Manager`, 'DeviceAdded', BOLT_PATH, null, Gio.DBusSignalFlags.NONE,
            (c, s, p, i, n, params) => this._boltAdded(params.deepUnpack()[0]));
        this._unlisten = this._connect?.onPhonesChanged?.(() => this._phones()) ?? null;
        this._phones();
    }

    disable() {
        this._cancellable?.cancel();
        this._cancellable = null;
        this._mixer?.disconnectObject(this);
        this._mixer = null;
        for (const id of [this._upower, this._bolt]) {
            if (id)
                Gio.DBus.system.signal_unsubscribe(id);
        }
        this._upower = this._bolt = 0;
        this._unlisten?.();
        this._unlisten = null;
        this._stopTimers();
        this._arrival = null;
        this._end();
    }

    _stopTimers() {
        for (const name of ['_settle', '_showing']) {
            if (this[name])
                GLib.source_remove(this[name]);
            this[name] = 0;
        }
    }

    get _seeding() {
        return GLib.get_monotonic_time() - this._since < SEED_MS * 1000;
    }

    // --- Who arrived ----------------------------------------------------------------------------------------------

    _streamAdded(id) {
        if (this._seeding || !this._mixer || this._mixer.get_state() !== Gvc.MixerControlState.READY)
            return;
        const stream = this._mixer.lookup_stream_id(id);
        if (!(stream instanceof Gvc.MixerSink))
            return;
        const worn = WORN.includes(stream.get_form_factor?.() ?? '') || (stream.get_name?.() ?? '').startsWith('bluez');
        if (!worn)
            return;
        this._heard({stream, name: plain(stream.get_description() ?? '', 40), icon: 'audio-headphones-symbolic', sound: true});
    }

    _deviceAdded(path) {
        if (this._seeding)
            return;
        Gio.DBus.system.call(UPOWER, path, 'org.freedesktop.DBus.Properties', 'GetAll', new GLib.Variant('(s)', [`${UPOWER}.Device`]),
            new GLib.VariantType('(a{sv})'), Gio.DBusCallFlags.NO_AUTO_START, BUS_TIMEOUT, this._cancellable, (connection, result) => {
                let r = null;
                try {
                    const [props] = connection.call_finish(result).recursiveUnpack();
                    r = reading(path, props);
                } catch {}
                if (r && HELLO().includes(r.kind)) {
                    this._heard({name: r.model || r.label, icon: r.icon, percent: Math.round(r.percent),
                        sound: SOUND().includes(r.kind)});
                }
            });
    }

    _boltAdded(path) {
        if (this._seeding)
            return;
        Gio.DBus.system.call(BOLT, path, 'org.freedesktop.DBus.Properties', 'GetAll', new GLib.Variant('(s)', [`${BOLT}1.Device`]),
            new GLib.VariantType('(a{sv})'), Gio.DBusCallFlags.NO_AUTO_START, BUS_TIMEOUT, this._cancellable, (connection, result) => {
                try {
                    const [props] = connection.call_finish(result).recursiveUnpack();
                    this._heard({name: plain(props.Name ?? '', 40) || _('Dock'), icon: 'thunderbolt-symbolic'});
                } catch {}
            });
    }

    _phones() {
        const here = (this._connect?.phones?.() ?? []);
        const names = new Set(here.map(phone => phone.name));
        for (const name of [...phonesHere]) {
            if (!names.has(name))
                phonesHere.delete(name);
        }
        for (const phone of here) {
            if (!phonesHere.has(phone.name)) {
                phonesHere.add(phone.name);
                if (!this._seeding) {
                    this._heard({name: plain(phone.name ?? '', 40) || _('Your phone'), icon: 'smartphone-symbolic',
                        percent: phone.level >= 0 ? phone.level : undefined});
                }
            }
        }
    }

    // Two reports of one pair of headphones (the mixer's and UPower's) a moment apart become one
    _heard(news) {
        this._arrival = this._arrival?.sound && news.sound ? {...this._arrival, ...news, name: this._arrival.name || news.name} : news;
        if (this._settle)
            return;
        this._settle = GLib.timeout_add(GLib.PRIORITY_DEFAULT, SETTLE_MS, () => {
            this._settle = 0;
            const arrival = this._arrival;
            this._arrival = null;
            if (arrival)
                this._say(arrival);
            return GLib.SOURCE_REMOVE;
        });
    }

    // --- In the pill ----------------------------------------------------------------------------------------------

    _say(arrival) {
        if (this._quiet() || this._showing)
            return;
        // A stream that is now the default output means the sound moved there
        arrival.moved = !!arrival.stream && this._mixer?.get_default_sink?.() === arrival.stream;
        arrival.stream = null;
        this._shown = arrival;
        this.pill();
        this._icon.icon_name = arrival.icon;
        this._label.text = this.text;
        this._activities.set('hello', this);
        // It holds no place: under anything that was there first it says nothing at all
        if (this._activities.top !== this) {
            this._end();
            return;
        }
        this._showing = GLib.timeout_add(GLib.PRIORITY_DEFAULT, SHOW_MS, () => {
            this._showing = 0;
            this._end();
            return GLib.SOURCE_REMOVE;
        });
    }

    _end() {
        this._shown = null;
        if (this._activities.get('hello') === this)
            this._activities.set('hello', null);
        retire(this._content);
        this._content = null;
        this._icon = this._label = null;
    }

    get text() {
        const shown = this._shown;
        if (!shown)
            return '';
        return Number.isFinite(shown.percent) ? `${shown.name} · ${shown.percent}%` : shown.name;
    }

    get accessible() {
        return _('%s connected').format(this.text);
    }

    get row() {
        return {icon: this._shown?.icon ?? 'emblem-ok-symbolic', text: this.text};
    }

    pill() {
        if (!this._content) {
            this._content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-pill-hello'});
            this._icon = new St.Icon({icon_size: 15, y_align: Clutter.ActorAlign.CENTER});
            this._label = new St.Label({style_class: 'amethystora-pill-hello-text', y_align: Clutter.ActorAlign.CENTER});
            this._label.clutter_text.ellipsize = Pango.EllipsizeMode.END;
            this._content.add_child(this._icon);
            this._content.add_child(this._label);
        }
        return this._content;
    }

    card() {
        const body = new St.BoxLayout({style_class: 'amethystora-pill-card-body', orientation: Clutter.Orientation.VERTICAL});
        const shown = this._shown;
        const title = new St.Label({style_class: 'amethystora-pill-card-label', text: _('%s connected').format(shown?.name ?? '')});
        title.clutter_text.ellipsize = Pango.EllipsizeMode.END;
        body.add_child(title);
        const lines = [];
        if (Number.isFinite(shown?.percent))
            lines.push(`${shown.percent}%`);
        if (shown?.moved)
            lines.push(_('sound moved here'));
        if (lines.length)
            body.add_child(new St.Label({style_class: 'amethystora-pill-card-text', text: lines.join(' · ')}));
        return body;
    }

    get shown() {
        return this._shown ?? null;
    }

    // For the harness: as if this device arrived now
    arriveForTest(news) {
        this._since = 0;
        this._heard(news);
    }
}
