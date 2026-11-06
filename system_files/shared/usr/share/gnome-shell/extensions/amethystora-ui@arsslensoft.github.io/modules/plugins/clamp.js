// What a plugin printed, as the shell will draw it (plans/PLAN-plugins.md, "What keeps the shell up"). The host has read and
// cleaned it already; the shell is the compositor, so it clamps it again and never trusts the host's work, a script's file or
// anything else it was handed: text only (never markup), a theme icon's name and never a path, at most twenty elements, every
// number inside its range, and no system plate, whatever the plugin said. No gi imports: build_files/shared/test-plugins-clamp.mjs
// runs this file under node.

export const MAX_CARD = 20;
export const MAX_MENU = 20;
const CLASSES = ['accent', 'good', 'warning', 'critical'];
const KINDS = ['text', 'row', 'progress', 'button', 'switch', 'list', 'meter'];
// Control and format characters (bidirectional marks and overrides among them) and line and paragraph separators
const UNSEEN = /[\p{Cc}\p{Cf}\p{Zl}\p{Zp}]/gu;
const ICON = /^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/;
const ACTION = /^[a-z0-9-]{1,32}$/;
const ID = /^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)+$/;

// One plain line of at most `max` characters, or '' for anything that is not text
export function line(value, max = 120) {
    if (typeof value !== 'string')
        return '';
    const flat = value.replace(UNSEEN, ' ').replace(/\s+/g, ' ').trim();
    const letters = [...flat];
    return letters.length > max ? `${letters.slice(0, max - 1).join('')}…` : flat;
}

// Lines of text kept as lines (a desktop card's), each cleaned
export function lines(value, max = 6, each = 80) {
    if (typeof value !== 'string')
        return [];
    return value.split('\n').map(l => line(l, each)).filter(l => l !== '').slice(0, max);
}

export function icon(value) {
    return typeof value === 'string' && ICON.test(value) && !value.endsWith('.') ? value : '';
}

export function number(value, low, high, fallback = null) {
    return typeof value === 'number' && Number.isFinite(value) ? Math.min(high, Math.max(low, value)) : fallback;
}

export function action(value, allowed = null) {
    return typeof value === 'string' && ACTION.test(value) && (!allowed || allowed.includes(value)) ? value : null;
}

export function element(raw, actions) {
    if (!raw || typeof raw !== 'object' || !KINDS.includes(raw.type))
        return null;
    const label = line(raw.label, 80);
    switch (raw.type) {
    case 'text':
        return {type: 'text', text: line(raw.text, 400)};
    case 'row':
        return {type: 'row', label, value: line(raw.value, 120)};
    case 'progress':
        return {type: 'progress', label, value: number(raw.value, 0, 1, 0)};
    case 'button':
        return {type: 'button', label, action: action(raw.action, actions)};
    case 'switch':
        return {type: 'switch', label, on: raw.on === true, action: action(raw.action, actions)};
    case 'list':
        return {type: 'list', items: Array.isArray(raw.items) ? raw.items.slice(0, MAX_CARD).map(i => line(i, 120)) : []};
    case 'meter': {
        const min = number(raw.min, -1e12, 1e12, 0);
        const max = number(raw.max, -1e12, 1e12, 100);
        if (min >= max)
            return null;
        return {type: 'meter', label, value: number(raw.value, min, max, min), min, max};
    }
    }
    return null;
}

// An output, whole: {text, icon, class, click, hidden, private, progress, until, menu, card}
export function output(raw, actions = null) {
    if (!raw || typeof raw !== 'object')
        return null;
    const menu = (Array.isArray(raw.menu) ? raw.menu : []).slice(0, MAX_MENU).map(item => {
        if (!item || typeof item !== 'object')
            return null;
        if (item.separator === true)
            return {separator: true};
        return {label: line(item.label, 80), icon: icon(item.icon), action: action(item.action, actions)};
    }).filter(Boolean);
    const card = (Array.isArray(raw.card) ? raw.card : []).slice(0, MAX_CARD).map(e => element(e, actions)).filter(Boolean);
    const until = Number.isSafeInteger(raw.until) && raw.until > 0 && raw.until < 2 ** 40 ? raw.until : null;
    return {
        text: line(raw.text, 200),
        icon: icon(raw.icon),
        class: CLASSES.includes(raw.class) ? raw.class : '',
        click: action(raw.click, actions),
        hidden: raw.hidden === true,
        private: raw.private === true,
        progress: number(raw.progress, 0, 1, null),
        until,
        menu,
        card,
    };
}

