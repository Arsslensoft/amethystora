// The phone in the pill: a call ringing on a paired phone (silence it, or answer with a text), and a link the phone
// shared. Both come from the phone link's module (modules/connect.js), which believes only the image's own program.
// A link is shown, never opened by itself: Open in browser checks it again and hands it to the browser; Not now
// leaves it in the notification list, so that nothing a phone sends is lost on the way.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import Pango from 'gi://Pango';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {buttonRow, cardButton, chip, retire} from './clock.js';
import {plain, shownAddress} from './text.js';

export class Phone {
    constructor(activities, notice, connect) {
        this._activities = activities;
        this._notice = notice;
        this._connect = connect;
    }

    enable() {
        this._call = null;
        this._link = null;
        this._listener = {
            call: (device, state, number, name) => this._onCall(device, state, number, name),
            url: (device, url) => this._onUrl(device, url),
        };
        this._connect.addListener(this._listener);
    }

    disable() {
        this._connect?.removeListener(this._listener);
        this._listener = null;
        // A link not answered yet waits in the list rather than going with the pill
        if (this._link)
            this._connect?.keepLink(this._link.device, this._link.url, true);
        this._endCall();
        this._endLink();
    }

    // --- The call -------------------------------------------------------------------------------------------------

    _onCall(device, state, number, name) {
        if (state === 'ringing') {
            this._endCall();
            const who = plain(name, 40) || plain(number, 40);
            this._call = new CallActivity(device, who, this);
            this._activities.set('call', this._call);
        } else if (this._call && (this._call.device === device || this._call.device?.id === device?.id)) {
            this._endCall();
        }
    }

    _endCall() {
        const call = this._call;
        this._call = null;
        if (call && this._activities.get('call') === call)
            this._activities.set('call', null);
        call?.destroy();
    }

    muteRinger(call) {
        try {
            const result = call.device.muteRinger?.();
            result?.catch?.(e => console.debug(`Amethystora UI: the phone did not take Mute ringer: ${e.message}`));
        } catch (e) {
            console.debug(`Amethystora UI: the phone did not take Mute ringer: ${e.message}`);
        }
    }

    replyWithText(call) {
        this._connect.messages(call.device);
    }

    // --- The link -------------------------------------------------------------------------------------------------

    _onUrl(device, url) {
        // Checked again here, whoever handed it over: one web address, http or https, nothing else
        const shown = this._connect.checkAddress(url) ? shownAddress(url) : null;
        if (!shown)
            return false;
        // A link not answered yet makes room for the new one in the notification list
        if (this._link)
            this._connect.keepLink(this._link.device, this._link.url, true);
        this._endLink();
        this._link = new LinkActivity(device, url, shown, this);
        this._activities.set('link', this._link);
        return true;
    }

    open(link) {
        const url = this._connect.checkAddress(link.url);
        this._endLink();
        if (!url)
            return;
        try {
            Gio.AppInfo.launch_default_for_uri(url, global.create_app_launch_context(0, -1));
            this._notice(_('Opened in the browser'), 'web-browser-symbolic', 3);
        } catch (e) {
            console.warn(`Amethystora UI: the link could not be opened: ${e.message}`);
        }
    }

    copy(link) {
        St.Clipboard.get_default().set_text(St.ClipboardType.CLIPBOARD, link.url);
    }

    notNow(link) {
        this._connect.keepLink(link.device, link.url, true);
        this._endLink();
    }

    _endLink() {
        const link = this._link;
        this._link = null;
        if (link && this._activities.get('link') === link)
            this._activities.set('link', null);
        link?.destroy();
    }

    get lastLink() {
        return this._link?.url ?? null;
    }
}

class CallActivity {
    constructor(device, who, phone) {
        this.id = 'call';
        this.plate = '';
        this.device = device;
        this._who = who;
        this._phone = phone;
    }

    get text() {
        return this._who ? _('%s is calling').format(this._who) : _('Your phone is ringing');
    }

    get accessible() {
        return this._who ? _('%s is calling your phone').format(this._who) : _('Your phone is ringing');
    }

    get row() {
        return {icon: 'call-start-symbolic', text: this.text};
    }

