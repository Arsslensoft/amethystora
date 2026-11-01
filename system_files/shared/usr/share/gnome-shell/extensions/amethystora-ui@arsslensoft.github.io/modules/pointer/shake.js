// Telling a shake of the mouse from any other movement: the pointer going from side to side, quickly and far, at
// least four times in a row. Only the horizontal movement counts. A turn is the pointer coming back more than a few
// pixels from the furthest point it reached, so the jitter of a hand resting on the mouse never makes one, whatever
// rate the mouse reports at. No GNOME imports, so that the rule can be tried outside the shell.

export const RING = 12; // turns remembered, at most (four are needed)
export const WINDOW = 800; // ms in which the turns and the travel have to fit
export const TURNS = 4; // turns within the window
export const TRAVEL = 500; // logical px travelled sideways within the window, more than this
export const HYSTERESIS = 3; // logical px back from the furthest point before it counts as a turn
export const STEP = 8; // ms: samples closer together than this are not looked at

// The share of a stroke from t0 to t1 that lies after `from`; a stroke of no length counts whole while it is inside
function share(t0, t1, from) {
    if (t1 <= t0)
        return t1 >= from ? 1 : 0;
    return Math.max(0, Math.min(1, (t1 - Math.max(t0, from)) / (t1 - t0)));
}

export class ShakeDetector {
    constructor() {
        this.reset();
    }

    // Forget everything: after a button was held, or when the detector starts
    reset() {
        this.dir = 0;
        this.start = null;
        this.ext = null;
        this.t0 = 0;
        this.turns = [];
    }

    // One position, x in logical px at t ms; true while the movement so far is a shake
    sample(x, t) {
        if (this.start === null) {
            this.start = this.ext = x;
            this.t0 = t;
            return false;
        }
        if (this.dir === 0) {
            if (Math.abs(x - this.start) > HYSTERESIS) {
                this.dir = Math.sign(x - this.start);
                this.ext = x;
            }
        } else if ((x - this.ext) * this.dir > 0) {
            // Still going the same way
            this.ext = x;
        } else if ((this.ext - x) * this.dir > HYSTERESIS) {
            // Back from the furthest point: the stroke that reached it is a turn
            this.turns.push({t0: this.t0, t1: t, len: Math.abs(this.ext - this.start)});
            if (this.turns.length > RING)
                this.turns.shift();
            this.start = this.ext;
            this.ext = x;
            this.t0 = t;
            this.dir = -this.dir;
        }
        const from = t - WINDOW;
        let count = 0;
        // The travel inside the window: each stroke by the share of its time inside it, and the stroke under way
        let travel = Math.abs(this.ext - this.start) * share(this.t0, t, from);
        for (const turn of this.turns) {
            if (turn.t1 < from)
                continue;
            count++;
            travel += turn.len * share(turn.t0, turn.t1, from);
        }
        return count >= TURNS && travel > TRAVEL;
    }
}
