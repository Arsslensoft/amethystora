// Desks (round 4, R4-09; plans/beyond/spec-r4-shelf.md): a laptop lives at several desks. GNOME already remembers how
// the screens were arranged; a desk remembers the rest: the sound output, where the dock is, a Focus profile and the
// power mode, and offers a saved set of windows, always asking. Offered, and off for a new account.
//
// A desk is known by its screens (maker, model and serial number) and Thunderbolt devices (desks/identity.js). The
// first time the computer is at one, a card asks whether to set it up with how things are now; later arrivals set what
// differs at once, with a toast to undo it; leaving puts back what is still as the desk set it. With learning on, a
// change of the sound output or the dock at a known desk asks whether the desk should remember it.
//
// Off (ame desktop desks off, the image's default), the module holds its settings and one handler. On: the monitor
// manager's monitors-changed, a monitor of desks.json, and with learning the mixer's output and the dock's two keys; one
// timeout of 2 s after the screens change, none otherwise.
//
// For the suites and the harness: state(), evaluate(), card, toast.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as MessageTray from 'resource:///org/gnome/shell/ui/messageTray.js';
import * as Volume from 'resource:///org/gnome/shell/ui/status/volume.js';

import {Applier} from './desks/apply.js';
import {makeCard, makeToast} from './desks/card.js';
import {defaultName, hasWords, readDesk} from './desks/identity.js';
import {DesksState, FILE} from './desks/state.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.desks';
// The screens settle for this long before the desk is worked out
const SETTLE = 2000;
// The first-arrival card is answered Not now after this long; the toast goes after this long
const CARD_FOR = 60000;
const TOAST_FOR = 8000;
// What the desk itself changed is not taken for the person's own change for this long
const QUIET = 3000;

const now = () => Math.floor(GLib.get_real_time() / 1e6);

// Kept while GNOME Shell runs, across the disable and enable of every lock and unlock: whether the session has started
// (an enable after that is an unlock, not a log-in), the desk last worked out (an unlock at the same desk is no
// arrival), and the changes the person said Not now to remembering
let sessionStarted = false;
let lastDesk = null;
const notRemembered = new Set();