    pill() {
        if (!this._content) {
            this._content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-pill-call'});
            this._content.add_child(new St.Icon({
                style_class: 'amethystora-pill-call-icon',
                icon_name: 'call-start-symbolic',
                icon_size: 15,
                y_align: Clutter.ActorAlign.CENTER,
            }));
            const label = new St.Label({style_class: 'amethystora-pill-call-text', text: this.text, y_align: Clutter.ActorAlign.CENTER});
            label.clutter_text.ellipsize = Pango.EllipsizeMode.END;
            this._content.add_child(label);
            this._content.add_child(chip(_('Mute ringer'), () => this._phone.muteRinger(this)));
        }
        return this._content;
    }

    card(api) {
        const body = new St.BoxLayout({style_class: 'amethystora-pill-card-body', orientation: Clutter.Orientation.VERTICAL});
        const title = new St.Label({style_class: 'amethystora-pill-card-label', text: this.accessible});
        title.clutter_text.line_wrap = true;
        body.add_child(title);
        const text = new St.Label({
            style_class: 'amethystora-pill-card-text',
            text: _('From the paired phone. Answer on the phone; from here you can silence it or send a text back.'),
        });
        text.clutter_text.line_wrap = true;
        body.add_child(text);
        const buttons = [cardButton(_('Mute ringer'), () => this._phone.muteRinger(this), {primary: true})];
        if (this.device.launchable) {
            buttons.push(cardButton(_('Reply with a text'), () => {
                this._phone.replyWithText(this);
                api.close();
            }));
        }
        body.add_child(buttonRow(buttons));
        return body;
    }

    destroy() {
        retire(this._content);
        this._content = null;
    }
}

class LinkActivity {
    constructor(device, url, shown, phone) {
        this.id = 'link';
        this.plate = 'link';
        this.device = device;
        this.url = url;
        this._shown = shown;
        this._phone = phone;
        this._name = plain(device.name, 48) || _('Your phone');
    }

    get text() {
        return this._shown;
    }

    get accessible() {
        return _('A link from %s: %s').format(this._name, this._shown);
    }

    get row() {
        return {icon: 'emblem-web-symbolic', text: _('Link · %s').format(this._shown)};
    }

    pill() {
        if (!this._content) {
            this._content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-pill-link'});
            this._content.add_child(new St.Icon({
                style_class: 'amethystora-pill-link-icon',
                icon_name: 'amethystora-phone',
                fallback_icon_name: 'phone-symbolic',
                icon_size: 18,
                y_align: Clutter.ActorAlign.CENTER,
            }));
            const label = new St.Label({style_class: 'amethystora-pill-link-address', text: this._shown, y_align: Clutter.ActorAlign.CENTER});
            label.clutter_text.ellipsize = Pango.EllipsizeMode.MIDDLE;
            label.set_text_direction(Clutter.TextDirection.LTR);
            this._content.add_child(label);
            const open = chip(_('Open'), () => this._phone.open(this));
            open.add_style_class_name('amethystora-pill-small-button');
            open.add_style_class_name('amethystora-pill-small-button-primary');
            const later = chip(_('Not now'), () => this._phone.notNow(this));
            later.add_style_class_name('amethystora-pill-small-button');
            this._content.add_child(open);
            this._content.add_child(later);
        }
        return this._content;
    }

    card() {
        const body = new St.BoxLayout({style_class: 'amethystora-pill-card-body', orientation: Clutter.Orientation.VERTICAL});
        const title = new St.Label({style_class: 'amethystora-pill-card-label', text: _('%s sent a link').format(this._name)});
        title.clutter_text.ellipsize = Pango.EllipsizeMode.END;
        body.add_child(title);
        const address = new St.Label({style_class: 'amethystora-pill-card-text amethystora-pill-card-address', text: this._shown});
        address.clutter_text.line_wrap = true;
        address.clutter_text.line_wrap_mode = Pango.WrapMode.WORD_CHAR;
        address.set_text_direction(Clutter.TextDirection.LTR);
        body.add_child(address);
        body.add_child(buttonRow([
            cardButton(_('Open in browser'), () => this._phone.open(this), {primary: true}),
            cardButton(_('Copy'), () => this._phone.copy(this)),
            cardButton(_('Not now'), () => this._phone.notNow(this)),
        ]));
        return body;
    }

    destroy() {
        retire(this._content);
        this._content = null;
    }
}

