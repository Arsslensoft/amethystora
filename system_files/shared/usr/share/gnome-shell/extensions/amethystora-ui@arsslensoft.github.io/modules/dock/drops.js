// Files dragged from a window over the dock (spec 1.11, 1.12, 4.9, 4.11): onto a stack they are moved into its folder
// (copied with Ctrl) by Files itself, which shows its own progress, asks about clashes and can undo it; onto the Phone
// they go to the paired phone through the Phone app; onto the dock elsewhere, folders are kept there as stacks.
//
// GNOME Shell has no drop for a window's drag: the compositor refuses a drop on the shell and forgets the dragged data
// before it says the drag ended. The dock therefore reads the list of files while the drag hovers one of its targets,
// and takes the end of the drag, with the button up, over that target as the drop. Only local files count, each
// turned into a Gio.File first; nothing is written or deleted by the dock itself.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as DND from 'resource:///org/gnome/shell/ui/dnd.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as Util from 'resource:///org/gnome/shell/misc/util.js';

import {placeCard, popIn, popOut} from './cards.js';
import {Ring} from './launch.js';
import {EASE, Layer, scaleFactor} from './util.js';

const MOST_BYTES = 1 << 20;
const READ_WAIT = 1000;
const GROWN = 1.18;
const GROW = 200;
const LABEL_GAP = 10;
const PHONE_TURN = 1100;
const BADGE_SHOWN = 2000;
const BADGE_FADE = 300;
const PHONE_CARD = {gap: 14, verdict: 1600, quiet: 10000};
const PHONE_APP = '/usr/bin/amethystora-phone';
export const PHONE_ID = 'amethystora-phone.desktop';

// A text/uri-list as local files: CRLF lines, comments left out, file:// of this computer only, each one a path
export function parseUriList(text) {
    const files = [];
    for (const raw of String(text ?? '').split(/\r?\n/)) {
        const line = raw.trim();
        // A file of this computer only: GIO keeps the path of another host's address, which would name a file here
        // that the address did not
        const lower = line.toLowerCase();
        if (!line || line.startsWith('#') || !(lower.startsWith('file:///') || lower.startsWith('file://localhost/')))
            continue;
        let path = null;
        try {
            path = Gio.File.new_for_uri(line).get_path();
        } catch {}
        if (path && !files.some(file => file.path === path))
            files.push({uri: Gio.File.new_for_path(path).get_uri(), path});
    }
    return files;
}

// What Files' FileOperations2 CopyURIs and MoveURIs take
export function fileOperationArgs(uris, destination) {
    return new GLib.Variant('(assa{sv})', [uris, destination,
        {timestamp: new GLib.Variant('u', global.get_current_time())}]);
}

export class Drops {
    constructor(dock) {
        this._dock = dock;
        this.last = null;
    }

    start() {
        Main.xdndHandler.connectObject(
            'drag-begin', () => this._begin(),
            'drag-end', () => this._end(), this);
    }

    stop() {
        Main.xdndHandler.disconnectObject(this);
        this._forget();
        this._card?.destroy();
        this._card = null;
        this._cardDevice?.proxy.disconnectObject(this);
        this._cardDevice = null;
        if (this._cardTimer)
            GLib.source_remove(this._cardTimer);
        this._cardTimer = 0;
        for (const badge of this._badges ?? [])
            badge.destroy();
        this._badges = [];
    }

    get dragging() {
        return Boolean(this._drag);
    }

    _begin() {
        this._forget();
        this._drag = {target: null, read: null, cancellable: new Gio.Cancellable()};
        this._monitor = {
            dragMotion: event => {
                const target = this._drag?.target;
                if (target && !target.actor.contains(event.targetActor))
                    this._leave();
                return DND.DragMotionResult.CONTINUE;
            },
        };
        DND.addDragMonitor(this._monitor);
        this._dock.dragChanged();
    }

    _forget() {
        if (this._monitor)
            DND.removeDragMonitor(this._monitor);
        this._monitor = null;
        this._leave();
        this._drag?.cancellable.cancel();
        this._drag = null;
        this._label?.destroy();
        this._label = null;
    }

