// How busy the computer is, at the start of the right side of the top bar: a bar for each processor, busiest first,
// a bar for the memory in use, and what the network sends and receives each second. Each opens a menu with more: the
// processor's name, speed and temperature, how much memory and swap is free, the totals sent and received, a chart
// of the last four minutes, the processes that use the most, and how long the computer has been running.
//
// It costs as little as it can, since it is always there: the bar reads three small files every five seconds and
// draws again only when what it shows changed, with no animation. The processes are looked at only while one of the
// menus is open. Only network interfaces with a device behind them are counted, so that what a VPN or a container
// sends is not counted twice. The colours are the theme's (shell-meters.css).

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';

import * as Format from './meters/format.js';
import * as Sources from './meters/sources.js';
import * as Widgets from './meters/widgets.js';

// Seconds between two readings, in the bar and, while a menu is open, of the processes
const INTERVAL = 5;
// The first look at the processes after a menu opens compares with one made as it opened, this long before
const FIRST_LOOK_MS = 1500;
const MINUTES = Math.round(Widgets.SAMPLES * INTERVAL / 60);
const SYSTEM_MONITOR = 'io.missioncenter.MissionCenter.desktop';
const ROLES = ['amethystora-meters-processor', 'amethystora-meters-memory', 'amethystora-meters-network'];
const NETWORK_MANAGER = 'org.freedesktop.NetworkManager';
// NetworkManager's word for no connectivity at all
const CONNECTIVITY_NONE = 1;

function remember(list, value) {
    list.push(value);
    if (list.length > Widgets.SAMPLES)
        list.shift();
}

// One of the three, in the top bar with its menu. The menu ends with a button for the system monitor while it is
// installed.
const Meter = GObject.registerClass(
class Meter extends PanelMenu.Button {
    _init(name, module) {
        super._init(0.5, name);
        this.add_style_class_name('amethystora-meters-button');
        this.menu.actor.add_style_class_name('amethystora-meters-menu');
        this.body = new St.BoxLayout({
            style_class: 'amethystora-meters-content',
            orientation: Clutter.Orientation.VERTICAL,
        });
        this.menu.box.add_child(this.body);

        this._actions = new St.BoxLayout({style_class: 'amethystora-meters-actions', x_align: Clutter.ActorAlign.CENTER});
        this._monitor = new St.Button({
            style_class: 'icon-button amethystora-meters-action',
            can_focus: true,
            child: new St.Icon({icon_name: 'utilities-system-monitor-symbolic'}),
        });
        this._monitor.connect('clicked', () => {
            this.menu.close();
            Shell.AppSystem.get_default().lookup_app(SYSTEM_MONITOR)?.activate();
        });
        this._actions.add_child(this._monitor);
        this.menu.box.add_child(this._actions);

        this.menu.connect('open-state-changed', (menu, open) => {
            if (open) {
                const app = Shell.AppSystem.get_default().lookup_app(SYSTEM_MONITOR);
                this._actions.visible = app !== null;
                if (app)
                    this._monitor.accessible_name = app.get_name();
            }
            module.menuToggled();
        });
    }
});

class ProcessorMeter {
    constructor(module, count) {
        this.button = new Meter(_('Processor'), module);
        this._bars = new Widgets.Bars(count);
        this.button.add_child(this._bars);

        const content = this.button.body;
        content.add_child(Widgets.heading(_('Processor')));
        this._use = new Widgets.Row(_('In use'));
        content.add_child(this._use.actor);
        this._capacity = Widgets.capacity();
        content.add_child(this._capacity);
        this._model = new St.Label({style_class: 'amethystora-meters-details amethystora-meters-model'});
        this._model.set_text_direction(Clutter.TextDirection.LTR);
        content.add_child(this._model);
        this._speed = new Widgets.Row(_('Frequency'), true);
        content.add_child(this._speed.actor);
        this._temperature = new Widgets.Row(_('Temperature'), true);
        content.add_child(this._temperature.actor);
        this._history = new Widgets.History(MINUTES);
        content.add_child(this._history.actor);
        content.add_child(Widgets.heading(_('Busiest processes')));
        this.processes = new Widgets.ProcessList();
        content.add_child(this.processes.actor);
        content.add_child(Widgets.heading(_('Time since start-up')));
        this._uptime = new St.Label({style_class: 'amethystora-meters-uptime', x_align: Clutter.ActorAlign.CENTER});
        content.add_child(this._uptime);
    }

