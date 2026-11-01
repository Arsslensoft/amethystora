// Whether the agentic features are on for this account, and whether its agent may make changes, as amethystora-agent
// itself says (`status`, `tools status`: the one answer the rest of the image goes by). Asked when the module starts and
// again whenever the account's two switches or the administrator's file change: three file monitors, no polling.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

const AGENT = 'amethystora-agent';
const CONF = '/etc/amethystora/agent.conf';

// Whether argv ended well; nothing waits on it but the promise
function finished(argv) {
    return new Promise(resolve => {
        let process;
        try {
            process = Gio.Subprocess.new(argv, Gio.SubprocessFlags.STDOUT_SILENCE | Gio.SubprocessFlags.STDERR_SILENCE);
        } catch {
            resolve(false);
            return;
        }
        process.wait_check_async(null, (source, result) => {
            try {
                resolve(source.wait_check_finish(result));
            } catch {
                resolve(false);
            }
        });
    });
}

// What argv printed, or '' when it failed
export function output(argv) {
    return new Promise(resolve => {
        let process;
        try {
            process = Gio.Subprocess.new(argv, Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_MERGE);
        } catch {
            resolve({ok: false, text: ''});
            return;
        }
        process.communicate_utf8_async(null, null, (source, result) => {
            try {
                const [, stdout] = source.communicate_utf8_finish(result);
                resolve({ok: source.get_successful(), text: stdout ?? ''});
            } catch {
                resolve({ok: false, text: ''});
            }
        });
    });
}

export class AgentStatus {
    // changed() is called when either answer changes
    constructor(changed) {
        this._changed = changed;
        this.on = false;
        this.act = false;
        // The chosen agent's command name (claude, opencode...), from status
        this.agent = '';
        this._serial = 0;
        this._monitors = [];
    }

    start() {
        // The account's folder, made if it is not there yet: GLib watches a file in a missing folder by looking every
        // few seconds, where an existing one is watched by inotify
        const folder = GLib.build_filenamev([GLib.get_user_config_dir(), 'amethystora']);
        GLib.mkdir_with_parents(folder, 0o700);
        for (const path of [GLib.build_filenamev([folder, 'agentic']), GLib.build_filenamev([folder, 'agent-tools']), CONF]) {
            try {
                const monitor = Gio.File.new_for_path(path).monitor_file(Gio.FileMonitorFlags.NONE, null);
                monitor.connect('changed', () => this.ask());
                this._monitors.push(monitor);
            } catch (e) {
                console.warn(`Amethystora UI: the agent module cannot watch ${path}: ${e.message}`);
            }
        }
        this.ask();
    }

    stop() {
        this._serial++;
        for (const monitor of this._monitors)
            monitor.cancel();
        this._monitors = [];
        this.on = false;
        this.act = false;
        this._changed = null;
    }

    // Only the latest answer counts
    async ask() {
        const serial = ++this._serial;
        const status = await output([AGENT, 'status']);
        const act = status.ok && await finished([AGENT, 'tools', 'status']);
        if (serial !== this._serial)
            return;
        // "Agentic features: on (claude)", or "(cursor-agent, not installed yet)"
        const agent = /\(([a-z-]+)/.exec(status.text)?.[1] ?? '';
        if (status.ok === this.on && act === this.act && agent === this.agent)
            return;
        this.on = status.ok;
        this.act = act;
        this.agent = agent;
        this._changed?.();
    }
}

// An agent's name for people, as amethystora-agent's title() has it (the product names are their makers')
export function agentTitle(name) {
    return {
        claude: 'Claude Code', opencode: 'Opencode', codex: 'Codex', agy: 'Antigravity CLI', gemini: 'Gemini CLI',
        copilot: 'Copilot CLI', 'cursor-agent': 'Cursor CLI',
    }[name] ?? name;
}
