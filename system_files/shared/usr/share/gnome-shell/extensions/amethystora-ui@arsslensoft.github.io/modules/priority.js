// GNOME Shell's main thread a step ahead of the apps when every core is busy (plans/PLAN-shell-performance.md, phase 4):
// a nice of -10 for that one thread, asked of rtkit as Mutter asks it for its KMS thread (-15), which rtkit grants a
// session's own processes (its floor is -15, and it never makes a thread realtime this way). Under four busy cores the
// overview's stalls fell from 220-340 ms to about 80 and a redraw's longest work from 5.4 to 3.5 ms; on an idle computer
// nothing changes. The thread stays preemptible, and its weight only counts while the shell has work. Asked once a
// process, at module scope: a lock does not change it. AMETHYSTORA_SHELL_PRIORITY=0 in GNOME Shell's environment leaves
// the thread as it was.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

const NICE = -10;

if (GLib.getenv('AMETHYSTORA_SHELL_PRIORITY') !== '0') {
    // The main thread's id is the process's
    const thread = new Gio.Credentials().get_unix_pid();
    Gio.DBus.system.call('org.freedesktop.RealtimeKit1', '/org/freedesktop/RealtimeKit1', 'org.freedesktop.RealtimeKit1',
        'MakeThreadHighPriority', new GLib.Variant('(ti)', [thread, NICE]), null, Gio.DBusCallFlags.NONE, 5000, null,
        (bus, result) => {
            try {
                bus.call_finish(result);
            } catch (e) {
                console.log(`Amethystora UI: GNOME Shell keeps its priority (${e.message})`);
            }
        });
}
