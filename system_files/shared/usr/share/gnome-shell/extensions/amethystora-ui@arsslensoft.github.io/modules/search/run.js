// Running the few programs the search asks something of, always as an argument vector and never through a shell:
// what they print is read when they end, and a search that moves on stops the one it no longer needs.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

// What argv printed on its standard output, or null when it failed, was stopped or could not start
export function output(argv, cancellable = null) {
    return new Promise(resolve => {
        let process;
        try {
            process = Gio.Subprocess.new(argv, Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_SILENCE);
        } catch (e) {
            console.warn(`Amethystora UI: the search could not run ${argv[0]}: ${e.message}`);
            resolve(null);
            return;
        }
        const stop = cancellable?.connect(() => process.force_exit()) ?? 0;
        process.communicate_utf8_async(null, null, (source, result) => {
            if (stop)
                cancellable.disconnect(stop);
            try {
                const [, stdout] = source.communicate_utf8_finish(result);
                resolve(source.get_successful() ? stdout : null);
            } catch {
                resolve(null);
            }
        });
    });
}

// Whether argv ended well; it runs on by itself, and nothing waits on it but this promise
export function finished(argv) {
    return new Promise(resolve => {
        let process;
        try {
            process = Gio.Subprocess.new(argv, Gio.SubprocessFlags.STDOUT_SILENCE | Gio.SubprocessFlags.STDERR_SILENCE);
        } catch (e) {
            console.warn(`Amethystora UI: the search could not run ${argv[0]}: ${e.message}`);
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

// Whether ms went by without cancellable being cancelled; the timer is removed either way
export function pause(ms, cancellable = null) {
    return new Promise(resolve => {
        if (cancellable?.is_cancelled()) {
            resolve(false);
            return;
        }
        let stop = 0;
        const source = GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms, () => {
            if (stop)
                cancellable.disconnect(stop);
            resolve(true);
            return GLib.SOURCE_REMOVE;
        });
        stop = cancellable?.connect(() => {
            GLib.source_remove(source);
            resolve(false);
        }) ?? 0;
    });
}
