// The thumbnail at the bottom right of the screen after a capture, in place of GNOME's notification (board 48, Pin a
// capture): the picture, and under it Edit (in Gradia), Copy, Pin on top, Show in Files and Delete. A click on the
// picture opens it in the image viewer. It goes by itself after six seconds, and only once the pointer has left it.
// It is a glass pane (the screenshot module lists it among its surfaces) and, like everything else of the desktop, it
// is not drawn over a full-screen window.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';
import Shell from 'gi://Shell';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {GRADIA, contentOf, copyToClipboard, gradiaInstalled} from './capture.js';
import {RoundedPicture} from './picture.js';
import {addUnderLock, blurForShare} from './pins.js';

// The board's sizes: the card 210 px wide with 10 px round it, the picture 110 px high with corners of 10, the card's
// own corners of 20, 16 px from the screen's edges
const WIDTH = 210;
const PICTURE_HEIGHT = 110;
const PICTURE_RADIUS = 10;
const EDGE = 16;
const LINGER = 6000;
const SLIDE = 150;

export class Card {
    // A capture's picture, the file it was saved to (or null), whether it only went to the clipboard, and what the
    // module offers: pin(picture, from) when pinning is on, and the icon of a pin
    constructor(picture, file, copied, {pin = null, pinIcon = null, shared = false} = {}) {
        this._picture = picture;
        this._file = file;
        // Mirrored at the bottom left in a right-to-left language
        this._rtl = Clutter.get_default_text_direction() === Clutter.TextDirection.RTL;

        this.actor = new St.BoxLayout({
            style_class: 'amethystora-screenshot-thumb',
            orientation: Clutter.Orientation.VERTICAL,
            width: WIDTH,
            reactive: true,
            track_hover: true,
            opacity: 0,
        });

        const frame = new St.Widget({
            style_class: 'amethystora-screenshot-thumb-picture',
            height: PICTURE_HEIGHT,
            x_expand: true,
            layout_manager: new Clutter.BinLayout(),
            reactive: Boolean(file),
            accessible_name: _('Open the screenshot'),
        });
        this._image = new RoundedPicture(contentOf(picture), picture.width, picture.height, PICTURE_RADIUS, true, {
            x_expand: true,
            y_expand: true,
        });
        frame.add_child(this._image);
        // Taken while the screen is shared: blurred for everyone, as a pinned picture is
        blurForShare(this._image, shared);
        if (file) {
            const click = new Clutter.ClickGesture();
            click.connect('recognize', () => this._act(() => this._openDefault()));
            frame.add_action(click);
        }
        if (copied) {
            const badge = new St.BoxLayout({
                style_class: 'amethystora-screenshot-badge',
                x_align: Clutter.ActorAlign.CENTER,
                y_align: Clutter.ActorAlign.CENTER,
            });
            badge.add_child(new St.Icon({icon_name: 'object-select-symbolic', icon_size: 16}));
            const words = new St.Label({text: _('Copied'), y_align: Clutter.ActorAlign.CENTER});
            badge.add_child(words);
            frame.add_child(badge);
        }
        this.actor.add_child(frame);

        // The row: each button where it applies, spread across the card's width
        const row = new St.BoxLayout({style_class: 'amethystora-screenshot-thumb-row', x_expand: true});
        const buttons = [];
        if (file && gradiaInstalled())
            buttons.push(this._button('document-edit-symbolic', _('Edit'), () => this._openInGradia()));
        buttons.push(this._button('edit-copy-symbolic', _('Copy'), () => copyToClipboard(this._picture)));
        if (pin) {
            const button = this._button(null, _('Pin on top'), () => pin(this._picture, this._pinPlace()), pinIcon);
            button.add_style_class_name('amethystora-screenshot-thumb-pin');
            buttons.push(button);
        }
        if (file) {
            buttons.push(this._button('folder-open-symbolic', _('Show in Files'), () => this._showInFiles()));
            buttons.push(this._button('user-trash-symbolic', _('Delete'), () => this._trash()));
        }
        buttons.forEach((button, i) => {
            if (i > 0)
                row.add_child(new St.Widget({x_expand: true}));
            row.add_child(button);
        });
        this.actor.add_child(row);

        this.actor.connect('notify::hover', () => {
            if (this.actor.hover)
                this._stopTimer();
            else
                this._startTimer();
        });
        this.actor.connect('destroy', () => this._stopTimer());

        addUnderLock(this.actor);
        const monitor = Main.layoutManager.primaryMonitor;
        const [, height] = this.actor.get_preferred_height(WIDTH);
        this.actor.set_position(this._rtl ? monitor.x + EDGE : monitor.x + monitor.width - EDGE - WIDTH,
            monitor.y + monitor.height - EDGE - height);
        this._slide = EDGE + height;
        this.actor.translation_y = this._slide;
        this.actor.ease({
            opacity: 255,
            translation_y: 0,
            duration: SLIDE,
            mode: Clutter.AnimationMode.EASE_OUT_CUBIC,
        });
        this._startTimer();
    }

