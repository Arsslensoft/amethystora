// The agent row: the last row of the search, "Ask the agent: …", while the agentic features are on for this account
// and an administrator allows them. Enter opens the chosen agent in a terminal with what was typed as its first
// question (amethystora-agent ask -- TEXT); nothing starts before Enter, and the agent asks before it acts, as it
// always does. Whether the features are on is asked of amethystora-agent once when the provider starts and again
// when the account's switch or the administrator's file changes (two file monitors, no polling).

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {finished} from './run.js';

const AGENT = 'amethystora-agent';
const LONGEST = 1000;
const LETTERS = /\p{L}/gu;
// C0 and C1 control characters
const CONTROLS = /[\u0000-\u001f\u007f-\u009f]/gu;

export class AgentProvider {
    constructor(ext) {
        this._ext = ext;
        this.id = 'amethystora-agent';
        this.isRemoteProvider = false;
        this.canLaunchSearch = false;
        this._on = false;
        this._text = '';
        this._serial = 0;
        this._monitors = [];
        this._asking = null;
    }

    enable() {
        // The account's folder, made if it is not there yet: GLib watches a file in a missing folder by looking every
        // few seconds, where an existing one is watched by inotify
        const folder = GLib.build_filenamev([GLib.get_user_config_dir(), 'amethystora']);
        GLib.mkdir_with_parents(folder, 0o700);
        const watched = [
            GLib.build_filenamev([folder, 'agentic']),
            '/etc/amethystora/agent.conf',
        ];
        for (const path of watched) {
            try {
                const monitor = Gio.File.new_for_path(path).monitor_file(Gio.FileMonitorFlags.NONE, null);
                monitor.connect('changed', () => this._ask());
                this._monitors.push(monitor);
            } catch (e) {
                console.warn(`Amethystora UI: the search cannot watch ${path}: ${e.message}`);
            }
        }
        this._ask();
    }

    destroy() {
        this._serial++;
        for (const monitor of this._monitors)
            monitor.cancel();
        this._monitors = [];
        this._on = false;
    }

    // Exit 0 is on; only the latest answer counts
    _ask() {
        const serial = ++this._serial;
        finished([AGENT, 'status']).then(on => {
            if (serial === this._serial && on !== this._on) {
                this._on = on;
                this.changed?.();
            }
        });
    }

    get on() {
        return this._on;
    }

    // What would be asked, or null when the row is not offered for this text
    static question(text) {
        const question = text.replace(CONTROLS, ' ').trim();
        if (!question || question.startsWith('-') || question.length > LONGEST || /^clip:/iu.test(question) ||
            (question.match(LETTERS) ?? []).length < 3)
            return null;
        return question;
    }

    getInitialResultSet(terms) {
        this._text = AgentProvider.question(terms.join(' ')) ?? '';
        return Promise.resolve(this._on && this._text ? ['ask'] : []);
    }

    getSubsearchResultSet(previous, terms) {
        return this.getInitialResultSet(terms);
    }

    getResultMetas(ids) {
        const gicon = Gio.FileIcon.new(this._ext.dir.get_child('icons').get_child('amethystora-search-agent-symbolic.svg'));
        return Promise.resolve(ids.map(id => id === 'ask' && this._text
            ? {
                id, name: _('Ask the agent: %s').format(this._text), gicon, pictureClass: 'amethystora-search-pic-agent',
                hint: _('Enter'), question: this._text,
            }
            : {id, name: id}));
    }

    filterResults(ids, max) {
        return ids.slice(0, max);
    }

    activateResult(id, terms) {
        const question = id === 'ask' && this._on ? AgentProvider.question(terms.join(' ')) : null;
        if (question)
            finished([AGENT, 'ask', '--', question]);
    }
}
