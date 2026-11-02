// Send the screen to a TV: a tile in quick settings that opens GNOME Network Displays, which lists the TVs and screens
// nearby and sends to the one chosen. That app takes no name of a TV from anyone else, so its own window is the list. The
// first time, the tile's menu says what it fetches, and a terminal fetches it from Flathub only after the person presses
// Install (`ame desktop send-to-tv setup`).
//
// Offered and off (the key send-to-tv of org.gnome.shell.extensions.amethystora-ui.quick); off, none of this is made, and
// nothing runs until the tile is pressed. The sending itself is a screen share like any other, so the pill says so for as
// long as it lasts and the share guard covers the windows it lists, without a line here. Nothing here says that the app is
// sending: the capture watcher cannot tell whose share it is.

import Shell from 'gi://Shell';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as Util from 'resource:///org/gnome/shell/misc/util.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import {QuickMenuToggle} from 'resource:///org/gnome/shell/ui/quickSettings.js';

const APP = 'org.gnome.NetworkDisplays.desktop';
const TERMINAL = '/usr/libexec/amethystora-in-terminal';
const AME = '/usr/bin/ame';

export class TvTile {
    constructor(privacy) {
        this._privacy = privacy;
        this.tile = new QuickMenuToggle({
            title: _('Send to the TV'),
            iconName: 'video-display-symbolic',
            toggleMode: false,
            menuEnabled: true,
        });
        this.tile._amethystoraQuickId = 'send-to-tv';
        this.tile.menu.setHeader('video-display-symbolic', _('Send your screen'));
        this.tile.connect('clicked', () => this.press());
        this._note = new PopupMenu.PopupMenuItem('', {reactive: false, can_focus: false});
        this._note.label.clutter_text.line_wrap = true;
        this.tile.menu.addMenuItem(this._note);
        this._action = new PopupMenu.PopupMenuItem('');
        this._action.connect('activate', () => this._act());
        this.tile.menu.addMenuItem(this._action);
        this.opened();
    }

    _app() {
        return Shell.AppSystem.get_default().lookup_app(APP);
    }

    // Quick settings opened: the words for what there is now
    opened() {
        const installed = !!this._app();
        const covered = this._privacy()?.guardOn ?? false;
        if (installed) {
            // Said of the share guard only as it is: it covers the windows of its own list while the whole screen is shared
            this._note.label.text = covered
                ? _('Network Displays lists the TVs and screens nearby. It has not been tried with a real TV yet, and a TV that needs a direct Wi-Fi link is not expected to work. While the whole screen is shared, the share guard covers the windows it lists.')
                : _('Network Displays lists the TVs and screens nearby. It has not been tried with a real TV yet, and a TV that needs a direct Wi-Fi link is not expected to work.');
            this._action.label.text = _('Open Network Displays');
        } else {
            this._note.label.text = _('This needs GNOME Network Displays, a free app from Flathub of about 6 MB, and its runtime if you do not have it. Nothing is fetched until you press Install.');
            this._action.label.text = _('Install');
        }
    }

    // The tile. Installed, the app opens; otherwise the menu says what would be fetched
    press() {
        if (this._app())
            this._act();
        else
            this.tile.menu.open();
    }

    // The menu's button: the app, or the terminal that asks before it fetches
    _act() {
        const app = this._app();
        Main.overview.hide();
        Main.panel.closeQuickSettings();
        if (app)
            app.activate();
        else
            Util.spawn([TERMINAL, AME, 'desktop', 'send-to-tv', 'setup']);
    }

    destroy() {
        this.tile.menu.destroy();
        this.tile.destroy();
    }
}
