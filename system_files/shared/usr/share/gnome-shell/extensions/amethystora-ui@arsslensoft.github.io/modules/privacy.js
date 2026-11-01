// What the desktop keeps to itself, and what it shows of the machine's protection (plans/beyond/spec-r4-privacy.md):
//
//  - the share guard: while the whole screen is shared, the windows of chosen apps (Notes, Security, Backups, password
//    managers) and Files windows in the Private folder are covered, blurred or with a solid card (privacy/guard.js);
//  - the shield: a tile in quick settings, or a mark in the top bar, that says how the machine's protection stands and
//    opens Security for every line of it; it switches nothing (privacy/shield.js);
//  - network by app, offered and off: the network meter's menu lists which apps are talking to the network, and to
//    where, only while it is open (privacy/connections.js);
//  - walk-away lock, a security switch (WALK_AWAY in /etc/amethystora/security.conf, off): the screen locks when the
//    paired phone has left and the computer is left alone; it never unlocks (privacy/away.js);
//  - a new network's zone: the first time a network is joined, a notification names its firewall zone and what that
//    means, with Open Security; it switches nothing (privacy/zone.js).
//
// Each part exists only while its key (or, for walk-away lock, a phone leaving) asks for it, and idles on signals: no
// timer runs while nothing happens. The keys are org.gnome.shell.extensions.amethystora-ui.privacy, written by ame
// desktop share-guard, shield, network-apps and zone-notice, Control and the setup file.
//
// For the VM suite: state(), and the parts by their getters.

import {Away} from './privacy/away.js';
import {Connections} from './privacy/connections.js';
import {Guard} from './privacy/guard.js';
import {Shield} from './privacy/shield.js';
import {Zone} from './privacy/zone.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.privacy';

export class Privacy {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._settings = this._ext.getSettings(SCHEMA);
        this._settings.connectObject(
            'changed::share-guard', () => this._part('the share guard', () => this._syncGuard()),
            'changed::share-guard-apps', () => this._part('the share guard', () => this._guard?.appsChanged()),
            'changed::share-guard-private', () => this._part('the share guard', () => this._guard?.appsChanged()),
            'changed::shield', () => this._part('the shield', () => this._syncShield()),
            'changed::network-apps', () => this._part('network by app', () => this._syncConnections()),
            'changed::zone-notice', () => this._part('the zone notice', () => this._syncZone()),
            this);
        // Each part on its own: one that fails leaves the others running, the share guard above all
        this._part('the share guard', () => this._syncGuard());
        this._part('the shield', () => this._syncShield());
        this._part('network by app', () => this._syncConnections());
        this._part('the zone notice', () => this._syncZone());
        this._part('walk-away lock', () => {
            this._away = new Away(this._ext);
            this._away.start();
        });
    }

    disable() {
        this._settings?.disconnectObject(this);
        // The share guard first, so that its covers and the compositor's unredirection go whatever the others do
        this._part('the share guard', () => this._guard?.stop());
        this._guard = null;
        this._part('walk-away lock', () => this._away?.stop());
        this._away = null;
        this._part('the zone notice', () => this._zone?.stop());
        this._zone = null;
        this._part('network by app', () => this._connections?.stop());
        this._connections = null;
        this._part('the shield', () => this._shield?.stop());
        this._shield = null;
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
    }

    _part(name, run) {
        try {
            run();
        } catch (e) {
            console.error(`Amethystora UI: ${name} failed: ${e.message}
${e.stack}`);
        }
    }

    _syncGuard() {
        const look = this._settings.get_string('share-guard');
        if (look === 'off') {
            this._guard?.stop();
            this._guard = null;
            return;
        }
        if (!this._guard) {
            this._guard = new Guard(this._ext, this._settings);
            this._guard.start();
        }
        this._guard.setLook(look);
    }

    // Whether the share guard covers the window (asked by the glass)
    covers(window) {
        return this._guard?.covers(window) ?? false;
    }

    _syncShield() {
        const mode = this._settings.get_string('shield');
        if (this._shield?.mode === mode)
            return;
        this._shield?.stop();
        this._shield = null;
        if (mode === 'off')
            return;
        this._shield = new Shield(this._ext, mode);
        this._shield.start();
    }

    _syncConnections() {
        const on = this._settings.get_boolean('network-apps');
        if (on && !this._connections) {
            this._connections = new Connections(this._ext);
            this._connections.start();
        } else if (!on && this._connections) {
            this._connections.stop();
            this._connections = null;
        }
    }

    _syncZone() {
        const on = this._settings.get_boolean('zone-notice');
        if (on && !this._zone) {
            this._zone = new Zone(this._ext);
            this._zone.start();
        } else if (!on && this._zone) {
            this._zone.stop();
            this._zone = null;
        }
    }

    // --- For the VM suite ---------------------------------------------------------------------------------------------

    state() {
        return {
            guard: this._guard?.state() ?? null,
            shield: this._shield?.state() ?? null,
            connections: this._connections?.state() ?? null,
            away: this._away?.state() ?? null,
            zone: this._zone?.state() ?? null,
        };
    }

    get guard() {
        return this._guard;
    }

    get shield() {
        return this._shield;
    }

    get connections() {
        return this._connections;
    }

    get away() {
        return this._away;
    }

    get zone() {
        return this._zone;
    }
}
