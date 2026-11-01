// clip: the history Amethystora Clipboard already keeps, in memory, shown only when the text typed starts with "clip:"
// and never otherwise; words after it pick the items that hold them. Enter puts the item back on the clipboard and
// pastes it where the person was typing. The history is read from the clipboard extension's own read-only view at
// each search (history(), content()), and nothing of it is kept or written here: in private mode, while the keyring
// keeps the history locked, or with the clipboard extension off, there is nothing to show.

import St from 'gi://St';

import * as DateUtils from 'resource:///org/gnome/shell/misc/dateUtils.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {fold} from '../fuzzy/matcher.js';
import {paste} from '../emoji/paste.js';

const CLIPBOARD = 'amethystora-clipboard@iarsslen.github.io';
const PREFIX = /^\s*clip:\s*/iu;
// The most of a text handed to a row's label
const LONGEST = 200;
const SHOWN = 8;

const COLOURS = /^\s*colou?r:\s*/iu;
// What a colour's row draws: the colour itself, from the picker's own #RRGGBB, rgb() or hsl() text
const COLOUR_TEXT = /^(#[0-9A-F]{6}|rgb\(\d{1,3}, \d{1,3}, \d{1,3}\)|hsl\(\d{1,3}, \d{1,3}%, \d{1,3}%\))$/;

// St's stylesheets read #rrggbb and rgb() but not hsl(): an hsl() colour as rgb()
function cssColour(text) {
    const hsl = /^hsl\((\d+), (\d+)%, (\d+)%\)$/.exec(text);
    if (!hsl)
        return text;
    const [h, s, l] = [Number(hsl[1]), Number(hsl[2]) / 100, Number(hsl[3]) / 100];
    const a = s * Math.min(l, 1 - l);
    const channel = n => {
        const k = (n + h / 30) % 12;
        return Math.round(255 * (l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1))));
    };
    return `rgb(${channel(0)}, ${channel(8)}, ${channel(4)})`;
}

function toolsModule() {
    return Main.extensionManager.lookup('amethystora-ui@iarsslen.github.io')?.stateObj?.modules?.get('tools') ?? null;
}

function clipboardExtension() {
    const state = Main.extensionManager.lookup(CLIPBOARD)?.stateObj;
    return typeof state?.history === 'function' && typeof state.content === 'function' ? state : null;
}

export class ClipboardProvider {
    constructor(host) {
        this._host = host;
        this.id = 'amethystora-clipboard';
        this.isRemoteProvider = false;
        this.canLaunchSearch = false;
        this._found = new Map();
        this._previous = new Map();
    }

    destroy() {
        this._found = new Map();
        this._previous = new Map();
    }

    getInitialResultSet(terms) {
        // The last search's results stay known until the next one is in: GNOME may open one of them meanwhile
        this._previous = this._found;
        this._found = new Map();
        const text = terms.join(' ');
        // colour: the colours the picker copied last (the tools module, in memory), newest first
        const colours = COLOURS.exec(text);
        if (colours) {
            const wanted = fold(text.slice(colours[0].length)).trim();
            const ids = [];
            for (const colour of toolsModule()?.colours?.() ?? []) {
                if (typeof colour !== 'string' || (wanted && !fold(colour).includes(wanted)))
                    continue;
                const id = `colour:${colour}`;
                this._found.set(id, {kind: 'colour', text: colour});
                ids.push(id);
            }
            return Promise.resolve(ids);
        }
        const match = PREFIX.exec(text);
        if (!match)
            return Promise.resolve([]);
        let history;
        try {
            history = clipboardExtension()?.history() ?? [];
        } catch (e) {
            console.warn(`Amethystora UI: the clipboard's history could not be read: ${e.message}`);
            history = [];
        }
        const wanted = fold(text.slice(match[0].length)).split(/\s+/u).filter(Boolean);
        const ids = [];
        for (const item of Array.isArray(history) ? history : []) {
            if (!Number.isInteger(item?.id))
                continue;
            if (item.kind === 'image' ? wanted.length > 0 : !wanted.every(word => fold(item.text).includes(word)))
                continue;
            const id = `clip:${item.id}`;
            this._found.set(id, item);
            ids.push(id);
        }
        return Promise.resolve(ids);
    }

    getSubsearchResultSet(previous, terms) {
        return this.getInitialResultSet(terms);
    }

    getResultMetas(ids) {
        return Promise.resolve(ids.map(id => {
            const item = (this._found.get(id) ?? this._previous.get(id));
            if (!item)
                return {id, name: id};
            if (item.kind === 'colour') {
                const css = COLOUR_TEXT.test(item.text) ? cssColour(item.text) : 'transparent';
                return {
                    id, name: item.text, description: _('Picked from the screen'),
                    createIcon: size => new St.Widget({
                        width: size, height: size,
                        style: `background-color: ${css}; border-radius: 6px; border: 1px solid rgba(255, 255, 255, 0.6);`,
                    }),
                    hint: _('Enter pastes'),
                };
            }
            const picture = item.kind === 'image';
            let name;
            if (picture) {
                name = item.width && item.height
                    ? _('Picture, %d × %d').format(item.width, item.height) : _('Picture');
            } else {
                name = (item.text.split('\n').find(line => line.trim()) ?? item.text).trim().slice(0, LONGEST) ||
                    item.text.slice(0, LONGEST);
            }
            let age = '';
            if (item.copiedAt) {
                try {
                    age = DateUtils.formatTimeSpan(item.copiedAt);
                } catch {}
            }
            return {
                id, name: name || ' ', description: age, pinned: item.pinned === true,
                gicon: null, symbol: picture ? 'image-x-generic-symbolic' : 'edit-paste-symbolic',
                hint: _('Enter pastes'),
            };
        }));
    }

    filterResults(ids, max) {
        return ids.slice(0, Math.min(max, SHOWN));
    }

    activateResult(id) {
        const item = (this._found.get(id) ?? this._previous.get(id));
        if (item?.kind === 'colour') {
            if (toolsModule()?.copyColour?.(item.text))
                paste({keysOnly: true, window: this._host.typingWindow});
            return;
        }
        const extension = clipboardExtension();
        if (!item || !extension)
            return;
        const content = extension.content(item.id);
        if (!content?.bytes)
            return;
        St.Clipboard.get_default().set_content(St.ClipboardType.CLIPBOARD, content.mimetype, content.bytes);
        paste({keysOnly: true, window: this._host.typingWindow});
    }
}