    // One bar for each processor there is now: when one goes offline or comes back, the row is made again
    showBar(loads) {
        const count = Math.max(loads.length, 1);
        if (count !== this._bars.count) {
            this._bars.destroy();
            this._bars = new Widgets.Bars(count);
            this.button.add_child(this._bars);
        }
        this._bars.setValues(loads);
    }

    showMenu(load, history) {
        this._use.set(Format.percent(load));
        this._capacity.value = load;
        this._history.charts[0].plot(history);
        this._history.setMarks([Format.percent(1), Format.percent(0.5), Format.percent(0)]);
    }

    showDetails(details, temperature) {
        if (this._model.text !== details.model)
            this._model.text = details.model;
        this._model.visible = details.model !== '';
        this._speed.set(details.speed === null ? '–' : Format.gigahertz(details.speed));
        this._temperature.set(temperature === null ? '–' : Format.celsius(temperature));
        const uptime = details.uptime === null ? '–' : Format.uptime(details.uptime);
        if (this._uptime.text !== uptime)
            this._uptime.text = uptime;
    }
}

class MemoryMeter {
    constructor(module) {
        this.button = new Meter(_('Memory'), module);
        this._bars = new Widgets.Bars(1);
        this.button.add_child(this._bars);

        const content = this.button.body;
        content.add_child(Widgets.heading(_('Memory')));
        this._use = new Widgets.Row(_('In use'));
        content.add_child(this._use.actor);
        this._capacity = Widgets.capacity();
        content.add_child(this._capacity);
        this._free = new St.Label({
            style_class: 'amethystora-meters-details amethystora-meters-free',
            x_align: Clutter.ActorAlign.END,
        });
        content.add_child(this._free);
        this._swap = new Widgets.Row(_('Swap in use'));
        content.add_child(this._swap.actor);
        this._swapCapacity = Widgets.capacity();
        content.add_child(this._swapCapacity);
        this._swapFree = new St.Label({
            style_class: 'amethystora-meters-details amethystora-meters-free',
            x_align: Clutter.ActorAlign.END,
        });
        content.add_child(this._swapFree);
        this._history = new Widgets.History(MINUTES);
        content.add_child(this._history.actor);
        content.add_child(Widgets.heading(_('Largest processes')));
        this.processes = new Widgets.ProcessList();
        content.add_child(this.processes.actor);
    }

    showBar(used) {
        this._bars.setValues([used]);
    }

    showMenu(memory, history) {
        const used = memory.total > 0 ? (memory.total - memory.available) / memory.total : 0;
        this._use.set(Format.percent(used));
        this._capacity.value = used;
        this._setFree(this._free, memory.available, memory.total);
        const swapUsed = memory.swapTotal > 0 ? (memory.swapTotal - memory.swapFree) / memory.swapTotal : 0;
        this._swap.set(Format.percent(swapUsed));
        this._swapCapacity.value = swapUsed;
        this._setFree(this._swapFree, memory.swapFree, memory.swapTotal);
        this._history.charts[0].plot(history);
        this._history.setMarks([Format.percent(1), Format.percent(0.5), Format.percent(0)]);
    }

    _setFree(actor, available, total) {
        // Translators: memory that is free, such as "7.3 GB available of 16 GB"
        const text = Format.fill(_('{available} available of {total}'),
            {available: Format.size(available), total: Format.size(total, false, true)});
        if (actor.text !== text)
            actor.text = text;
    }
}

