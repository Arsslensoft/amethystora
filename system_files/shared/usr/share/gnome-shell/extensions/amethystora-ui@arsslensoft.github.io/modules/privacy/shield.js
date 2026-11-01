// The shield: how the machine's protection stands, in one look, as a tile in quick settings (the default), a mark in the
// top bar, or the mark only while something needs a look. Its menu says what the security report found (to the
// administrators, who may read it), the network's firewall zone, encrypted DNS, USB protection, the Private folder, and
// GNOME's camera and microphone switches for apps. It switches nothing: Security keeps every switch, and every line opens
// Security on its page (readings.js says where each value comes from).
//
// Read as the menu or quick settings opens, never on a timer; the mark alone follows the report's file as the watcher
// writes it, so that its colour (and, in attention, whether it is there) is right without opening anything.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import St from 'gi://St';

import {gettext as _, ngettext, pgettext as C_} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import * as Util from 'resource:///org/gnome/shell/misc/util.js';
import {QuickMenuToggle, SystemIndicator} from 'resource:///org/gnome/shell/ui/quickSettings.js';

import {plain} from '../pill/text.js';
import * as Readings from './readings.js';

// By its full path: the session's PATH may hold a folder of the account's own before the system's
const SECURITY = '/usr/bin/amethystora-security';
const ROLE = 'amethystora-privacy-shield';
// The shell's own indicators at the right of the top bar; the mark stands to their left
const SYSTEM_ROLES = ['screenRecording', 'screenSharing', 'dwellClick', 'a11y', 'keyboard', 'quickSettings'];
const ICONS = {fine: 'security-high-symbolic', look: 'security-low-symbolic', none: 'security-medium-symbolic'};
const ZONES = {'amethystora-home': 'home', 'amethystora-away': 'away', 'amethystora': 'default'};

function label(styleClass, text = '') {
    const actor = new St.Label({style_class: styleClass, text, y_align: Clutter.ActorAlign.CENTER});
    actor.clutter_text.use_markup = false;
    return actor;
}

// A line of the menu: an icon in a square (or none), the words and a second line, a chip at the end; it opens Security
const Row = GObject.registerClass(
class AmethystoraPrivacyShieldRow extends PopupMenu.PopupBaseMenuItem {
    _init(iconName, page, open) {
        super._init({style_class: 'amethystora-privacy-row'});
        if (iconName) {
            const square = new St.Bin({style_class: 'amethystora-privacy-row-icon', y_align: Clutter.ActorAlign.CENTER});
            this._icon = new St.Icon({icon_name: iconName, icon_size: 16});
            square.set_child(this._icon);
            this.add_child(square);
        }
        const words = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_expand: true,
            y_align: Clutter.ActorAlign.CENTER});
        this._title = label('amethystora-privacy-row-title');
        this._detail = label('amethystora-privacy-row-detail');
        words.add_child(this._title);
        words.add_child(this._detail);
        this.add_child(words);
        this._chip = label('amethystora-privacy-chip');
        this._chip.x_align = Clutter.ActorAlign.END;
        this.add_child(this._chip);
        this.connect('activate', () => open(page));
    }

    set(title, detail = '', chip = '', iconName = null) {
        this._title.text = plain(title, 60);
        this._detail.text = detail;
        this._detail.visible = detail !== '';
        this._chip.text = chip;
        this._chip.visible = chip !== '';
        this.accessible_name = [title, detail, chip].filter(t => t).join(', ');
        if (iconName && this._icon)
            this._icon.icon_name = iconName;
    }
});

