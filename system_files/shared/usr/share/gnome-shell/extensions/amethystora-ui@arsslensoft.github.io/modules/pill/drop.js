// Drop things on it (R7-18, the key drop-targets): drag a file up to the pill and it opens into targets, each a feature
// that exists already: the phone link, the Shelf, Convert, Copy without hidden data, the agent. A target that is off or
// not installed is not offered, and nothing happens until a file is let go on one. Files only, as paths: to a helper as
// an argument after `--`, or on standard input, as each of them takes it today. Only during a drag is anything listened
// to beyond GNOME's two signals, and the paths are held in memory, for that drag, and for half a minute after a drop on
// Convert while its choice is open.
//
// A drag let go over the shell goes to nobody and Mutter forgets its data before the shell hears it ended, so the data is
// read as the targets open, as the Shelf and the dock read theirs.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Pango from 'gi://Pango';
import St from 'gi://St';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as DND from 'resource:///org/gnome/shell/ui/dnd.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as Util from 'resource:///org/gnome/shell/misc/util.js';

import {buttonRow, cardButton, retire} from './clock.js';
import {plain} from './text.js';
import {readDrag} from '../shelf/drop.js';

const PHONE_APP = '/usr/bin/amethystora-phone';
const CONVERT = '/usr/libexec/amethystora-convert';
const CLEAN_COPY = '/usr/libexec/amethystora-clean-copy';
// How far round the pill a drag opens the targets, and how far outside them it closes them, in logical pixels
const REACH_X = 160;
const REACH_Y = 40;
const LEAVE = 80;
const CARD_GAP = 8;
const FADE = 120;
const CHOICE_SECONDS = 30;

export class DropTargets {
    // ext: for the modules each target belongs to; pill: the pill's actor now; openCard: opens an activity's card;
    // glassChanged: the card is a surface of the glass; capture: says whether the screen is recorded or shared
    constructor(activities, {ext, pill, openCard, glassChanged, capture}) {
        this._activities = activities;
        this._ext = ext;
        this._pillActor = pill;
        this._openCard = openCard;
        this._glassChanged = glassChanged;
        this._capture = capture;
        this.id = 'drop';
        this.plate = '';
        this.beside = false;
    }

    enable() {
        this._off = false;
        Main.xdndHandler.connectObject('drag-begin', () => this._begin(), 'drag-end', () => this._end(), this);
    }

    disable() {
        this._off = true;
        Main.xdndHandler.disconnectObject(this);
        this._forget();
        this._endChoice();
    }

    glassSurfaces() {
        return this._card ? [{actor: this._card, material: 'glass', radius: null}] : [];
    }

    // --- The drag -------------------------------------------------------------------------------------------------

    _begin() {
        this._forget();
        const types = global.display.get_selection().get_mimetypes(Meta.SelectionType.SELECTION_DND) ?? [];
        if (!types.includes('text/uri-list') || this._ext.modules?.get('shelf')?.fromShelf?.())
            return;
        this._drag = {cancellable: new Gio.Cancellable(), read: null, over: null};
        this._monitor = {dragMotion: event => this._motion(event)};
        DND.addDragMonitor(this._monitor);
    }

    _forget() {
        if (this._monitor)
            DND.removeDragMonitor(this._monitor);
        this._monitor = null;
        this._drag?.cancellable.cancel();
        this._drag = null;
        this._close();
    }

    _scale() {
        return St.ThemeContext.get_for_stage(global.stage).scale_factor;
    }

    _inside(actor, x, y, margin = 0) {
        if (!actor)
            return false;
        const [ax, ay] = actor.get_transformed_position();
        const [w, h] = actor.get_transformed_size();
        return x >= ax - margin && x < ax + w + margin && y >= ay - margin && y < ay + h + margin;
    }

