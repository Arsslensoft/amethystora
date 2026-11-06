// Drawing on a screenshot in GNOME's own screenshot UI (Print), and what happens to the picture after. While an area
// or a whole screen is framed, a toolbar at the top offers a pen, boxes, a highlighter, an arrow, text and numbered
// markers, which are burnt into the picture. The capture goes to the clipboard and the screenshots folder and shows
// a small card to open it from; Ctrl+C only copies, Ctrl+S asks where to save, and Extract Text (Ctrl+E) hands the
// area to Gradia, which copies the text in it. A window is captured over the windows it belongs to. Everything is
// GNOME's UI with these pieces added while it is open; the keybindings that take a screenshot without it are GNOME's.
//
// Pin a capture (board 48): the card is a thumbnail with Edit, Copy, Pin on top, Show in Files and Delete
// (capture-thumbnail; off, GNOME's own words come back as a notification), and a capture can be pinned above every
// window, from the thumbnail or straight from the bar's Pin button or Ctrl+P (capture-pin; screenshot/pins.js).
//
// Hide what's private (R7-67): a button beside Pin covers addresses, card, phone and account numbers with solid boxes
// (hide-private; screenshot/cover.js). Any language (R7-47): another reads the box and translates it on this computer, once
// a language was chosen (translate-to; screenshot/translate.js).

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import Gettext from 'gettext';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as MessageTray from 'resource:///org/gnome/shell/ui/messageTray.js';
import * as Util from 'resource:///org/gnome/shell/misc/util.js';

import {
    GRADIA, captureFrame, captureWindow, contentOf, copyToClipboard, disposeSettings, gradiaInstalled, playShutter,
    saveAs, saveToFolder,
} from './screenshot/capture.js';
import {Card} from './screenshot/card.js';
import {DRAWING, MAX_WIDTH, MIN_WIDTH} from './screenshot/marks.js';
import {Pins, forgetWaiting} from './screenshot/pins.js';
import {Capture} from './pill/capture.js';
import {Session} from './screenshot/session.js';
import {RecordingTrim} from './screenshot/trim.js';

// The mode GNOME's screenshot UI opens in for an app's interactive screenshot through the screenshot portal
const PORTAL_MODE = 2;
const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.screenshot';
// Icons drawn for the toolbar, in icons/screenshot; the rest are the icon theme's
const OWN_ICONS = new Set(['move', 'pen', 'box', 'fill', 'highlight', 'arrow', 'text', 'marker', 'extract', 'pin',
    'through', 'cover', 'translate']);
// GNOME's own words for a capture, in its own translations, for the notification that stands in for the thumbnail
const gnome = text => Gettext.dgettext('gnome-shell', text);
const THEME_ICONS = {area: 'screenshot-ui-area-symbolic', display: 'screenshot-ui-display-symbolic'};

export class Screenshot {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        const ui = Main.screenshotUI;
        this._ui = ui;
        this._settings = this._ext.getSettings(SCHEMA);
        this._pairs = this._loadPairs();
        this._pending = new Set();
        this._icons = new Map();

        // GNOME's own methods, kept to be given back: the opening, wrapped, and the saving, which this takes over
        this._open = Object.getOwnPropertyDescriptor(ui, 'open');
        this._save = Object.getOwnPropertyDescriptor(ui, '_saveScreenshot');
        this._close = Object.getOwnPropertyDescriptor(ui, 'close');
        const open = ui.open;
        ui.open = (mode, ...rest) => this._onOpen(open, mode, rest);
        // GNOME closes the UI once a capture's saving returns: a capture refused while Hide what's private reads keeps it
        const close = ui.close;
        ui.close = (...rest) => {
            if (this._keepOpen) {
                this._keepOpen = false;
                return;
            }
            close.call(ui, ...rest);
        };
        // The pen's Text tool (modules/touch/ring.js) asks for one capture to go to Extract Text
        ui._saveScreenshot = () => this._take(this._forText && !this._ui?._castButton?.checked ? 'extract' : 'normal');

