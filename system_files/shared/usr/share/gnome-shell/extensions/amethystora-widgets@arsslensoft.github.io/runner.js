// Runs a widget's command the way the top bar does, for the bar (extension.js) and for
// `amethystora-widgets run` (check.js) alike: in the widget's folder, with the same PATH, the same
// variables and the same time limit, so that a widget that passes the check behaves the same in the bar.

import GLib from 'gi://GLib';
import Gio from 'gi://Gio';

import {lastUpdate, parseManifest, parseOutput} from './protocol.js';

export const WIDGETS_DIR = GLib.build_filenamev([GLib.get_user_config_dir(), 'amethystora', 'widgets']);
const STATE_DIR = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora', 'widgets']);

// The session's PATH is not a login shell's, so the bar would not find what a terminal finds. Both
// use this one instead: the account's own programs, Homebrew's, and the system's.
export const PATH = [
    GLib.build_filenamev([GLib.get_home_dir(), '.local', 'bin']),
    '/home/linuxbrew/.linuxbrew/bin',
    '/usr/local/bin',
    '/usr/bin',
].join(':');

// Seconds a command run on an interval may take before it is stopped
export const TIME_LIMIT = 30;
// Seconds before a command meant to keep running is started again, doubling up to the second
const RESTART = [5, 60];

export function loadWidget(dir) {
    const id = GLib.path_get_basename(dir);
    const widget = {id, dir, state: GLib.build_filenamev([STATE_DIR, id])};
    let text;
    try {
        text = new TextDecoder().decode(GLib.file_get_contents(GLib.build_filenamev([dir, 'widget.json']))[1]);
    } catch (e) {
        return {...widget, errors: [`widget.json cannot be read: ${e.message}`], warnings: []};
    }
    return {...widget, ...parseManifest(text)};
}

// A command's arguments. A command line is split as a shell would split it, but no shell runs it, and
// a program named with a relative path (./widget.sh) is one of the widget's own files.
export function commandArgv(widget, command) {
    const argv = Array.isArray(command) ? [...command] : GLib.shell_parse_argv(command)[1];
    if (argv[0].includes('/') && !GLib.path_is_absolute(argv[0]))
        argv[0] = GLib.build_filenamev([widget.dir, argv[0]]);
    return argv;
}

// Each command runs in a session of its own (setsid), so that stopping it stops everything it started
// too: a script's pipeline would otherwise outlive it, once for every time the widget is reloaded
function spawn(widget, command, flags) {
    GLib.mkdir_with_parents(widget.state, 0o700);
    const launcher = new Gio.SubprocessLauncher({flags});
    launcher.set_cwd(widget.dir);
    launcher.setenv('PATH', PATH, true);
    launcher.setenv('WIDGET_ID', widget.id, true);
    launcher.setenv('WIDGET_DIR', widget.dir, true);
    launcher.setenv('WIDGET_STATE', widget.state, true);
    return launcher.spawnv(['setsid', ...commandArgv(widget, command)]);
}

function stopGroup(proc) {
    const pid = proc.get_identifier();
    if (pid)
        Gio.Subprocess.new(['kill', '-TERM', '--', `-${pid}`], Gio.SubprocessFlags.STDERR_SILENCE);
    proc.force_exit();
}

function failure(proc, stderr = '') {
    const how = proc.get_if_signaled()
        ? `was killed by signal ${proc.get_term_sig()}`
        : `exited with status ${proc.get_exit_status()}`;
    const reason = stderr.trim().split('\n').pop();
    return reason ? `${how}: ${reason}` : how;
}

// A command from a widget's menu or click. It is never stopped, and nothing it prints is read: what it
// changed shows on the widget's next update. onDone gets the reason it failed, or null.
export function runAction(widget, command, onDone) {
    let proc;
    try {
        proc = spawn(widget, command, Gio.SubprocessFlags.STDOUT_SILENCE | Gio.SubprocessFlags.STDERR_SILENCE);
    } catch (e) {
        onDone(e.message);
        return;
    }
    proc.wait_async(null, () => onDone(proc.get_successful() ? null : failure(proc)));
}