    _motion(event) {
        const drag = this._drag;
        if (!drag)
            return DND.DragMotionResult.CONTINUE;
        const {x, y} = event;
        const s = this._scale();
        const pill = this._pillActor();
        if (!this._card) {
            if (pill?.mapped) {
                const [px, py] = pill.get_transformed_position();
                const [pw, ph] = pill.get_transformed_size();
                if (x >= px - REACH_X * s && x < px + pw + REACH_X * s && y >= py - REACH_Y * s && y < py + ph + REACH_Y * s)
                    this._open();
            }
        } else if (!this._inside(this._card, x, y, LEAVE * s) && !(pill && this._inside(pill, x, y, LEAVE * s))) {
            this._close();
            drag.read = null;
            // Its tiles went with the card: one jump past the edge must not touch a tile that is gone
            drag.over = null;
        }
        const over = this._tiles?.find(tile => this._inside(tile, x, y)) ?? null;
        if (over !== drag.over) {
            drag.over?.remove_style_pseudo_class('hover');
            over?.add_style_pseudo_class('hover');
            drag.over = over;
        }
        return DND.DragMotionResult.CONTINUE;
    }

    async _end() {
        const drag = this._drag;
        if (this._monitor)
            DND.removeDragMonitor(this._monitor);
        this._monitor = null;
        const released = (global.get_pointer()[2] & Clutter.ModifierType.BUTTON1_MASK) === 0;
        const target = drag && released ? drag.over?.target : null;
        const read = drag?.read;
        this._drag = null;
        this._close();
        if (!target || !read)
            return;
        const files = await read;
        // The module may have been turned off (a lock) while the drag's files were read: then nothing is started
        if (files?.length && !this._off)
            target.run(files);
    }

    // The drag's files, each {uri, path, name}; none for anything that is not a file of this computer
    async _read(cancellable) {
        const drop = await readDrag(cancellable).catch(() => null);
        const files = [];
        for (const uri of drop?.files ?? []) {
            const path = Gio.File.new_for_uri(uri).get_path();
            if (path)
                files.push({uri, path});
        }
        return files;
    }

    // --- The targets ----------------------------------------------------------------------------------------------

    _targets() {
        const modules = this._ext.modules;
        const targets = [];
        const phones = modules?.get('connect')?.sendable ?? [];
        if (phones.length) {
            targets.push({
                id: 'phone', label: _('Send to phone'), icon: 'smartphone-symbolic',
                run: files => {
                    const device = phones.length === 1 ? [`--device=${phones[0].id}`] : [];
                    Util.spawn([PHONE_APP, 'send', ...(phones.length === 1 ? ['--now'] : []), ...device, '--', ...files.map(f => f.path)]);
                },
            });
        }
        const shelf = modules?.get('shelf');
        if (shelf?.on) {
            targets.push({
                id: 'shelf', label: _('Shelf'), icon: 'view-grid-symbolic',
                run: files => shelf.takeFiles(files.map(f => f.uri)),
            });
        }
        if (modules?.get('tools')?.convertOn) {
            targets.push({
                id: 'convert', label: _('Convert'), icon: 'view-refresh-symbolic',
                run: files => this._askConvert(files),
            });
        }
        targets.push({
            id: 'clean', label: _('Copy without hidden data'), icon: 'security-high-symbolic',
            run: files => Util.spawn([CLEAN_COPY, '--', ...files.map(f => f.path)]),
        });
        const agent = modules?.get('agent');
        if (agent?.on) {
            targets.push({
                id: 'agent', label: _('Ask the agent'), icon: 'dialog-question-symbolic',
                run: files => agent.askAbout(files.map(f => f.path).join('\n')),
            });
        }
        return targets;
    }

