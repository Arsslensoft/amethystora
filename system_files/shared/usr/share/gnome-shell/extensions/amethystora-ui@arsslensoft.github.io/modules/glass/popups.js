// GNOME's popups on glass: every menu, the quick settings and their toggle menus, the calendar and notification
// drawer, notification banners, the on-screen displays, the window switcher, dialogs, the on-screen keyboard, the
// screenshot UI's panel and the resize popup. Each is found as GNOME is about to show it, the first time and every
// time after, so that surfaces made later get their glass before their first frame. The glass draws a veil of the
// theme's background over the picture (glass/effect.js), milky in a light scheme with dark text, faint in a dark one
// with light text, and thicker where the wallpaper behind is too bright or dark for the text.

import St from 'gi://St';

import * as AltTab from 'resource:///org/gnome/shell/ui/altTab.js';
import * as BoxPointer from 'resource:///org/gnome/shell/ui/boxpointer.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as ModalDialog from 'resource:///org/gnome/shell/ui/modalDialog.js';
import * as OsdWindow from 'resource:///org/gnome/shell/ui/osdWindow.js';
import * as SwitcherPopup from 'resource:///org/gnome/shell/ui/switcherPopup.js';

import {Stacks} from './stacks.js';

// The kinds of surface. Each pane has the corners its surface is drawn with (glass.css, and the desktop style's
// shell-styles.css). A dialog is ringed with a hairline; the toggle menus inside the quick settings lie on the
// settings' own glass and cast no shadow.
const KINDS = {
    menu: {},
    panel: {},
    inner: {flat: true},
    banner: {},
    osd: {},
    dialog: {ring: 0.05},
    keyboard: {flat: true},
};

// The style classes of a box pointer's content that make it a panel rather than a menu
const PANELS = ['quick-settings', 'datemenu-popover'];
const MENUS = ['popup-menu-content', 'candidate-popup-content'];

const hasClass = (actor, names) => names.some(name => actor.has_style_class_name?.(name));

export class Popups {
    constructor(glass) {
        this._glass = glass;
    }

    enable() {
        this._stacks = new Stacks();
        const glass = this._glass;

        // Every menu opens through its box pointer, which holds its content
        glass.wrap(BoxPointer.BoxPointer.prototype, 'open', (open, boxPointer, args) => {
            this._boxPointer(boxPointer);
            return open.apply(boxPointer, args);
        });
        glass.wrap(OsdWindow.OsdWindow.prototype, 'show', (show, osd, args) => {
            if (osd._hbox)
                this._attach(osd._hbox, 'osd');
            return show.apply(osd, args);
        });
        // The switcher's list is made as it is shown, with its pictures of windows (Alt+Tab)
        glass.wrap(SwitcherPopup.SwitcherPopup.prototype, 'show', (show, popup, args) => {
            const shown = show.apply(popup, args);
            if (popup._switcherList) {
                this._attach(popup._switcherList, 'osd');
                glass.windows?.glassClones(popup._switcherList);
            }
            return shown;
        });
        // The app switcher (Super+Tab) shows an app's windows below it later, each picture put into its place as the
        // row is first laid out
        glass.wrap(AltTab.AppSwitcherPopup.prototype, '_createThumbnails', (create, popup, args) => {
            const created = create.apply(popup, args);
            const thumbnails = popup._thumbnails;
            if (thumbnails) {
                glass.windows?.glassClones(thumbnails);
                for (const bin of thumbnails._thumbnailBins ?? [])
                    bin.connect('notify::child', () => bin.child && glass.windows?.glassClones(bin));
            }
            return created;
        });
        glass.wrap(ModalDialog.ModalDialog.prototype, 'open', (open, dialog, args) => {
            const box = dialog.dialogLayout?._dialog;
            if (box)
                this._attach(box, 'dialog');
            return open.apply(dialog, args);
        });

        // A banner is added to the tray's bin as it slides down from under the top bar
        const banners = Main.messageTray._bannerBin;
        banners.connectObject('child-added', (bin, banner) => this._attachBanner(banner), this);
        banners.get_children().forEach(banner => this._attachBanner(banner));

        const keyboard = Main.layoutManager.keyboardBox;
        keyboard.connectObject('child-added', (box, child) => this._attachKeyboard(child), this);
        keyboard.get_children().forEach(child => this._attachKeyboard(child));

        Main.uiGroup.connectObject('child-added', (group, child) => {
            if (hasClass(child, ['resize-popup']))
                this._attach(child, 'osd');
        }, this);

        if (Main.screenshotUI?._panel)
            this._attach(Main.screenshotUI._panel, 'panel');
    }

    disable() {
        Main.messageTray._bannerBin.disconnectObject(this);
        Main.layoutManager.keyboardBox.disconnectObject(this);
        Main.uiGroup.disconnectObject(this);
        this._stacks?.destroy();
        this._stacks = null;
    }

    _attach(actor, kind) {
        const {ring = 0, flat = false} = KINDS[kind];
        this._glass.attach(actor, {
            material: 'glass',
            radius: null,
            ring,
            shadow: flat ? null : this._glass.shadow,
            inClones: true,
        }, {classes: ['amethystora-glass-surface', `amethystora-glass-${kind}`], scheme: true});
    }

    _boxPointer(boxPointer) {
        const content = boxPointer.bin?.child;
        if (!content)
            return;
        if (hasClass(content, PANELS)) {
            this._attach(content, 'panel');
            // The quick settings' toggle menus open over the settings, beside their box pointer; the drawer's
            // notifications are inside it
            this._scan(boxPointer.get_parent() === Main.uiGroup ? content : boxPointer.get_parent());
        } else if (hasClass(content, MENUS)) {
            this._attach(content, 'menu');
        }
    }

    _scan(actor) {
        for (const child of actor.get_children()) {
            if (hasClass(child, ['quick-toggle-menu']))
                this._attach(child, 'inner');
            else if (hasClass(child, ['message-view']))
                this._stacks.watch(child);
            this._scan(child);
        }
    }

    _attachBanner(banner) {
        const glass = this._glass;
        glass.attach(banner, {
            material: 'glass',
            radius: null,
            shadow: glass.shadow,
            inClones: true,
            // Clear while the banner is still under the top bar, full once it is 5 pixels out from under it
            fade: top => {
                const panel = Main.panel;
                if (!panel.mapped)
                    return 1;
                const [, barTop] = panel.get_transformed_position();
                const barBottom = barTop + panel.height + 5 * St.ThemeContext.get_for_stage(global.stage).scale_factor;
                return Math.min(Math.max((top - barTop) / Math.max(1, barBottom - barTop), 0), 1);
            },
        }, {classes: ['amethystora-glass-surface', 'amethystora-glass-banner'], scheme: true});
    }

    _attachKeyboard(child) {
        if (child.name === 'keyboard' || hasClass(child, ['keyboard']))
            this._attach(child, 'keyboard');
    }
}
