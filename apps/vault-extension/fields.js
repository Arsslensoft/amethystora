// Which fields of a page are login and one-time-code fields, and writing into them. It runs in the extension's own isolated world,
// where the page's scripts cannot reach it, loaded by content.js's registration and by background.js for the popup. It keeps no
// state and starts nothing: on its own it only defines this object.
//
// `vaultFields` is assigned, never tested first: a page's <div id="vaultFields"> is a property of this window as well.
globalThis.vaultFields = (() => {
    'use strict';

    const TEXT_TYPES = new Set(['text', 'email', 'tel', 'number', 'password']);
    const CODE_WORDS = new Set(['otp', 'totp', 'hotp', '2fa', 'tfa', 'mfa', 'code', 'passcode', 'onetimecode', 'authcode', 'verifycode', 'verificationcode']);
    // A field called a code that has nothing to do with a login
    const NOT_CODES = new Set(['zip', 'postal', 'post', 'postcode', 'promo', 'coupon', 'discount', 'country', 'area', 'phone', 'gift', 'voucher', 'referral', 'invite', 'security', 'cvv', 'cvc', 'card', 'captcha', 'sort', 'tax', 'qr', 'bar', 'barcode']);
    const USER_WORDS = new Set(['user', 'username', 'userid', 'login', 'email', 'mail', 'account', 'identifier']);
    const MOST_INPUTS = 200;
    const MAX_USER = 256;

    const words = (field) => `${field.name} ${field.id}`.replace(/([a-z0-9])([A-Z])/g, '$1 $2').toLowerCase().split(/[^a-z0-9]+/).filter(Boolean);
    const autocomplete = (field) => (field.getAttribute('autocomplete') ?? '').toLowerCase().split(/\s+/);

    // What a page can hide a field by, so that nothing is ever written where a person cannot see it: not drawn, a fraction of an
    // opacity along the way, or put off the page's edge
    function visible(field) {
        if (!field?.isConnected || !field.checkVisibility({ checkOpacity: true, checkVisibilityCSS: true, opacityProperty: true, visibilityProperty: true })) {
            return false;
        }
        const box = field.getBoundingClientRect();
        if (box.width < 8 || box.height < 8 || box.right <= 0 || box.bottom + scrollY <= 0) {
            return false;
        }
        let opacity = 1;
        for (let element = field; element; element = element.parentElement) {
            opacity *= Number(getComputedStyle(element).opacity);
        }
        return opacity >= 0.5;
    }

    function isCode(field) {
        if (autocomplete(field).includes('one-time-code')) {
            return true;
        }
        const longest = field.maxLength;
        if (field.getAttribute('inputmode') === 'numeric' && longest >= 6 && longest <= 8) {
            return true;
        }
        if (longest > 10 || field.type === 'password') {
            return false;
        }
        const named = words(field);
        return named.some((word) => CODE_WORDS.has(word)) && !named.some((word) => NOT_CODES.has(word));
    }

    function kindOf(field) {
        if (field?.localName !== 'input' || field.disabled || field.readOnly || !TEXT_TYPES.has(field.type)) {
            return null;
        }
        if (isCode(field)) {
            return 'code';
        }
        if (field.type === 'password') {
            return 'login';
        }
        if (field.type === 'number' || field.type === 'tel') {
            return null;
        }
        const tokens = autocomplete(field);
        return tokens.includes('username') || tokens.includes('email') || words(field).some((word) => USER_WORDS.has(word)) ? 'login' : null;
    }

    // The visible inputs of the form this field is in, or of its document or shadow root, in order
    function neighbours(field) {
        const root = field.form ?? field.getRootNode();
        return [...root.querySelectorAll('input')].slice(0, MOST_INPUTS).filter(visible);
    }

    // The getters the page cannot shadow: a form with an input named `action` or `elements` answers `form.action` with that input
    const getter = (prototype, name) => Object.getOwnPropertyDescriptor(prototype, name).get;
    const formAction = getter(HTMLFormElement.prototype, 'action');
    const formElements = getter(HTMLFormElement.prototype, 'elements');
    const submitAction = [HTMLInputElement, HTMLButtonElement].map((type) => getter(type.prototype, 'formAction'));
    const MOST_ACTIONS = 8;

    // Where the forms of these fields, and the buttons in them, send what is typed: the origins the browser resolved them to. What is
    // not an http or https address is not a place a password goes; what cannot be read is said so, and the host refuses it.
    function actionsOf(fields) {
        const origins = new Set();
        const note = (read) => {
            try {
                const url = new URL(read());
                if (url.protocol === 'https:' || url.protocol === 'http:') {
                    origins.add(url.origin);
                }
            } catch {
                origins.add('unreadable');
            }
        };
        for (const form of new Set(fields.map((field) => field?.form).filter(Boolean))) {
            note(() => formAction.call(form));
            for (const element of [...formElements.call(form)].slice(0, MOST_INPUTS)) {
                if (element.hasAttribute('formaction')) {
                    note(() => (element instanceof HTMLInputElement ? submitAction[0] : submitAction[1]).call(element));
                }
            }
        }
        return origins.size > MOST_ACTIONS ? ['many'] : [...origins];
    }

    function put(field, text) {
        field.focus();
        field.value = text;
        field.dispatchEvent(new Event('input', { bubbles: true, composed: true }));
        field.dispatchEvent(new Event('change', { bubbles: true }));
    }

    function isPassword(field) {
        return field.type === 'password' && kindOf(field) === 'login';
    }

    function loginFields(field) {
        const all = neighbours(field);
        const passwordField = isPassword(field) ? field : all.find(isPassword);
        const before = passwordField ? all.slice(0, Math.max(all.indexOf(passwordField), 0)) : all;
        const isUser = (input) => input.type !== 'password' && kindOf(input) === 'login';
        const userField = isUser(field) ? field : before.filter(isUser).at(-1);
        return { userField, passwordField };
    }

    function fillLogin(field, user, password) {
        const { userField, passwordField } = loginFields(field);
        if (userField && user && visible(userField)) {
            put(userField, user);
        }
        if (passwordField && visible(passwordField)) {
            put(passwordField, password);
        }
    }

    // A code goes into its field, or a digit each into a row of one-character boxes
    function fillCode(field, code) {
        if (field.maxLength === 1) {
            const boxes = neighbours(field).filter((input) => input.maxLength === 1);
            if (boxes.length >= code.length) {
                [...code].forEach((digit, index) => put(boxes[index], digit));
                return;
            }
        }
        put(field, code);
    }

    // The fields a new password goes into: new-password ones if the page says which, otherwise all but the current one
    function newPasswordFields(field) {
        const passwords = neighbours(field).filter(isPassword);
        const marked = passwords.filter((input) => autocomplete(input).includes('new-password'));
        return marked.length ? marked : passwords.filter((input) => !autocomplete(input).includes('current-password'));
    }

    function fillNew(field, password) {
        newPasswordFields(field).forEach((input) => put(input, password));
    }

    function focused() {
        let active = document.activeElement;
        while (active?.shadowRoot?.activeElement) {
            active = active.shadowRoot.activeElement;
        }
        return active;
    }

    // The field the popup means: the focused one if it is of that kind, otherwise the first that is
    function target(kind) {
        const active = focused();
        if (kindOf(active) === kind && visible(active)) {
            return active;
        }
        return [...document.querySelectorAll('input')].slice(0, MOST_INPUTS).find((input) => kindOf(input) === kind && visible(input)) ?? null;
    }

    return {
        kindOf,
        visible,
        fillLogin,
        fillCode,
        fillNew,
        newPasswordFields,
        focused,

        // What the popup may offer in this frame. It reads no password, only whether one is there, and the username to be saved with it.
        survey() {
            const login = target('login');
            const found = login ? loginFields(login) : {};
            const code = target('code');
            const lengths = found.passwordField ? newPasswordFields(found.passwordField).map((input) => input.maxLength).filter((length) => length > 0) : [];
            return {
                // `self.origin` is the document's own, which is "null" under a sandbox that takes it away; `location` still names the site
                origin: self.origin,
                login: login !== null,
                code: code !== null,
                password: Boolean(found.passwordField),
                typed: Boolean(found.passwordField?.value),
                user: (found.userField?.value ?? '').slice(0, MAX_USER),
                length: lengths.length ? Math.min(...lengths) : 0,
                actions: { login: actionsOf([login, found.userField, found.passwordField]), code: actionsOf([code]) },
            };
        },

        // Each fill looks again at the last moment, so that a page that moved on during a prompt, or whose form now posts elsewhere,
        // gets nothing
        fromPopup(expected, what, values, actions) {
            if (self.origin !== expected) {
                return false;
            }
            const field = target(what === 'code' ? 'code' : 'login');
            if (!field) {
                return false;
            }
            const found = what === 'code' ? { passwordField: null } : loginFields(field);
            const now = what === 'code' ? actionsOf([field]) : actionsOf([field, found.userField, found.passwordField]);
            if (JSON.stringify(now) !== JSON.stringify(actions)) {
                return false;
            }
            if (what === 'login') {
                fillLogin(field, values.user, values.password);
            } else if (what === 'code') {
                fillCode(field, values.code);
            } else {
                fillNew(field, values.password);
            }
            return true;
        },

        // On Save, and only then, what the page's login form holds
        credentials(expected) {
            const field = self.origin === expected ? target('login') : null;
            const found = field ? loginFields(field) : {};
            return found.passwordField?.value ? { user: found.userField?.value ?? '', password: found.passwordField.value } : null;
        },
    };
})();
