// Numbers, sizes, rates and times, written the way the session's language writes them (the same ICU data the
// Amethystora apps use): "4,2 Mo/s" in French, Arabic digits in Arabic.

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

const formats = new Map();

function number(options) {
    const key = JSON.stringify(options);
    let format = formats.get(key);
    if (!format) {
        format = new Intl.NumberFormat(undefined, options);
        formats.set(key, format);
    }
    return format;
}

function unit(name, value, digits) {
    return number({style: 'unit', unit: name, minimumFractionDigits: digits, maximumFractionDigits: digits})
        .format(value);
}

// Decimal units: one decimal below ten of a unit, none above, and the next unit from a thousand
const STEPS = [
    [1e6, 'kilobyte', 1e3, 0],
    [1e7, 'megabyte', 1e6, 1],
    [1e9, 'megabyte', 1e6, 0],
    [1e10, 'gigabyte', 1e9, 1],
    [1e12, 'gigabyte', 1e9, 0],
    [1e13, 'terabyte', 1e12, 1],
    [Infinity, 'terabyte', 1e12, 0],
];

// Whole gives the memory a computer has as it is sold, in whole binary gigabytes from one up: the kernel keeps a
// little of it, so the total reads a bit under (15.6 of 16) and is rounded up
export function size(bytes, perSecond = false, whole = false) {
    if (whole && bytes >= 2 ** 30)
        return unit('gigabyte', Math.ceil(bytes / 2 ** 30), 0);
    const name = base => perSecond ? `${base}-per-second` : base;
    if (!(bytes >= 1))
        return unit(name('kilobyte'), 0, 0);
    if (bytes < 1000) {
        // Translators: something smaller than the size or share shown, such as "< 1 kB"
        return _('< %s').format(unit(name('kilobyte'), 1, 0));
    }
    const [, base, scale, digits] = STEPS.find(([below]) => bytes < below);
    return unit(name(base), bytes / scale, digits);
}

export function percent(fraction) {
    return number({style: 'percent', maximumFractionDigits: 0}).format(fraction);
}

// A process's share of the processor: under one percent is said as such rather than as nothing
export function share(fraction) {
    return fraction < 0.01 ? _('< %s').format(percent(0.01)) : percent(fraction);
}

export function count(value) {
    return number({maximumFractionDigits: 0}).format(value);
}

export function celsius(degrees) {
    return unit('celsius', degrees, 0);
}

export function gigahertz(value) {
    // Translators: a processor's speed, such as "3.4 GHz"
    return _('%s GHz').format(number({minimumFractionDigits: 1, maximumFractionDigits: 1}).format(value));
}

// Days when there are any, hours once there are days or hours, and minutes always: "2 days, 0 hours, 5 minutes"
export function uptime(seconds) {
    const days = Math.floor(seconds / 86400);
    const hours = Math.floor(seconds % 86400 / 3600);
    const minutes = Math.floor(seconds % 3600 / 60);
    return new Intl.DurationFormat(undefined, {
        style: 'long',
        daysDisplay: days > 0 ? 'always' : 'auto',
        hoursDisplay: days > 0 || hours > 0 ? 'always' : 'auto',
        minutesDisplay: 'always',
    }).format({days, hours, minutes});
}

export function minutesAgo(minutes) {
    return new Intl.RelativeTimeFormat(undefined, {numeric: 'always'}).format(-minutes, 'minute');
}

export function now() {
    return new Intl.RelativeTimeFormat(undefined, {numeric: 'auto'}).format(0, 'second');
}

// A sentence with named places for its values, so that a translation can put them in its own order
export function fill(text, values) {
    return text.replace(/\{(\w+)\}/g, (whole, name) => values[name] ?? whole);
}
