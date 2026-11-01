// Amethystora's own, under the Apache License 2.0 (not part of Clipboard Indicator): the text actions of a history item
// (plans/ui-ideas/spec-tools.md, R6-13 and R6-14). An item's actions button opens "Make a new copy": the text as pretty
// JSON, in upper, lower or title case, with its spaces trimmed, on one line or with its web address decoded, each a
// new item of the history and what the clipboard holds, and a count of its characters and words. A text short enough
// can be shown as a QR code, which Amethystora UI's tools module draws (and hides while the screen is shared).
//
// Plain functions of the text already held: nothing is sent or run. Its words are in the image's own catalog,
// domain amethystora, and never set as markup.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Pango from 'gi://Pango';
import St from 'gi://St';
import Gettext from 'gettext';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as ModalDialog from 'resource:///org/gnome/shell/ui/modalDialog.js';

const Domain = Gettext.domain('amethystora');
const _ = Domain.gettext.bind(Domain);
const ngettext = Domain.ngettext.bind(Domain);

// Past this, only the count is offered: a change of a large text is not worth holding twice
const LONGEST = 1024 * 1024;
const UI = 'amethystora-ui@arsslensoft.github.io';

function locale() {
    const name = GLib.get_language_names_with_category('LC_CTYPE')[0] ?? 'C';
    const tag = name.split(/[.@]/)[0].replace('_', '-');
    return tag === 'C' || tag === 'POSIX' ? undefined : tag;
}

function lines(text) {
    return text.split('\n').length;
}

// Each action that changes the text: [id, label, detail, result]
export function actionsFor(text) {
    if (typeof text !== 'string' || text === '' || text.length > LONGEST)
        return [];
    const tag = locale();
    const found = [];
    const add = (id, label, result, detail = '') => {
        if (typeof result === 'string' && result !== '' && result !== text)
            found.push({id, label, detail, result});
    };
    const trimmed = text.trim();
    if (/^[[{]/.test(trimmed)) {
        try {
            const pretty = JSON.stringify(JSON.parse(trimmed), null, 2);
            const count = lines(pretty);
            add('json', _('Pretty JSON'), pretty, ngettext('%d line', '%d lines', count).format(count));
        } catch {}
    }
    add('upper', _('UPPER CASE'), text.toLocaleUpperCase(tag));
    add('lower', _('lower case'), text.toLocaleLowerCase(tag));
    add('title', _('Title Case'), text.toLocaleLowerCase(tag).replace(/(^|[^\p{L}\p{M}\p{N}'’])(\p{L})/gu,
        (all, before, letter) => before + letter.toLocaleUpperCase(tag)));
    add('trim', _('Trim spaces'), text.split('\n').map(line => line.replace(/[ \t ]+/g, ' ').trim()).join('\n')
        .replace(/^\n+|\n+$/g, ''));
    add('line', _('One line'), text.replace(/\s*\n\s*/g, ' ').trim());
    if (/%[0-9A-Fa-f]{2}/.test(text)) {
        add('url', _('Decode a web address'), text.replace(/(?:%[0-9A-Fa-f]{2})+/g, run => {
            try {
                return decodeURIComponent(run);
            } catch {
                return run;
            }
        }));
    }
    return found;
}

// "41 characters · 3 words", with the lines when there are more than one
export function countOf(text) {
    const characters = [...text].length;
    const words = (text.match(/\S+/gu) ?? []).length;
    const parts = [ngettext('%d character', '%d characters', characters).format(characters),
        ngettext('%d word', '%d words', words).format(words)];
    const count = lines(text);
    if (count > 1)
        parts.push(ngettext('%d line', '%d lines', count).format(count));
    return parts.join(' · ');
}

function toolsModule() {
    return Main.extensionManager.lookup(UI)?.stateObj?.modules?.get('tools') ?? null;
}

export const TextActionsDialog = GObject.registerClass(
class TextActionsDialog extends ModalDialog.ModalDialog {
    // onCopy(text) makes the new item; onClose() runs once it is closed whatever was picked
    _init(text, onCopy, onClose) {
        super._init({styleClass: 'ci-text-actions-dialog', destroyOnClose: true});
        this._onCopy = onCopy;
        this.connect('closed', () => onClose?.());

        const title = new St.Label({style_class: 'ci-text-actions-title', text: _('Make a new copy')});
        this.contentLayout.add_child(title);
        const first = (text.split('\n').find(line => line.trim()) ?? text).trim().slice(0, 200);
        const preview = new St.Label({style_class: 'ci-text-actions-preview', text: first});
        preview.clutter_text.use_markup = false;
        preview.clutter_text.ellipsize = Pango.EllipsizeMode.END;
        this.contentLayout.add_child(preview);

        const list = new St.BoxLayout({style_class: 'ci-text-actions-list', orientation: Clutter.Orientation.VERTICAL});
        this.textActions = actionsFor(text);
        for (const action of this.textActions)
            list.add_child(this._row(action.label, action.detail, () => this._copy(action.result)));
        const tools = toolsModule();
        if (tools?.canShowQrCode?.(text)) {
            list.add_child(this._row(_('Show as QR code'), '', () => {
                this.close();
                tools.showQrCode(text);
            }));
            this.qr = true;
        }
        this.contentLayout.add_child(list);
        const count = new St.Label({style_class: 'ci-text-actions-count', text: countOf(text)});
        this.contentLayout.add_child(count);

        this.setButtons([{label: _('Close'), action: () => this.close(), key: Clutter.KEY_Escape, default: true}]);
    }

    _row(label, detail, action) {
        const row = new St.BoxLayout({x_expand: true});
        const name = new St.Label({text: label, x_expand: true, y_align: Clutter.ActorAlign.CENTER});
        row.add_child(name);
        if (detail)
            row.add_child(new St.Label({style_class: 'ci-text-actions-detail', text: detail, y_align: Clutter.ActorAlign.CENTER}));
        const button = new St.Button({style_class: 'ci-text-actions-row', child: row, can_focus: true, x_expand: true});
        button.accessible_name = label;
        button.connect('clicked', action);
        return button;
    }

    _copy(result) {
        this.close();
        this._onCopy(result);
    }
});
