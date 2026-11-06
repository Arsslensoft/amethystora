// The AI agent on the desktop, only while the agentic features are on for this account (the image ships them off):
// Super+Shift+A asks the agent about the text selected in any window, which it is given as data in a terminal
// (agent/selection.js); and while the agent may also make changes, the live pill shows each change its tools record,
// each with its own Undo, and Stop (agent/changes.js). Other modules ask `on` before they offer the agent (the
// not-responding dialog, modules/quit.js).
//
// Off, nothing is bound and nothing is read: only the account's two switches and the administrator's file are watched
// (agent/status.js), so that turning the agent on applies at once. The keys are in
// org.gnome.shell.extensions.amethystora-ui.agent (ame agent selection, ame agent pill, Control's AI agent page).

import Gio from 'gi://Gio';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as Util from 'resource:///org/gnome/shell/misc/util.js';

import {AgentAsk} from './agent/ask.js';
import {kindOfName} from './agent/askmodel.js';
import {AgentChanges} from './agent/changes.js';
import {AskCard, readPicture, readSelection} from './agent/selection.js';
import {Routines} from './agent/routines.js';
import {AgentStatus, agentTitle} from './agent/status.js';

const AGENT = 'amethystora-agent';
const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.agent';
const KEY = 'ask-selection';
// The pill's own question, and whether the tools' hand-off questions are asked there (R7-11, R7-12)
const ASK_KEY = 'ask-here';

