// Starting a program from GNOME Shell forks the shell, and fork() copies the page tables of the whole process: some 14 ms
// of a frozen main loop for each program on a computer with two screens, every time the quick settings ask pactl, the
// search asks the manual or the clipboard seals an entry. GLib starts it with posix_spawn() instead, which copies
// nothing, when the child may inherit the parent's descriptors; GLib and GNOME Shell open all of theirs close-on-exec,
// and a Gio.Subprocess sets the child's own 0, 1 and 2 as it is told, so the child gets exactly what it got before.
// GNOME Shell itself starts apps through GLib.spawn_async, which it wraps to reset what an app inherits, and which is
// left alone; Gio.Subprocess is only the extensions' (and the extension downloader's). A Gio.SubprocessLauncher
// cannot be reached from here: the modules that make one give it the flag themselves. At module scope and never
// undone, like keepsettings.js: it has to outlast a screen lock, which disables the extension.

import Gio from 'gi://Gio';

const INHERIT = Gio.SubprocessFlags.INHERIT_FDS;
const spawn = Gio.Subprocess.new;

if (!Gio.Subprocess.new.amethystoraQuick) {
    Gio.Subprocess.new = function (argv, flags) {
        return spawn.call(this, argv, (flags ?? Gio.SubprocessFlags.NONE) | INHERIT);
    };
    Gio.Subprocess.new.amethystoraQuick = true;
}
