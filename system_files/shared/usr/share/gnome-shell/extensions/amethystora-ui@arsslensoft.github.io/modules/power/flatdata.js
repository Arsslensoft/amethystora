// When a flat battery closes the computer (power/flat.js), without anything of GNOME, so that the build checks it under
// node (build_files/shared/test-flat-battery.mjs).

// The charge at which it asks, and how long it gives, in seconds
export const FLAT = 3;
export const COUNTDOWN = 60;

// UPower's numbers: the device's Type, State and WarningLevel
export const TYPE_BATTERY = 2;
const DISCHARGING = 2;
const WARNING_ACTION = 5;

// What a reading means now: 'none', or 'flat' when the computer's own battery, running down, is at FLAT or UPower is
// about to act by itself. A battery never seen above FLAT since this GNOME Shell started says nothing: a gauge that
// always reads 0% must not close a computer that works, and UPower's own action still stands behind it. After Not yet
// (`heldAt`, the whole percent it was said at) it asks again only once the charge has fallen below that.
export function flatState({present, type, state, percentage, warning = 0, seenAbove = false, heldAt = null}) {
    if (present !== true || type !== TYPE_BATTERY || state !== DISCHARGING || !seenAbove)
        return 'none';
    if (!Number.isFinite(percentage) || (percentage > FLAT && warning < WARNING_ACTION))
        return 'none';
    return heldAt !== null && Math.floor(percentage) >= heldAt ? 'none' : 'flat';
}

// Whether a reading shows a battery with charge in hand, which arms flatState
export const isAbove = ({present, type, percentage}) => present === true && type === TYPE_BATTERY &&
    Number.isFinite(percentage) && percentage > FLAT;