    // The list of files, read once a target is hovered, while the drag still holds it
    _read() {
        const drag = this._drag;
        if (drag.read)
            return drag.read;
        drag.read = new Promise(resolve => {
            const selection = global.display.get_selection();
            const types = selection.get_mimetypes(Meta.SelectionType.SELECTION_DND);
            if (!types.includes('text/uri-list')) {
                resolve([]);
                return;
            }
            const stream = Gio.MemoryOutputStream.new_resizable();
            selection.transfer_async(Meta.SelectionType.SELECTION_DND, 'text/uri-list', MOST_BYTES, stream,
                drag.cancellable, (o, result) => {
                    try {
                        selection.transfer_finish(result);
                        stream.close(null);
                        const bytes = stream.steal_as_bytes().toArray();
                        resolve(parseUriList(new TextDecoder().decode(bytes)));
                    } catch {
                        resolve([]);
                    }
                });
        });
        drag.read.then(files => {
            drag.files = files;
            if (this._drag === drag && drag.target?.kind === 'dock')
                this._checkFolders(drag);
        });
        return drag.read;
    }

    // Ctrl copies; so does a drag from the shelf, which only ever offers a copy (modules/shelf.js)
    _copying(buttons = global.get_pointer()[2]) {
        return (buttons & Clutter.ModifierType.CONTROL_MASK) !== 0 ||
            !!this._dock.ext.modules?.get('shelf')?.fromShelf?.();
    }

    // ---- The targets ----

    // A drag hovers a stack's item
    overStack(dash, state) {
        const copy = this._copying();
        const name = state.key.name;
        this._enter({kind: 'stack', dash, state, actor: state.item},
            copy ? _('Copy to %s').format(name) : _('Move to %s').format(name));
        this._grow(state);
        return copy ? DND.DragMotionResult.COPY_DROP : DND.DragMotionResult.MOVE_DROP;
    }

    // A drag hovers the Phone's item
    overPhone(dash, state) {
        const {devices, why} = this._phones();
        if (!devices.length) {
            this._enter({kind: 'none', dash, state, actor: state.item}, why);
            return DND.DragMotionResult.NO_DROP;
        }
        const label = devices.length === 1 ? _('Send to %s').format(devices[0].name) : _('Send to a phone');
        this._enter({kind: 'phone', dash, state, actor: state.item}, label);
        this._grow(state, true);
        return DND.DragMotionResult.COPY_DROP;
    }

    // A drag hovers the dock away from its items: folders are kept there as stacks
    overDock(dash) {
        if (this._drag?.target?.kind !== 'dock' || this._drag.target.dash !== dash) {
            this._enter({kind: 'dock', dash, actor: dash}, null);
            if (this._drag.files)
                this._checkFolders(this._drag);
        }
        return this._drag.target.folders ? DND.DragMotionResult.COPY_DROP : DND.DragMotionResult.CONTINUE;
    }

    _enter(target, label) {
        const drag = this._drag;
        if (!drag)
            return;
        if (drag.target?.actor !== target.actor || drag.target?.kind !== target.kind) {
            this._leave();
            drag.target = target;
            this._read();
        }
        drag.target.labelText = label;
        this._showLabel(target, label);
    }

    // The phone's people are reached only through the image's own Phone app, and only those ready to take files
    _phones() {
        const connect = this._dock.ext.modules?.get('connect');
        const link = connect?._link;
        if (!link || !connect._trusted)
            return {devices: [], why: _('Phone pairing is off')};
        const devices = link.devices.filter(device => device.ready && device.launchable);
        return {devices, why: devices.length ? null : _('No phone is connected')};
    }

    _grow(state, ring = false) {
        const target = this._drag?.target;
        if (!target || target.grown)
            return;
        target.grown = true;
        const visual = state.dash.visualOf(state.item);
        target.visual = visual;
        if (visual) {
            visual._amethystoraBusy = true;
            visual.ease({scale_x: GROWN, scale_y: GROWN, duration: GROW, mode: EASE.out});
        }
        if (ring && state.button._iconContainer) {
            const size = Math.round((state.dash.iconSize + 14) * scaleFactor());
            const layer = new Layer();
            const halo = new Ring(size);
            layer.add_child(halo);
            state.button._iconContainer.add_child(layer);
            const place = () => halo.set_position(Math.round((layer.width - size) / 2),
                Math.round((layer.height - size) / 2));
            layer.connect('notify::allocation', place);
            place();
            halo.ease({opacity: 255, duration: 210, mode: EASE.inOut});
            if (this._dock.motionLevel() === 'full')
                halo.ease({rotation_angle_z: 360, duration: PHONE_TURN, mode: EASE.linear, repeatCount: -1});
            target.ring = layer;
        }
    }