export class Agent {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._settings = this._ext.getSettings(SCHEMA);
        this._gicon = Gio.FileIcon.new(this._ext.dir.get_child('icons').get_child('amethystora-search-agent-symbolic.svg'));
        this._status = new AgentStatus(() => this._sync());
        this._settings.connectObject(
            `changed::${KEY}`, () => this._sync(),
            'changed::pill', () => this._sync(),
            'changed::doing', () => this._sync(),
            `changed::${ASK_KEY}`, () => this._sync(),
            'changed::ask-questions', () => this._sync(),
            'changed::routines', () => this._sync(),
            this);
        this._ask = new AgentAsk(this);
        this._status.start();
        this._sync();
    }

    disable() {
        this._settings?.disconnectObject(this);
        this._status?.stop();
        this._status = null;
        this._unbind();
        this._unbindAsk();
        this._stopChanges();
        this._ask?.stop();
        this._ask = null;
        this._stopRoutines();
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
        this._gicon = null;
    }

    // Whether the agentic features are on for this account, as amethystora-agent status says
    get on() {
        return this._status?.on ?? false;
    }

    // Whether its tools may make changes too (level 2)
    get act() {
        return this._status?.act ?? false;
    }

    get gicon() {
        return this._gicon;
    }

    get pill() {
        return this._pill;
    }

    get agentName() {
        return agentTitle(this._status?.agent || 'claude');
    }

    get _pill() {
        return this._ext.modules?.get('pill') ?? null;
    }

    _sync() {
        if (!this._settings)
            return;
        const bind = this.on && this._settings.get_strv(KEY).some(accelerator => accelerator !== '');
        if (bind)
            this._bind();
        else
            this._unbind();
        // The pill's list of changes (level 2) and its one line about what is read (R7-13, level 1 as well)
        const what = {changes: this.act && this._settings.get_boolean('pill'), doing: this.on && this._settings.get_boolean('doing')};
        if (this._changes && (this._changes.what.changes !== what.changes || this._changes.what.doing !== what.doing))
            this._stopChanges();
        if (what.changes || what.doing)
            this._startChanges(what);
        else
            this._stopChanges();
        // Little routines (R7-15) belong to the person, not to the agent: they run whether or not the agentic features are on
        if (this._settings.get_boolean('routines'))
            this._startRoutines();
        else
            this._stopRoutines();
        // The pill's question, and where the tools' hand-off questions are put to the person: only while the features are on
        const ask = this.on && this._settings.get_strv(ASK_KEY).some(accelerator => accelerator !== '');
        if (ask)
            this._bindAsk();
        else
            this._unbindAsk();
        if (this.on && this._settings.get_boolean('ask-questions'))
            this._ask?.export();
        else
            this._ask?.unexport();
        // The not-responding dialog offers the agent only while it is on
        this._ext.modules?.get('quit')?.agentChanged?.();
    }

    // --- Ask about a selection ----------------------------------------------------------------------------------

    _bind() {
        if (this._bound)
            return;
        const action = Main.wm.addKeybinding(KEY, this._settings, Meta.KeyBindingFlags.IGNORE_AUTOREPEAT,
            Shell.ActionMode.NORMAL | Shell.ActionMode.OVERVIEW, () => this.askAboutSelection());
        if (action === Meta.KeyBindingAction.NONE)
            console.warn('Amethystora UI: the key to ask about a selection could not be set');
        this._bound = true;
    }

    _unbind() {
        if (this._bound)
            Main.wm.removeKeybinding(KEY);
        this._bound = false;
        this._reading?.cancel();
        this._reading = null;
        this._card?.destroy();
        this._card = null;
    }

    _startRoutines() {
        if (this._routines)
            return;
        this._routines = new Routines(this);
        try {
            this._routines.start();
        } catch (e) {
            console.error(`Amethystora UI: the routines did not start: ${e.message}\n${e.stack}`);
            this._stopRoutines();
        }
    }

    _stopRoutines() {
        const routines = this._routines;
        this._routines = null;
        try {
            routines?.stop();
        } catch (e) {
            console.error(`Amethystora UI: the routines did not stop cleanly: ${e.message}`);
        }
    }

    get routines() {
        return this._routines ?? null;
    }

    _bindAsk() {
        if (this._askBound)
            return;
        const action = Main.wm.addKeybinding(ASK_KEY, this._settings, Meta.KeyBindingFlags.IGNORE_AUTOREPEAT,
            Shell.ActionMode.NORMAL | Shell.ActionMode.OVERVIEW, () => this.askHere());
        if (action === Meta.KeyBindingAction.NONE)
            console.warn('Amethystora UI: the key to ask the agent in the pill could not be set');
        this._askBound = true;
    }

    _unbindAsk() {
        if (this._askBound)
            Main.wm.removeKeybinding(ASK_KEY);
        this._askBound = false;
    }

    // The key: the question's field in the pill. Without a live pill there is nowhere to answer, so the agent opens in a terminal
    askHere() {
        if (!this.on)
            return;
        if (!this._ask?.open())
            Util.spawn([AGENT]);
    }

    // Reads the selection and opens the card; with nothing selected, the pill says so for a moment
    async askAboutSelection() {
        if (!this.on || this._card || this._reading)
            return;
        const reading = new Gio.Cancellable();
        this._reading = reading;
        const text = await readSelection(reading);
        if (this._reading !== reading)
            return;
        this._reading = null;
        if (!text) {
            // Ask about this (R7-14), once the person chose it: no text, but a picture that was copied
            const picture = this._settings?.get_boolean('ask-about') ? await readPicture(new Gio.Cancellable()) : null;
            if (picture && this.on && this._ask) {
                this._ask.aboutPicture(picture);
                return;
            }
            this._pill?.notice?.(_('Select some text first'), 'edit-select-all-symbolic', 3);
            return;
        }
        this._card = new AskCard(text, this.agentName, this._gicon, () => {
            this._card = null;
        });
        this._card.open();
    }

    // What the pill's Ask the agent target is given: the files dropped on it. Once the person chose Ask about this (R7-14), a picture or
    // a text file is shown with where it goes and asked about in the pill; otherwise, and for any other file, their paths are text for
    // the card that asks in a terminal
    askAbout(files) {
        if (this._settings?.get_boolean('ask-about') && this._ask && files.length === 1 && kindOfName(files[0].path)) {
            this._ask.aboutFiles(files.map(f => f.path));
            return;
        }
        this._askAboutText(files.map(f => f.path).join('\n'));
    }

    _askAboutText(text) {
        if (!this.on || this._card || !text)
            return;
        this._card = new AskCard(text, this.agentName, this._gicon, () => {
            this._card = null;
        });
        this._card.open();
    }

    get card() {
        return this._card ?? null;
    }

    // --- The agent's changes in the live pill -------------------------------------------------------------------

    _startChanges(what) {
        if (this._changes)
            return;
        const pill = this._pill;
        if (!pill?.setAgent)
            return;
        this._changes = new AgentChanges(pill, () => this.agentName, this._gicon, what);
        this._changes.onNoted = () => this._ask?.noted();
        try {
            this._changes.start();
        } catch (e) {
            console.error(`Amethystora UI: the agent's changes did not start: ${e.message}\n${e.stack}`);
            this._stopChanges();
        }
    }

    _stopChanges() {
        this._changes?.stop();
        this._changes = null;
    }

    get changes() {
        return this._changes ?? null;
    }

    // For the suites
    state() {
        return {on: this.on, act: this.act, bound: !!this._bound, card: !!this._card, watching: !!this._changes,
            pill: this._changes?.status ?? null};
    }
}