    _open() {
        const drag = this._drag;
        const targets = this._targets();
        if (!drag || !targets.length)
            return;
        drag.read ??= this._read(drag.cancellable);
        drag.read.then(files => {
            if (this._drag === drag && this._card)
                this._name(files);
        });
        const card = new St.BoxLayout({style_class: 'amethystora-pill-drop', reactive: true, opacity: 0});
        // A drag over it is the shell's: no window under it gets the drop, and the pointer says copy
        card._delegate = {
            handleDragOver: source => source === Main.xdndHandler ? DND.DragMotionResult.COPY_DROP : DND.DragMotionResult.CONTINUE,
        };
        this._tiles = targets.map(target => {
            const tile = new St.BoxLayout({style_class: 'amethystora-pill-drop-tile', orientation: Clutter.Orientation.VERTICAL});
            tile.add_child(new St.Icon({icon_name: target.icon, icon_size: 18, x_align: Clutter.ActorAlign.CENTER}));
            const label = new St.Label({text: target.label, x_align: Clutter.ActorAlign.CENTER});
            label.clutter_text.line_wrap = true;
            label.clutter_text.line_wrap_mode = Pango.WrapMode.WORD_CHAR;
            label.clutter_text.ellipsize = Pango.EllipsizeMode.NONE;
            tile.add_child(label);
            tile.target = target;
            card.add_child(tile);
            return tile;
        });
        Main.layoutManager.addTopChrome(card);
        this._card = card;
        const pill = this._pillActor();
        const [px, py] = pill.get_transformed_position();
        const [pw, ph] = pill.get_transformed_size();
        const [, width] = card.get_preferred_width(-1);
        const monitor = Main.layoutManager.primaryMonitor;
        const x = Math.max(monitor.x, Math.min(px + (pw - width) / 2, monitor.x + monitor.width - width));
        card.set_position(Math.round(x), Math.round(py + ph + CARD_GAP * this._scale()));
        card.ease({opacity: 255, duration: FADE, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
        this._glassChanged();
        this._words = _('Drop it on one');
        this.pill();
        this._label.text = this._words;
        this._activities.set('drop', this);
    }

    _name(files) {
        if (!files.length || !this._label)
            return;
        const named = plain(GLib.path_get_basename(files[0].path), 40);
        this._words = this._capture?.active
            ? ngettext('Drop %d file', 'Drop %d files', files.length).format(files.length)
            : files.length === 1
                ? _('Drop %s').format(named)
                : ngettext('Drop %d file', 'Drop %d files', files.length).format(files.length);
        this._label.text = this._words;
        this._activities.update('drop');
    }

    _close() {
        this._tiles = null;
        const card = this._card;
        this._card = null;
        if (card) {
            card.destroy();
            this._glassChanged();
        }
        if (!this._choice)
            this._endActivity();
    }

    _endActivity() {
        if (this._activities.get('drop') === this)
            this._activities.set('drop', null);
        retire(this._content);
        this._content = null;
        this._label = null;
        this._words = '';
    }

    // --- Convert asks what to --------------------------------------------------------------------------------------

    _askConvert(files) {
        this._endChoice();
        this._choice = files;
        this._words = _('Convert to…');
        this.pill();
        this._label.text = this._words;
        this._activities.set('drop', this);
        this._choiceTimer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, CHOICE_SECONDS, () => {
            this._choiceTimer = 0;
            this._endChoice();
            return GLib.SOURCE_REMOVE;
        });
        this._openCard('drop');
    }

    _endChoice() {
        if (this._choiceTimer)
            GLib.source_remove(this._choiceTimer);
        this._choiceTimer = 0;
        if (this._choice) {
            this._choice = null;
            this._endActivity();
        }
    }

    // --- In the pill and the card ---------------------------------------------------------------------------------

    get text() {
        return this._words ?? '';
    }

    get accessible() {
        return this.text;
    }

    get row() {
        return {icon: 'document-send-symbolic', text: this.text};
    }

    pill() {
        if (!this._content) {
            this._content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-pill-drop-now'});
            this._content.add_child(new St.Icon({icon_name: 'document-send-symbolic', icon_size: 14, y_align: Clutter.ActorAlign.CENTER}));
            this._label = new St.Label({y_align: Clutter.ActorAlign.CENTER});
            this._label.clutter_text.ellipsize = Pango.EllipsizeMode.MIDDLE;
            this._content.add_child(this._label);
        }
        return this._content;
    }

    card(api) {
        const body = new St.BoxLayout({style_class: 'amethystora-pill-card-body', orientation: Clutter.Orientation.VERTICAL});
        if (!this._choice) {
            body.add_child(new St.Label({style_class: 'amethystora-pill-card-text', text: this.text}));
            return body;
        }
        body.add_child(new St.Label({style_class: 'amethystora-pill-card-label', text: _('Convert to')}));
        const paths = this._choice.map(f => f.path);
        const choose = what => () => {
            Util.spawn([CONVERT, what, '--', ...paths]);
            this._endChoice();
            api.close();
        };
        // The words Files' menu has for the same six
        const row = (...pairs) => buttonRow(pairs.map(([what, label]) => cardButton(label, choose(what), {small: true})));
        body.add_child(row(['png', 'PNG'], ['jpeg', 'JPEG'], ['webp', 'WebP']));
        body.add_child(row(['half', _('Half the size')], ['1920', _('1920 px wide')], ['pdf', 'PDF']));
        return body;
    }

    // For the harness: what would be offered
    offered() {
        return this._targets().map(target => target.id);
    }
}