export class Desks {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._settings = this._ext.getSettings(SCHEMA);
        this._settings.connectObject(
            'changed::desks', () => this._sync(),
            'changed::desks-learn', () => this._syncLearning(), this);
        this._on = false;
        this._sync();
    }

    disable() {
        this._stop({turnedOff: false});
        this._settings?.disconnectObject(this);
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
    }

    _sync() {
        const on = this._settings.get_boolean('desks');
        if (on && !this._on)
            this._start();
        else if (!on && this._on)
            this._stop({turnedOff: true});
    }

    _start() {
        this._on = true;
        // Counted on, never from 0 again: a read still under way from before a stop would match a new start's token
        // and put up a second card over the first, which nothing then takes away
        this._token = (this._token ?? 0) + 1;
        this._quietUntil = 0;
        this._state = new DesksState();
        this._applier = new Applier(this._ext);
        global.backend.get_monitor_manager().connectObject('monitors-changed', () => this._queue('change'), this);
        try {
            this._fileMonitor = Gio.File.new_for_path(FILE).monitor_file(Gio.FileMonitorFlags.NONE, null);
            this._fileMonitor.connectObject('changed', (m, f, o, event) => {
                if (event === Gio.FileMonitorEvent.CHANGES_DONE_HINT || event === Gio.FileMonitorEvent.CREATED ||
                    event === Gio.FileMonitorEvent.DELETED)
                    this._fileChanged();
            }, this);
        } catch {}
        this._syncLearning();
        // After every module has started: the desk the session starts at (set without a toast), or, after an unlock, the
        // desk the computer was brought to while locked (an arrival like any other)
        this._queue(sessionStarted ? 'unlock' : 'start');
        sessionStarted = true;
    }

    _stop({turnedOff}) {
        if (!this._on)
            return;
        this._on = false;
        this._token++;
        global.backend.get_monitor_manager().disconnectObject(this);
        this._fileMonitor?.disconnectObject(this);
        this._fileMonitor?.cancel();
        this._fileMonitor = null;
        this._stopLearning();
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = 0;
        this._closeCard();
        this._closeToast();
        this._question?.destroy();
        this._question = null;
        // Turned off by the person, what the desk set stays as it is and is no longer put back
        if (turnedOff && this._state?.current)
            this._state.current = null;
        this._state = null;
        this._applier = null;
    }

    // ---- The desk the computer is at ----

    // why: start (the session's first), unlock (an enable after it) or change (the screens changed); a start or an
    // unlock waiting is not turned into a change by a screen change meanwhile
    _queue(why) {
        this._why = this._why && why === 'change' ? this._why : why;
        // A screen's own sound output may come or go with it, and the sound server may switch to it: that is not the
        // person changing it at the desk they are leaving
        this._quietUntil = GLib.get_monotonic_time() / 1000 + SETTLE + QUIET;
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, SETTLE, () => {
            this._timer = 0;
            const why = this._why;
            this._why = null;
            this.evaluate({why}).catch(e => console.warn(`Amethystora UI: desks: ${e.message}`));
            return GLib.SOURCE_REMOVE;
        });
    }

    async evaluate({why = 'change'} = {}) {
        if (!this._on)
            return;
        const token = ++this._token;
        // (The harness, whose only screen is virtual, stands in other screens through this field)
        const here = (this._readDesk ?? readDesk)();
        this._here = here;
        // An unlock at the desk the computer was at is no arrival: nothing is asked or set again
        const same = here.id === lastDesk;
        lastDesk = here.id;
        if (why === 'unlock' && same)
            return;
        const current = this._state.current;
        const known = this._state.find(here);
        if (current && current.desk === known?.id) {
            known.seen = now();
            this._state.put(known);
            return;
        }
        // Somewhere else: first what the last desk set goes back
        this._closeCard();
        this._closeToast();
        if (current) {
            this._quiet();
            await this._applier.putBack(current.before, current.wrote);
            if (token !== this._token)
                return;
            this._state.current = null;
        }
        const desk = this._state.find(here);
        if (!desk) {
            // A desk Not now was said to is not asked about again (ame desktop desks ask)
            if (!this._state.declined(here.id))
                await this._ask(here, token);
            return;
        }
        await this._arrive(desk, {toast: why !== 'start', token});
    }

    // A desk seen before: what differs is set, and a toast says so
    async _arrive(desk, {toast, token}) {
        const now_ = await this._applier.read();
        if (token !== this._token)
            return;
        this._quiet();
        // What was set is recorded as it is set, so that leaving puts it back even if the module stopped meanwhile
        const state = this._state;
        const {wrote} = await this._applier.apply(desk.set, now_,
            (before, written) => (state.current = {desk: desk.id, before, wrote: written}));
        if (token !== this._token)
            return;
        desk.seen = now();
        this._state.put(desk);
        // Noun phrases, said after the desk's name: nothing in the sentence agrees with them
        const things = [];
        if (wrote.sound)
            things.push(_('the sound output'));
        if (wrote.dock)
            things.push(_('the dock'));
        if (wrote.focus)
            things.push(_('Focus'));
        if (wrote.power)
            things.push(_('the power mode'));
        if (toast && things.length)
            this._showToast(desk, things);
        if (desk.set.saved)
            this._askSaved(desk);
    }

    // A desk never seen: a card asks whether to set it up with how things are now
    async _ask(here, token) {
        const values = await this._applier.read();
        if (token !== this._token || !this._on)
            return;
        // Focus off is no choice of a profile: a desk set up then leaves Focus as it is, and never turns off one picked
        if (values.focus === 'off')
            delete values.focus;
        const desk = {id: here.id, base: here.base, name: null, seen: now(), builtin: here.builtin, screens: here.screens,
            thunderbolt: here.thunderbolt, set: values, lidShut: here.lidShut};
        const sets = this._ext.modules?.get('workspaces')?.sets?.list ?? [];
        this._closeCard();
        this._card = makeCard({
            title: defaultName(desk),
            // With its name already the number of screens, the line beside it names them
            has: desk.screens.length > 1 && !desk.lidShut ? desk.screens.map(s => s.name || s.product).join(' · ')
                : hasWords(desk),
            rows: this._rows(values),
            buttons: [
                [_('Set up'), true, () => this._setUp(desk)],
                [_('Not now'), false, () => this._notNow(here.id)],
            ],
            note: sets.length ? _('A saved set can be opened here too: choose it in Control.') : null,
        });
        this._cardTimer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, CARD_FOR, () => {
            this._cardTimer = 0;
            this._closeCard();
            return GLib.SOURCE_REMOVE;
        });
        Main.overview.connectObject('showing', () => this._closeCard(), this);
        this._glass();
    }

    // Not now is kept: that desk does not ask again (left unanswered, the card asks at the next arrival)
    _notNow(id) {
        this._closeCard();
        if (this._on)
            this._state.decline(id);
    }

    _setUp(desk) {
        this._closeCard();
        if (!this._on)
            return;
        delete desk.lidShut;
        this._state.put(desk);
        // How things were is how the desk is set: nothing to put back on leaving
        this._state.current = {desk: desk.id, before: {}, wrote: {}};
    }

    _rows(values) {
        return [
            [_('Sound'), values.sound?.label || _('As it is')],
            [_('The dock'), this._dockWords(values.dock)],
            [_('Focus'), this._focusWords(values.focus)],
            [_('Power'), this._powerWords(values.power)],
        ];
    }

    _dockWords(dock) {
        if (!dock)
            return _('As it is');
        const primary = dock.screens === 'primary';
        switch (dock.edge) {
        case 'left':
            return primary ? _('On the main screen, on the left') : _('On the left');
        case 'right':
            return primary ? _('On the main screen, on the right') : _('On the right');
        default:
            return primary ? _('On the main screen, at the bottom') : _('At the bottom');
        }
    }

    _focusWords(focus) {
        return {work: _('Work'), games: _('Games'), sleep: _('Sleep'), off: _('Off')}[focus] ?? _('As it is');
    }

    _powerWords(power) {
        return {performance: _('Performance'), 'power-saver': _('Power saver'), balanced: _('Balanced')}[power] ??
            _('As it is');
    }

    _name(desk) {
        return desk.name || defaultName(desk);
    }

    _showToast(desk, things) {
        this._closeToast();
        const list = new Intl.ListFormat(undefined, {type: 'conjunction'}).format(things);
        // Translators: a desk's name, then the things it set, as a list of noun phrases ("the sound output", "the dock",
        // "Focus", "the power mode"): "Set for Office desk: the sound output and the dock"
        const text = _('Set for %s: %s').format(this._name(desk), list);
        this._toast = makeToast(text, _('Undo'), () => this._undo());
        this._toastTimer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, TOAST_FOR, () => {
            this._toastTimer = 0;
            this._closeToast();
            return GLib.SOURCE_REMOVE;
        });
        this._glass();
    }

    async _undo() {
        this._closeToast();
        const current = this._state?.current;
        if (!current)
            return;
        this._quiet();
        await this._applier.putBack(current.before, current.wrote);
        if (this._on && this._state.current?.desk === current.desk)
            this._state.current = {desk: current.desk, before: {}, wrote: {}};
    }

    _closeCard() {
        if (this._cardTimer)
            GLib.source_remove(this._cardTimer);
        this._cardTimer = 0;
        Main.overview.disconnectObject(this);
        const had = !!this._card;
        this._card?.destroy();
        this._card = null;
        if (had)
            this._glass();
    }

    _closeToast() {
        if (this._toastTimer)
            GLib.source_remove(this._toastTimer);
        this._toastTimer = 0;
        const had = !!this._toast;
        this._toast?.destroy();
        this._toast = null;
        if (had)
            this._glass();
    }

    // A saved set of windows is only ever offered
    _askSaved(desk) {
        const workspaces = this._ext.modules?.get('workspaces');
        const name = desk.set.saved;
        if (!workspaces?.sets?.list?.some?.(set => set.name === name))
            return;
        this._question?.destroy();
        const source = MessageTray.getSystemSource();
        const notification = new MessageTray.Notification({
            source,
            title: _('Open the saved set “%s”?').format(name),
            body: this._name(desk),
            isTransient: true,
        });
        notification.addAction(_('Open'), () => {
            if (workspaces.openSet)
                workspaces.openSet(name);
            else
                workspaces.sets?.open?.(name);
        });
        notification.addAction(_('Not now'), () => {});
        notification.connect('destroy', () => {
            if (this._question === notification)
                this._question = null;
        });
        this._question = notification;
        source.addNotification(notification);
    }

    // ---- desks.json, changed by the recipe ----

    _fileChanged() {
        if (!this._state || this._state.isOwnWrite())
            return;
        this._state.load();
        // A desk forgotten while the computer is at it: nothing more is put back for it
        const current = this._state.current;
        if (current && !this._state.byId(current.desk))
            this._state.current = null;
    }

    // ---- Learning ----

    _syncLearning() {
        this._stopLearning();
        if (!this._on || !this._settings.get_boolean('desks-learn'))
            return;
        this._learning = true;
        // An output that just came (headphones connected) is the system's switch to it, not the person's choice
        Volume.getMixerControl()?.connectObject(
            'active-output-update', () => this._learn('sound'),
            'output-added', () => this._quiet(), this);
        this._ext.modules?.get('dock')?.settings?.connectObject(
            'changed::edge', () => this._learn('dock'),
            'changed::screens', () => this._learn('dock'), this);
    }

    _stopLearning() {
        if (!this._learning)
            return;
        this._learning = false;
        Volume.getMixerControl()?.disconnectObject(this);
        this._ext.modules?.get('dock')?.settings?.disconnectObject(this);
        this._learnQuestion?.destroy();
        this._learnQuestion = null;
    }

    _quiet() {
        this._quietUntil = GLib.get_monotonic_time() / 1000 + QUIET;
    }

    // The person changed the sound output or the dock at a known desk: should the desk remember it?
    _learn(kind) {
        if (!this._on || GLib.get_monotonic_time() / 1000 < this._quietUntil)
            return;
        const current = this._state.current;
        const desk = current ? this._state.byId(current.desk) : null;
        if (!desk)
            return;
        const value = kind === 'sound' ? this._applier.sound() : this._applier.dock();
        if (!value)
            return;
        // A question about this kind asked for a value it no longer has goes: back to what the desk keeps, or to a
        // value Not now was said to
        const withdraw = () => {
            if (this._learnKind === kind)
                this._learnQuestion?.destroy();
        };
        const known = desk.set[kind];
        if (kind === 'sound' && known?.sink === value.sink && known?.port === value.port)
            return withdraw();
        if (kind === 'dock' && known?.edge === value.edge && known?.screens === value.screens)
            return withdraw();
        // Asked once: Not now to this change at this desk is kept while GNOME Shell runs
        const asked = `${desk.id}|${kind}|${JSON.stringify(value)}`;
        if (notRemembered.has(asked))
            return withdraw();
        this._learnQuestion?.destroy();
        const source = MessageTray.getSystemSource();
        const notification = new MessageTray.Notification({
            source,
            title: _('Remember this for %s?').format(this._name(desk)),
            body: kind === 'sound' ? _('Sound: %s').format(value.label) : _('The dock: %s').format(this._dockWords(value)),
            isTransient: true,
        });
        notification.addAction(_('Remember'), () => {
            const again = this._state?.byId(desk.id);
            if (!again)
                return;
            again.set[kind] = value;
            this._state.put(again);
        });
        notification.addAction(_('Not now'), () => notRemembered.add(asked));
        notification.connect('destroy', () => {
            if (this._learnQuestion === notification)
                this._learnQuestion = null;
        });
        this._learnQuestion = notification;
        this._learnKind = kind;
        source.addNotification(notification);
    }

    // ---- The glass, the suites and the harness ----

    glassSurfaces() {
        const surfaces = [];
        if (this._card)
            surfaces.push({actor: this._card, material: 'glass', radius: 22});
        if (this._toast)
            surfaces.push({actor: this._toast, material: 'glass', radius: null});
        return surfaces;
    }

    _glass() {
        this._ext.modules?.get('glass')?.surfacesChanged?.();
    }

    get card() {
        return this._card ?? null;
    }

    get toast() {
        return this._toast ?? null;
    }

    state() {
        return {
            on: this._on,
            timer: !!this._timer,
            learning: !!this._learning,
            fileMonitor: !!this._fileMonitor,
            here: this._here ?? null,
            current: this._state?.current ?? null,
            desks: this._state?.desks.map(d => ({id: d.id, name: d.name, set: d.set})) ?? [],
            card: !!this._card,
            toast: this._toast ? this._toast.accessible_name : null,
            question: !!this._question,
            learnQuestion: !!this._learnQuestion,
        };
    }
}
