// What a removable disk's numbers say, from one reading to the next: nothing of the shell here, so that
// build_files/shared/test-truth.mjs runs it under node.
//
// Linux gives an account no count of the data still to be written per device: that count (the bdi's) is in debugfs,
// which is root's. What an account can read: the device's own sectors written (/sys/class/block/X/stat; its I/O in flight
// counts reads too, which say nothing here), the whole machine's Dirty and Writeback (/proc/meminfo), `sync -f`, which
// returns once a file system's data has reached its device, and who holds the disk (amethystora-what-uses).
//
// So the words "still on its way" are an estimate over the whole machine's rise since this disk was quiet, and "it is
// all there" is said only when ALL of this holds: a `sync -f` that moved no sector from its start to its end (a sync that
// wrote something is followed by another), the machine's dirty data not having risen across it or since, and nothing
// of the account holding the disk. A file system the kernel cannot flush from here (a FUSE one: ntfs-3g) never gets the
// words: it stays "still writing" until it is put away. Where the machine cannot know, the model says less.

const SECTOR = 512;
// A rise of this much in the machine's dirty data asks a stick to start writing now rather than in 30 seconds
const NUDGE_KIB = 8 * 1024;
// A rise of this much in the machine's dirty data after a sync started means something wrote again (1 MiB)
const RISE_KIB = 1024;
// A quiet disk's sectors are only read once the machine's dirty data has risen this much past its lowest (256 KiB)
const PEEK_KIB = 256;
// Readings without a sector moving before the disk counts as settled
const STILL = 2;
// A disk says it is being written to only from this many sectors (1 MiB): reading from it updates a few, now and then
const NOTICED = 2048;

// The states in which the pill ticks every second; a held or unproven disk is looked at on the slow tick
export const BUSY = ['writing', 'flushing', 'checking', 'verifying', 'done'];

// A line of the block device's stat, as the sectors written so far, or null
export function parseStat(text) {
    const fields = String(text ?? '').trim().split(/\s+/).map(Number);
    if (fields.length < 11 || fields.some(n => !Number.isFinite(n)))
        return null;
    return {written: fields[6]};
}

// Dirty plus Writeback from /proc/meminfo, in KiB, or null
export function parseDirty(text) {
    const value = name => {
        const found = new RegExp(`^${name}:\\s+(\\d+) kB`, 'm').exec(String(text ?? ''));
        return found ? Number(found[1]) : null;
    };
    const dirty = value('Dirty');
    const writeback = value('Writeback');
    return dirty === null || writeback === null ? null : dirty + writeback;
}