// What the tile and the mark both show: a section of a menu, filled from the readings
class Panel {
    constructor(open) {
        this.section = new PopupMenu.PopupMenuSection();
        this.section.actor.add_style_class_name('amethystora-privacy-shield');

        this._header = new PopupMenu.PopupBaseMenuItem({style_class: 'amethystora-privacy-header'});
        this._headerIcon = new St.Icon({style_class: 'amethystora-privacy-header-icon', icon_size: 30,
            icon_name: ICONS.none});
        this._header.add_child(this._headerIcon);
        const words = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_expand: true,
            y_align: Clutter.ActorAlign.CENTER});
        this._headerTitle = label('amethystora-privacy-header-title', _('Security'));
        this._headerDetail = label('amethystora-privacy-header-detail');
        words.add_child(this._headerTitle);
        words.add_child(this._headerDetail);
        this._header.add_child(words);
        this._header.connect('activate', () => open(''));
        this.section.addMenuItem(this._header);

        this.network = new Row('network-wireless-symbolic', 'settings/network', open);
        this.dns = new Row('web-browser-symbolic', '', open);
        this.usb = new Row('media-removable-symbolic', '', open);
        this.private = new Row('changes-prevent-symbolic', '', open);
        for (const row of [this.network, this.dns, this.usb, this.private])
            this.section.addMenuItem(row);
        this.section.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        this.camera = new Row(null, 'settings/devices', open);
        this.microphone = new Row(null, 'settings/devices', open);
        this.section.addMenuItem(this.camera);
        this.section.addMenuItem(this.microphone);

        const footer = new PopupMenu.PopupBaseMenuItem({style_class: 'amethystora-privacy-footer'});
        const note = label('amethystora-privacy-footer-note', _('Every switch lives in Security'));
        note.x_expand = true;
        footer.add_child(note);
        footer.add_child(label('amethystora-privacy-link', _('Open Security')));
        footer.accessible_name = _('Open Security');
        footer.connect('activate', () => open(''));
        this.section.addMenuItem(footer);
    }

    // The report part: fine, something needs a look, or nothing to read
    showReport(report, twelve = false) {
        const state = summary(report);
        this._headerIcon.icon_name = ICONS[state];
        for (const s of ['fine', 'look', 'none'])
            this._headerIcon[s === state ? 'add_style_class_name' : 'remove_style_class_name'](`amethystora-privacy-${s}`);
        if (state === 'none') {
            this._headerTitle.text = _('Security');
            this._headerDetail.text = _('Open Security to see the report');
        } else if (state === 'fine') {
            this._headerTitle.text = _('Protected');
            // The time in the person's clock, 12- or 24-hour
            const time = report.written
                // Translators: the time of day on a 12-hour and on a 24-hour clock, as GLib.DateTime.format takes it
                ? GLib.DateTime.new_from_unix_local(report.written).format(twelve ? _('%l:%M %p') : _('%H:%M')).trim() : '';
            this._headerDetail.text = ngettext('Checked %s · %d check fine', 'Checked %s · %d checks fine', report.fine)
                .format(time, report.fine);
        } else {
            const n = report.failing.length;
            this._headerTitle.text = ngettext('One thing needs a look', '%d things need a look', n).format(n);
            this._headerDetail.text = report.failing.slice(0, 2).map(t => plain(t, 60)).filter(t => t).join(' · ');
        }
    }

    showNetwork(network) {
        if (!network) {
            this.network.set(_('Offline'), '', '', 'network-offline-symbolic');
            return;
        }
        const zone = ZONES[network.zone];
        const detail = {
            home: _('Phone pairing open here'),
            away: _('Answers nothing that asks'),
            default: _('Given the default zone'),
        }[zone] ?? '';
        const chip = {home: _('Home'), away: _('Away'), default: _('Default')}[zone] ?? plain(network.zone, 24);
        this.network.set(network.name || _('Network'), detail, chip,
            network.wireless ? 'network-wireless-symbolic' : 'network-wired-symbolic');
    }

    showDns(dns) {
        if (!dns) {
            this.dns.set(_('Encrypted DNS'), '', _('Off'));
            return;
        }
        const on = dns.mode === 'yes' || dns.mode === 'true';
        const server = plain(dns.server, 60);
        const detail = on ? server ? _('%s, over TLS').format(server) : _('Over TLS') : '';
        const chip = on ? _('On') : dns.mode === 'opportunistic' ? _('Opportunistic') : _('Off');
        this.dns.set(_('Encrypted DNS'), detail, chip);
    }

    showUsb(usb) {
        const detail = {on: _('New devices wait for you'), locked: _('New devices wait while the screen is locked')};
        const chip = {off: _('Off'), on: _('On'), locked: _('Locked')};
        this.usb.set(_('USB protection'), detail[usb] ?? '', chip[usb] ?? _('Off'));
    }

    showPrivate(folder) {
        this.private.visible = !!folder?.setUp;
        if (folder?.setUp)
            this.private.set(_('Private folder'), folder.open ? _('Open · closes when the screen locks') : _('Closed'));
    }

    showDevices(camera, microphone, webcamBlocked) {
        this.camera.set(_('Camera for apps'), _('Apps that ask through the portal'),
            webcamBlocked ? _('Blocked') : camera ? _('Off') : _('Allowed'));
        this.microphone.set(_('Microphone for apps'), '', microphone ? _('Off') : _('Allowed'));
    }

    destroy() {
        this.section.destroy();
    }
}

