// What's new: the first session after an update, the pill reads "Updated · what's new" for a minute, and a click shows
// the first three things the update brought and opens the Manual's page for it. The words are made when the image is
// built (build_files/shared/build-whats-new.py writes /usr/share/amethystora/whats-new.json and the Manual's
// whats-new.md): the notes of the stream's last few releases, of which the card shows those newer than the version the
// account saw, so that skipping versions or updating to a rebuild loses nothing. Updates with nothing for people to
// notice say nothing.
//
// Said once per update: the version the account saw last is kept in ~/.local/state/amethystora/whats-new.json (written
// whole, with a copy beside it), compared when the live pill starts (at login and at every unlock), and recorded as
// soon as the pill says it, with the key off too (so that turning it on does not announce an old update). An account
// that never saw a version (a new one) records it and says nothing; an older version than the one seen (a rollback)
// says nothing and is not recorded.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Util from 'resource:///org/gnome/shell/misc/util.js';

import {buttonRow, cardButton, retire} from './clock.js';
import {plain} from './text.js';

const INFO = '/usr/share/amethystora/whats-new.json';
const MANUAL = '/usr/bin/amethystora-manual';
const STATE_DIR = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora']);
const STATE = GLib.build_filenamev([STATE_DIR, 'whats-new.json']);
// Seconds the pill says it, the most items kept, the ones in the card
const SHOWN_FOR = 60;
const MOST = 12;
const IN_CARD = 3;
const VERSION = /^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/;
const ICONS = {
    desktop: 'video-display-symbolic',
    security: 'security-high-symbolic',
    control: 'preferences-system-symbolic',
    apps: 'view-app-grid-symbolic',
    updates: 'software-update-available-symbolic',
};

const decoder = new TextDecoder();

function readJson(path) {
    try {
        const [ok, bytes] = GLib.file_get_contents(path);
        return ok ? JSON.parse(decoder.decode(bytes)) : null;
    } catch {
        return null;
    }
}

// What the image says the stream's last few releases brought: {version, releases: [{version, items: [{title, area}]}]},
// the newest first, or null
export function readInfo(path = INFO) {
    const info = readJson(path);
    if (!info || typeof info.version !== 'string' || !VERSION.test(info.version))
        return null;
    const releases = (Array.isArray(info.releases) ? info.releases : [])
        .filter(release => typeof release?.version === 'string' && VERSION.test(release.version))
        .map(release => ({
            version: release.version,
            items: (Array.isArray(release.items) ? release.items : [])
                .filter(item => typeof item?.title === 'string' && plain(item.title, 90) !== '')
                .map(item => ({title: plain(item.title, 90), area: ICONS[item.area] ? item.area : 'system'})),
        }));
    return {version: info.version, releases};
}

// A version's place in its stream's order: [date, build of that day] from "44.20261104" or "latest-44.20261104.2",
// or null for anything else
export function versionKey(version) {
    const match = /(\d{8})(?:\.(\d+))?$/.exec(version ?? '');
    return match ? [Number(match[1]), Number(match[2] ?? 0)] : null;
}

function newer(a, b) {
    return a[0] !== b[0] ? a[0] > b[0] : a[1] > b[1];
}

// What the releases newer than `seen` brought, newest first, each title once, at most MOST; when `seen` cannot be
// placed, the newest release's alone
export function itemsSince(releases, seen) {
    const from = versionKey(seen);
    const chosen = from
        ? releases.filter(release => versionKey(release.version) && newer(versionKey(release.version), from))
        : releases.slice(0, 1);
    const titles = new Set();
    return chosen.flatMap(release => release.items)
        .filter(item => !titles.has(item.title) && titles.add(item.title))
        .slice(0, MOST);
}

// The version this account saw last, from the file or its copy; undefined when it never saw one
function readSeen() {
    for (const path of [STATE, `${STATE}.bak`]) {
        const state = readJson(path);
        if (state && typeof state.seen === 'string' && VERSION.test(state.seen))
            return state.seen;
    }
    return undefined;
}

export function writeSeen(version) {
    const text = `${JSON.stringify({version: 1, seen: version})}\n`;
    try {
        GLib.mkdir_with_parents(STATE_DIR, 0o700);
        // The copy first: a crash between the two leaves one whole file
        GLib.file_set_contents(`${STATE}.bak`, text);
        GLib.file_set_contents(STATE, text);
    } catch (e) {
        console.warn(`Amethystora UI: what's new could not record the version seen: ${e.message}`);
    }
}

// The words under an item: where in the system it is
function areaText(area) {
    switch (area) {
    case 'desktop':
        return _('On the desktop');
    case 'security':
        return _('In Security');
    case 'control':
        return _('In Control');
    case 'apps':
        return _('In the apps');
    case 'updates':
        return _('In Updates');
    default:
        return _('Across the system');
    }
}

export class News {
    constructor(activities, settings, info = null) {
        this.id = 'news';
        this.plate = 'news';
        this._activities = activities;
        this._settings = settings;
        this._info = info;
    }

