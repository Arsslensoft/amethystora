// The battery: resting the pointer on it in the top bar says how long it lasts or how long until it is full, from
// UPower's own estimate; and at 20%, the pill offers power saving once. It never turns power saving on by itself:
// only the person's Turn on does, as GNOME's own switch would. On a machine without a battery there is nothing.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';
import UPower from 'gi://UPowerGlib';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {loadInterfaceXML} from 'resource:///org/gnome/shell/misc/fileUtils.js';

import {buttonRow, cardButton, chip, retire} from './clock.js';
import {hoursMinutes} from './text.js';

const UPOWER = 'org.freedesktop.UPower';
const DISPLAY_DEVICE = '/org/freedesktop/UPower/devices/DisplayDevice';
const PROFILES = 'org.freedesktop.UPower.PowerProfiles';
const PROFILES_PATH = '/org/freedesktop/UPower/PowerProfiles';
// The charge at which power saving is offered, and how long the pointer rests before the time left shows (ms)
const LOW = 20;
const HOVER_DELAY = 300;

let DeviceProxy = null;

// What the battery shows for a reading, without anything of the shell: {offer, hover, detail}. `profile` is the power
// profile's name ('power-saver' while power saving is on).
export function batteryView({present, state, percentage, timeToEmpty = 0, timeToFull = 0, profile = ''}) {
    if (!present)
        return {offer: false, hover: null, detail: null};
    const discharging = state === UPower.DeviceState.DISCHARGING || state === UPower.DeviceState.PENDING_DISCHARGE;
    const charging = state === UPower.DeviceState.CHARGING;
    let hover = null;
    let detail = null;
    if (discharging && timeToEmpty > 0) {
        hover = _('%s left').format(hoursMinutes(timeToEmpty));
        detail = _('from the battery\'s own estimate');
    } else if (charging && timeToFull > 0) {
        hover = _('Full in %s').format(hoursMinutes(timeToFull));
        detail = _('from the battery\'s own estimate');
    } else if (discharging || charging) {
        hover = _('Working out the time left');
    } else if (state === UPower.DeviceState.FULLY_CHARGED) {
        hover = _('Fully charged');
    } else if (state === UPower.DeviceState.PENDING_CHARGE) {
        hover = _('Plugged in, not charging');
    }
    const offer = discharging && percentage <= LOW && profile !== 'power-saver';
    return {offer, hover, detail};
}

export class Battery {
    constructor(activities, notice, comfort, glass) {
        this.id = 'battery';
        this.plate = 'battery';
        this._activities = activities;
        this._notice = notice;
        this._comfort = comfort;
        this._glass = glass;
    }

    enable() {
        this._cancellable = new Gio.Cancellable();
        this._armed = true;
        DeviceProxy ??= Gio.DBusProxy.makeProxyWrapper(loadInterfaceXML('org.freedesktop.UPower.Device'));
        new DeviceProxy(Gio.DBus.system, UPOWER, DISPLAY_DEVICE, (proxy, error) => {
            if (error || !this._cancellable)
                return;
            this._proxy = proxy;
            proxy.connectObject('g-properties-changed', () => this._sync(), this);
            this._sync();
        }, this._cancellable, Gio.DBusProxyFlags.DO_NOT_AUTO_START);
        this._unlistenComfort = this._comfort?.listen(() => this._sync());
        // Whether power-profiles-daemon is there to take Turn on
        this._profilesWatch = Gio.bus_watch_name(Gio.BusType.SYSTEM, PROFILES, Gio.BusNameWatcherFlags.NONE,
            () => {
                this._profilesUp = true;
                this._sync();
            },
            () => {
                this._profilesUp = false;
                this._sync();
            });

        // The time left, when the pointer rests on GNOME's battery in the top bar
        const indicator = Main.panel.statusArea.quickSettings?._system;
        if (indicator) {
            this._indicator = indicator;
            this._trackHover = indicator.track_hover;
            indicator.track_hover = true;
            indicator.connectObject('notify::hover', () => this._hoverChanged(), this);
        }
        Main.panel.statusArea.quickSettings?.menu?.connectObject('open-state-changed', () => this._hideTip(), this);
    }

    disable() {
        this._cancellable?.cancel();
        this._cancellable = null;
        this._proxy?.disconnectObject(this);
        this._proxy = null;
        this._unlistenComfort?.();
        this._unlistenComfort = null;
        if (this._profilesWatch)
            Gio.bus_unwatch_name(this._profilesWatch);
        this._profilesWatch = 0;
        if (this._indicator) {
            this._indicator.disconnectObject(this);
            this._indicator.track_hover = this._trackHover;
            this._indicator = null;
        }
        Main.panel.statusArea.quickSettings?.menu?.disconnectObject(this);
        this._hideTip();
        this._end();
    }

    reading() {
        const proxy = this._proxy;
        if (!proxy)
            return {present: false};
        return {
            present: proxy.IsPresent === true,
            state: proxy.State,
            percentage: proxy.Percentage ?? 100,
            timeToEmpty: Number(proxy.TimeToEmpty ?? 0),
            timeToFull: Number(proxy.TimeToFull ?? 0),
            profile: this._comfort?.powerSaver ? 'power-saver' : '',
        };
    }

    _sync() {
        const reading = this.reading();
        const view = batteryView(reading);
        // The offer comes once, and again only after the battery has been above 20% and falls again
        if (!reading.present || reading.percentage > LOW || reading.state === UPower.DeviceState.CHARGING)
            this._armed = true;
        // Without the comfort module (which says whether power saving is on) or power-profiles-daemon, no offer
        const canOffer = !!this._comfort && this._profilesOwned();
        if (view.offer && this._armed && canOffer)
            this._show();
        else if (!view.offer)
            this._end();
        if (this._tip)
            this._fillTip(view);
    }

