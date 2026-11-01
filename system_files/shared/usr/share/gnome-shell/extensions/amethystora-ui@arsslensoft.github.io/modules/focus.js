// Focus profiles: Work, Games and Sleep. A profile lays itself over the person's settings while it is on, and when it
// ends everything is as they left it. It is picked by hand (the Focus tile in quick settings, ame desktop focus,
// Control) and ends by hand, after an hour or tomorrow; two triggers, each off until turned on, start Sleep at Night
// Light's times and Games while a game is in front. One profile at a time: the one picked by hand, else Games, else
// Sleep.
//
// What a profile changes, and how (modules/focus/*.js and plans/beyond/spec-focus.md):
//   banners and their sounds   the pill's hold, in memory; GNOME's Do Not Disturb is never written
//   the dock                    its touch autohide laid over the dock's key, in memory
//   glass and motion            comfort's "lighter", as power saver makes them, in memory
//   Night Light                 paused through gnome-settings-daemon's own pause, or warmed: its two keys written, what
//                               they were kept in ~/.local/state/amethystora/focus.json and put back at the end, at
//                               the next start after a crash, and at disable (but at the lock screen)
//   the power mode              a hold with the power daemon, let go at the end and whenever the shell stops
//   the accent                  the theme module's class on the shell's top group, in memory
//   the wallpaper               Daylight's night picture over GNOME's, in memory
//   grey                        a grey matrix laid over the colour filter's pass (access.setExtraMatrix), in memory;
//                               a desaturate effect over the shell where the access module is not running
// Switching from one profile to another changes only what differs.
//
// Off, with both triggers off, nothing runs: one handler on the module's own keys and the tile's actors in quick
// settings, nothing else.
//
// For the suites and the harness: state(), pick(id), pickEnd(kind), tile, mark.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import St from 'gi://St';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as LoginManager from 'resource:///org/gnome/shell/misc/loginManager.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as MessageTray from 'resource:///org/gnome/shell/ui/messageTray.js';

import {filterMatrix} from './access/matrices.js';
import {FocusMark} from './focus/mark.js';
import {NightLight, colorSettings} from './focus/night.js';
import {NightPicture} from './focus/picture.js';
import {PowerHold} from './focus/power.js';
import {CHOICES, OFF, PROFILES, SCHEMA, profileLines, profileName, readProfile} from './focus/profiles.js';
import {FocusState} from './focus/state.js';
import {FocusTile} from './focus/tile.js';
import {GameTrigger, NightTrigger, forgetGame, forgetNight, nightTurnedOn} from './focus/triggers.js';
import {timeOfDay} from './pill/text.js';

const HOUR = 3600;
const DAY = 24 * HOUR;
// "Tomorrow" ends at this hour of the coming morning
const MORNING = 6;
const GREY = 'amethystora-focus-grey';

function now() {
    return Math.floor(GLib.get_real_time() / 1e6);
}

// The next 06:00 after now, in seconds since the epoch
function nextMorning() {
    const date = new Date();
    const morning = new Date(date);
    morning.setHours(MORNING, 0, 0, 0);
    if (morning <= date)
        morning.setDate(morning.getDate() + 1);
    return Math.floor(morning.getTime() / 1000);
}

export class Focus {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._settings = this._ext.getSettings(SCHEMA);
        this._state = new FocusState();
        // The module's settings objects, each made once here and disposed of in disable()
        this._color = colorSettings();
        this._night = new NightLight(this._state, this._color);
        this._power = new PowerHold();
        this._picture = new NightPicture(this._ext, () => this._reduce());
        this._mark = new FocusMark(this);
        this._current = {id: 'off', origin: null};
        this._applied = OFF;
        this._grey = null;
        this._endChoice = null;

        this._tile = new FocusTile(this);
        this._module('quick')?.setFocusBlock?.(this._tile.block);
        Main.panel.statusArea.quickSettings?.menu.connectObject('open-state-changed', (menu, open) => {
            if (open)
                this._refreshTile();
        }, this);

        this._settings.connectObject('changed', (settings, key) => this._changed(key), this);