    // The 32 px buttons of the row, each named for a screen reader and its tooltip
    _button(iconName, name, action, gicon = null) {
        const icon = gicon ? new St.Icon({gicon, icon_size: 16}) : new St.Icon({icon_name: iconName, icon_size: 16});
        const button = new St.Button({
            style_class: 'amethystora-screenshot-thumb-button',
            child: icon,
            accessible_name: name,
            can_focus: true,
        });
        button.connect('clicked', () => this._act(action));
        return button;
    }

    // Where a picture pinned from here starts: its bottom right corner (bottom left in a right-to-left language) at the
    // card's
    _pinPlace() {
        const [x, y] = this.actor.get_transformed_position();
        return {corner: this._rtl ? 'bottom-left' : 'bottom-right', x: this._rtl ? x : x + this.actor.width,
            y: y + this.actor.height};
    }

    _startTimer() {
        this._stopTimer();
        this._timer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, LINGER, () => {
            this._timer = 0;
            this.close();
            return GLib.SOURCE_REMOVE;
        });
    }

    _stopTimer() {
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = 0;
    }

    // Every button, and the picture, does what it says and then puts the card away
    _act(action) {
        try {
            action();
        } catch (e) {
            console.warn(`Amethystora UI: the screenshot's button did not work: ${e.message}`);
        }
        this.close();
    }

    _context() {
        return global.create_app_launch_context(0, -1);
    }

    _openDefault() {
        Gio.app_info_launch_default_for_uri(this._file.get_uri(), this._context());
    }

    // Gradia's own launcher with the picture, as `gio launch` would, with the shell's launch context so that Gradia
    // comes to the front
    _openInGradia() {
        const app = Shell.AppSystem.get_default().lookup_app(`${GRADIA}.desktop`);
        app?.get_app_info()?.launch_uris([this._file.get_uri()], this._context());
    }

    _showInFiles() {
        Gio.DBus.session.call('org.freedesktop.FileManager1', '/org/freedesktop/FileManager1',
            'org.freedesktop.FileManager1', 'ShowItems', new GLib.Variant('(ass)', [[this._file.get_uri()], '']),
            null, Gio.DBusCallFlags.NONE, -1, null, null);
    }

    // To the trash rather than gone: a slip of the pointer is put right in Files
    _trash() {
        this._file.trash_async(GLib.PRIORITY_DEFAULT, null, (file, result) => {
            try {
                file.trash_finish(result);
            } catch (e) {
                console.warn(`Amethystora UI: the screenshot was not moved to the trash: ${e.message}`);
            }
        });
    }

    get closing() {
        return Boolean(this._closing);
    }

    // Down and out; `instantly` when another capture takes its place or the module stops
    close(instantly = false) {
        if (this._closing)
            return;
        this._closing = true;
        this._stopTimer();
        this.actor.reactive = false;
        if (instantly) {
            this.actor.destroy();
            return;
        }
        this.actor.ease({
            opacity: 0,
            translation_y: this._slide,
            duration: SLIDE,
            mode: Clutter.AnimationMode.EASE_IN_CUBIC,
            onStopped: () => this.actor.destroy(),
        });
    }
}
