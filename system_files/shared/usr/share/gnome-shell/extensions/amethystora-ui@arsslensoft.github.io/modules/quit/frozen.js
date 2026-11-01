// GNOME's own "not responding" dialog, the one Mutter asks for over a window that stopped answering, in the image's
// look: the app's icon, "Notes is not responding", a sentence on what each choice costs, Wait first (and what Escape
// and Enter do), Force quit after it, and while the agentic features are on, Ask the agent why, which hands the app's
// process number (nothing the app wrote) to `amethystora-agent diagnose --frozen PID` and waits. GNOME's dialog is
// wrapped, never copied: its content and its buttons are made again by its own methods' callers here, and its answers
// are its own (_onWait, _onClose), so Mutter does what it always does with them.

import Clutter from 'gi://Clutter';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as CloseDialog from 'resource:///org/gnome/shell/ui/closeDialog.js';
import * as Util from 'resource:///org/gnome/shell/misc/util.js';

const AGENT = 'amethystora-agent';

// Whether an actor is inside a not-responding question (GNOME's `close-dialog`, which grabs the key focus into its window
// whenever Mutter asks it to): the only place the card and the list take the keyboard back from
export function inCloseDialog(actor) {
    for (let a = actor; a; a = a.get_parent()) {
        if (a.has_style_class_name?.('close-dialog'))
            return true;
    }
    return false;
}

// The window a dialog is about: GNOME keeps it in a field of its own, which its window property does not always reach
function windowOf(dialog) {
    return dialog._window ?? dialog.window;
}

export class FrozenDialog {
    // agentOn(): whether the agentic features are on now
    constructor(agentOn) {
        this._agentOn = agentOn;
    }

    enable() {
        const proto = CloseDialog.CloseDialog.prototype;
        this._original = {content: proto._createDialogContent, init: proto._initDialog};
        const self = this;
        const original = this._original;
        proto._createDialogContent = function () {
            return self._original ? self._content(this) : original.content.call(this);
        };
        proto._initDialog = function () {
            const fresh = !this._dialog;
            original.init.call(this);
            if (fresh && self._original)
                self._dress(this);
        };
    }

    disable() {
        if (!this._original)
            return;
        const proto = CloseDialog.CloseDialog.prototype;
        proto._createDialogContent = this._original.content;
        proto._initDialog = this._original.init;
        this._original = null;
    }

    // The words and the icon, in place of GNOME's title and description
    _content(dialog) {
        const app = Shell.WindowTracker.get_default().get_window_app(windowOf(dialog));
        const box = new St.BoxLayout({style_class: 'amethystora-quit-frozen-box', orientation: Clutter.Orientation.VERTICAL, x_expand: true});
        if (app)
            box.add_child(new St.Bin({style_class: 'amethystora-quit-confirm-icon', child: app.create_icon_texture(42), x_align: Clutter.ActorAlign.CENTER}));
        const name = app?.get_name() ?? windowOf(dialog).get_title() ?? '';
        const title = new St.Label({style_class: 'amethystora-quit-frozen-title', text: _('%s is not responding').format(name),
            x_align: Clutter.ActorAlign.CENTER});
        title.clutter_text.use_markup = false;
        title.clutter_text.line_wrap = true;
        box.add_child(title);
        const text = new St.Label({style_class: 'amethystora-quit-frozen-text', text: _('Waiting may be enough. Quitting loses what was not saved.'),
            x_align: Clutter.ActorAlign.CENTER});
        text.clutter_text.line_wrap = true;
        box.add_child(text);
        return box;
    }

    // Wait first, the safe answer to Escape and to Enter; Force quit after it; Ask the agent why while it is on
    _dress(dialog) {
        const layout = dialog._dialog;
        layout._dialog.add_style_class_name('amethystora-quit-frozen');
        layout.clearButtons();
        const wait = layout.addButton({label: _('Wait'), action: () => dialog._onWait(), key: Clutter.KEY_Escape});
        wait.add_style_class_name('amethystora-quit-button');
        wait.add_style_class_name('amethystora-quit-primary');
        const quit = layout.addButton({label: _('Force quit'), action: () => dialog._onClose()});
        quit.add_style_class_name('amethystora-quit-button');
        quit.add_style_class_name('amethystora-quit-bad');
        for (const button of [wait, quit])
            button.x_expand = false;
        layout.buttonLayout.layout_manager.homogeneous = false;
        layout.buttonLayout.x_align = Clutter.ActorAlign.CENTER;

        const pid = windowOf(dialog).get_pid();
        if (this._agentOn() && pid > 0) {
            const ask = new St.Button({style_class: 'amethystora-quit-ask', label: _('Ask the agent why'), can_focus: true,
                x_align: Clutter.ActorAlign.CENTER});
            ask.connect('clicked', () => {
                // Asked only while still on, and only with the number Mutter knows the app by
                if (this._agentOn())
                    Util.spawn([AGENT, 'diagnose', '--frozen', String(pid)]);
                dialog._onWait();
            });
            layout._dialog.add_child(ask);
        }
    }
}
