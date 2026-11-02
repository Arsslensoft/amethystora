// The real sound's level for the pill's bars (R7-21): PipeWire's own pw-record, on the one stream of the player shown, at a
// low rate and for as long as the bars show, in a scope that cannot take the desktop with it. The shell looks at its pipe
// every 150 ms from a timer of its own, and takes the last 150 ms of what has gathered there: pw-record writes at the
// player's pace (every 21 ms for an ordinary player, much oftener for a quick one), and reading as it writes would wake the
// shell that often. Only each slice's height is worked out, and the bytes go nowhere: nothing here writes sound (20-tests.sh
// greps for it). It says it is a level meter, as GNOME's own volume control does, so that a sound meter is not taken for
// somebody recording the microphone, and it takes no other stream than the one it was given: with `node.dont-fallback` a
// stream that is gone by the time it links ends the program, where it would otherwise follow the default microphone
// (build_files/shared/test-levels.sh holds this against a private PipeWire).

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {barsOf} from './tint.js';

const RATE = 4000;
// A slice of sound, in milliseconds and in bytes (16-bit samples): each may redraw the top bar
const SLICE_MS = 150;
const SLICE = RATE * SLICE_MS / 1000 * 2;
// The most read at once: the pipe holds 64 KiB, which is eight seconds of this
const MOST = 1 << 14;
const COMMAND = [
    '/usr/bin/systemd-run', '--user', '--scope', '--quiet', '--collect', '-p', 'CPUWeight=20', '-p', 'MemoryMax=32M', '--',
    '/usr/bin/pw-record', '--rate', `${RATE}`, '--channels', '1', '--format', 's16', '--latency', '150ms',
    '-P', '{ node.passive=true node.dont-reconnect=true node.dont-fallback=true application.name="Sound levels" application.id=org.gnome.VolumeControl media.role=Production }',
];

export class Levels {
    // heights(bars) is called with four heights for each slice; ended() once the program has stopped by itself
    constructor(heights, ended) {
        this._heights = heights;
        this._ended = ended;
    }

    // serial: the stream's serial number, which is PulseAudio's index of it
    start(serial) {
        this.stop();
        this._serial = serial;
        this._cancellable = new Gio.Cancellable();
        try {
            this._process = Gio.Subprocess.new([...COMMAND, '--target', `${serial}`, '-'],
                Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_SILENCE);
        } catch (e) {
            console.debug(`Amethystora UI: the sound levels did not start: ${e.message}`);
            this._process = null;
            this._serial = null;
            return false;
        }
        const pipe = this._process.get_stdout_pipe();
        const cancellable = this._cancellable;
        this._timer = GLib.timeout_add(GLib.PRIORITY_LOW, SLICE_MS, () => {
            this._look(pipe, cancellable);
            return GLib.SOURCE_CONTINUE;
        });
        return true;
    }

    stop() {
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = 0;
        this._reading = false;
        this._carry = null;
        this._cancellable?.cancel();
        this._cancellable = null;
        this._process?.force_exit();
        this._process = null;
        this._serial = null;
    }

    get serial() {
        return this._serial ?? null;
    }

    // One read of what has gathered since the last: it returns at once with that, or waits for the next quantum when the
    // pipe is empty at this very moment
    _look(pipe, cancellable) {
        if (this._reading)
            return;
        this._reading = true;
        pipe.read_bytes_async(MOST, GLib.PRIORITY_LOW, cancellable, (stream, result) => {
            let bytes;
            try {
                bytes = stream.read_bytes_finish(result);
            } catch {
                bytes = null;
            }
            if (cancellable.is_cancelled())
                return;
            this._reading = false;
            // A pipe that cannot be read, or that is closed: the program has gone, and with it the timer
            if (!bytes || bytes.get_size() === 0) {
                this.stop();
                this._ended();
                return;
            }
            // The last slice of it, from a whole sample: a half one is kept for the next
            let data = bytes.toArray();
            if (this._carry !== null && this._carry !== undefined) {
                const joined = new Uint8Array(data.length + 1);
                joined[0] = this._carry;
                joined.set(data, 1);
                data = joined;
                this._carry = null;
            }
            if (data.length & 1) {
                this._carry = data[data.length - 1];
                data = data.subarray(0, data.length - 1);
            }
            const from = Math.max(0, data.length - SLICE);
            this._heights(barsOf(new Int16Array(data.buffer.slice(data.byteOffset + from, data.byteOffset + data.length))));
        });
    }
}