class NetworkMeter {
    constructor(module) {
        this.button = new Meter(_('Network'), module);
        this._rates = new St.BoxLayout({
            style_class: 'amethystora-meters-rates',
            orientation: Clutter.Orientation.VERTICAL,
            y_align: Clutter.ActorAlign.CENTER,
        });
        this._sending = new St.Label({style_class: 'amethystora-meters-rate', x_align: Clutter.ActorAlign.END});
        this._receiving = new St.Label({style_class: 'amethystora-meters-rate', x_align: Clutter.ActorAlign.END});
        this._rates.add_child(this._sending);
        this._rates.add_child(this._receiving);
        this.button.add_child(this._rates);

        const content = this.button.body;
        content.add_child(Widgets.heading(_('Network')));
        this._sent = new Widgets.Row(_('Sending'));
        this._received = new Widgets.Row(_('Receiving'));
        this._sentTotal = new Widgets.Row(_('Sent in all'));
        this._receivedTotal = new Widgets.Row(_('Received in all'));
        for (const row of [this._sent, this._received, this._sentTotal, this._receivedTotal])
            content.add_child(row.actor);
        this._history = new Widgets.History(MINUTES, true);
        content.add_child(this._history.actor);
    }

    // A dash, not a rate that means nothing, while the computer is offline (narrowed) or before the first reading
    showBar(rates, online = true) {
        const [sending, receiving] = rates && online
            ? [Format.size(rates.sent, true), Format.size(rates.received, true)] : ['–', '–'];
        if (online)
            this._rates.remove_style_class_name('amethystora-meters-offline');
        else
            this._rates.add_style_class_name('amethystora-meters-offline');
        for (const [label, text] of [[this._sending, sending], [this._receiving, receiving]]) {
            if (label.text !== text)
                label.text = text;
        }
    }

    showMenu(rates, totals, sent, received) {
        this._sent.set(rates ? Format.size(rates.sent, true) : '–');
        this._received.set(rates ? Format.size(rates.received, true) : '–');
        this._sentTotal.set(Format.size(totals.sent));
        this._receivedTotal.set(Format.size(totals.received));
        const top = Widgets.networkScale(Math.max(0, ...sent, ...received));
        this._history.charts[0].plot(sent, top);
        this._history.charts[1].plot(received, top);
        const scale = Format.size(top, true);
        this._history.setMarks([scale, Format.count(0), scale]);
    }
}

export class Meters {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._interfaces = new Sources.Interfaces();
        this._processes = new Sources.Processes();
        this._history = {load: [], memory: [], sent: [], received: []};

        // A bar for each processor that is online, found by its number: the numbers need not run from 0 to n - 1
        this._times = Sources.processorTimes();
        this._processor = new ProcessorMeter(this, Math.max(this._times.each.size, 1));
        this._memory = new MemoryMeter(this);
        this._network = new NetworkMeter(this);
        this._meters = [this._processor, this._memory, this._network];
        this._meters.forEach((meter, i) => Main.panel.addToStatusArea(ROLES[i], meter.button, i, 'right'));

        // Offline is what NetworkManager calls no connectivity at all. GLib's view (no route out) stands in until it
        // answers, and without it.
        this._connectivity = Gio.NetworkMonitor.get_default();
        this._connectivity.connectObject('network-changed', () => this._connectivityChanged(), this);
        this._online = this._connectivity.network_available;
        const cancellable = this._nmCancellable = new Gio.Cancellable();
        Gio.DBusProxy.new_for_bus(Gio.BusType.SYSTEM, Gio.DBusProxyFlags.DO_NOT_AUTO_START, null,
            NETWORK_MANAGER, '/org/freedesktop/NetworkManager', NETWORK_MANAGER, cancellable,
            (object, result) => {
                try {
                    const proxy = Gio.DBusProxy.new_for_bus_finish(result);
                    if (cancellable.is_cancelled())
                        return;
                    this._nm = proxy;
                } catch {
                    return;
                }
                this._nm.connectObject('g-properties-changed', () => this._connectivityChanged(), this);
                this._connectivityChanged();
            });