// The device's mapped name as sysfs has it: a plain kernel name, or null for anything else
export function blockName(device) {
    const name = String(device ?? '').replace(/^\/dev\//, '');
    return /^[A-Za-z0-9._-]+$/.test(name) ? name : null;
}

// Whether a sync of this file system reaches its device: a FUSE one answers for itself (ntfs-3g's syncfs does not flush
// the block device), so for it nothing here can prove the data is all there
export function provable(fsType) {
    const type = String(fsType ?? '').toLowerCase();
    return type !== '' && !type.startsWith('fuse');
}

export class WriteWatch {
    // proof: false for a file system whose sync proves nothing (see provable())
    constructor({proof = true} = {}) {
        this.state = 'quiet';
        this._proof = proof;
        this._last = null;
        this._from = null;
        this._still = 0;
        this._pct = 0;
    }

    // Whether a reading of this disk's own sectors is worth taking now: always while it is not quiet, and while quiet only
    // once the machine's dirty data has risen past its lowest. Keeps that lowest, so it is called on every quiet tick.
    peek(dirty) {
        if (this.state !== 'quiet' || !this._last || dirty === null || dirty === undefined)
            return true;
        this._quiet = Math.min(this._quiet ?? dirty, dirty);
        this._nudged = Math.min(this._nudged ?? dirty, dirty);
        return dirty - this._quiet >= PEEK_KIB || this._from !== null;
    }

    // One reading: {written, dirty (KiB, or null)}. Says what to do: {flush} when the disk looks settled and must prove
    // it, {nudge} when the machine's dirty data rose while this disk stays quiet, {verify} when everything but the
    // question "does anything of the account hold it?" says it is all there.
    step({written, dirty}) {
        const last = this._last;
        this._last = {written};
        if (!last) {
            this._quiet = dirty ?? 0;
            this._nudged = dirty ?? 0;
            return {};
        }
        const moved = written !== last.written;
        const known = dirty !== null && dirty !== undefined;
        this._dirty = known ? dirty : this._dirty ?? 0;
        let told = {};
        switch (this.state) {
        case 'quiet':
            this._quiet = Math.min(this._quiet, this._dirty);
            this._nudged = Math.min(this._nudged, this._dirty);
            if (moved) {
                this._from ??= last.written;
                if (written - this._from >= NOTICED)
                    this._begin(this._from, written);
            } else if (this._dirty - this._nudged >= NUDGE_KIB) {
                this._from = null;
                this._nudged = this._dirty;
                return {nudge: true};
            } else {
                this._from = null;
            }
            break;
        case 'writing':
            this._written = written;
            this._still = moved ? 0 : this._still + 1;
            if (this._still >= STILL) {
                this._startSync(written);
                told = {flush: true};
            }
            break;
        case 'flushing':
            this._written = written;
            this._low();
            break;
        case 'checking':
            this._written = written;
            this._low();
            if (!known && this._proof) {
                // Nothing says whether more was written meanwhile
                this.state = 'unproven';
            } else if (this._rose) {
                this._back();
            } else if (written !== this._syncFrom) {
                // The sync wrote: only one that wrote nothing proves anything
                this._startSync(written);
                told = {flush: true};
            } else if (!this._proof) {
                this.state = 'unproven';
            } else {
                this.state = 'verifying';
                told = {verify: true};
            }
            break;
        case 'verifying':
        case 'held':
            this._written = written;
            this._low();
            if (moved || this._rose)
                this._back();
            break;
        case 'unproven':
            this._written = written;
            if (moved)
                this._back();
            break;
        case 'done':
            this._low();
            if (moved || this._rose)
                this._begin(last.written, written);
            break;
        }
        this._track();
        return told;
    }

    // The lowest the machine's dirty data has been since the sync started; a rise past RISE_KIB is something writing again
    _low() {
        this._floor = Math.min(this._floor ?? this._dirty, this._dirty);
        if (this._dirty - this._floor >= RISE_KIB)
            this._rose = true;
    }

    _startSync(written) {
        this.state = 'flushing';
        this._syncFrom = written;
        this._floor = this._dirty;
        this._rose = false;
        this._still = 0;
    }

    // Writing again, in the same copy: what it has shown stays shown
    _back() {
        this.state = 'writing';
        this._still = 0;
        this._rose = false;
    }

    // A new copy
    _begin(from, written) {
        this.state = 'writing';
        this._from = null;
        this._start = from;
        this._written = written;
        this._still = 0;
        this._rose = false;
        this._pct = 0;
        this._floor = this._dirty;
    }

    // The percent only ever goes up within a copy: the machine's other writes move the estimate both ways
    _track() {
        if (['quiet', 'done'].includes(this.state))
            return;
        const {written, left} = this._counts();
        const whole = written + left;
        if (whole > 0)
            this._pct = Math.max(this._pct, Math.min(99, Math.floor(100 * written / whole)));
    }

    // The sync came back: true when it worked. The next reading says whether the disk stayed still.
    flushed(ok) {
        if (this.state !== 'flushing')
            return;
        this.state = ok ? 'checking' : 'writing';
        this._still = 0;
    }

    // The sync did not come back in time: nothing can be proved, so nothing is claimed
    timedOut() {
        if (this.state === 'flushing')
            this.state = 'unproven';
    }

    // The answer to {verify}: true when nothing of the account holds the disk, which makes the words true; false when
    // the disk is still in use, and is looked at again with release(); null when that could not be read, and then
    // nothing is claimed
    verified(clean) {
        if (this.state !== 'verifying')
            return;
        this._low();
        if (this._rose)
            this._back();
        else
            this.state = clean === null ? 'unproven' : clean ? 'done' : 'held';
    }

    // The holders went: prove it all over again. True when a sync must run.
    release() {
        if (this.state !== 'held')
            return false;
        this._startSync(this._written ?? this._syncFrom);
        return true;
    }

    // Back to watching, after the pill has said it is done
    rest() {
        this.state = 'quiet';
        this._quiet = this._dirty ?? 0;
        this._nudged = this._dirty ?? 0;
        this._pct = 0;
    }

    _counts() {
        const written = Math.max(0, ((this._written ?? 0) - (this._start ?? 0)) * SECTOR);
        const left = Math.max(0, (this._dirty ?? 0) - (this._quiet ?? 0)) * 1024;
        return {written, left};
    }

    // What the pill and the card say: bytes still on their way (an estimate over the whole machine), and the percent of it
    // written, which never goes down within a copy
    get progress() {
        const {written, left} = this._counts();
        return {written, left, percent: this._pct};
    }
}