    enable() {
        this._settings.connectObject('changed::whats-new', () => {
            if (!this._settings.get_boolean('whats-new'))
                this._end();
        }, this);
        // Read and recorded with the key off too, so that turning it on later does not announce an old update
        const info = this._info ?? readInfo();
        if (!info)
            return;
        const seen = readSeen();
        if (seen === info.version)
            return;
        // Started from an older deployment (a rollback): nothing said, and the newer version stays the one seen
        const now = versionKey(info.version);
        const before = versionKey(seen);
        if (now && before && newer(before, now))
            return;
        writeSeen(info.version);
        // A new account, the key off, or updates with nothing to tell: recorded, and nothing said
        const items = seen === undefined ? [] : itemsSince(info.releases ?? [], seen);
        if (!this._settings.get_boolean('whats-new') || items.length === 0)
            return;
        this._news = {version: info.version, from: seen, items, at: new Date()};
        this.pill();
        this._activities.set('news', this);
        this._timer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, SHOWN_FOR, () => {
            this._timer = 0;
            this._end();
            return GLib.SOURCE_REMOVE;
        });
    }

    disable() {
        this._settings.disconnectObject(this);
        this._end();
    }

    _end() {
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = 0;
        if (this._activities.get('news') === this)
            this._activities.set('news', null);
        retire(this._content);
        this._content = null;
    }

    get shown() {
        return this._activities.get('news') === this;
    }

    get news() {
        return this._news ?? null;
    }

    get text() {
        return _('Updated · what’s new');
    }

    get accessible() {
        return _('Updated: what’s new');
    }

    get row() {
        return {icon: 'software-update-available-symbolic', text: _('Updated')};
    }

    pill() {
        if (!this._content) {
            this._content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-pill-news'});
            this._content.add_child(new St.Label({text: this.text, y_align: Clutter.ActorAlign.CENTER}));
        }
        return this._content;
    }

    // "Updated this morning", by the hour the session saw it
    _title() {
        const hour = this._news.at.getHours();
        if (hour >= 5 && hour < 12)
            return _('Updated this morning');
        if (hour >= 12 && hour < 18)
            return _('Updated this afternoon');
        if (hour >= 18 && hour < 23)
            return _('Updated this evening');
        return _('Updated tonight');
    }

    readAll() {
        this._end();
        try {
            Util.spawn([MANUAL, 'whats-new']);
        } catch (e) {
            console.warn(`Amethystora UI: the Manual did not open: ${e.message}`);
        }
    }

    card(api) {
        const news = this._news;
        const body = new St.BoxLayout({style_class: 'amethystora-pill-card-body amethystora-pill-news-card',
            orientation: Clutter.Orientation.VERTICAL});
        const head = new St.BoxLayout({style_class: 'amethystora-pill-news-head'});
        head.add_child(new St.Icon({
            gicon: new Gio.ThemedIcon({names: ['amethystora-logo', 'software-update-available-symbolic']}),
            icon_size: 30,
            style_class: 'amethystora-pill-news-logo',
            y_align: Clutter.ActorAlign.CENTER,
        }));
        const words = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, y_align: Clutter.ActorAlign.CENTER});
        words.add_child(new St.Label({style_class: 'amethystora-pill-news-title', text: this._title()}));
        words.add_child(new St.Label({
            style_class: 'amethystora-pill-news-versions',
            // Translators: the version updated to, and the one before it
            text: news.from ? _('%s · from %s').format(news.version, news.from) : news.version,
        }));
        head.add_child(words);
        body.add_child(head);
        for (const item of news.items.slice(0, IN_CARD)) {
            const row = new St.BoxLayout({style_class: 'amethystora-pill-news-row'});
            const plate = new St.Bin({style_class: 'amethystora-pill-news-icon', y_align: Clutter.ActorAlign.START});
            plate.set_child(new St.Icon({icon_name: ICONS[item.area] ?? 'starred-symbolic', icon_size: 18}));
            row.add_child(plate);
            const text = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_expand: true});
            const title = new St.Label({style_class: 'amethystora-pill-news-item', text: item.title});
            title.clutter_text.line_wrap = true;
            text.add_child(title);
            text.add_child(new St.Label({style_class: 'amethystora-pill-news-area', text: areaText(item.area)}));
            row.add_child(text);
            body.add_child(row);
        }
        const foot = new St.BoxLayout({style_class: 'amethystora-pill-news-foot'});
        foot.add_child(buttonRow([
            cardButton(_('Read it all'), () => {
                api.close();
                this.readAll();
            }, {primary: true}),
            cardButton(_('Not now'), () => {
                api.close();
                this._end();
            }),
        ]));
        foot.add_child(new St.Label({
            style_class: 'amethystora-pill-news-once',
            text: _('Said once per update'),
            x_expand: true,
            x_align: Clutter.ActorAlign.END,
            y_align: Clutter.ActorAlign.CENTER,
        }));
        body.add_child(foot);
        return body;
    }
}
