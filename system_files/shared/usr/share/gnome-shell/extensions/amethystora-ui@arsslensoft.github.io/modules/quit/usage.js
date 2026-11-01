// What each running app uses, for the force quit list: the processor time and memory of its windows' processes and
// of every process under them (a terminal's commands, a browser's helpers), from the kernel's own /proc files. The
// processes are walked off the shell's thread, a few files at a time, and only while the list is open.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

const decoder = new TextDecoder();
// The kernel's clock ticks a second (USER_HZ, 100 on every architecture Fedora builds for) and a memory page's size
const TICKS = 100;
const PAGE = 4096;

function readLater(path, cancellable) {
    return new Promise(resolve => {
        Gio.File.new_for_path(path).load_contents_async(cancellable, (file, result) => {
            try {
                const [, bytes] = file.load_contents_finish(result);
                resolve(decoder.decode(bytes));
            } catch {
                // Gone meanwhile: left out
                resolve(null);
            }
        });
    });
}

// Every process: its parent, its processor ticks so far and its resident memory, by its number; and when it was read
export async function walk(cancellable) {
    const pids = [];
    try {
        const dir = GLib.Dir.open('/proc', 0);
        for (let name = dir.read_name(); name; name = dir.read_name()) {
            if (/^\d+$/.test(name))
                pids.push(name);
        }
        dir.close();
    } catch {
        return null;
    }
    const at = GLib.get_monotonic_time();
    const found = new Map();
    for (let i = 0; i < pids.length; i += 16) {
        if (cancellable.is_cancelled())
            return null;
        // eslint-disable-next-line no-await-in-loop
        const stats = await Promise.all(pids.slice(i, i + 16).map(pid => readLater(`/proc/${pid}/stat`, cancellable)));
        stats.forEach((stat, n) => {
            // The name in brackets may hold anything: the fields follow the last bracket
            const close = stat?.lastIndexOf(')') ?? -1;
            if (close < 0)
                return;
            const fields = stat.slice(close + 2).split(' ');
            found.set(Number(pids[i + n]), {
                parent: Number(fields[1]),
                ticks: (Number(fields[11]) || 0) + (Number(fields[12]) || 0),
                start: fields[19],
                memory: (Number(fields[21]) || 0) * PAGE,
            });
        });
    }
    return {at, processes: found};
}

// The processes under each of `roots` (their numbers), the roots included
function family(processes, roots) {
    const children = new Map();
    for (const [pid, process] of processes) {
        if (!children.has(process.parent))
            children.set(process.parent, []);
        children.get(process.parent).push(pid);
    }
    const seen = new Set();
    const queue = roots.filter(pid => processes.has(pid));
    while (queue.length > 0) {
        const pid = queue.pop();
        if (seen.has(pid))
            continue;
        seen.add(pid);
        queue.push(...children.get(pid) ?? []);
    }
    return seen;
}

// An app's use between two walks: {cpu: share of the whole processor in per cent, or null with one walk, memory: bytes}
export function usage(before, now, roots) {
    const pids = family(now.processes, roots);
    let memory = 0, ticks = 0;
    for (const pid of pids) {
        const process = now.processes.get(pid);
        memory += process.memory;
        const earlier = before?.processes.get(pid);
        // The same process, not a new one that got its number: counted from the earlier reading, else from its start
        ticks += earlier && earlier.start === process.start ? process.ticks - earlier.ticks : 0;
    }
    if (!before)
        return {cpu: null, memory};
    const seconds = (now.at - before.at) / 1e6;
    const cpu = seconds > 0 ? Math.min(100, 100 * ticks / (seconds * TICKS * GLib.get_num_processors())) : 0;
    return {cpu, memory};
}
