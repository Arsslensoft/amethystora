// Welcome back: the first session after an update's restart, the pill offers to open again the apps that were open
// before it, in their places (workspaces/resume.js keeps and opens them). "Reopen your apps" stays in the pill for ten
// minutes; Not now, or the ten minutes, forget it. Only the apps' own names and icons are shown, from their launchers.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';

import {buttonRow, cardButton, retire} from './clock.js';
import {plain, timeOfDay} from './text.js';

// Tiles in the card before "+N more"
const TILES = 5;
const NOTES = 'amethystora-notes.desktop';

export class ResumeCard {
    // `resume` gives the workspaces module's part (workspaces/resume.js), or null
    constructor(activities, resume) {
        this.id = 'resume';
        this.plate = '';
        this._activities = activities;
        this._resume = resume;
    }

    enable() {
        const offer = this._resume()?.offer;
        if (!offer)
            return;
        this._offer = offer;
        this.pill();
        this._activities.set('resume', this);
        this._timer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT,
            Math.max(1, Math.ceil((offer.until - Date.now()) / 1000)), () => {
                this._timer = 0;
                this._end();
                return GLib.SOURCE_REMOVE;
            });
    }

    disable() {
        this._end();
    }

    _end() {
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = 0;
        if (this._activities.get('resume') === this)
            this._activities.set('resume', null);
        retire(this._content);
        this._content = null;
    }

    get shown() {
        return this._activities.get('resume') === this;
    }

    get text() {
        return _('Reopen your apps');
    }

    get accessible() {
        return this._offer?.why === 'update' ? _('Reopen the apps that were open before the restart')
            : _('Reopen the apps that were open');
    }

    get row() {
        return {icon: 'view-restore-symbolic', text: this.text};
    }

    pill() {
        if (!this._content) {
            this._content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-pill-resume'});
            this._content.add_child(new St.Icon({icon_name: 'view-restore-symbolic', icon_size: 14,
                y_align: Clutter.ActorAlign.CENTER}));
            this._content.add_child(new St.Label({text: this.text, y_align: Clutter.ActorAlign.CENTER}));
        }
        return this._content;
    }

    reopen() {
        this._resume()?.reopen();
        this._end();
    }

    notNow() {
        this._resume()?.dismiss();
        this._end();
    }

    _summary() {
        const offer = this._offer;
        const apps = offer.apps.length;
        if (offer.why === 'battery') {
            return ngettext('The computer closed itself when the battery ran low. %d app was open.',
                'The computer closed itself when the battery ran low. %d apps were open.', apps).format(apps);
        }
        if (offer.why === 'asleep') {
            return ngettext('The computer went off while it was locked. %d app was open.',
                'The computer went off while it was locked. %d apps were open.', apps).format(apps);
        }
        if (offer.workspaces > 1) {
            return ngettext('The update is in. %d app was open on %d workspaces before the restart.',
                'The update is in. %d apps were open on %d workspaces before the restart.', apps)
                .format(apps, offer.workspaces);
        }
        return ngettext('The update is in. %d app was open before the restart.',
            'The update is in. %d apps were open before the restart.', apps).format(apps);
    }

    _tile(app, workspace) {
        const tile = new St.BoxLayout({style_class: 'amethystora-pill-resume-tile',
            orientation: Clutter.Orientation.VERTICAL, x_align: Clutter.ActorAlign.CENTER});
        tile.add_child(app.create_icon_texture(40));
        const name = new St.Label({style_class: 'amethystora-pill-news-area', text: plain(app.get_name(), 14),
            x_align: Clutter.ActorAlign.CENTER});
        tile.add_child(name);
        tile.add_child(new St.Label({style_class: 'amethystora-pill-resume-space', text: `${workspace + 1}`,
            x_align: Clutter.ActorAlign.CENTER}));
        return tile;
    }

    card(api) {
        const offer = this._offer;
        const body = new St.BoxLayout({style_class: 'amethystora-pill-card-body amethystora-pill-news-card',
            orientation: Clutter.Orientation.VERTICAL});
        body.add_child(new St.Label({
            style_class: 'amethystora-pill-news-versions',
            // Translators: the time the session after the update began
            text: (offer.why === 'update' ? _('Updated · %s') : _('Back · %s')).format(timeOfDay(new Date(offer.at))),
        }));
        body.add_child(new St.Label({style_class: 'amethystora-pill-news-title', text: _('Welcome back')}));
        const summary = new St.Label({style_class: 'amethystora-pill-card-text', text: this._summary()});
        summary.clutter_text.line_wrap = true;
        body.add_child(summary);

        const appSystem = Shell.AppSystem.get_default();
        const tiles = new St.BoxLayout({style_class: 'amethystora-pill-resume-tiles'});
        const found = offer.apps.map(a => ({app: appSystem.lookup_app(a.app), workspace: a.workspace}))
            .filter(a => a.app);
        const shown = found.length > TILES + 1 ? TILES : found.length;
        for (const {app, workspace} of found.slice(0, shown))
            tiles.add_child(this._tile(app, workspace));
        if (found.length > shown) {
            const more = new St.BoxLayout({style_class: 'amethystora-pill-resume-tile',
                orientation: Clutter.Orientation.VERTICAL});
            more.add_child(new St.Label({style_class: 'amethystora-pill-resume-more',
                text: `+${found.length - shown}`, x_align: Clutter.ActorAlign.CENTER}));
            more.add_child(new St.Label({style_class: 'amethystora-pill-news-area', text: _('More'),
                x_align: Clutter.ActorAlign.CENTER}));
            tiles.add_child(more);
        }
        body.add_child(tiles);

        let note = _('Each opens in its place; no file, page or tab was kept.');
        if (offer.apps.some(a => a.app === NOTES))
            note = `${note} ${_('Notes opens locked.')}`;
        const kept = new St.Label({style_class: 'amethystora-pill-news-once', text: note});
        kept.clutter_text.line_wrap = true;
        body.add_child(kept);
        body.add_child(buttonRow([
            cardButton(_('Not now'), () => {
                api.close();
                this.notNow();
            }),
            cardButton(_('Reopen'), () => {
                api.close();
                this.reopen();
            }, {primary: true}),
        ]));
        return body;
    }
}
