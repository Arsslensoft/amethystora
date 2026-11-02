// When Today offers Your year (modules/today/yearwindow.js of Amethystora UI), under node and gjs alike:
//   node build_files/shared/test-your-year.mjs
// A failure throws, which makes either exit with an error.

import {offeredYear} from
    '../../system_files/shared/usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io/modules/today/yearwindow.js';

const OLD = new Date(2020, 0, 1).getTime() / 1000;
const check = (when, born, seen, want) => {
    const got = offeredYear(when, born, seen);
    if (got !== want)
        throw new Error(`offeredYear(${when}, ${born}, ${seen}): ${got}, wanted ${want}`);
};
check(new Date(2026, 5, 15), OLD, 0, null);
check(new Date(2026, 11, 14), OLD, 0, null);
check(new Date(2026, 11, 15), OLD, 0, 2026);
check(new Date(2026, 11, 31), OLD, 0, 2026);
check(new Date(2027, 0, 15), OLD, 0, 2026);
check(new Date(2027, 0, 16), OLD, 0, null);
// Once
check(new Date(2026, 11, 20), OLD, 2026, null);
check(new Date(2027, 0, 2), OLD, 2026, null);
check(new Date(2027, 11, 20), OLD, 2026, 2027);
// Too new for a year to say anything; a home with no birth kept counts as old
check(new Date(2026, 11, 20), new Date(2026, 11, 1).getTime() / 1000, 0, null);
check(new Date(2026, 11, 20), 0, 0, 2026);
(globalThis.print ?? console.log)('test-your-year: ok');
