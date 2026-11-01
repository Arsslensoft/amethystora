// What the meters read, all of it from the kernel's own files. The three the top bar needs are read at once, on the
// shell's own thread: the kernel makes them up in memory when asked, so the read never waits for a disk. The process
// list, many small files and only while a menu shows it, is read off that thread.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

const decoder = new TextDecoder();

function readNow(path) {
    try {
        const [ok, bytes] = GLib.file_get_contents(path);
        return ok ? decoder.decode(bytes) : null;
    } catch {
        return null;
    }
}

function readLater(path, cancellable) {
    return new Promise(resolve => {
        Gio.File.new_for_path(path).load_contents_async(cancellable, (file, result) => {
            try {
                const [, bytes] = file.load_contents_finish(result);
                resolve(decoder.decode(bytes));
            } catch {
                // Gone, or another account's: left out
                resolve(null);
            }
        });
    });
}

// The processor's time in ticks, for the whole machine and for each processor by number. Busy is everything but
// idling: user, nice, system, interrupts (irq and softirq) and steal, the time a virtual machine's host gave to
// another; guest time is already counted in user and nice. Idle is idle and iowait, waiting for a disk with nothing
// else to do.
export function processorTimes() {
    const text = readNow('/proc/stat');
    const result = {all: null, each: new Map()};
    for (const line of text?.split('\n') ?? []) {
        if (!line.startsWith('cpu'))
            break;
        const fields = line.split(/\s+/);
        const [user, nice, system, idle, iowait, irq, softirq, steal] =
            fields.slice(1, 9).map(field => Number(field) || 0);
        const times = {busy: user + nice + system + irq + softirq + steal, idle: idle + iowait};
        if (fields[0] === 'cpu')
            result.all = times;
        else
            result.each.set(Number(fields[0].slice(3)), times);
    }
    return result;
}

// The share of a processor's time that was busy between two readings, 0 when nothing passed
export function load(before, after) {
    if (!before || !after)
        return 0;
    const busy = after.busy - before.busy;
    const total = busy + after.idle - before.idle;
    return total > 0 ? Math.min(Math.max(busy / total, 0), 1) : 0;
}

// Sizes in bytes
export function memory() {
    const values = {};
    for (const line of readNow('/proc/meminfo')?.split('\n') ?? []) {
        const match = /^(\w+):\s+(\d+)/.exec(line);
        if (match)
            values[match[1]] = Number(match[2]) * 1024;
    }
    return {
        total: values.MemTotal ?? 0,
        available: values.MemAvailable ?? values.MemFree ?? 0,
        swapTotal: values.SwapTotal ?? 0,
        swapFree: values.SwapFree ?? 0,
    };
}

// The bytes received and sent by each network interface that has a device behind it: Ethernet, Wi-Fi, a phone
// sharing its connection over USB. Loopback, tunnels and VPNs, bridges, and the links of containers and virtual
// machines have none; what they carry crosses a real interface as well, and is counted there, once.
export class Interfaces {
    constructor() {
        this._physical = new Map();
    }

    read() {
        const counters = new Map();
        for (const line of readNow('/proc/net/dev')?.split('\n').slice(2) ?? []) {
            const colon = line.indexOf(':');
            if (colon < 0)
                continue;
            const name = line.slice(0, colon).trim();
            if (!this._isPhysical(name))
                continue;
            const fields = line.slice(colon + 1).trim().split(/\s+/).map(Number);
            counters.set(name, {received: fields[0] || 0, sent: fields[8] || 0});
        }
        // A name that comes back may be another device
        for (const name of this._physical.keys()) {
            if (!counters.has(name))
                this._physical.delete(name);
        }
        return counters;
    }

    _isPhysical(name) {
        let physical = this._physical.get(name);
        if (physical === undefined) {
            physical = GLib.file_test(`/sys/class/net/${name}/device`, GLib.FileTest.EXISTS);
            this._physical.set(name, physical);
        }
        return physical;
    }
}

// The menu's details: the processor's name, its mean speed in GHz, and the seconds since start-up
export function processorDetails() {
    const text = readNow('/proc/cpuinfo') ?? '';
    const model = /^model name\s*:\s*(.*)$/m.exec(text)?.[1].trim() ?? '';
    const speeds = [...text.matchAll(/^cpu MHz\s*:\s*([\d.]+)/gm)].map(match => Number(match[1]));
    const speed = speeds.length > 0 ? speeds.reduce((a, b) => a + b, 0) / speeds.length / 1000 : null;
    const uptime = Number(readNow('/proc/uptime')?.split(' ')[0]);
    return {model, speed, uptime: Number.isFinite(uptime) ? uptime : null};
}

