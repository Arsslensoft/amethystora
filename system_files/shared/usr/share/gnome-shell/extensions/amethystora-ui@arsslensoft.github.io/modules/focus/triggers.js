// The two triggers, each off until the person turns it on.
//
// Sleep at Night Light's times: Night Light's span as gnome-settings-daemon has it (its sunset and sunrise when the
// schedule follows the sun and it knows them, else the times set for it), whether or not Night Light itself is on. One
// timeout to the next boundary of the span, set again at each boundary, when the times change and after a suspend
// (the timeout's clock stops while the computer sleeps); nothing else runs. While the schedule follows the sun and the
// daemon has not said when it sets yet (just after the module starts, at every unlock), the sun's times it said last
// decide; with none yet in this shell, nothing is decided (the answer the module had when it last stopped stands)
// until they arrive or a few seconds pass.
//
// Games while a game is in front: when a game's window gets the keyboard Games starts, and it ends when that game's
// last window closes, so that a look at the browser keeps it. A game is the windows of an app whose launcher says Game
// among its categories, launchers and stores aside (Steam's own window is not a game), or the windows of one Steam
// game, which Steam starts without a launcher the shell knows and whose windows' class is steam_app_N (Flatpak Steam's
// games are inside Steam's sandbox, so their app is Steam's).
//
// Picking Off by hand while a trigger holds sets it aside until its next start: the next night (the time Off was
// picked, which the module keeps in its off-at key, is read against the night it falls in), or another game (or the
// same one started again). The lock screen stops the module, so what the two triggers knew is kept at module scope,
// which outlives a stop in the same shell.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';

import * as LoginManager from 'resource:///org/gnome/shell/misc/loginManager.js';

import {colorProxy} from './night.js';

// How long the sun's times are waited for before Night Light's own times are used (s)
const SUN_WAIT = 3;
// Launchers and stores whose launcher says Game but whose windows are no game
const LAUNCHERS = new Set(['com.valvesoftware.Steam', 'steam', 'net.lutris.Lutris', 'lutris', 'net.lutris.lutris',
    'com.heroicgameslauncher.hgl', 'heroic', 'com.usebottles.bottles', 'com.vysp3r.ProtonPlus', 'net.davidotek.pupgui2',
    'io.itch.itch', 'itch', 'page.kramo.Cartridges', 'com.github.Matoking.protontricks', 'io.github.hedge_dev.hedgemodmanager',
    'org.prismlauncher.PrismLauncher', 'com.atlauncher.ATLauncher', 'io.github.philipk.boilr']);
const STEAM_GAME = /^steam_app_[0-9]+$/;

// What the triggers knew when the module last stopped: the night's answer, the sun's times (they move by minutes a day,
// so the last ones decide well enough until the daemon says them again), the game followed, and the game set aside
// with the windows it had then (a game started again has others)
let nightWas = null;
let sunWas = null;
// When the Sleep trigger was last turned on by its key in this shell (s): an Off picked before then, while it was off,
// sets nothing aside (off-at says when Off was last picked, whether or not a trigger held then)
let nightOnSince = 0;
let gameNow = null;
let gameAside = null;

// A trigger turned off by its key forgets what it knew
export function forgetNight() {
    nightWas = null;
    nightOnSince = 0;
}

// The Sleep trigger turned on by its key
export function nightTurnedOn() {
    nightOnSince = Math.floor(Date.now() / 1000);
}

export function forgetGame() {
    gameNow = null;
    gameAside = null;
}

// Whether `hour` (decimal) falls between `from` and `to`, a span that may pass midnight
export function between(hour, from, to) {
    if (from === to)
        return false;
    return from < to ? hour >= from && hour < to : hour >= from || hour < to;
}

function hourNow() {
    const now = GLib.DateTime.new_now_local();
    return now.get_hour() + now.get_minute() / 60 + now.get_seconds() / 3600;
}

// A decimal hour today (or `days` from today) as a Date
function at(hour, days = 0) {
    const date = new Date();
    date.setDate(date.getDate() + days);
    date.setHours(Math.floor(hour), Math.round((hour - Math.floor(hour)) * 60) % 60, 0, 0);
    return date;
}

