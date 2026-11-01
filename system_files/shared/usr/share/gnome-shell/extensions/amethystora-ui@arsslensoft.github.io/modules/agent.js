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

import {AgentChanges} from './agent/changes.js';
import {AskCard, readSelection} from './agent/selection.js';
import {AgentStatus, agentTitle} from './agent/status.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.agent';
const KEY = 'ask-selection';

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
            this);
        this._status.start();
        this._sync();
    }

    disable() {
        this._settings?.disconnectObject(this);
        this._status?.stop();
        this._status = null;
        this._unbind();
        this._stopChanges();
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
        if (this.act && this._settings.get_boolean('pill'))
            this._startChanges();
        else
            this._stopChanges();
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
            this._pill?.notice?.(_('Select some text first'), 'edit-select-all-symbolic', 3);
            return;
        }
        this._card = new AskCard(text, this.agentName, this._gicon, () => {
            this._card = null;
        });
        this._card.open();
    }

    get card() {
        return this._card ?? null;
    }

    // --- The agent's changes in the live pill -------------------------------------------------------------------

    _startChanges() {
        if (this._changes)
            return;
        const pill = this._pill;
        if (!pill?.setAgent)
            return;
        this._changes = new AgentChanges(pill, () => this.agentName, this._gicon);
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
