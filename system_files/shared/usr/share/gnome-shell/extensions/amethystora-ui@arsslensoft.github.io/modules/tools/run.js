// Runs one of the programs the tools ask (qrencode, the image's own helpers): an argument vector, never a shell; what
// it is handed on its standard input, never on its command line; a time limit, and what it prints capped.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

Gio._promisify(Gio.Subprocess.prototype, 'communicate_utf8_async');

// Resolves to {ok, stdout} (ok: it ended with 0); rejects only when it could not be started or was cancelled
export async function run(argv, {input = null, timeout = 10000, limit = 1024 * 1024, cancellable = null} = {}) {
    const flags = Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_SILENCE |
        (input === null ? Gio.SubprocessFlags.NONE : Gio.SubprocessFlags.STDIN_PIPE);
    const process = Gio.Subprocess.new(argv, flags);
    const timer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, timeout, () => {
        process.force_exit();
        return GLib.SOURCE_REMOVE;
    });
    const cancelled = cancellable?.connect(() => process.force_exit()) ?? 0;
    try {
        const [stdout] = await process.communicate_utf8_async(input, cancellable);
        const ok = process.get_if_exited() && process.get_exit_status() === 0;
        const text = stdout ?? '';
        return {ok: ok && text.length <= limit, stdout: text.length <= limit ? text : ''};
    } finally {
        GLib.source_remove(timer);
        if (cancelled)
            cancellable.disconnect(cancelled);
    }
}