// A plugin as the host's List and Updated give it. A plate is never part of it: the pill gives every plugin's activity ''
export function plugin(raw) {
    if (!raw || typeof raw !== 'object' || typeof raw.id !== 'string' || !ID.test(raw.id) || raw.id.length > 64)
        return null;
    const places = {};
    for (const [surface, options] of Object.entries(raw.places && typeof raw.places === 'object' ? raw.places : {})) {
        const o = options && typeof options === 'object' ? options : {};
        switch (surface) {
        case 'pill': case 'tile': case 'today': case 'menu':
            places[surface] = {};
            break;
        case 'bar':
            places.bar = {position: ['left', 'center', 'right'].includes(o.position) ? o.position : 'right', order: number(o.order, -100, 100, 0)};
            break;
        case 'desktop':
            places.desktop = {corner: ['top-left', 'top-right', 'bottom-left', 'bottom-right'].includes(o.corner) ? o.corner : 'top-right'};
            break;
        case 'search':
            places.search = {prefix: typeof o.prefix === 'string' && /^[a-z0-9:_-]{0,16}$/.test(o.prefix) ? o.prefix : ''};
            break;
        case 'dock':
            if (typeof o.app === 'string' && /^[A-Za-z0-9][A-Za-z0-9._-]{0,120}\.desktop$/.test(o.app))
                places.dock = {app: o.app};
            break;
        }
    }
    const actions = Array.isArray(raw.actions) ? raw.actions.filter(a => action(a)).slice(0, 16) : [];
    return {
        id: raw.id,
        name: line(raw.name, 40) || raw.id,
        version: line(raw.version, 32),
        author: line(raw.author, 80),
        draft: raw.draft === true,
        shipped: raw.shipped === true,
        paused: raw.paused === true,
        reason: ['crash', 'slow', 'memory', 'updates', 'person'].includes(raw.reason) ? raw.reason : '',
        waiting: raw.waiting === 'vault' ? 'vault' : '',
        error: line(raw.error, 200),
        granted: (Array.isArray(raw.granted) ? raw.granted : []).filter(g => typeof g === 'string' && g.length <= 256).slice(0, 40),
        places,
        actions,
        output: output(raw.output, actions),
        // What a draft's author said it would show, for a run that finds nothing to reach
        sample: output(raw.sample, actions),
    };
}

// What a plugin may reach, in the words of its permissions' normal forms, for the card's line: [{kind, what}]
export function reach(granted) {
    const out = [];
    for (const perm of granted) {
        const [kind, what = ''] = perm.split(/=(.*)/s, 2);
        out.push({kind, what});
    }
    return out;
}

// A script's pill (ame pill): {name, text, progress, until, icon, owner} from the file the helper wrote, or null
export function script(raw) {
    if (!raw || typeof raw !== 'object')
        return null;
    const name = line(raw.name, 40);
    if (!name)
        return null;
    return {
        name,
        text: line(raw.text, 120),
        progress: number(raw.progress, 0, 1, null),
        until: Number.isSafeInteger(raw.until) && raw.until > 0 && raw.until < 2 ** 40 ? raw.until : null,
        icon: icon(raw.icon),
        owner: Number.isSafeInteger(raw.owner) && raw.owner > 1 ? raw.owner : 0,
        started: number(raw.started, 0, 2 ** 40, 0),
        private: raw.private === true,
    };
}

// The most script pills at once, and the most updates a script may send in a minute before its pill is ended
export const MAX_SCRIPTS = 8;
export const MAX_SCRIPT_UPDATES = 600;