// fine, look or none
function summary(report) {
    if (!report)
        return 'none';
    return report.failing.length > 0 ? 'look' : 'fine';
}

const ShieldToggle = GObject.registerClass(
class AmethystoraPrivacyShieldToggle extends QuickMenuToggle {
    _init() {
        super._init({title: _('Shield'), iconName: ICONS.none, toggleMode: false});
        this._amethystoraQuickId = 'shield';
        this.add_style_class_name('amethystora-privacy-tile');
        // It switches nothing: a click anywhere opens what it says
        this.connect('clicked', () => this.menu.open());
    }
});

const ShieldMark = GObject.registerClass(
class AmethystoraPrivacyShieldMark extends PanelMenu.Button {
    _init() {
        super._init(0.5, _('Shield'));
        this.add_style_class_name('amethystora-privacy-mark-button');
        this.menu.actor.add_style_class_name('amethystora-privacy-mark-menu');
        this.icon = new St.Icon({style_class: 'system-status-icon amethystora-privacy-mark', icon_name: ICONS.none});
        this.add_child(this.icon);
    }
});

export class Shield {
    constructor(ext, mode) {
        this._ext = ext;
        this.mode = mode;
    }

    start() {
        this._privacy = new Gio.Settings({schema_id: 'org.gnome.desktop.privacy'});
        this._interface = new Gio.Settings({schema_id: 'org.gnome.desktop.interface'});
        this._panel = new Panel(page => this._open(page));
        if (this.mode === 'tile') {
            this._toggle = new ShieldToggle();
            this._toggle.menu.addMenuItem(this._panel.section);
            this._indicator = new SystemIndicator();
            this._indicator.quickSettingsItems.push(this._toggle);
            Main.panel.statusArea.quickSettings.addExternalIndicator(this._indicator);
            Main.panel.statusArea.quickSettings.menu.connectObject('open-state-changed', (menu, open) => {
                if (open)
                    this.refresh();
            }, this);
        } else {
            this._mark = new ShieldMark();
            this._mark.menu.addMenuItem(this._panel.section);
            this._mark.menu.connectObject('open-state-changed', (menu, open) => {
                if (open)
                    this.refresh();
            }, this);
            Main.panel.statusArea[ROLE]?.destroy();
            Main.panel.addToStatusArea(ROLE, this._mark, this._position(), 'right');
            // The mark follows the report as the watcher writes it
            try {
                this._monitor = Gio.File.new_for_path(Readings.REPORT).monitor_file(Gio.FileMonitorFlags.NONE, null);
                this._monitor.connectObject('changed', () => this._readReport(), this);
            } catch {}
            this._readReport();
        }
        this._sync(null);
    }

    stop() {
        this._cancellable?.cancel();
        this._cancellable = null;
        this._reportCancellable?.cancel();
        this._reportCancellable = null;
        this._monitor?.disconnectObject(this);
        this._monitor?.cancel();
        this._monitor = null;
        Main.panel.statusArea.quickSettings?.menu?.disconnectObject(this);
        this._mark?.menu.disconnectObject(this);
        this._mark?.destroy();
        this._mark = null;
        // The tile's menu lives in quick settings' own grid, not under the tile: destroyed apart, as the battery's
        this._toggle?.menu.destroy();
        this._toggle?.destroy();
        this._toggle = null;
        this._indicator?.destroy();
        this._indicator = null;
        this._panel = null;
        // Disposed now rather than by the collector (dock.js says why)
        this._privacy?.run_dispose();
        this._privacy = null;
        this._interface?.run_dispose();
        this._interface = null;
    }

    // Left of the shell's own indicators, as the phone's icon stands
    _position() {
        const containers = SYSTEM_ROLES.map(role => Main.panel.statusArea[role]?.container)
            .filter(container => container?.get_parent());
        if (containers.length === 0)
            return 0;
        const siblings = containers[0].get_parent().get_children();
        return Math.min(...containers.map(container => siblings.indexOf(container)));
    }

    _open(page) {
        Main.panel.closeQuickSettings?.();
        this._mark?.menu.close();
        this._opened = page;
        Util.spawn(page ? [SECURITY, page] : [SECURITY]);
    }

