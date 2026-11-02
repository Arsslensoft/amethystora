// Show me, on my screen (plans/ui-ideas/r7/spec-help.md, R7-61): the Manual's Show me asks here, over D-Bus, to walk one of
// the guides in guide/guides.js: a ring round the real thing and a card that says what to do (guide/walk.js). While the key
// is off nothing is on the bus and the Manual's link says so; while no guide runs nothing runs but the key's signal.
//
// What a caller passes is only ever a guide's id: the steps are in the image, and a guide presses nothing.
//
// Key of org.gnome.shell.extensions.amethystora-ui.guide, read live: show-me.

import Gio from 'gi://Gio';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {guides} from './guide/guides.js';
import {Walk} from './guide/walk.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.guide';
const PATH = '/org/amethystora/Guide';
const INTERFACE = `
<node>
  <interface name="org.amethystora.Guide">
    <method name="Start">
      <arg type="s" direction="in" name="id"/>
      <arg type="b" direction="out" name="started"/>
    </method>
    <method name="Stop"/>
  </interface>
</node>`;

export class Guide {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._settings = this._ext.getSettings(SCHEMA);
        this._settings.connectObject('changed::show-me', () => this._apply(), this);
        this._apply();
    }

    disable() {
        this._settings?.disconnectObject(this);
        this._leave();
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
    }

    _apply() {
        const wanted = this._settings.get_boolean('show-me');
        if (wanted && !this._dbus) {
            this._dbus = Gio.DBusExportedObject.wrapJSObject(INTERFACE, this);
            this._dbus.export(Gio.DBus.session, PATH);
        } else if (!wanted) {
            this._leave();
        }
    }

    _leave() {
        this._walk?.end();
        this._walk = null;
        this._dbus?.unexport();
        this._dbus = null;
    }

    // ---- D-Bus ----

    Start(id) {
        const guide = guides(_).find(one => one.id === id);
        if (!guide)
            return false;
        // The Manual, put out of the way for the guide before this one, stays so: this one brings it back
        this._walk?.end(false);
        this._walk = new Walk(guide, () => {
            this._walk = null;
        });
        return true;
    }

    Stop() {
        this._walk?.end();
    }

    // ---- For the tests ----

    snapshot() {
        return {exported: !!this._dbus, walking: !!this._walk};
    }
}
