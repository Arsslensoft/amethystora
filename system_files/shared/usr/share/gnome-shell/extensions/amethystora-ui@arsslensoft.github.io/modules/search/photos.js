// Your photos, by what is in them (R7-63): offered and off, the key `photos`. After `photo:` (or `photos:`) and a few words,
// the pictures of Pictures that look like what was described: `amethystora-photos search` compares the words with the
// account's index, which `amethystora-photos index` works out on mains power while the computer is left alone and keeps sealed
// in the Vault, so it answers only while the vault is open. One short process for each pause in what is typed, stopped when
// the search moves on; nothing is kept between searches, and no name of a photo is logged. Enter opens the photo.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {output, pause} from './run.js';

const HELPER = '/usr/libexec/amethystora-photos';
// In a scope of its own, so that its model's memory is not GNOME Shell's
const SCOPE = ['systemd-run', '--user', '--scope', '--quiet', '--collect', '-p', 'MemoryMax=400M', '-p', 'CPUWeight=50'];
const HITS = 6;
// The model is loaded for each question
const QUIET = 450;
const ASK = /^\s*photos?\s*:\s*(.+)$/iu;
const CLOSED = 'photos:closed';

export class PhotosProvider {
    constructor() {
        this.id = 'amethystora-photos';
        this.isRemoteProvider = false;
        this.canLaunchSearch = false;
        this._found = new Map();
        this._previous = new Map();
    }

    destroy() {
        this._gone = true;
        this._found = new Map();
        this._previous = new Map();
    }

    // Pictures, as GLib names it; never the home itself
    _pictures() {
        const folder = GLib.get_user_special_dir(GLib.UserDirectory.DIRECTORY_PICTURES);
        return folder && folder !== GLib.get_home_dir() ? folder : null;
    }

    async getInitialResultSet(terms, cancellable) {
        this._previous = this._found;
        this._found = new Map();
        const asked = ASK.exec(terms.join(' '));
        const words = asked ? asked[1].split(/\s+/u).filter(Boolean).slice(0, 20) : [];
        const pictures = this._pictures();
        if (!words.length || !pictures || !await pause(QUIET, cancellable) || this._gone)
            return [];
        const printed = await output([...SCOPE, HELPER, 'search', ...words], cancellable);
        if (cancellable?.is_cancelled() || printed === null || this._gone)
            return [];
        let hits;
        try {
            hits = JSON.parse(printed);
        } catch {
            return [];
        }
        // The vault is closed or not set up: one row that says so, never the photos
        if (hits?.error === 'closed' || hits?.error === 'not-set-up') {
            this._found.set(CLOSED, CLOSED);
            return [CLOSED];
        }
        const ids = [];
        for (const hit of Array.isArray(hits) ? hits.slice(0, HITS) : []) {
            if (typeof hit?.path !== 'string' || !hit.path.startsWith(`${pictures}/`) || hit.path.includes('/../'))
                continue;
            const id = `photo:${hit.path}`;
            this._found.set(id, hit.path);
            ids.push(id);
        }
        return ids;
    }

    getSubsearchResultSet(previous, terms, cancellable) {
        return this.getInitialResultSet(terms, cancellable);
    }

    getResultMetas(ids) {
        const pictures = this._pictures() ?? '';
        return Promise.resolve(ids.map(id => {
            if (id === CLOSED) {
                return {
                    id, name: _('Your photos are sealed in the Vault'), description: _('Open the Vault, then search again'),
                    createIcon: size => new St.Icon({icon_name: 'dialog-password-symbolic', icon_size: size}),
                };
            }
            const path = this._found.get(id) ?? this._previous.get(id);
            if (!path)
                return {id, name: id};
            const name = GLib.path_get_basename(path);
            // Where it is, as a breadcrumb from Pictures: "Pictures › Holiday"
            const folders = GLib.path_get_dirname(path).slice(pictures.length).split('/').filter(Boolean);
            const where = [GLib.path_get_basename(pictures), ...folders].join(' › ');
            const [type] = Gio.content_type_guess(name, null);
            const gicon = Gio.content_type_get_icon(type);
            return {
                id, name, description: _('%s · by what is in it').format(where), descriptionLtr: true,
                createIcon: size => new St.Icon({gicon, icon_size: size}), hint: _('Enter opens'),
            };
        }));
    }

    filterResults(ids, max) {
        return ids.slice(0, Math.min(max, HITS));
    }

    activateResult(id) {
        if (id === CLOSED) {
            Shell.AppSystem.get_default().lookup_app('amethystora-vault.desktop')?.activate();
            return;
        }
        const path = this._found.get(id) ?? this._previous.get(id);
        const pictures = this._pictures();
        if (!path || !pictures || !path.startsWith(`${pictures}/`))
            return;
        // Not on the compositor's thread (a slow mount), and the error is not logged: its text names the file
        Gio.AppInfo.launch_default_for_uri_async(Gio.File.new_for_path(path).get_uri(), global.create_app_launch_context(0, -1), null,
            (_source, result) => {
                try {
                    Gio.AppInfo.launch_default_for_uri_finish(result);
                } catch (e) {
                    console.warn(`Amethystora UI: the photo did not open (error ${e.code ?? 'unknown'})`);
                }
            });
    }
}
