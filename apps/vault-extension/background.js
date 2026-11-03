// Everything between a page and the native host goes through here, and every origin the host is told is the browser's own word:
// the tab's URL for the popup. Nothing a page or a content script says about where it is reaches the host. Nothing is kept here
// either: no storage permission, and a secret lives only as long as the call it is passing through.
'use strict';

const HOST = 'org.amethystora.vault';
const SCRIPT = 'vault-fields';
const POPUP = chrome.runtime.getURL('popup.html');
const REFUSED = { ok: false, why: 'refused' };
// One site, as the popup asks for it: a scheme, a host and `/*`
const SITE = /^https?:\/\/[^*/]+\/\*$/;

// One message over one port: a port keeps Chromium's service worker and Firefox's event page alive while a person answers a prompt,
// and the host exits as it closes
function ask(message) {
    return new Promise((resolve) => {
        let answered = false;
        const port = chrome.runtime.connectNative(HOST);
        port.onMessage.addListener((answer) => {
            answered = true;
            resolve(answer);
            port.disconnect();
        });
        port.onDisconnect.addListener(() => {
            if (!answered) {
                resolve({ ok: false, why: 'no-host' });
            }
        });
        port.postMessage({ v: 1, ...message });
    });
}

function originOf(url) {
    try {
        const { protocol, origin } = new URL(url);
        return protocol === 'https:' || protocol === 'http:' ? origin : null;
    } catch {
        return null;
    }
}

// --- The button in a field: it only opens the popup --------------------------------------------------------------------------------

// The in-page button never releases a secret and shows nothing of the vault: a click on it opens the extension's own popup, which a page
// can neither cover, move nor restyle, and the fill is a click there. Where the browser will not open the toolbar popup from here, the
// same page opens in a small window of the extension's own, for the tab that asked.
async function showPopup(tab) {
    try {
        await chrome.action.openPopup({ windowId: tab.windowId });
    } catch {
        await chrome.windows.create({ url: `${POPUP}?tab=${tab.id}`, type: 'popup', width: 380, height: 560 });
    }
    return { ok: true };
}

// --- The popup: the tab it was opened on, which activeTab (or the site's grant) lets it look into ----------------------------------

async function tabOrigin(tabId) {
    const tab = await chrome.tabs.get(tabId);
    return tab.url ? originOf(tab.url) : null;
}

// fields.js in every frame the browser lets it into, then what each can offer. A frame of another origin is left out, and so is a
// document whose own origin is not the page's, such as one under a sandbox: it says "null", whatever its address.
async function survey(tabId, expected) {
    const target = { tabId, allFrames: true };
    await chrome.scripting.executeScript({ target, files: ['fields.js'] });
    const results = await chrome.scripting.executeScript({ target, func: () => globalThis.vaultFields.survey() });
    return results.filter(({ result }) => result?.origin === expected).map(({ frameId, result }) => ({ ...result, frameId }));
}

function into(tabId, frameId, func, args) {
    return chrome.scripting.executeScript({ target: { tabId, frameIds: [frameId] }, func, args }).then(([first]) => first?.result);
}

// A username the popup shows or saves: no control or direction-changing characters, a bounded length
const MOST_TEXT = 256;
const clean = (text) => (typeof text === 'string' ? text.replace(/[\p{Cc}\p{Cf}\p{Zl}\p{Zp}]/gu, '').slice(0, MOST_TEXT) : '');

async function popupState(tabId) {
    const origin = await tabOrigin(tabId);
    if (!origin) {
        return { ok: false, why: 'not-a-page' };
    }
    let frames = null;
    try {
        frames = await survey(tabId, origin);
    } catch {
        // A page no extension may touch, such as a store's
    }
    const actions = frames ? [...new Set(frames.flatMap((frame) => [...frame.actions.login, ...frame.actions.code]))] : undefined;
    const state = await ask({ op: 'state', origin, top: origin, actions });
    if (!state.ok || state.vault !== 'open') {
        return { ...state, origin };
    }
    if (!frames) {
        return { ...state, origin, fields: null };
    }
    const any = (what) => frames.some((frame) => frame[what]);
    const top = frames.find((frame) => frame.frameId === 0);
    return { ...state, origin, fields: { login: any('login'), code: any('code'), password: any('password'), typed: top?.typed ?? false, user: clean(top?.user) } };
}

const FILL = (expected, what, values, actions) => globalThis.vaultFields.fromPopup(expected, what, values, actions);