    _twelve() {
        return this._interface?.get_string('clock-format') === '12h';
    }

    // The tile's or the mark's look, from the report
    _sync(report) {
        const state = summary(report);
        this._state = state;
        if (this._toggle) {
            this._toggle.icon_name = ICONS[state];
            for (const s of ['fine', 'look', 'none']) {
                this._toggle[s === state ? 'add_style_class_name' : 'remove_style_class_name'](
                    `amethystora-privacy-tile-${s}`);
            }
        }
        if (this._mark) {
            this._mark.icon.icon_name = ICONS[state];
            for (const s of ['fine', 'look', 'none']) {
                this._mark.icon[s === state ? 'add_style_class_name' : 'remove_style_class_name'](
                    `amethystora-privacy-${s}`);
            }
            this._mark.container.visible = this.mode === 'mark' || state === 'look';
        }
    }

    _readReport() {
        this._reportCancellable?.cancel();
        const cancellable = this._reportCancellable = new Gio.Cancellable();
        Readings.report(cancellable).then(report => {
            if (cancellable.is_cancelled())
                return;
            this._report = report;
            this._sync(report);
            this._panel?.showReport(report, this._twelve());
        }).catch(() => {});
    }

    // Everything the menu shows, read again
    async refresh() {
        this._cancellable?.cancel();
        const cancellable = this._cancellable = new Gio.Cancellable();
        const settle = promise => promise.catch(() => null);
        const [report, network, dns, usb, folder, webcam] = await Promise.all([
            settle(Readings.report(cancellable)),
            settle(Readings.network(cancellable)),
            settle(Readings.dns(cancellable)),
            settle(Readings.usb(cancellable)),
            settle(Readings.privateFolder(cancellable)),
            settle(Readings.webcamBlocked(cancellable)),
        ]);
        if (cancellable.is_cancelled() || !this._panel)
            return;
        this._report = report;
        this._sync(report);
        const panel = this._panel;
        panel.showReport(report, this._twelve());
        panel.showNetwork(network);
        panel.showDns(dns);
        panel.showUsb(usb ?? 'off');
        panel.showPrivate(folder);
        panel.showDevices(this._privacy.get_boolean('disable-camera'), this._privacy.get_boolean('disable-microphone'),
            webcam === true);
        if (this._toggle) {
            const parts = [];
            if (report)
                parts.push(report.failing.length > 0
                    ? ngettext('%d needs a look', '%d need a look', report.failing.length).format(report.failing.length)
                    : _('Protected'));
            const zone = ZONES[network?.zone];
            // A tile holds some 18 characters, and the menu says the zone: after the report's word only a short home
            // or away, and nothing after a count, which comes first
            if (zone && !report) {
                parts.push({home: _('Home'), away: _('Away'), default: _('Default')}[zone]);
            } else if (zone && report.failing.length === 0 && zone !== 'default') {
                parts.push({home: C_('shield', 'home'), away: C_('shield', 'away')}[zone]);
            }
            this._toggle.subtitle = parts.join(' · ') || null;
        }
        this._done = (this._done ?? 0) + 1;
    }

    // --- For the VM suite ---------------------------------------------------------------------------------------------

    state() {
        const rows = this._panel ? ['network', 'dns', 'usb', 'private', 'camera', 'microphone'].map(name => {
            const row = this._panel[name];
            return {name, visible: row.visible, title: row._title.text, detail: row._detail.text, chip: row._chip.text};
        }) : [];
        return {
            mode: this.mode,
            state: this._state ?? 'none',
            tile: !!this._toggle,
            subtitle: this._toggle?.subtitle ?? null,
            checked: this._toggle?.checked ?? false,
            mark: this._mark ? {visible: this._mark.container.visible, icon: this._mark.icon.icon_name} : null,
            monitor: !!this._monitor,
            header: this._panel ? [this._panel._headerTitle.text, this._panel._headerDetail.text] : null,
            rows,
            refreshed: this._done ?? 0,
            opened: this._opened ?? null,
        };
    }

    get toggle() {
        return this._toggle;
    }

    get markButton() {
        return this._mark;
    }

    // What a row would open, for the suite: the page, or '' for the overview
    openForTest(name) {
        const row = name === 'header' ? this._panel?._header : this._panel?.[name];
        row?.activate(Clutter.get_current_event());
    }
}
