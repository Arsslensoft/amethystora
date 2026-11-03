// The popup, as the Extension board draws it. It only asks background.js and shows the answer: it holds no secret at any point, and
// nothing fills until one of its buttons is clicked. No page can read or open it, since nothing of the extension is web accessible.
'use strict';

const say = (name, ...values) => chrome.i18n.getMessage(name, values);
// A name or username shown or saved: no control or direction-changing characters (a name can come from anywhere a vault is synced from)
const clean = (text) => String(text ?? '').replace(/[\p{Cc}\p{Cf}\p{Zl}\p{Zp}]/gu, '').slice(0, 256);
const main = document.getElementById('main');
const status = document.getElementById('status');
const pill = document.getElementById('pill');

// The board's drawings, as path data
const ICONS = {
    fill: ['M8 12h.01M12 12h.01M16 12h.01', 'M6 7h12a3 3 0 0 1 3 3v4a3 3 0 0 1-3 3H6a3 3 0 0 1-3-3v-4a3 3 0 0 1 3-3z'],
    code: ['M13 3 5 14h6l-1 7 8-11h-6z'],
    make: ['M4 12a8 8 0 0 1 14-5l2 2M20 5v4h-4M20 12a8 8 0 0 1-14 5l-2-2M4 19v-4h4'],
    save: ['M12 4v11M7 11l5 5 5-5M5 20h14'],
};

let tabId = null;
let origin = null;
let host = '';

function setPill(words, closed) {
    pill.hidden = false;
    pill.textContent = words;
    pill.classList.toggle('closed', closed);
}

function paragraph(text) {
    const element = document.createElement('p');
    element.textContent = text;
    return element;
}

// "For github.com", with the domain kept left to right in any language
function forSite(site) {
    const element = document.createElement('p');
    const [before, after = ''] = say('for_site', '\u0001').split('\u0001');
    const domain = document.createElement('bdi');
    domain.className = 'domain';
    domain.textContent = site;
    element.append(before, domain, after);
    return element;
}

function action(words, icon, act, primary) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = primary ? 'action primary' : 'action';
    if (icon) {
        const svgName = 'http://www.w3.org/2000/svg';
        const svg = document.createElementNS(svgName, 'svg');
        svg.setAttribute('viewBox', '0 0 24 24');
        svg.setAttribute('aria-hidden', 'true');
        for (const data of ICONS[icon]) {
            const path = document.createElementNS(svgName, 'path');
            path.setAttribute('d', data);
            svg.append(path);
        }
        button.append(svg);
    }
    button.append(words);
    button.addEventListener('click', () => act(button));
    return button;
}

// A letter on a stable colour, as the window shows an item without a logo
function hue(name) {
    let value = 0;
    for (const character of name) {
        value = (value * 31 + character.codePointAt(0)) % 360;
    }
    return value;
}

function itemRow(name, detail, pressed, choose) {
    const row = document.createElement(choose ? 'button' : 'div');
    row.className = 'item';
    if (choose) {
        row.type = 'button';
        row.setAttribute('aria-pressed', String(pressed));
        row.addEventListener('click', () => choose(row));
    } else {
        row.style.background = 'var(--chosen)';
    }
    const tile = document.createElement('span');
    tile.className = 'tile';
    tile.setAttribute('aria-hidden', 'true');
    name = clean(name);
    detail = clean(detail);
    tile.style.background = `hsl(${hue(name)} 45% 28%)`;
    tile.style.color = `hsl(${hue(name)} 80% 86%)`;
    tile.textContent = [...name][0]?.toUpperCase() ?? '?';
    const who = document.createElement('span');
    who.className = 'who';
    const title = document.createElement('b');
    title.textContent = name;
    who.append(title);
    if (detail) {
        const small = document.createElement('span');
        small.textContent = detail;
        who.append(small);
    }
    row.append(tile, who);
    return row;
}

// One item shown as chosen, or a list to choose from with the first chosen
function chooser(rows, detailOf, changed) {
    let chosen = rows[0];
    if (rows.length === 1) {
        return { element: itemRow(chosen.name, detailOf(chosen), true, null), chosen: () => chosen };
    }
    const list = document.createElement('div');
    list.className = 'list';
    for (const row of rows) {
        list.append(itemRow(row.name, detailOf(row), row === chosen, (element) => {
            chosen = row;
            list.querySelectorAll('.item').forEach((other) => other.setAttribute('aria-pressed', String(other === element)));
            changed?.();
        }));
    }
    return { element: list, chosen: () => chosen };
}

function showMessage(text) {
    status.hidden = false;
    status.textContent = text;
}

function failure(answer) {
    const words = {
        'no-host': say('no_host'),
        'no-suffix-list': say('no_suffix_list'),
        'cross-site': say('cross_site'),
        'no-match': say('no_match', host),
        'no-field': say('no_field'),
        exists: say('exists'),
        closed: say('closed_vault'),
        failed: say('failed'),
        'too-large': say('too_large'),
        'other-site-form': say('form_elsewhere'),
    };
    return words[answer.problem] ?? words[answer.why] ?? say('not_filled');
}

// Goes on in background.js if the popup closes, as it does when a prompt takes the focus
async function run(message, button, afterwards) {
    button.disabled = true;
    const answer = await chrome.runtime.sendMessage({ tabId, expected: origin, ...message });
    button.disabled = false;
    if (!answer?.ok) {
        showMessage(failure(answer ?? {}));
    } else if (afterwards) {
        afterwards(answer);
    } else {
        window.close();
    }
}