// The processor's temperature sensor, looked for once: Intel's package sensor, or AMD's die (else control)
// temperature, from the first processor socket. Null when the machine has none of them.
const SENSORS = {
    coretemp: ['temp1_input', 'temp2_input'],
    k10temp: ['temp2_input', 'temp1_input'],
    zenpower: ['temp1_input', 'temp2_input'],
};

export function findTemperature() {
    const base = '/sys/class/hwmon';
    const names = [];
    try {
        const dir = GLib.Dir.open(base, 0);
        for (let name = dir.read_name(); name; name = dir.read_name())
            names.push(name);
        dir.close();
    } catch {
        return null;
    }
    names.sort((a, b) => Number(a.replace(/\D/g, '')) - Number(b.replace(/\D/g, '')));
    for (const name of names) {
        const files = SENSORS[readNow(`${base}/${name}/name`)?.trim()];
        const file = files?.map(f => `${base}/${name}/${f}`).find(path => GLib.file_test(path, GLib.FileTest.EXISTS));
        if (file)
            return file;
    }
    return null;
}

export function temperature(path) {
    const value = Number(readNow(path));
    return path && Number.isFinite(value) && value > 0 ? value / 1000 : null;
}

// How much of a command line is looked at, in characters
const COMMAND_LENGTH = 1024;

// Interpreters whose program is the file they were given
const INTERPRETER = /^(python[\d.]*|gjs)$/;

function baseName(path) {
    return path.slice(path.lastIndexOf('/') + 1) || path;
}

// The name a person knows a process by: the program's file name when it was started by its path, the script's
// when that program is Python or gjs, else the whole command; a kernel thread keeps the kernel's short name
function displayName(commandLine, short) {
    const words = commandLine.split(' ').filter(word => word !== '');
    if (words.length === 0)
        return short;
    if (!words[0].startsWith('/') && !words[0].startsWith('.'))
        return words.join(' ');
    const program = baseName(words[0]);
    if (INTERPRETER.test(program)) {
        const script = words.slice(1).find(word => !word.startsWith('-'));
        if (script?.includes('/'))
            return baseName(script);
    }
    return program;
}

// Every process the account can see, walked while a menu is open: its CPU ticks so far and, when it can be read, its
// proportional share of memory. A process's name is worked out once. ticksOnly (the power module's looks) reads the
// stat file alone: no memory, which walks each process's page tables, and the kernel's short name.
export class Processes {
    constructor() {
        this._names = new Map();
    }

    async walk(cancellable, ticksOnly = false) {
        const pids = [];
        try {
            const dir = GLib.Dir.open('/proc', 0);
            for (let name = dir.read_name(); name; name = dir.read_name()) {
                if (/^\d+$/.test(name))
                    pids.push(name);
            }
            dir.close();
        } catch {
            return new Map();
        }

        const found = new Map();
        // A few at a time, so that the reads do not crowd out the shell's own work
        for (let i = 0; i < pids.length; i += 16) {
            if (cancellable.is_cancelled())
                break;
            // eslint-disable-next-line no-await-in-loop
            const batch = await Promise.all(pids.slice(i, i + 16).map(pid => this._read(pid, cancellable, ticksOnly)));
            for (const process of batch) {
                if (process)
                    found.set(process.key, process);
            }
        }
        for (const key of this._names.keys()) {
            if (!found.has(key))
                this._names.delete(key);
        }
        return found;
    }

    async _read(pid, cancellable, ticksOnly = false) {
        const stat = await readLater(`/proc/${pid}/stat`, cancellable);
        const close = stat?.lastIndexOf(')') ?? -1;
        if (close < 0)
            return null;
        const short = stat.slice(stat.indexOf('(') + 1, close);
        const fields = stat.slice(close + 2).split(' ');
        // The process's start time tells a new process apart from an old one that had the same number
        const key = `${pid}:${fields[19]}`;
        const ticks = (Number(fields[11]) || 0) + (Number(fields[12]) || 0);
        if (ticksOnly)
            return {key, name: short, ticks, pss: null};

        const [rollup, commandLine] = await Promise.all([
            readLater(`/proc/${pid}/smaps_rollup`, cancellable),
            this._names.has(key) ? null : readLater(`/proc/${pid}/cmdline`, cancellable),
        ]);
        if (!this._names.has(key)) {
            // Only the start of a command line is a name: any account's process can have one of megabytes
            this._names.set(key, commandLine === null
                ? short : displayName(commandLine.slice(0, COMMAND_LENGTH).replace(/\0/g, ' '), short));
        }
        const pss = /^Pss:\s+(\d+)/m.exec(rollup ?? '');
        return {key, name: this._names.get(key), ticks, pss: pss ? Number(pss[1]) * 1024 : null};
    }

    clear() {
        this._names.clear();
    }
}