    _leave() {
        const target = this._drag?.target;
        if (!target)
            return;
        this._drag.target = null;
        target.ring?.destroy();
        if (target.visual) {
            const visual = target.visual;
            visual.ease({
                scale_x: 1, scale_y: 1, duration: GROW, mode: EASE.out,
                onComplete: () => (visual._amethystoraBusy = false),
            });
        }
        target.dash?.pill?.remove_style_class_name('amethystora-dock-pill-drop');
        this._label?.hide();
    }

    // Whether every file dragged is a folder, before the dock says it keeps them. Asked without waiting (a stale
    // network mount would otherwise hold the whole session), once a drag; until the answer the drop keeps nothing.
    async _checkFolders(drag) {
        drag.folders ??= this._foldersOf(drag.files ?? [], drag.cancellable);
        let folders;
        try {
            folders = await drag.folders;
        } catch {
            return;
        }
        const target = drag.target;
        if (this._drag !== drag || target?.kind !== 'dock')
            return;
        target.folders = folders;
        if (folders) {
            target.dash.pill?.add_style_class_name('amethystora-dock-pill-drop');
            target.labelText = _('Keep in the dock');
            this._showLabel(target, target.labelText);
        }
    }

    async _foldersOf(files, cancellable) {
        if (!files.length)
            return false;
        for (const {path} of files.slice(0, 64)) {
            const file = Gio.File.new_for_path(path);
            // eslint-disable-next-line no-await-in-loop
            const type = await new Promise(resolve => file.query_info_async('standard::type',
                Gio.FileQueryInfoFlags.NONE, GLib.PRIORITY_DEFAULT, cancellable, (o, result) => {
                    try {
                        resolve(file.query_info_finish(result).get_file_type());
                    } catch {
                        resolve(null);
                    }
                }));
            if (type !== Gio.FileType.DIRECTORY)
                return false;
        }
        return true;
    }

    _showLabel(target, text) {
        if (!text) {
            this._label?.hide();
            return;
        }
        if (!this._label) {
            this._label = new St.Label({style_class: 'dash-label amethystora-dock-label'});
            this._label.clutter_text.use_markup = false;
            Main.layoutManager.addTopChrome(this._label);
        }
        const label = this._label;
        label.text = text;
        label.show();
        label.opacity = 255;
        const sf = scaleFactor();
        const dash = target.dash;
        const [, , width, height] = label.get_preferred_size();
        let x, y;
        if (target.kind === 'dock') {
            const pill = dash.pillRect();
            x = pill.x + (pill.width - width) / 2;
            y = pill.y - LABEL_GAP * sf - height;
            if (dash.edgeName !== 'bottom') {
                y = pill.y + (pill.height - height) / 2;
                x = dash.edgeName === 'left' ? pill.x + pill.width + LABEL_GAP * sf : pill.x - LABEL_GAP * sf - width;
            }
        } else {
            const box = target.state.restRect();
            const grown = GROWN - 1;
            if (dash.edgeName === 'bottom') {
                x = box.x + (box.width - width) / 2;
                y = box.y - box.height * grown - LABEL_GAP * sf - height;
            } else if (dash.edgeName === 'left') {
                x = box.x + box.width * GROWN + LABEL_GAP * sf;
                y = box.y + (box.height - height) / 2;
            } else {
                x = box.x - box.width * grown - LABEL_GAP * sf - width;
                y = box.y + (box.height - height) / 2;
            }
        }
        label.set_position(Math.round(Math.clamp(x, 0, global.stage.width - width)), Math.round(y));
    }

    // ---- The drop ----

