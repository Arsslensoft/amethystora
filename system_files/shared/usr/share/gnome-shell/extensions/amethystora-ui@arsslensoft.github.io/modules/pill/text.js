// Text that others wrote, made safe to show, and the durations the pill says. A title from a media player, a phone's
// name, a caller, a shared address and a notification's title are somebody else's words: each is shown as plain
// text (never markup), on one line, without the invisible characters that could turn it round or hide part of it.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';

// Control and format characters (bidirectional marks and overrides among them) and line and paragraph separators
const UNSEEN = /[\p{Cc}\p{Cf}\p{Zl}\p{Zp}]/gu;

// One plain line of at most `length` characters (by what a person counts as one), or '' for anything not a string
export function plain(text, length = 120) {
    if (typeof text !== 'string')
        return '';
    const line = text.replace(UNSEEN, ' ').replace(/\s+/g, ' ').trim();
    const letters = [...line];
    return letters.length > length ? `${letters.slice(0, length - 1).join('')}…` : line;
}

// An address as the pill shows it: without its scheme (with it, for the phone link's notification), the host in its
// ASCII form when it holds other characters (so that a look-alike name cannot pass for another site), and nothing
// invisible. `null` when it is not one.
export function shownAddress(url, withScheme = false) {
    let uri;
    try {
        uri = GLib.Uri.parse(url, GLib.UriFlags.NONE);
    } catch {
        return null;
    }
    let host = uri.get_host();
    if (!host)
        return null;
    if (/[^\x20-\x7e]/.test(host))
        host = GLib.hostname_to_ascii(host) ?? null;
    if (!host)
        return null;
    const port = uri.get_port() > 0 ? `:${uri.get_port()}` : '';
    const query = uri.get_query() ? `?${uri.get_query()}` : '';
    const fragment = uri.get_fragment() ? `#${uri.get_fragment()}` : '';
    let rest = `${uri.get_path()}${query}${fragment}`;
    if (rest === '/')
        rest = '';
    const scheme = withScheme ? `${uri.get_scheme()}://` : '';
    return `${scheme}${host}${port}${rest}`.replace(UNSEEN, '');
}

// mm:ss, or h:mm:ss from an hour
export function clock(seconds) {
    const s = Math.max(0, Math.round(seconds));
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const pad = n => String(n).padStart(2, '0');
    return h > 0 ? `${h}:${pad(m)}:${pad(s % 60)}` : `${pad(m)}:${pad(s % 60)}`;
}

// A duration in words for a screen reader: "3 minutes 58 seconds"
export function spoken(seconds) {
    const s = Math.max(0, Math.round(seconds));
    const m = Math.floor(s / 60);
    const rest = s % 60;
    const minutes = ngettext('%d minute', '%d minutes', m).format(m);
    const secs = ngettext('%d second', '%d seconds', rest).format(rest);
    if (m === 0)
        return secs;
    return rest === 0 ? minutes : `${minutes} ${secs}`;
}

// "2 h 42 min", "42 min"
export function hoursMinutes(seconds) {
    const total = Math.max(1, Math.round(seconds / 60));
    const h = Math.floor(total / 60);
    const m = total % 60;
    if (h === 0)
        return _('%d min').format(m);
    return _('%d h %d min').format(h, m);
}

// One for the module's life, disposed by its disable() rather than by the collector (dock.js says why)
let interfaceSettings = null;

export function disposeSettings() {
    interfaceSettings?.run_dispose();
    interfaceSettings = null;
}

// The time of day as the person's clock shows it (12- or 24-hour)
export function timeOfDay(date) {
    interfaceSettings ??= new Gio.Settings({schema_id: 'org.gnome.desktop.interface'});
    const settings = interfaceSettings;
    const twelve = settings.get_string('clock-format') === '12h';
    const dt = GLib.DateTime.new_from_unix_local(Math.floor(date.getTime() / 1000));
    // Translators: the time of day on a 12-hour and on a 24-hour clock, as GLib.DateTime.format takes it
    return dt.format(twelve ? _('%l:%M %p') : _('%H:%M')).trim();
}
