// Share this Wi-Fi: in the Wi-Fi menu of quick settings, while the computer is on a Wi-Fi network a phone can join
// with a password alone (or none), a row that shows a QR code for it in the menu. A guest's phone camera joins with it,
// with no password read aloud or typed. The password is asked of NetworkManager, as the account, only when the row is
// clicked, and goes with the code when the menu closes. Hidden while the screen is shared (qr.js).
//
// Nothing runs while the menu is closed: whether to offer the row is worked out each time it opens.

import Gio from 'gi://Gio';
import NM from 'gi://NM';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

import {QrCard} from './qr.js';

Gio._promisify(NM.RemoteConnection.prototype, 'get_secrets_async');

const SECURITY = '802-11-wireless-security';

// A value of the WIFI: text, its \ ; , : and " escaped as the format asks
export function escapeWifi(value) {
    return String(value).replace(/[\\;,:"]/g, c => `\\${c}`);
}

// The text a phone's camera reads: WIFI:T:WPA;S:name;P:password;; (H:true for a network that hides its name)
export function wifiText({ssid, kind, password = '', hidden = false}) {
    const parts = [`T:${kind}`, `S:${escapeWifi(ssid)}`];
    if (kind !== 'nopass')
        parts.push(`P:${escapeWifi(password)}`);
    if (hidden)
        parts.push('H:true');
    return `WIFI:${parts.join(';')};;`;
}

// What a phone needs to know of a connection, besides its password: null for one it cannot join with a password alone
// (802.1X, WEP), a hotspot this computer makes, or not Wi-Fi
export function describe(connection) {
    const wireless = connection?.get_setting_wireless?.();
    if (!wireless || wireless.get_mode?.() === 'ap')
        return null;
    const bytes = wireless.get_ssid();
    const ssid = bytes ? NM.utils_ssid_to_utf8(bytes.get_data()) : '';
    if (!ssid)
        return null;
    const security = connection.get_setting_wireless_security?.();
    const management = security?.get_key_mgmt?.() ?? 'none-open';
    const kind = {'none-open': 'nopass', 'owe': 'nopass', 'wpa-psk': 'WPA', 'sae': 'SAE'}[management] ?? null;
    if (!kind)
        return null;
    return {ssid, kind, hidden: !!wireless.get_hidden?.()};
}

export class WifiShare {
    enable() {
        // GNOME makes its quick settings indicators while it starts: a Wi-Fi menu not there yet is joined when it comes
        if (!this._attach())
            Main.panel.statusArea.quickSettings?._indicators?.connectObject('child-added', () => this._attach(), this);
    }

    _attach() {
        const toggle = this._toggle();
        if (!toggle?.menu || this._menu)
            return !!this._menu;
        Main.panel.statusArea.quickSettings?._indicators?.disconnectObject(this);
        this._menu = toggle.menu;
        this._row = new PopupMenu.PopupImageMenuItem(_('Share this Wi-Fi'), 'send-to-symbolic');
        this._row.add_style_class_name('amethystora-tools-wifi-share');
        this._row.visible = false;
        // Clicking it shows the code here: the item's own activate would close the menu
        this._row.activate = () => {
            this._show().catch(e => console.warn(`Amethystora UI: the Wi-Fi could not be shared: ${e.message}`));
        };
        this._codeItem = new PopupMenu.PopupBaseMenuItem({reactive: false, can_focus: false});
        this._codeItem.add_style_class_name('amethystora-tools-wifi-code');
        this._codeItem.visible = false;
        // Under the networks, above the separator and All Networks
        this._menu.addMenuItem(this._row, 1);
        this._menu.addMenuItem(this._codeItem, 2);
        this._menu.connectObject('open-state-changed', (menu, open) => {
            if (open)
                this._sync();
            else
                this._hideCode();
        }, this);
        return true;
    }

    disable() {
        Main.panel.statusArea.quickSettings?._indicators?.disconnectObject(this);
        this._cancellable?.cancel();
        this._cancellable = null;
        this._menu?.disconnectObject(this);
        this._menu = null;
        this._hideCode();
        this._row?.destroy();
        this._row = null;
        this._codeItem?.destroy();
        this._codeItem = null;
    }

    _toggle() {
        return Main.panel.statusArea.quickSettings?._network?._wirelessToggle ?? null;
    }

    // The active Wi-Fi connection of the computer's Wi-Fi devices, and what a phone needs of it
    _active() {
        const toggle = this._toggle();
        for (const device of toggle?._nmDevices ?? []) {
            const active = device.get_active_connection?.();
            if (active?.get_state?.() !== NM.ActiveConnectionState.ACTIVATED)
                continue;
            const connection = active.get_connection?.();
            const about = describe(connection);
            if (about)
                return {connection, about};
        }
        return null;
    }

    _sync() {
        if (!this._row)
            return;
        this._row.visible = !!this._active();
    }

    async _show() {
        const active = this._active();
        if (!active) {
            this._row.visible = false;
            return;
        }
        if (this._codeItem.visible) {
            this._hideCode();
            return;
        }
        const {connection, about} = active;
        let password = '';
        if (about.kind !== 'nopass') {
            this._cancellable?.cancel();
            this._cancellable = new Gio.Cancellable();
            const cancellable = this._cancellable;
            try {
                const secrets = await connection.get_secrets_async(SECURITY, cancellable);
                password = secrets?.deepUnpack?.()?.[SECURITY]?.psk?.deepUnpack?.() ?? '';
            } catch (e) {
                if (e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                    return;
                password = '';
            }
            if (cancellable.is_cancelled() || !this._codeItem)
                return;
            if (!password) {
                this._showMessage(_('The password of this network could not be read'));
                return;
            }
        }
        this._card = new QrCard({
            text: wifiText({...about, password}),
            caption: _('A phone\'s camera joins %s').format(about.ssid),
            size: 200,
        });
        this._codeItem.add_child(this._card);
        this._codeItem.visible = true;
    }

    _showMessage(text) {
        this._hideCode();
        this._card = new St.Label({style_class: 'amethystora-tools-qr-hidden', text, x_expand: true});
        this._codeItem.add_child(this._card);
        this._codeItem.visible = true;
    }

    _hideCode() {
        this._card?.destroy();
        this._card = null;
        if (this._codeItem)
            this._codeItem.visible = false;
    }

    // For the suites
    get state() {
        return {
            row: !!this._row, offered: !!this._row?.visible, code: !!this._codeItem?.visible,
            shown: !!this._card?.shown, covered: !!this._card?.covered,
        };
    }
}