    _profilesOwned() {
        return this._profilesUp === true;
    }

    // --- The offer ------------------------------------------------------------------------------------------------

    get text() {
        return _('Power saving?');
    }

    get accessible() {
        return _('Battery at %d percent: turn on power saving?').format(Math.round(this.reading().percentage));
    }

    get row() {
        return {icon: 'battery-caution-symbolic', text: _('Battery at %d%%').format(Math.round(this.reading().percentage))};
    }

    pill() {
        if (!this._content) {
            this._content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-pill-battery'});
            this._content.add_child(new St.Label({text: this.text, y_align: Clutter.ActorAlign.CENTER}));
            const button = chip(_('Turn on'), () => this.turnOn());
            button.add_style_class_name('amethystora-pill-battery-button');
            this._content.add_child(button);
        }
        return this._content;
    }

    _show() {
        this.pill();
        this._activities.set('battery', this);
    }

    _end() {
        if (this._activities.get('battery') === this)
            this._activities.set('battery', null);
        retire(this._content);
        this._content = null;
    }

    // Asked for once: Not now and Turn on both end it until the battery has been charged above 20% again
    _answered() {
        this._armed = false;
        this._end();
    }

    turnOn() {
        this._answered();
        Gio.DBus.system.call(PROFILES, PROFILES_PATH, 'org.freedesktop.DBus.Properties', 'Set',
            new GLib.Variant('(ssv)', [PROFILES, 'ActiveProfile', new GLib.Variant('s', 'power-saver')]),
            null, Gio.DBusCallFlags.NONE, -1, null, (connection, result) => {
                try {
                    connection.call_finish(result);
                    this._notice(_('Power saver on'), 'power-profile-power-saver-symbolic', 3, 'saver');
                } catch (e) {
                    console.warn(`Amethystora UI: power saving could not be turned on: ${e.message}`);
                }
            });
    }

    card(api) {
        const body = new St.BoxLayout({style_class: 'amethystora-pill-card-body', orientation: Clutter.Orientation.VERTICAL});
        body.add_child(new St.Label({
            style_class: 'amethystora-pill-card-label',
            text: _('The battery is at %d%%').format(Math.round(this.reading().percentage)),
        }));
        const text = new St.Label({
            style_class: 'amethystora-pill-card-text',
            text: _('Power saving makes the glass solid and animations shorter, and asks less of the processor. It stays on until you turn it off.'),
        });
        text.clutter_text.line_wrap = true;
        body.add_child(text);
        body.add_child(buttonRow([
            cardButton(_('Turn on'), () => {
                api.close();
                this.turnOn();
            }, {primary: true}),
            cardButton(_('Not now'), () => {
                api.close();
                this._answered();
            }),
        ]));
        return body;
    }

    // --- The time left on hover -----------------------------------------------------------------------------------

    _hoverChanged() {
        if (this._hoverTimer)
            GLib.source_remove(this._hoverTimer);
        this._hoverTimer = 0;
        if (!this._indicator?.hover) {
            this._hideTip();
            return;
        }
        this._hoverTimer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, HOVER_DELAY, () => {
            this._hoverTimer = 0;
            this._showTip();
            return GLib.SOURCE_REMOVE;
        });
    }

    _showTip() {
        const view = batteryView(this.reading());
        if (!view.hover || Main.panel.statusArea.quickSettings?.menu?.isOpen)
            return;
        if (!this._tip) {
            this._tip = new St.BoxLayout({
                style_class: 'amethystora-pill-tip',
                orientation: Clutter.Orientation.VERTICAL,
                reactive: false,
            });
            this._tipFirst = new St.Label({style_class: 'amethystora-pill-tip-first'});
            this._tipSecond = new St.Label({style_class: 'amethystora-pill-tip-second'});
            this._tip.add_child(this._tipFirst);
            this._tip.add_child(this._tipSecond);
            Main.layoutManager.addTopChrome(this._tip);
            this._tip.connect('notify::width', () => this._placeTip());
            this._glass?.()?.surfacesChanged?.();
        }
        this._fillTip(view);
        this._placeTip();
    }

    _fillTip(view) {
        if (!view.hover) {
            this._hideTip();
            return;
        }
        this._tipFirst.text = view.hover;
        this._tipSecond.text = view.detail ?? '';
        this._tipSecond.visible = !!view.detail;
    }

    // Under the battery, its end edge at the battery's end edge, 8 below the bar
    _placeTip() {
        const indicator = this._indicator;
        if (!this._tip || !indicator)
            return;
        const scaleFactor = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        const [x, y] = indicator.get_transformed_position();
        const [w] = indicator.get_transformed_size();
        const barBottom = Main.panel.get_transformed_position()[1] + Main.panel.height;
        const rtl = Clutter.get_default_text_direction() === Clutter.TextDirection.RTL;
        const tipX = rtl ? x : x + w - this._tip.width;
        const monitor = Main.layoutManager.primaryMonitor;
        this._tip.set_position(
            Math.round(Math.max(monitor.x, Math.min(tipX, monitor.x + monitor.width - this._tip.width))),
            Math.round(Math.max(barBottom, y) + 8 * scaleFactor));
    }

    _hideTip() {
        if (this._hoverTimer)
            GLib.source_remove(this._hoverTimer);
        this._hoverTimer = 0;
        if (!this._tip)
            return;
        const tip = this._tip;
        this._tip = null;
        Main.layoutManager.removeChrome(tip);
        tip.destroy();
        this._glass?.()?.surfacesChanged?.();
    }

    glassSurfaces() {
        return this._tip ? [{actor: this._tip, material: 'glass', radius: 14}] : [];
    }
}
