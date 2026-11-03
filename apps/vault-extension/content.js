// The small button in a login or one-time-code field, on a site the person asked for it. It costs nothing until a field takes the
// focus: two listeners, no scan of the page, no observer and no timer, and the host is not started by anything here.
//
// The button releases nothing and shows nothing of the vault. A click the browser marks as trusted only asks the background script
// to open the extension's own popup, which a page can neither cover, move nor restyle; the fill is a click there. So nothing a page
// does to the button, or around it, can make it give anything up.
(() => {
    'use strict';

    // Once per frame, though it may be injected again; a page's element named so is not `true`
    if (globalThis.vaultButton === true) {
        return;
    }
    globalThis.vaultButton = true;

    // Never in a frame of another origin: not even listening. background.js and the host each refuse it again.
    try {
        if (window !== window.top && window.top.location.origin !== location.origin) {
            return;
        }
    } catch {
        return;
    }

    const fields = globalThis.vaultFields;
    const say = (name, ...values) => chrome.i18n.getMessage(name, values);
    const SIZE = 22;
    const GEM = [
        ['34,88.3 128,43.3 128,107.6 92,124.8', '#dcbcff'],
        ['128,43.3 164,60.5 164,124.8 128,107.6', '#9148f0'],
        ['164,141.7 164,206 128,223.3 128,159', '#5d1ac4'],
        ['128,223.3 34,178.3 92,141.7 128,159', '#7429e0'],
        ['34,178.3 34,88.3 92,124.8 92,141.7', '#b37df9'],
        ['164,60.5 222,32.7 222,88.3 164,124.8', '#b37df9'],
        ['164,124.8 222,88.3 222,214.3 164,214.3', '#9148f0'],
    ];

    let shown = null;

    // Styles through the CSSOM alone: a page's Content-Security-Policy has no say over them, and every one is !important
    function styled(element, styles) {
        for (const [name, value] of Object.entries(styles)) {
            element.style.setProperty(name, value, 'important');
        }
        return element;
    }

    function gem() {
        const svgName = 'http://www.w3.org/2000/svg';
        const svg = document.createElementNS(svgName, 'svg');
        svg.setAttribute('viewBox', '34 32.7 188 190.5');
        svg.setAttribute('aria-hidden', 'true');
        styled(svg, { width: '14px', height: '14px', display: 'block' });
        for (const [points, fill] of GEM) {
            const polygon = document.createElementNS(svgName, 'polygon');
            polygon.setAttribute('points', points);
            polygon.setAttribute('fill', fill);
            svg.append(polygon);
        }
        return svg;
    }

    function place() {
        if (!shown) {
            return;
        }
        const box = shown.field.getBoundingClientRect();
        const rtl = getComputedStyle(shown.field).direction === 'rtl';
        styled(shown.host, { left: `${Math.round(rtl ? box.left + 6 : box.right - SIZE - 6)}px`, top: `${Math.round(box.top + (box.height - SIZE) / 2)}px` });
    }

    function hide() {
        if (shown) {
            shown.host.remove();
            removeEventListener('scroll', place, true);
            removeEventListener('resize', place);
            shown = null;
        }
    }

    // A click the browser made for a person, and nothing else: a page's synthetic one does not open a window
    function clicked(event) {
        if (event.isTrusted) {
            chrome.runtime.sendMessage({ kind: 'open-popup' }).catch(() => {});
        }
    }

    function show(field) {
        hide();
        const host = document.createElement('div');
        styled(host, { all: 'initial', position: 'fixed', inset: 'auto', margin: '0', padding: '0', border: '0', width: `${SIZE}px`, height: `${SIZE}px`, overflow: 'visible', background: 'transparent', 'z-index': '2147483647', display: 'block', opacity: '1', visibility: 'visible' });
        const shadow = host.attachShadow({ mode: 'closed' });
        const button = document.createElement('button');
        button.type = 'button';
        button.setAttribute('aria-label', say('fill_from_vault'));
        button.title = say('fill_from_vault');
        button.tabIndex = -1;
        styled(button, { display: 'flex', 'align-items': 'center', 'justify-content': 'center', width: `${SIZE}px`, height: `${SIZE}px`, margin: '0', padding: '0', border: '0', 'border-radius': '6px', background: 'rgba(145,72,240,0.2)', cursor: 'pointer' });
        button.append(gem());
        button.addEventListener('click', clicked);
        shadow.append(button);
        // Keeps the focus in the field, and the page out of what happens in here
        for (const type of ['pointerdown', 'mousedown']) {
            host.addEventListener(type, (event) => {
                event.preventDefault();
                event.stopPropagation();
            });
        }
        for (const type of ['pointerup', 'mouseup', 'click']) {
            host.addEventListener(type, (event) => event.stopPropagation());
        }
        // In the top layer, over whatever the page puts on top of its fields
        host.popover = 'manual';
        document.documentElement.append(host);
        host.showPopover();
        shown = { field, host };
        place();
        addEventListener('scroll', place, { capture: true, passive: true });
        addEventListener('resize', place, { passive: true });
    }

    function focusChanged() {
        const field = fields.focused();
        if (shown?.field === field) {
            return;
        }
        if (fields.kindOf(field) && fields.visible(field)) {
            show(field);
        } else {
            hide();
        }
    }

    document.addEventListener('focusin', focusChanged, { capture: true, passive: true });
    document.addEventListener('focusout', (event) => {
        // Focus going nowhere is the window losing it, and the button stays; anywhere else but the button, it goes
        if (shown && (event.relatedTarget ? event.relatedTarget !== shown.host : !shown.field.isConnected)) {
            hide();
        }
    }, { capture: true, passive: true });
    focusChanged();
})();