export class NightTrigger {
    // offAt(): when Off was last picked by hand, in seconds since the epoch
    // color: Night Light's settings, the focus module's, which it disposes of
    constructor(changed, offAt, color) {
        this._changed = changed;
        this._offAt = offAt;
        this._colorSettings = color;
        this.inSpan = false;
        this._dismissed = false;
        this._pending = false;
    }

    enable() {
        this._color = this._colorSettings;
        if (!this._color)
            return;
        this._color.connectObject(
            'changed::night-light-schedule-automatic', () => this._update(),
            'changed::night-light-schedule-from', () => this._update(),
            'changed::night-light-schedule-to', () => this._update(),
            this);
        // Following the sun: nothing decided until the daemon says when it sets, or a few seconds pass
        this._pending = this._color.get_boolean('night-light-schedule-automatic') && !sunWas;
        if (this._pending) {
            this._waitId = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, SUN_WAIT, () => {
                this._waitId = 0;
                this._settle();
                return GLib.SOURCE_REMOVE;
            });
        }
        this._sunCancellable = new Gio.Cancellable();
        colorProxy(this._sunCancellable, proxy => {
            this._sun = proxy;
            proxy.connectObject('g-properties-changed', () => this._update(), this);
            // Late, after the wait gave up: the sun's times count from now
            if (this._pending)
                this._settle();
            else
                this._update();
        }, () => this._settle());
        LoginManager.getLoginManager().connectObject('prepare-for-sleep', (manager, sleeping) => {
            if (!sleeping)
                this._update();
        }, this);
        this._update();
    }

    disable() {
        this._unarm();
        if (this._waitId)
            GLib.source_remove(this._waitId);
        this._waitId = 0;
        this._sunCancellable?.cancel();
        this._sunCancellable = null;
        this._sun?.disconnectObject(this);
        this._sun = null;
        this._color?.disconnectObject(this);
        this._color = null;
        LoginManager.getLoginManager().disconnectObject(this);
    }

    get holding() {
        if (this._pending)
            return !!nightWas;
        return this.inSpan && !this._dismissed;
    }

    get armed() {
        return !!this._timer;
    }

    get pending() {
        return this._pending;
    }

    // Off picked by hand: set aside until the span starts again
    dismiss() {
        if (this.inSpan || this._pending)
            this._dismissed = true;
        nightWas = false;
    }

    // Night Light's span, in decimal hours (the sun's times only once they arrived)
    span() {
        const color = this._color;
        if (!color)
            return null;
        if (color.get_boolean('night-light-schedule-automatic')) {
            const sunset = this._sun?.g_name_owner ? this._sun.Sunset : 0;
            const sunrise = this._sun?.g_name_owner ? this._sun.Sunrise : 0;
            if (sunset > 0 && sunrise > 0) {
                sunWas = {from: sunset, to: sunrise};
                return sunWas;
            }
            // Before the daemon says them again: the last ones it said
            if (sunWas && !this._sun)
                return sunWas;
        }
        return {from: color.get_double('night-light-schedule-from'), to: color.get_double('night-light-schedule-to')};
    }

    // When the span ends, as a Date (for the tile's words)
    get ends() {
        const span = this.span();
        if (!span)
            return null;
        const end = at(span.to);
        if (end <= new Date())
            end.setDate(end.getDate() + 1);
        return end;
    }

    _settle() {
        if (this._waitId)
            GLib.source_remove(this._waitId);
        this._waitId = 0;
        if (!this._pending)
            return;
        const was = this.holding;
        this._pending = false;
        this._update(was);
    }

    _update(was = this.holding) {
        if (this._pending || !this._color)
            return;
        const span = this.span();
        const hour = hourNow();
        this.inSpan = !!span && between(hour, span.from, span.to);
        if (!this.inSpan) {
            this._dismissed = false;
        } else {
            // Off picked during this night (before a lock, a log-out, a restart or a suspend) keeps it aside; Off
            // picked in an earlier night does not. Every Off writes off-at.
            let start = at(span.from);
            if (start > new Date())
                start = at(span.from, -1);
            this._dismissed = this._offAt() >= Math.max(Math.floor(start.getTime() / 1000), nightOnSince);
        }
        nightWas = this.holding;
        this._arm(span, hour);
        if (was !== this.holding)
            this._changed();
    }

    _unarm() {
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = 0;
    }

    // One timeout to the next boundary of the span
    _arm(span, hour) {
        this._unarm();
        if (!span || span.from === span.to)
            return;
        const ahead = t => {
            const d = ((t - hour) % 24 + 24) % 24;
            return d < 1 / 3600 ? 24 : d;
        };
        const seconds = Math.ceil(Math.min(ahead(span.from), ahead(span.to)) * 3600) + 1;
        this._timer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, seconds, () => {
            this._timer = 0;
            this._update();
            return GLib.SOURCE_REMOVE;
        });
    }
}