export class Runner {
    // onUpdate(state, warnings) for each update the command prints, onError(message) when it fails
    constructor(widget, onUpdate, onError) {
        this._widget = widget;
        this._onUpdate = onUpdate;
        this._onError = onError;
        this._cancellable = new Gio.Cancellable();
        this._proc = null;
        this._source = 0;
        this._restart = RESTART[0];
    }

    start() {
        if (this._widget.manifest.interval > 0)
            this._run();
        else
            this._follow();
    }

    stop() {
        this._cancellable.cancel();
        if (this._proc)
            stopGroup(this._proc);
        this._proc = null;
        if (this._source)
            GLib.source_remove(this._source);
        this._source = 0;
    }

    // Run now rather than at the next interval, after an action may have changed what it shows
    refresh() {
        if (this._widget.manifest.interval === 0 || this._proc || this._cancellable.is_cancelled())
            return;
        if (this._source)
            GLib.source_remove(this._source);
        this._source = 0;
        this._run();
    }

    _later(seconds, func) {
        if (this._cancellable.is_cancelled())
            return;
        this._source = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, seconds, () => {
            this._source = 0;
            func();
            return GLib.SOURCE_REMOVE;
        });
    }

    _update(text) {
        const {state, errors, warnings} = parseOutput(text);
        if (errors.length)
            this._onError(errors.join('\n'));
        else
            this._onUpdate(state, warnings);
    }

    // "interval" above 0: run the command, read the last thing it printed, and run it again that many
    // seconds after it finished
    _run() {
        const interval = this._widget.manifest.interval;
        let proc;
        try {
            proc = spawn(this._widget, this._widget.manifest.command,
                Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_PIPE);
        } catch (e) {
            this._onError(`it could not be started: ${e.message}`);
            this._later(interval, () => this._run());
            return;
        }
        this._proc = proc;
        let late = false;
        const limit = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, TIME_LIMIT, () => {
            late = true;
            stopGroup(proc);
            return GLib.SOURCE_REMOVE;
        });
        proc.communicate_utf8_async(null, this._cancellable, (_proc, result) => {
            if (!late)
                GLib.source_remove(limit);
            if (this._cancellable.is_cancelled())
                return;
            this._proc = null;
            try {
                const [, stdout, stderr] = proc.communicate_utf8_finish(result);
                if (late)
                    this._onError(`it took longer than ${TIME_LIMIT} seconds and was stopped`);
                else if (!proc.get_successful())
                    this._onError(`it ${failure(proc, stderr ?? '')}`);
                else
                    this._update(lastUpdate(stdout ?? ''));
            } catch (e) {
                this._onError(e.message);
            }
            this._later(interval, () => this._run());
        });
    }

    // "interval" 0: the command keeps running, and every line it prints is an update. Started again
    // when it stops, later each time it stops without having printed anything.
    _follow() {
        let proc;
        try {
            proc = spawn(this._widget, this._widget.manifest.command,
                Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_SILENCE);
        } catch (e) {
            this._onError(`it could not be started: ${e.message}`);
            this._retry();
            return;
        }
        this._proc = proc;
        const stdout = new Gio.DataInputStream({base_stream: proc.get_stdout_pipe(), close_base_stream: true});
        const read = () => stdout.read_line_async(GLib.PRIORITY_DEFAULT, this._cancellable, (_stream, result) => {
            let line = null;
            try {
                [line] = stdout.read_line_finish_utf8(result);
            } catch (e) {
                if (this._cancellable.is_cancelled())
                    return;
                // Not text: stop it rather than read on past the line
                stopGroup(proc);
            }
            if (line === null) {
                proc.wait_async(null, () => {
                    if (this._cancellable.is_cancelled())
                        return;
                    this._proc = null;
                    this._onError(`it stopped, and it should keep running: it ${failure(proc)}`);
                    this._retry();
                });
                return;
            }
            if (line.trim()) {
                this._restart = RESTART[0];
                this._update(line);
            }
            read();
        });
        read();
    }

    _retry() {
        this._later(this._restart, () => this._follow());
        this._restart = Math.min(this._restart * 2, RESTART[1]);
    }
}
