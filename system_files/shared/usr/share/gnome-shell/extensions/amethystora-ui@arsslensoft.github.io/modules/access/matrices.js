// The colour filters as 3 × 3 matrices on linear RGB.
//
// What an eye with each deficiency sees is taken from Machado, Oliveira and Fernandes, "A Physiologically-based Model
// for Simulation of Color Vision Deficiency" (IEEE Transactions on Visualization and Computer Graphics 15(6), 2009),
// their matrices for severity 1.0. A filter does not show that: it helps such an eye. What the eye loses (the colour
// less its simulation) is moved into what it does see: for red and green deficiencies the lost red goes into green and
// blue at 0.7 and the lost green and blue are given back to themselves; for blue-yellow the lost blue goes into red
// and green at 0.7. So each filter is I + Shift · (I − Sim), one matrix, worked out here once. Grey is the luminance of
// linear RGB (ITU-R BT.709's weights) in all three channels.

const IDENTITY = [1, 0, 0, 0, 1, 0, 0, 0, 1];

// What each eye sees, row by row (the paper's table, severity 1.0)
const SEEN = {
    protan: [0.152286, 1.052583, -0.204868, 0.114503, 0.786281, 0.099216, -0.003882, -0.048116, 1.051998],
    deutan: [0.367322, 0.860646, -0.227968, 0.280085, 0.672501, 0.047413, -0.011820, 0.042940, 0.968881],
    tritan: [1.255528, -0.076749, -0.178779, -0.078411, 0.930809, 0.147602, 0.004733, 0.691367, 0.303900],
};

// Where what is lost goes
const RED_GREEN = [0, 0, 0, 0.7, 1, 0, 0.7, 0, 1];
const BLUE_YELLOW = [1, 0, 0.7, 0, 1, 0.7, 0, 0, 0];

const LUMA = [0.2126, 0.7152, 0.0722];

// a × b: b first, then a
export function multiply(a, b) {
    const out = [];
    for (let row = 0; row < 3; row++) {
        for (let col = 0; col < 3; col++) {
            let sum = 0;
            for (let k = 0; k < 3; k++)
                sum += a[row * 3 + k] * b[k * 3 + col];
            out.push(sum);
        }
    }
    return out;
}

function corrected(seen, shift) {
    const lost = IDENTITY.map((v, i) => v - seen[i]);
    return multiply(shift, lost).map((v, i) => IDENTITY[i] + v);
}

const FULL = {
    protan: corrected(SEEN.protan, RED_GREEN),
    deutan: corrected(SEEN.deutan, RED_GREEN),
    tritan: corrected(SEEN.tritan, BLUE_YELLOW),
    grey: [...LUMA, ...LUMA, ...LUMA],
};

export const FILTERS = Object.keys(FULL);

// The filter's matrix at a strength from 0 (nothing) to 1 (the full filter), row by row
export function filterMatrix(filter, strength) {
    const full = FULL[filter];
    if (!full)
        return [...IDENTITY];
    const k = Math.max(0, Math.min(1, strength));
    return full.map((v, i) => IDENTITY[i] + k * (v - IDENTITY[i]));
}