    async _end() {
        const drag = this._drag;
        const target = drag?.target;
        const buttons = global.get_pointer()[2];
        const copy = this._copying(buttons);
        const released = (buttons & Clutter.ModifierType.BUTTON1_MASK) === 0;
        if (this._monitor)
            DND.removeDragMonitor(this._monitor);
        this._monitor = null;
        if (!drag || !target || !released || !['stack', 'phone', 'dock'].includes(target.kind)) {
            this._forget();
            this._dock.dragChanged();
            return;
        }
        // The drag is over: what it hovered goes back, and a new drag may begin while this one's files are read
        this._leave();
        this._label?.destroy();
        this._label = null;
        this._drag = null;
        this._dock.dragChanged();
        let files = [];
        if (drag.read) {
            let timer = 0;
            files = await Promise.race([drag.read, new Promise(resolve => {
                timer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, READ_WAIT, () => {
                    timer = 0;
                    resolve([]);
                    return GLib.SOURCE_REMOVE;
                });
            })]);
            if (timer)
                GLib.source_remove(timer);
        }
        drag.cancellable.cancel();
        if (this._dock.stopped)
            return;
        this.drop(target, files, copy);
    }

    // What a drop does with its files; open for tests, which have no window to drag from
    drop(target, files, copy) {
        if (target.kind === 'stack')
            return this._toStack(target, files, copy);
        if (target.kind === 'phone')
            return this._toPhone(target, files);
        if (target.kind === 'dock' && target.folders)
            return this._keep(files);
        return null;
    }

    _toStack(target, files, copy) {
        const place = target.state.key;
        const destination = Gio.File.new_for_path(place.path);
        const uris = files.filter(file => {
            const parent = Gio.File.new_for_path(file.path).get_parent();
            return !parent?.equal(destination);
        }).map(file => file.uri);
        this.last = {target: place.name, uris, copy, sent: 0, at: GLib.get_monotonic_time()};
        if (!uris.length)
            return this.last;
        const record = this.last;
        Gio.DBus.session.call('org.gnome.Nautilus', '/org/gnome/Nautilus/FileOperations2',
            'org.gnome.Nautilus.FileOperations2', copy ? 'CopyURIs' : 'MoveURIs',
            fileOperationArgs(uris, destination.get_uri()), null, Gio.DBusCallFlags.NONE, -1, null, (bus, result) => {
                try {
                    bus.call_finish(result);
                } catch (e) {
                    const message = Gio.DBusError.strip_remote_error(e.message ?? String(e));
                    Main.notifyError(copy ? _('The files could not be copied to %s').format(place.name)
                        : _('The files could not be moved to %s').format(place.name), message);
                    return;
                }
                record.sent = uris.length;
                this._badge(target.state, uris.length);
            });
        return record;
    }

    _badge(state, count) {
        const container = state.button?._iconContainer;
        const icon = state.button?.icon?.icon;
        if (!container || !icon || !container.mapped)
            return;
        const layer = new Layer();
        const badge = new St.Label({style_class: 'amethystora-dock-drop-badge', text: `+${count.toLocaleString()}`});
        badge.clutter_text.use_markup = false;
        layer.add_child(badge);
        container.add_child(layer);
        this._badges ??= [];
        this._badges.push(layer);
        const sf = scaleFactor();
        const place = () => {
            const box = icon.get_allocation_box();
            let [x, y] = [box.x1, box.y1];
            for (let parent = icon.get_parent(); parent && parent !== container; parent = parent.get_parent()) {
                const around = parent.get_allocation_box();
                x += around.x1;
                y += around.y1;
            }
            const [, , width] = badge.get_preferred_size();
            const rtl = state.dash.get_text_direction() === Clutter.TextDirection.RTL;
            const right = rtl ? x - 6 * sf + width : x + box.get_width() + 6 * sf;
            badge.set_position(Math.round(right - width), Math.round(y - 8 * sf));
        };
        place();
        let timer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, BADGE_SHOWN, () => {
            timer = 0;
            badge.ease({
                opacity: 0, duration: BADGE_FADE, mode: EASE.out,
                onComplete: () => {
                    this._badges = this._badges.filter(other => other !== layer);
                    layer.destroy();
                },
            });
            return GLib.SOURCE_REMOVE;
        });
        layer.connect('destroy', () => {
            if (timer)
                GLib.source_remove(timer);
            timer = 0;
        });
    }

    _keep(files) {
        let kept = 0;
        for (const {path} of files) {
            if (this._dock.stacks?.add(path))
                kept++;
        }
        this.last = {
            target: 'dock', uris: files.map(file => file.uri), copy: false, sent: kept,
            at: GLib.get_monotonic_time(),
        };
        return this.last;
    }

    _toPhone(target, files) {
        const {devices} = this._phones();
        const paths = files.map(file => file.path);
        this.last = {
            target: 'phone', uris: files.map(file => file.uri), copy: true, sent: 0,
            at: GLib.get_monotonic_time(),
        };
        if (!devices.length || !paths.length)
            return this.last;
        if (devices.length === 1) {
            const [device] = devices;
            this.last.argv = [PHONE_APP, 'send', '--now', `--device=${device.id}`, '--', ...paths];
            Util.spawn(this.last.argv);
            this.last.sent = paths.length;
            this._phoneCard(target, device);
        } else {
            this.last.argv = [PHONE_APP, 'send', '--', ...paths];
            Util.spawn(this.last.argv);
            this.last.sent = paths.length;
        }
        return this.last;
    }

    // ---- How far the phone got ----

    _phoneCard(target, device) {
        this._closePhoneCard(true);
        const dash = target.dash;
        const card = new St.BoxLayout({
            style_class: 'amethystora-dock-phone-card',
            orientation: Clutter.Orientation.VERTICAL,
        });
        const text = new St.Label({
            style_class: 'amethystora-dock-phone-text',
            text: _('Sending to %s').format(device.name),
        });
        text.clutter_text.use_markup = false;
        const track = new St.Widget({
            style_class: 'amethystora-dock-phone-track', layout_manager: new Clutter.BinLayout(),
            x_expand: true,
        });
        const fill = new St.Widget({
            style_class: 'amethystora-dock-phone-fill', x_align: Clutter.ActorAlign.START,
            y_expand: true,
        });
        track.add_child(fill);
        card.add_child(text);
        card.add_child(track);
        Main.layoutManager.addChrome(card);
        placeCard(card, dash, target.state.restCentre(), PHONE_CARD.gap);
        popIn(card, dash, {from: 10, duration: 220, level: this._dock.motionLevel()});
        this._card = card;
        this._cardParts = {text, track, fill, dash, device, name: device.name};
        this._transfers = new Map();
        this._dock.glassChanged();
        device.proxy.connectObject('g-signal', (proxy, sender, signal, params) => {
            if (signal === 'Transfer')
                this._transfer(params);
        }, this);
        this._cardDevice = device;
        this._quiet();
    }

    // Nothing heard for a while (the Phone app asks the person first): the card goes without a word
    _quiet() {
        if (this._cardTimer)
            GLib.source_remove(this._cardTimer);
        this._cardTimer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, PHONE_CARD.quiet, () => {
            this._cardTimer = 0;
            this._closePhoneCard();
            return GLib.SOURCE_REMOVE;
        });
    }

    _transfer(params) {
        let id, incoming, done, total, state;
        try {
            [id, , incoming, done, total, state] = params.deepUnpack();
        } catch {
            return;
        }
        if (incoming || typeof id !== 'string')
            return;
        if (!this._transfers.has(id) && state !== 'started' && state !== 'progress')
            return;
        this._transfers.set(id, {done: Math.max(0, Number(done)), total: Math.max(0, Number(total)), state});
        this._quiet();
        const all = [...this._transfers.values()];
        const sum = all.reduce((a, t) => a + t.done, 0);
        const whole = all.reduce((a, t) => a + t.total, 0);
        const share = whole > 0 ? Math.min(1, sum / whole) : 0;
        const parts = this._cardParts;
        // Translators: the phone's name and how far the files have gone, as in "Sending to Pixel · 64%"
        parts.text.text = _('Sending to %s · %s').format(parts.name,
            new Intl.NumberFormat(undefined, {style: 'percent'}).format(share));
        const width = parts.track.width;
        parts.fill.width = Math.round(width * share);
        const ended = all.every(t => ['finished', 'failed', 'cancelled'].includes(t.state));
        if (ended) {
            const failed = all.some(t => t.state !== 'finished');
            parts.text.text = failed ? _('Not sent') : _('Sent');
            parts.fill.width = failed ? parts.fill.width : width;
            if (this._cardTimer)
                GLib.source_remove(this._cardTimer);
            this._cardTimer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, PHONE_CARD.verdict, () => {
                this._cardTimer = 0;
                this._closePhoneCard();
                return GLib.SOURCE_REMOVE;
            });
        }
    }

    _closePhoneCard(now = false) {
        if (this._cardTimer)
            GLib.source_remove(this._cardTimer);
        this._cardTimer = 0;
        this._cardDevice?.proxy.disconnectObject(this);
        this._cardDevice = null;
        const card = this._card;
        this._card = null;
        if (!card)
            return;
        if (now) {
            card.destroy();
            this._dock.glassChanged();
        } else {
            popOut(card, this._cardParts.dash, {
                to: 10, duration: 220, level: this._dock.motionLevel(),
                onComplete: () => this._dock.glassChanged(),
            });
        }
    }

    get card() {
        return this._card;
    }
}