export class GameTrigger {
    constructor(changed) {
        this._changed = changed;
        this._key = null;
        this._watched = new Set();
    }

    enable() {
        this._categories = new Map();
        global.display.connectObject(
            'notify::focus-window', () => this._focused(),
            'window-created', (display, window) => {
                if (this._key && this.keyOf(window) === this._key)
                    this._watch(window);
            },
            this);
        // The game followed before the module stopped (the lock screen), while it still has a window
        if (gameNow && this._windowsOf(gameNow).length > 0)
            this._follow(gameNow);
        else
            gameNow = null;
        this._focused();
    }

    disable() {
        global.display.disconnectObject(this);
        for (const window of this._watched)
            window.disconnectObject(this);
        this._watched.clear();
        this._key = null;
        this._categories = null;
    }

    get holding() {
        return !!this._key && !this._aside(this._key);
    }

    // Whether this game was set aside: the same game, with one of the windows it had then (started again, it has
    // others, and is no longer aside)
    _aside(key) {
        if (!gameAside || gameAside.key !== key)
            return false;
        const seqs = new Set(global.get_window_actors().map(actor => actor.meta_window.get_stable_sequence()));
        if (gameAside.seqs.some(seq => seqs.has(seq)))
            return true;
        gameAside = null;
        return false;
    }

    // What the trigger follows: app:ID or class:steam_app_N
    get app() {
        return this._key;
    }

    dismiss() {
        if (this._key) {
            gameAside = {key: this._key, seqs: this._windowsOf(this._key).map(w => w.get_stable_sequence())};
            if (!gameAside.seqs.length)
                gameAside = null;
        }
    }

    // The game a window belongs to, or null: a Steam game by its window's class (outside a sandbox, or in Steam's own),
    // else its app when the app's launcher says Game and it is no launcher or store
    keyOf(window) {
        const wmClass = window.get_wm_class() ?? '';
        if (STEAM_GAME.test(wmClass)) {
            const sandbox = window.get_sandboxed_app_id?.() ?? null;
            return !sandbox || sandbox === 'com.valvesoftware.Steam' ? `class:${wmClass}` : null;
        }
        const app = Shell.WindowTracker.get_default().get_window_app(window);
        const id = app?.get_id();
        if (!id || LAUNCHERS.has(id.replace(/\.desktop$/, '')))
            return null;
        if (!this._categories.has(id)) {
            const categories = app.get_app_info()?.get_categories() ?? '';
            this._categories.set(id, categories.split(';').includes('Game'));
        }
        return this._categories.get(id) ? `app:${id}` : null;
    }

    _windowsOf(key) {
        return global.get_window_actors().map(actor => actor.meta_window).filter(w => this.keyOf(w) === key);
    }

    // A game's window has the keyboard: Games starts, unless a game holds it already or this one was set aside
    _focused() {
        const window = global.display.focus_window;
        const key = window ? this.keyOf(window) : null;
        if (!key)
            return;
        if (key === this._key) {
            this._watch(window);
            return;
        }
        if (this.holding)
            return;
        this._follow(key);
        if (this.holding)
            this._changed();
    }

    _follow(key) {
        for (const window of this._watched)
            window.disconnectObject(this);
        this._watched.clear();
        this._key = key;
        gameNow = key;
        for (const window of this._windowsOf(key))
            this._watch(window);
    }

    _watch(window) {
        if (this._watched.has(window))
            return;
        this._watched.add(window);
        window.connectObject('unmanaged', () => {
            window.disconnectObject(this);
            this._watched.delete(window);
            if (this._watched.size === 0)
                this._ended();
        }, this);
    }

    // The game's last window closed
    _ended() {
        const was = this.holding;
        if (gameAside?.key === this._key)
            gameAside = null;
        this._key = null;
        gameNow = null;
        if (was)
            this._changed();
        // Another game may already have the keyboard
        this._focused();
    }
}
