// The Amethystora gem at the left end of the top bar, and a short menu of the system's own tools. Every entry is
// fixed here, with no settings: an app starts by its launcher's ID and a command as an argument vector, never
// through a shell, and an entry whose app is not installed is hidden while the menu is open, never installed
// from the click. A middle click shows or hides the overview.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GObject from 'gi://GObject';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {Extension, gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import * as Util from 'resource:///org/gnome/shell/misc/util.js';

// The image's own drawing, in its colours rather than a silhouette the panel would paint white
const LOGO = '/usr/share/icons/hicolor/scalable/apps/amethystora-logo.svg';

// The menu from the top: an app by its launcher's ID, or a command; null is the line between two groups. A
// function, because _() has the extension's translations only once the extension is made.
const entries = () => [
    {label: _('About this computer'), argv: ['gnome-control-center', 'system', 'about']},
    {label: _('Manual'), app: 'amethystora-manual.desktop'},
    null,
    {label: _('Settings'), app: 'org.gnome.Settings.desktop'},
    {label: _('Software centre'), app: 'io.github.kolunmi.Bazaar.desktop'},
    null,
    // GNOME's own, which reads an extension's details from the extension: Extension Manager asks
    // extensions.gnome.org, which has none of the image's own
    {label: _('Extensions'), app: 'org.gnome.Extensions.desktop'},
    {label: _('System monitor'), app: 'io.missioncenter.MissionCenter.desktop'},
    {label: _('Terminal'), argv: ['xdg-terminal-exec']},
    null,
    {label: _('Containers'), app: 'com.ranfdev.DistroShelf.desktop'},
];

const LogoMenu = GObject.registerClass(
class LogoMenu extends PanelMenu.Button {
    _init() {
        super._init(0.5, _('Logo menu'));
        this.add_child(new St.Icon({
            gicon: new Gio.FileIcon({file: Gio.File.new_for_path(LOGO)}),
            icon_size: 20,
            style_class: 'system-status-icon amethystora-logo-menu-icon',
        }));

        // An app installed or taken away since the menu last opened. Connected before the entries are added,
        // so that the lines between groups, which GNOME hides beside a hidden entry on the same signal, see
        // the entries as they are now.
        const apps = Shell.AppSystem.get_default();
        const appItems = [];
        this.menu.connect('open-state-changed', (menu, open) => {
            if (open) {
                for (const [item, id] of appItems)
                    item.visible = apps.lookup_app(id) !== null;
            }
        });
        for (const entry of entries()) {
            if (!entry) {
                this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
                continue;
            }
            const item = this.menu.addAction(entry.label, () => {
                if (entry.argv)
                    Util.spawn(entry.argv);
                else
                    apps.lookup_app(entry.app)?.activate();
            });
            if (entry.app)
                appItems.push([item, entry.app]);
        }

        // The button's own click opens the menu with any button, on the press. The middle one is taken first,
        // on its press too and in the capture phase, so that it shows the overview and opens no menu.
        const overview = new Clutter.ClickGesture({required_button: Clutter.BUTTON_MIDDLE, recognize_on_press: true});
        overview.connect('recognize', () => {
            this.menu.close();
            if (Main.overview.shouldToggleByCornerOrButton())
                Main.overview.toggle();
        });
        this.add_action_full('amethystora-logo-menu-overview', Clutter.EventPhase.CAPTURE, overview);
    }
});

export default class LogoMenuExtension extends Extension {
    enable() {
        this._button = new LogoMenu();
        Main.panel.addToStatusArea(this.uuid, this._button, 0, 'left');
    }

    disable() {
        this._button.destroy();
        this._button = null;
    }
}