function renderClosed(vault) {
    const closed = vault === 'closed';
    setPill(closed ? say('closed') : say('not_set_up'), true);
    main.append(action(closed ? say('open_vault') : say('set_up_vault'), null, (button) => run({ kind: 'open' }, button), true));
}

// Picking an entry that does not match the page: the person sees which domain gets it first
function renderOthers(others) {
    const open = action(say('another_account'), null, () => {
        open.remove();
        const confirm = document.createElement('p');
        const pick = chooser(others, (row) => row.account, () => refresh());
        const refresh = () => {
            confirm.textContent = say('code_goes_to', clean(pick.chosen().name), host);
        };
        refresh();
        main.insertBefore(pick.element, foot);
        main.insertBefore(confirm, foot);
        main.insertBefore(action(say('fill_the_code'), 'code', (button) => run({ kind: 'fill-code', id: pick.chosen().id, picked: true }, button)), foot);
    });
    open.classList.add('quiet');
    main.append(open);
}

let foot = null;
let whoBox = null;

// What a new login would be saved as, shown and editable before anything is saved: the username a page put in its form is the page's
function renderWho(user) {
    const label = document.createElement('label');
    label.className = 'who-label';
    const [before, after = ''] = say('username_for', '\u0001').split('\u0001');
    const domain = document.createElement('bdi');
    domain.className = 'domain';
    domain.textContent = host;
    label.append(before, domain, after);
    whoBox = document.createElement('input');
    whoBox.type = 'text';
    whoBox.value = clean(user);
    whoBox.maxLength = 256;
    whoBox.autocomplete = 'off';
    whoBox.spellcheck = false;
    label.append(whoBox);
    main.append(label);
}

function renderOpen(state) {
    setPill(say('open'), false);
    main.append(forSite(host));
    const { logins = [], codes = [], others = [], fields } = state;
    if (!fields) {
        main.append(paragraph(say('no_access')));
        return;
    }
    // A form that posts to another site gets nothing: the host refuses it as well
    const fills = state.form !== 'other-site';
    if (!fills) {
        main.append(paragraph(say('form_elsewhere')));
    }
    if (logins.length) {
        const pick = chooser(logins, (row) => row.user);
        main.append(pick.element);
        if (!logins.every((row) => row.exact)) {
            main.append(paragraph(say('same_site_note', state.site, host)));
        }
        if (fields.login && fills) {
            main.append(action(say('fill_the_password'), 'fill', (button) => run({ kind: 'fill-login', id: pick.chosen().id }, button), true));
        }
    }
    if (codes.length && fields.code && fills) {
        const pick = chooser(codes, (row) => row.account);
        if (codes.length > 1) {
            main.append(pick.element);
        }
        main.append(action(say('fill_the_code'), 'code', (button) => run({ kind: 'fill-code', id: pick.chosen().id }, button), !logins.length));
    }
    if (fields.password) {
        renderWho(fields.user);
        if (fills) {
            main.append(action(say('make_new_password'), 'make', (button) => run({ kind: 'fill-new', user: whoBox.value }, button)));
        }
    }
    if (fields.typed) {
        main.append(action(say('save_this_login'), 'save', (button) => run({ kind: 'save', user: whoBox.value }, button, () => showMessage(say('saved')))));
    }
    if (others.length && fields.code && !codes.length && fills) {
        renderOthers(others);
    }
    if (!logins.length && !codes.length && !fields.password) {
        main.append(paragraph(say('nothing_here', host)));
    }
    foot = paragraph(say('nothing_until_click'));
    main.append(foot);
}

// Only ever on a click here: the browser asks the person, and the sites granted are the browser's own record
async function renderButtonSwitch() {
    const { hostname, protocol } = new URL(origin);
    const pattern = `${protocol}//${hostname}/*`;
    const has = await chrome.permissions.contains({ origins: [pattern] });
    const button = action(has ? say('hide_button') : say('show_button'), null, () => {
        // Directly in the click: a browser allows the request only so
        const asked = has ? chrome.permissions.remove({ origins: [pattern] }) : chrome.permissions.request({ origins: [pattern] });
        asked.finally(() => window.close());
    });
    button.classList.add('quiet');
    main.append(button);
}

async function start() {
    document.documentElement.dir = say('@@bidi_dir');
    document.documentElement.lang = chrome.i18n.getUILanguage();
    document.getElementById('title').textContent = say('title');
    // In a window of its own the tab it is for is in its address; in the toolbar it is the active one
    const named = Number(new URLSearchParams(location.search).get('tab'));
    tabId = Number.isSafeInteger(named) && named > 0 ? named : ((await chrome.tabs.query({ active: true, currentWindow: true }))[0]?.id ?? null);
    const state = tabId === null ? { ok: false, why: 'not-a-page' } : await chrome.runtime.sendMessage({ kind: 'state', tabId });
    origin = state?.origin ?? null;
    host = origin ? new URL(origin).host : '';
    if (state?.why === 'not-a-page') {
        main.append(paragraph(say('not_a_page')));
    } else if (!state?.ok) {
        main.append(paragraph(failure(state ?? {})));
    } else if (state.vault === 'open') {
        renderOpen(state);
        await renderButtonSwitch();
    } else {
        renderClosed(state.vault);
    }
}

start();