        // The first rate comes with the first reading, five seconds in: a dash until then
        this._counters = this._interfaces.read();
        this._readAt = GLib.get_monotonic_time();
        this._rates = null;
        this._read(true);
        this._timer = GLib.timeout_add_seconds(GLib.PRIORITY_LOW, INTERVAL, () => {
            this._read();
            return GLib.SOURCE_CONTINUE;
        });
    }

    disable() {
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = 0;
        this._listeners = null;
        this._stopLooking();
        this._connectivity?.disconnectObject(this);
        this._connectivity = null;
        this._nmCancellable?.cancel();
        this._nmCancellable = null;
        this._nm?.disconnectObject(this);
        this._nm = null;
        for (const meter of this._meters ?? []) {
            meter.processes?.hideTip();
            meter.button.destroy();
        }
        this._meters = null;
        this._processor = this._memory = this._network = null;
        this._interfaces = this._processes = null;
        this._history = null;
        this._networkSection = null;
        this._networkOpen = false;
    }

    // The bar's reading, every few seconds
    _read(first = false) {
        const times = Sources.processorTimes();
        const loads = [...times.each].map(([cpu, now]) => Sources.load(this._times.each.get(cpu), now));
        this._load = Sources.load(this._times.all, times.all);
        this._times = times;
        remember(this._history.load, this._load);
        this._processor.showBar(loads.sort((a, b) => b - a));
        // Whoever rides this reading (the power module's busy app): the whole processor's load, nothing read for it
        for (const listener of [...this._listeners ?? []])
            listener({load: this._load});

        this._memoryNow = Sources.memory();
        const {total, available} = this._memoryNow;
        const used = total > 0 ? (total - available) / total : 0;
        remember(this._history.memory, used);
        this._memory.showBar(used);

        // What each interface moved since the last reading; one that came or went in between counts from the next
        const now = GLib.get_monotonic_time();
        const counters = this._interfaces.read();
        const seconds = (now - this._readAt) / 1e6;
        if (!first && seconds > 0) {
            let sent = 0, received = 0;
            for (const [name, counter] of counters) {
                const before = this._counters.get(name);
                if (before) {
                    sent += Math.max(counter.sent - before.sent, 0);
                    received += Math.max(counter.received - before.received, 0);
                }
            }
            this._rates = {sent: sent / seconds, received: received / seconds};
            this._counters = counters;
            this._readAt = now;
            remember(this._history.sent, this._rates.sent);
            remember(this._history.received, this._rates.received);
        }
        this._network.showBar(this._rates, this._online);

        if (this._looking)
            this._showMenus();
        if (this._networkOpen)
            this._networkSection?.tick();
    }

    // Network by app (modules/privacy/connections.js), the privacy module's: a section at the end of the network menu,
    // told as that menu opens and closes and at each reading while it is open. Null takes it away.
    setNetworkSection(section) {
        if (this._networkSection === section)
            return;
        const old = this._networkSection;
        if (old?.actor?.get_parent())
            old.actor.get_parent().remove_child(old.actor);
        this._networkSection = section;
        if (section && this._network) {
            this._network.button.body.add_child(section.actor);
            if (this._networkOpen)
                section.shown(true, this._network.button.menu);
        }
    }

    _connectivityChanged() {
        const state = this._nm?.get_cached_property('Connectivity')?.unpack();
        const online = state === undefined ? this._connectivity.network_available : state !== CONNECTIVITY_NONE;
        if (online === this._online)
            return;
        this._online = online;
        // Back online: nothing moved yet that the next reading will not tell
        if (this._rates)
            this._rates = {sent: 0, received: 0};
        this._network.showBar(this._rates, online);
    }

    _showMenus() {
        this._processor.showMenu(this._load ?? 0, this._history.load);
        this._memory.showMenu(this._memoryNow, this._history.memory);
        let sent = 0, received = 0;
        for (const counter of this._counters.values()) {
            sent += counter.sent;
            received += counter.received;
        }
        this._network.showMenu(this._online ? this._rates : null, {sent, received},
            this._history.sent, this._history.received);
    }

    // A function called after each reading with {load}; returns what stops it. Nothing more is read for it.
    listen(listener) {
        this._listeners ??= new Set();
        this._listeners.add(listener);
        return () => this._listeners?.delete(listener);
    }

    // The processor's button in the top bar, which a chip of the power module stands before
    get processorButton() {
        return this._processor?.button ?? null;
    }

    // The processes are looked at only while one of the menus is open: once as it opens, again shortly after, so that
    // the processor's share is measured over a short time, and then with every reading
    menuToggled() {
        const networkOpen = this._network?.button.menu.isOpen ?? false;
        if (networkOpen !== !!this._networkOpen) {
            this._networkOpen = networkOpen;
            this._networkSection?.shown(networkOpen, this._network?.button.menu ?? null);
        }
        const open = this._meters?.some(meter => meter.button.menu.isOpen) ?? false;
        if (open && !this._looking) {
            this._looking = true;
            this._cancellable = new Gio.Cancellable();
            this._showMenus();
            this._look();
            this._lookTimer = GLib.timeout_add(GLib.PRIORITY_LOW, FIRST_LOOK_MS, () => {
                this._look();
                this._lookTimer = GLib.timeout_add_seconds(GLib.PRIORITY_LOW, INTERVAL, () => {
                    this._look();
                    return GLib.SOURCE_CONTINUE;
                });
                return GLib.SOURCE_REMOVE;
            });
        } else if (!open && this._looking) {
            this._stopLooking();
        }
    }

    _stopLooking() {
        this._looking = false;
        if (this._lookTimer)
            GLib.source_remove(this._lookTimer);
        this._lookTimer = 0;
        this._cancellable?.cancel();
        this._cancellable = null;
        this._processes?.clear();
        this._lastLook = null;
        for (const meter of this._meters ?? [])
            meter.processes?.hideTip();
    }

    async _look() {
        if (this._walking)
            return;
        this._walking = true;
        const cancellable = this._cancellable;
        try {
            const details = Sources.processorDetails();
            if (this._sensor === undefined)
                this._sensor = Sources.findTemperature();
            this._processor.showDetails(details, Sources.temperature(this._sensor));

            const machine = Sources.processorTimes().all;
            const found = await this._processes.walk(cancellable);
            if (cancellable.is_cancelled() || !this._looking)
                return;
            this._showProcesses(found, machine);
        } catch (e) {
            console.warn(`Amethystora UI: the meters could not list the processes: ${e.message}`);
        } finally {
            this._walking = false;
        }
    }

    // Processes of the same name are one row, with how many there are; the processor's share is of the whole
    // machine's time since the last look, so that one process keeping one of eight processors busy shows 13 %
    _showProcesses(found, machine) {
        const last = this._lastLook;
        this._lastLook = {found, machine};
        const ticks = last && machine ? machine.busy + machine.idle - last.machine.busy - last.machine.idle : 0;

        const groups = new Map();
        for (const process of found.values()) {
            let group = groups.get(process.name);
            if (!group) {
                group = {name: process.name, count: 0, cpu: 0, memory: 0, measured: false};
                groups.set(process.name, group);
            }
            group.count++;
            const before = last?.found.get(process.key);
            if (before)
                group.cpu += Math.max(process.ticks - before.ticks, 0);
            if (process.pss !== null) {
                group.memory += process.pss;
                group.measured = true;
            }
        }
        const title = group => group.count > 1 ? `${group.name} (x${Format.count(group.count)})` : group.name;
        const all = [...groups.values()];

        const busiest = ticks > 0
            ? all.filter(group => group.cpu > 0).sort((a, b) => b.cpu - a.cpu).slice(0, 6)
            : [];
        this._processor.processes.set(busiest.map(group => [title(group), Format.share(group.cpu / ticks)]));
        const largest = all.filter(group => group.measured).sort((a, b) => b.memory - a.memory).slice(0, 6);
        this._memory.processes.set(largest.map(group => [title(group), Format.size(group.memory)]));
    }
}