        // Before GNOME's keys, and before Escape closes the UI, so that the text field can take it
        ui.connectObject(
            'captured-event', (actor, event) => this._onEvent(event),
            'closed', () => {
                this._forText = false;
                this._endSession();
            }, this);

        // Pinned pictures, put back after a screen lock while pinning is on
        this._pins = new Pins(this);
        if (this.pinOn)
            this._pins.enable();
        else
            forgetWaiting();
        this._settings.connectObject(
            'changed::capture-pin', () => this._pinChanged(),
            'changed::hide-private', () => this._session?.syncCover(),
            'changed::translate-to', () => this._session?.syncTranslate(),
            'changed::capture-thumbnail', () => {
                if (!this._settings.get_boolean('capture-thumbnail'))
                    this._card?.close(true);
            }, this);
        // Trim on a recording's notice (R6-19), while recording-trim is on
        this.trim = new RecordingTrim(ui, this._settings);
        this.trim.enable();
    }

    // GNOME's screenshot UI on a selection, whose capture copies the text in it (through Gradia), for the pen's Text
    // tool. False when Gradia is not there to read the text.
    openForText() {
        if (!this._ui || !gradiaInstalled())
            return false;
        this._forText = true;
        Promise.resolve(this._ui.open()).then(() => {
            // GNOME did not open it (a recording runs): the next capture is an ordinary one
            if (!this._ui?.visible)
                this._forText = false;
            else if (this._ui._selectionButton)
                this._ui._selectionButton.checked = true;
        }).catch(logError);
        return true;
    }

    disable() {
        const ui = this._ui;
        if (!ui)
            return;
        this.trim?.disable();
        this.trim = null;
        this._settings?.disconnectObject(this);
        ui.disconnectObject(this);
        this._endSession();
        this._pins?.disable();
        this._pins = null;
        this._notifications?.destroy();
        this._notifications = null;
        for (const [name, descriptor] of [['open', this._open], ['_saveScreenshot', this._save], ['close', this._close]]) {
            if (descriptor)
                Object.defineProperty(ui, name, descriptor);
            else
                delete ui[name];
        }
        for (const id of this._pending ?? [])
            Gio.DBus.session.signal_unsubscribe(id);
        this._pending = null;
        this._card?.close(true);
        this._card = null;
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
        disposeSettings();
        this._icons = null;
        this._ui = null;
    }

    // ---- Pin a capture ----

    get pinOn() {
        return this._settings?.get_boolean('capture-pin') ?? false;
    }

    // Off: no Pin anywhere, and what is pinned lets go; on again: the bar offers it at its next opening
    _pinChanged() {
        if (!this.pinOn)
            this._pins?.clear();
        this._session?.syncPin();
    }

    // Hide what's private (R7-67, screenshot/cover.js)
    get coverOn() {
        return this._settings?.get_boolean('hide-private') ?? false;
    }

    // Any language (R7-47, screenshot/translate.js): empty until a language was chosen
    get translateTo() {
        return this._settings?.get_string('translate-to') ?? '';
    }

    copyNow() {
        this._take('copy').catch(logError);
    }

    // The pinned pictures' buttons and the thumbnail are glass panes
    glassSurfaces() {
        const surfaces = this._pins?.glassSurfaces() ?? [];
        if (this._card && !this._card.closing)
            surfaces.push({actor: this._card.actor, material: 'glass', radius: null});
        return surfaces;
    }

    surfacesChanged() {
        this._ext.modules?.get('glass')?.surfacesChanged?.();
    }

    // The button and Ctrl+P: the area (or the screen) straight on top, where it was
    pin() {
        this._take('pin').catch(logError);
    }

    // A pinned picture's Save as…
    saveAs(picture) {
        saveAs(picture, this._pending).catch?.(logError);
    }

    // For the VM suite: the pinned pictures as a person sees them
    pins() {
        return this._pins?.list.map(pin => pin.state) ?? [];
    }

    _onOpen(open, mode, rest) {
        const ui = this._ui;
        const wasOpen = ui.visible;
        const opening = open.call(ui, mode, ...rest);
        // GNOME has just made its window previews, before its first wait: which window each one is
        if (!wasOpen)
            this._windows = this._mapWindows();
        const portal = mode === PORTAL_MODE;
        return Promise.resolve(opening).then(() => {
            if (!ui.visible || this._ui !== ui)
                return;
            try {
                if (this._session)
                    this._session.setPortal(portal);
                else
                    this._session = new Session(this, ui, portal);
            } catch (e) {
                console.error(`Amethystora UI: the screenshot tools did not open: ${e.message}\n${e.stack}`);
                this._endSession();
            }
        });
    }

    _endSession() {
        try {
            this._session?.destroy();
        } catch (e) {
            console.error(`Amethystora UI: the screenshot tools did not close cleanly: ${e.message}`);
        }
        this._session = null;
        this._windows = null;
        this._savePairs();
    }

    // GNOME's window previews are made in the order of the windows of each monitor it shows; the same windows in the
    // same order, each checked against its preview's frame, say which window a preview is
    _mapWindows() {
        const map = new Map();
        const workspace = global.workspace_manager.get_active_workspace();
        this._ui._windowSelectors.forEach((selector, monitor) => {
            const windows = global.get_window_actors().map(actor => actor.metaWindow).filter(window =>
                !window.is_override_redirect() && window.located_on_workspace(workspace) &&
                window.get_monitor() === monitor);
            const previews = selector.windows();
            if (windows.length !== previews.length)
                return;
            previews.forEach((preview, i) => {
                const frame = windows[i].get_frame_rect();
                const box = preview.boundingBox;
                if (frame.x === box.x && frame.y === box.y && frame.width === box.width && frame.height === box.height)
                    map.set(preview, windows[i]);
            });
        });
        return map;
    }

    _onEvent(event) {
        const session = this._session;
        if (!session)
            return Clutter.EVENT_PROPAGATE;
        const type = event.type();
        if (type === Clutter.EventType.SCROLL) {
            // One wheel step is a pixel of line width; the scroll still goes on to whatever is under the pointer,
            // but the width slider, which would move again
            const direction = event.get_scroll_direction();
            if (direction !== Clutter.ScrollDirection.UP && direction !== Clutter.ScrollDirection.DOWN)
                return Clutter.EVENT_PROPAGATE;
            session.scrollWidth(direction === Clutter.ScrollDirection.UP ? 1 : -1);
            const target = global.stage.get_event_actor(event);
            return session.slider.contains(target) ? Clutter.EVENT_STOP : Clutter.EVENT_PROPAGATE;
        }
        if (type !== Clutter.EventType.KEY_PRESS)
            return Clutter.EVENT_PROPAGATE;

        const symbol = event.get_key_symbol();
        const ctrl = (event.get_state() & Clutter.ModifierType.CONTROL_MASK) !== 0;
        const ui = this._ui;
        const recording = ui._castButton.checked;
        // With the text field open too: the text is placed first, and goes into the picture
        if (ctrl) {
            // Copying only; while recording, or for an app that asked for a screenshot, GNOME's own Ctrl+C takes it
            if ((symbol === Clutter.KEY_c || symbol === Clutter.KEY_C) && !recording && !session.portal) {
                this._take('copy').catch(logError);
                return Clutter.EVENT_STOP;
            }
            if ((symbol === Clutter.KEY_s || symbol === Clutter.KEY_S) && !recording) {
                this._take('save-as').catch(logError);
                return Clutter.EVENT_STOP;
            }
            // Exactly what the button does, and nothing while the button is off
            if ((symbol === Clutter.KEY_e || symbol === Clutter.KEY_E) && session.extractLive) {
                this.extractText();
                return Clutter.EVENT_STOP;
            }
            // Pin, as its button; GNOME's own P (the pointer, with or without Ctrl) stays for P alone
            if ((symbol === Clutter.KEY_p || symbol === Clutter.KEY_P) && session.pinLive) {
                this.pin();
                return Clutter.EVENT_STOP;
            }
        }
        return session.handleKey(event);
    }

    // The button and Ctrl+E
    extractText() {
        this._take('extract').catch(logError);
    }

    // A capture: `normal` (the capture button, Enter, Space) to the clipboard, the folder and the card; `copy` to the
    // clipboard and the card; `save-as` where the person says; `extract` to the folder and then Gradia. Everything
    // the picture needs is read from the UI at once, and the UI goes while the picture is finished behind it (GNOME
    // closes it itself once a normal capture returns); only an app that asked for the screenshot keeps it until the
    // file is there, since GNOME answers the app "cancelled" as soon as the UI has closed.
    _take(kind) {
        const ui = this._ui;
        if (this._busy || !ui)
            return Promise.resolve();
        // Hide what's private is still reading the picture: it is not taken without the boxes it was asked to put on
        if (this._session?.coverReading) {
            this._session.coverWait();
            // The close GNOME makes right after a normal capture's saving returns, and only that one
            if (kind === 'normal') {
                this._keepOpen = true;
                GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
                    this._keepOpen = false;
                    return GLib.SOURCE_REMOVE;
                });
            }
            return Promise.resolve();
        }
        let picturing;
        try {
            picturing = this._picture();
        } catch (e) {
            picturing = Promise.reject(e);
        }
        // Nothing to take: the UI stays as it is
        if (!picturing)
            return Promise.resolve();
        this._busy = true;
        // A pin goes where the area was, read before the UI closes
        const place = kind === 'pin' ? this._selectedPlace() : null;
        const done = this._finish(kind, ui, picturing, place);
        if (kind === 'normal' && this._session?.portal)
            return done;
        if (kind !== 'normal')
            ui.close();
        return Promise.resolve();
    }

    // The area or the screen chosen, in the stage's logical pixels
    _selectedPlace() {
        const [x, y, width, height] = this._ui._getSelectedGeometry(false);
        return {x, y, width, height};
    }

    async _finish(kind, ui, picturing, place) {
        try {
            const picture = await picturing;
            // The picture is made: the next screenshot need not wait for this one's file chooser, which may stay open
            this._busy = false;
            if (!picture || this._ui !== ui)
                return;
            switch (kind) {
            case 'normal': {
                copyToClipboard(picture);
                const file = saveToFolder(picture);
                if (file)
                    ui.emit('screenshot-taken', file);
                this._showCard(picture, file, false);
                break;
            }
            case 'copy':
                copyToClipboard(picture);
                this._showCard(picture, null, true);
                break;
            case 'save-as': {
                const file = await saveAs(picture, this._pending);
                if (file && this._ui)
                    this._showCard(picture, file, false, false);
                break;
            }
            case 'extract': {
                const file = saveToFolder(picture);
                if (file) {
                    ui.emit('screenshot-taken', file);
                    Util.spawn(['flatpak', 'run', GRADIA, `--ocr-file=${file.get_path()}`]);
                }
                break;
            }
            // Neither saved nor copied: only on top, until it is saved from its menu
            case 'pin':
                if (this.pinOn)
                    this._pins?.pin(picture, place);
                break;
            }
        } catch (e) {
            console.error(`Amethystora UI: the screenshot was not taken: ${e.message}\n${e.stack}`);
        } finally {
            this._busy = false;
        }
    }

    // The picture as it will be kept: the area or the screen with the marks, or the window with its parents; null
    // for nothing to take
    _picture() {
        const ui = this._ui;
        this._session?.commit();
        if (ui._selectionButton.checked) {
            const [,, width, height] = ui._areaSelector.getGeometry();
            if (width <= 2 || height <= 2)
                return null;
        }
        if (ui._windowButton.checked) {
            const chosen = ui._windowSelectors.flatMap(selector => selector.windows()).find(w => w.checked);
            if (!chosen)
                return null;
            playShutter();
            const scale = chosen.bufferScale;
            return captureWindow(ui, chosen, this._windows?.get(chosen) ?? null).then(p => p && {...p, scale});
        }
        if (!ui._selectionButton.checked && !ui._screenButton.checked)
            return null;
        playShutter();
        // A copy: while an app's screenshot is finished the UI stays, and an undo or a new mark must not reach it;
        // the picture's pixels to a logical pixel, for a pin's size
        const scale = ui._scale;
        return captureFrame(ui, [...this._session?.marks ?? []]).then(p => p && {...p, scale});
    }

    // pasted: the picture is on the clipboard too (not after Save as)
    _showCard(picture, file, copied, pasted = true) {
        this._card?.close(true);
        this._card = null;
        if (!this._settings.get_boolean('capture-thumbnail')) {
            this._notify(picture, file, pasted);
            return;
        }
        // Whether the screen is shared or recorded now, as GNOME's own indicators know it
        const capture = new Capture();
        const unlisten = capture.listen(() => {});
        const shared = capture.active;
        unlisten();
        const card = new Card(picture, file, copied, {
            shared,
            pin: this.pinOn ? (p, place) => this._pins?.pin(p, place) : null,
            pinIcon: this.icon('pin'),
        });
        this._card = card;
        card.actor.connect('destroy', () => {
            if (this._card === card)
                this._card = null;
            this.surfacesChanged();
        });
        this.surfacesChanged();
    }

    // The thumbnail turned off: GNOME's own notification, in GNOME's words, so that a capture still says it happened
    _notify(picture, file, pasted) {
        if (!this._notifications) {
            this._notifications = new MessageTray.Source({
                title: gnome('Screen Capture'),
                iconName: 'screenshooter-symbolic',
            });
            this._notifications.connect('destroy', source => {
                if (this._notifications === source)
                    this._notifications = null;
            });
            Main.messageTray.add(this._notifications);
        }
        const notification = new MessageTray.Notification({
            source: this._notifications,
            title: gnome('Screenshot captured'),
            body: pasted ? gnome('You can paste the image from the clipboard') : '',
            datetime: GLib.DateTime.new_now_local(),
            gicon: contentOf(picture),
            isTransient: true,
        });
        if (file) {
            const context = () => global.create_app_launch_context(0, -1);
            notification.addAction(gnome('Show in Files'), () => {
                Gio.DBus.session.call('org.freedesktop.FileManager1', '/org/freedesktop/FileManager1',
                    'org.freedesktop.FileManager1', 'ShowItems', new GLib.Variant('(ass)', [[file.get_uri()], '']),
                    null, Gio.DBusCallFlags.NONE, -1, null, null);
            });
            notification.connect('activated', () => {
                try {
                    Gio.app_info_launch_default_for_uri(file.get_uri(), context());
                } catch (e) {
                    console.warn(`Amethystora UI: the screenshot could not be opened: ${e.message}`);
                }
            });
        }
        this._notifications.addNotification(notification);
    }

    gradiaInstalled() {
        return gradiaInstalled();
    }

    icon(name) {
        let icon = this._icons.get(name);
        if (!icon) {
            icon = OWN_ICONS.has(name)
                ? Gio.FileIcon.new(this._ext.dir.get_child('icons').get_child('screenshot')
                    .get_child(`${name}-symbolic.svg`))
                : Gio.ThemedIcon.new(THEME_ICONS[name]);
            this._icons.set(name, icon);
        }
        return icon;
    }

    // The colour and width each drawing tool had when it was last used, kept between screenshots and sessions
    _loadPairs() {
        const pairs = new Map();
        const stored = this._settings.get_value('tool-settings').recursiveUnpack();
        for (const [tool, [colour, width]] of Object.entries(stored)) {
            if (DRAWING.has(tool) && /^#[0-9a-f]{6}$/i.test(colour) && width >= MIN_WIDTH && width <= MAX_WIDTH)
                pairs.set(tool, [colour.toLowerCase(), width]);
        }
        return pairs;
    }

    // Written once, as the UI closes, rather than for every move of the slider
    _savePairs() {
        if (!this._settings || !this._pairsChanged)
            return;
        this._pairsChanged = false;
        const value = {};
        for (const [tool, pair] of this._pairs)
            value[tool] = pair;
        this._settings.set_value('tool-settings', new GLib.Variant('a{s(sd)}', value));
    }

    pairOf(tool) {
        return this._pairs.get(tool) ?? null;
    }

    rememberPair(tool, colour, width) {
        const old = this._pairs.get(tool);
        if (old && old[0] === colour && old[1] === width)
            return;
        this._pairs.set(tool, [colour, width]);
        this._pairsChanged = true;
    }
}