// Fills after the host answered, which may have been a person answering a prompt: the frame is looked for again, and the tab has to
// be on the origin the person saw. The host is told where the form posts, and refuses a form that posts to another site.
async function popupFill(tabId, expected, what, request) {
    if ((await tabOrigin(tabId)) !== expected) {
        return REFUSED;
    }
    const need = what === 'code' ? 'code' : what === 'login' ? 'login' : 'password';
    const frame = (await survey(tabId, expected)).find((candidate) => candidate[need]);
    if (!frame) {
        return { ok: false, why: 'refused', problem: 'no-field' };
    }
    const actions = frame.actions[what === 'code' ? 'code' : 'login'];
    // A new password has the length the page's fields take, if that is one a password can have
    const length = what === 'new' && frame.length >= 8 && frame.length <= 128 ? frame.length : undefined;
    const answer = await ask({ ...request, length, actions, origin: expected, top: expected });
    if (!answer.ok) {
        return answer;
    }
    if ((await tabOrigin(tabId)) !== expected) {
        return REFUSED;
    }
    const filled = await into(tabId, frame.frameId, FILL, [expected, what, answer, actions]);
    return filled ? { ok: true } : REFUSED;
}

// The only way a typed secret goes to the host: the person clicked Save, having seen the username and the site
async function popupSave(tabId, expected, user) {
    if ((await tabOrigin(tabId)) !== expected) {
        return REFUSED;
    }
    await chrome.scripting.executeScript({ target: { tabId, frameIds: [0] }, files: ['fields.js'] });
    const found = await into(tabId, 0, (origin) => globalThis.vaultFields.credentials(origin), [expected]);
    if (!found) {
        return { ok: false, why: 'refused', problem: 'no-field' };
    }
    return ask({ op: 'save', origin: expected, top: expected, user: clean(user), password: found.password });
}

function fromPopup(message) {
    const { tabId, expected } = message;
    if (!Number.isSafeInteger(tabId) || (message.kind !== 'state' && typeof expected !== 'string')) {
        return Promise.resolve(REFUSED);
    }
    switch (message.kind) {
        case 'state':
            return popupState(tabId);
        case 'fill-login':
            return popupFill(tabId, expected, 'login', { op: 'login', id: message.id });
        case 'fill-code':
            // The only place a person picks an entry that does not match the page, having been shown the domain
            return popupFill(tabId, expected, 'code', { op: 'code', id: message.id, picked: message.picked === true });
        case 'fill-new':
            return popupFill(tabId, expected, 'new', { op: 'generate', user: clean(message.user) });
        case 'save':
            return popupSave(tabId, expected, message.user);
        case 'open':
            return ask({ op: 'open', origin: expected, top: expected });
        default:
            return Promise.resolve(REFUSED);
    }
}

// No externally_connectable and no web_accessible_resources, so nothing of a page's can get here; this only tells the extension's own
// parts apart. A content script's `sender.url` is its page's, so it cannot pass for the popup, which may also be in a window of its
// own (and then has a tab too) with the tab it is for in its address.
chrome.runtime.onMessage.addListener((message, sender, respond) => {
    if (sender.id !== chrome.runtime.id || typeof message?.kind !== 'string') {
        respond(REFUSED);
        return false;
    }
    let answer = Promise.resolve(REFUSED);
    if (sender.url?.split('?')[0] === POPUP) {
        answer = fromPopup(message);
    } else if (sender.tab && message.kind === 'open-popup') {
        answer = showPopup(sender.tab);
    }
    answer.catch(() => ({ ok: false, why: 'failed' })).then(respond);
    return true;
});

// --- The button in a site's fields: only on sites the person asked for it -------------------------------------------------------

// The sites granted are the browser's own record, so that nothing has to be kept here
async function register() {
    const { origins = [] } = await chrome.permissions.getAll();
    const sites = origins.filter((pattern) => SITE.test(pattern));
    const had = await chrome.scripting.getRegisteredContentScripts({ ids: [SCRIPT] });
    if (had.length) {
        await chrome.scripting.unregisterContentScripts({ ids: [SCRIPT] });
    }
    if (sites.length) {
        await chrome.scripting.registerContentScripts([{ id: SCRIPT, matches: sites, js: ['fields.js', 'content.js'], runAt: 'document_idle', allFrames: true }]);
    }
}

// A page already open gets it too, rather than only after a reload. Only the sites register() takes, never a browser's "all sites".
async function added({ origins = [] }) {
    await register();
    const sites = origins.filter((pattern) => SITE.test(pattern));
    for (const tab of sites.length ? await chrome.tabs.query({ url: sites }) : []) {
        chrome.scripting.executeScript({ target: { tabId: tab.id, allFrames: true }, files: ['fields.js', 'content.js'] }).catch(() => {});
    }
}

chrome.runtime.onInstalled.addListener(register);
chrome.runtime.onStartup.addListener(register);
chrome.permissions.onAdded.addListener(added);
chrome.permissions.onRemoved.addListener(register);