        // A profile whose end passed while the shell was not running is over
        this._expire();
        this._syncTriggers();
        // What a run before this one left written (a crash, a log-out, the lock screen) is taken over or put back
        const effective = this._effective();
        this._night.start(effective.id === 'off' ? 'as-set' : readProfile(this._settings, effective.id).night);
        this._armEnd();
        this._sync();
    }

    disable() {
        // At the lock screen GNOME turns the extension off and on again at unlock: a warmed or paused Night Light
        // stays as it is behind the lock while the profile is still on, and the next start takes it over
        const locked = Main.sessionMode.isLocked;
        this._settings?.disconnectObject(this);
        Main.panel.statusArea.quickSettings?.menu.disconnectObject(this);
        if (this._idle)
            GLib.source_remove(this._idle);
        this._idle = 0;
        this._unarmEnd();
        this._games?.disable();
        this._games = null;
        this._nightTrigger?.disable();
        this._nightTrigger = null;
        this._question?.destroy();
        this._question = null;

        // At the lock screen the pill stops right after this module, taking its hold and the mark with it: nothing is
        // said into a pill that is going away
        const pill = this._module('pill');
        if (!locked) {
            pill?.setFocusMark?.(null);
            pill?.setFocus?.(null);
        }
        this._module('dock')?.setAutohideOverride?.(null);
        this._module('comfort')?.setLighter?.(false);
        this._module('theme')?.setFocusAccent?.(null);
        this._setGrey(false);
        this._picture?.stop();
        this._picture = null;
        this._power?.stop();
        this._power = null;
        this._night?.stop(locked && this._applied.night !== 'as-set');
        this._night = null;
        this._mark?.destroy();
        this._mark = null;

        this._module('quick')?.setFocusBlock?.(null);
        this._tile?.destroy();
        this._tile = null;
        this._applied = OFF;
        this._current = null;
        this._state = null;
        // Disposed here, on the main thread, rather than left to the collector (a GJS/GLib deadlock against dconf)
        this._color?.run_dispose();
        this._color = null;
        this._settings?.run_dispose();
        this._settings = null;
    }

    _module(name) {
        return this._ext.modules?.get(name) ?? null;
    }

    _reduce() {
        const comfort = this._module('comfort');
        return comfort ? comfort.reduceMotion : !St.Settings.get().enable_animations;
    }

    // ---- What is on ----

    get _hand() {
        const id = this._settings.get_string('profile');
        return PROFILES.includes(id) ? id : 'off';
    }

    _effective() {
        const hand = this._hand;
        if (hand !== 'off')
            return {id: hand, origin: 'hand'};
        if (this._games?.holding)
            return {id: 'games', origin: 'game'};
        if (this._nightTrigger?.holding)
            return {id: 'sleep', origin: 'night'};
        return {id: 'off', origin: null};
    }

    _changed(key) {
        switch (key) {
        case 'off-at':
            // Off picked by hand: a trigger that holds now waits for its next start
            this._games?.dismiss();
            this._nightTrigger?.dismiss();
            break;
        case 'profile': {
            const id = this._hand;
            if (id !== 'off')
                this._askSet(id);
            this._armEnd();
            break;
        }
        case 'ends':
            this._armEnd();
            break;
        case 'trigger-sleep':
            // Turned on now: an Off picked while it was off sets nothing aside
            if (this._settings.get_boolean('trigger-sleep') && !this._nightTrigger)
                nightTurnedOn();
            this._syncTriggers();
            break;
        case 'trigger-games':
            this._syncTriggers();
            break;
        }
        this._later();
    }

    // Several keys written together (ame writes off-at, ends and profile in a row) make one change
    _later() {
        if (this._idle)
            return;
        this._idle = GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
            this._idle = 0;
            this._sync();
            return GLib.SOURCE_REMOVE;
        });
    }

    _syncTriggers() {
        const games = this._settings.get_boolean('trigger-games');
        if (games && !this._games) {
            this._games = new GameTrigger(() => this._later());
            this._games.enable();
        } else if (!games && this._games) {
            this._games.disable();
            this._games = null;
            forgetGame();
        }
        const night = this._settings.get_boolean('trigger-sleep');
        if (night && !this._nightTrigger) {
            this._nightTrigger = new NightTrigger(() => this._later(), () => this._settings?.get_int64('off-at') ?? 0,
                this._color);
            this._nightTrigger.enable();
        } else if (!night && this._nightTrigger) {
            this._nightTrigger.disable();
            this._nightTrigger = null;
            forgetNight();
        }
    }

    // ---- Applying a profile ----

    _sync() {
        if (!this._settings)
            return;
        const effective = this._effective();
        const profile = effective.id === 'off' ? OFF : readProfile(this._settings, effective.id);
        this._current = effective;
        this._apply(profile);
        this._refreshTile();
        this._refreshMark();
    }

    // Only what differs from what is applied now changes; a new power hold is taken before the old one goes
    _apply(profile) {
        const on = profile.id !== 'off';
        const pill = this._module('pill');
        pill?.setFocus?.(on ? {banners: profile.banners, apps: profile.apps, sounds: profile.sounds} : null);
        this._module('dock')?.setAutohideOverride?.(profile.dock === 'touch' ? 'touch' : null);
        this._module('comfort')?.setLighter?.(profile.lighter);
        this._night.set(profile.night);
        this._power.set(profile.power, on ? _('Focus: %s').format(profileName(profile.id)) : '');
        this._module('theme')?.setFocusAccent?.(profile.accent === 'theme' ? null : profile.accent);
        this._picture.set(profile.wallpaper === 'night');
        this._setGrey(profile.grey);
        this._applied = profile;
    }

    _setGrey(on) {
        // Through the colour filter's one effect on the stage, so that the two never stack nor fight
        const access = this._module('access');
        if (access?.setExtraMatrix && this._grey !== 'desaturate') {
            access.setExtraMatrix(GREY, on ? filterMatrix('grey', 1) : null);
            this._grey = on ? 'access' : null;
            return;
        }
        const group = Main.layoutManager.uiGroup;
        if (on && !this._grey) {
            this._grey = 'desaturate';
            this._desaturate = new Clutter.DesaturateEffect({factor: 1});
            group.add_effect_with_name(GREY, this._desaturate);
        } else if (!on && this._grey) {
            group.remove_effect(this._desaturate);
            this._desaturate = null;
            this._grey = null;
        }
    }

    // ---- The end ----

    _expire() {
        const ends = this._settings.get_int64('ends');
        if (this._hand !== 'off' && ends > 0 && now() >= ends)
            this._write({ends: 0, profile: 'off'});
    }

    _unarmEnd() {
        if (this._endTimer)
            GLib.source_remove(this._endTimer);
        this._endTimer = 0;
        LoginManager.getLoginManager().disconnectObject(this);
    }

    // One timeout to the end of a profile picked by hand, set again after a suspend
    _armEnd() {
        this._unarmEnd();
        const ends = this._settings.get_int64('ends');
        if (this._hand === 'off' || ends <= 0)
            return;
        const left = ends - now();
        if (left <= 0) {
            this._expire();
            return;
        }
        // At most a day at a time: an end written by hand far ahead is set again each day until it comes
        this._endTimer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, Math.min(left, DAY), () => {
            this._endTimer = 0;
            this._expire();
            this._armEnd();
            return GLib.SOURCE_REMOVE;
        });
        LoginManager.getLoginManager().connectObject('prepare-for-sleep', (manager, sleeping) => {
            if (!sleeping)
                this._armEnd();
        }, this);
    }

    _endFor(kind) {
        if (kind === 'hour')
            return now() + HOUR;
        if (kind === 'tomorrow')
            return nextMorning();
        return 0;
    }

    // The end shown on the tile: while a profile picked by hand is on, what the key says ("tomorrow" ends on the dot at
    // six; an hour from a moment almost never does); while Off, the one picked here for the next; none while a
    // trigger holds the profile, which ends by itself
    _endKind() {
        if (this._current?.origin === 'game' || this._current?.origin === 'night')
            return null;
        if (this._hand === 'off')
            return this._endChoice ?? 'off';
        const ends = this._settings.get_int64('ends');
        if (ends <= 0)
            return 'off';
        const end = new Date(ends * 1000);
        return end.getHours() === MORNING && end.getMinutes() === 0 && end.getSeconds() === 0 ? 'tomorrow' : 'hour';
    }

    // ---- What the person does ----

    _write(values) {
        this._settings.delay();
        for (const [key, value] of Object.entries(values)) {
            if (key === 'profile')
                this._settings.set_string(key, value);
            else
                this._settings.set_int64(key, value);
        }
        this._settings.apply();
    }

    // A chip, the pill's Turn off, or the suites: a profile picked by hand
    pick(id) {
        if (!CHOICES.includes(id) || !this._settings)
            return;
        if (id === 'off') {
            this._games?.dismiss();
            this._nightTrigger?.dismiss();
            this._write({'off-at': now(), ends: 0, profile: 'off'});
        } else {
            this._write({ends: this._endFor(this._endChoice ?? 'off'), profile: id});
        }
    }

    // An end picked on the tile: for the profile on now, or for the next one picked
    pickEnd(kind) {
        if (!['off', 'hour', 'tomorrow'].includes(kind) || !this._settings)
            return;
        this._endChoice = kind;
        if (this._hand !== 'off')
            this._write({ends: this._endFor(kind)});
        this._refreshTile();
    }

    // ---- What the person sees ----

    // The words at the end of the tile's first row, and in the pill's card
    _stateText() {
        const {id, origin} = this._current ?? {id: 'off'};
        if (id === 'off')
            return _('Off');
        if (origin === 'game')
            return _('On while a game is in front');
        let ends = 0;
        if (origin === 'night')
            ends = Math.floor((this._nightTrigger?.ends?.getTime() ?? 0) / 1000);
        else
            ends = this._settings.get_int64('ends');
        if (ends <= 0)
            return _('On until you turn it off');
        const end = new Date(ends * 1000);
        const today = new Date();
        if (end.toDateString() === today.toDateString() || (origin === 'night' && ends - now() < 24 * HOUR))
            return _('On until %s').format(timeOfDay(end));
        return _('On until tomorrow');
    }

    _refreshTile() {
        if (!this._tile || !this._current)
            return;
        this._tile.refresh({
            id: this._current.id,
            state: this._stateText(),
            lines: profileLines(this._applied),
            end: this._endKind(),
        });
    }

    _refreshMark() {
        const pill = this._module('pill');
        if (this._current.id === 'off') {
            pill?.setFocusMark?.(null);
            this._mark.destroy();
            return;
        }
        this._mark.show(this._current.id, this._stateText(), profileLines(this._applied));
        pill?.setFocusMark?.(this._mark);
        this.markChanged();
    }

    // The mark's words changed (the clock's minute, another profile): the pill fits them
    markChanged() {
        this._module('pill')?.focusMarkChanged?.();
    }

    get held() {
        return this._module('pill')?.hold?.focusHeld ?? 0;
    }

    // ---- A saved workspace set, asked first ----

    // Only for a profile picked by hand, only a set that exists, never at a start or an unlock
    _askSet(id) {
        const name = this._settings.get_string(`${id}-set`);
        const workspaces = this._module('workspaces');
        const set = name ? workspaces?.sets?.list?.find?.(s => s.name === name) : null;
        if (!set)
            return;
        const count = new Set((set.windows ?? []).map(w => w.app)).size;
        this._question?.destroy();
        const source = MessageTray.getSystemSource();
        const notification = new MessageTray.Notification({
            source,
            title: ngettext('Open %s’s %d app?', 'Open %s’s %d apps?', count).format(profileName(id), count),
            body: name,
            isTransient: true,
            // Feedback to what the person just did: the profile's own hold lets it through
            forFeedback: true,
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
        // Picked in a menu (quick settings' tile): GNOME shows no banner while a menu is open, and a transient one
        // would be gone unseen; it is asked once the menu has closed
        const menu = Main.panel.menuManager.activeMenu;
        if (!menu?.isOpen) {
            source.addNotification(notification);
            return;
        }
        const closed = menu.connect('open-state-changed', (m, open) => {
            if (open)
                return;
            menu.disconnect(closed);
            if (this._question === notification)
                source.addNotification(notification);
        });
    }

    // ---- For the suites and the harness ----

    get tile() {
        return this._tile;
    }

    get mark() {
        return this._mark;
    }

    state() {
        const applied = this._applied;
        return {
            profile: this._current?.id ?? 'off',
            origin: this._current?.origin ?? null,
            hand: this._settings ? this._hand : 'off',
            applied: {...applied},
            timers: {end: !!this._endTimer, night: this._nightTrigger?.armed ?? false},
            triggers: {
                games: this._games ? {holding: this._games.holding, game: this._games.app} : null,
                sleep: this._nightTrigger ? {holding: this._nightTrigger.holding, inSpan: this._nightTrigger.inSpan,
                    pending: this._nightTrigger.pending} : null,
            },
            power: {held: this._power?.held ?? null, connected: this._power?.connected ?? false},
            night: this._night?.value ?? null,
            picture: this._picture?.shown ?? 0,
            grey: !!this._grey,
            greyBy: this._grey,
            held: this.held,
            question: !!this._question,
        };
    }
}
