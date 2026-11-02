// The account's systemd manager tells a unit's changes only to a connection that subscribed, and a subscription is the
// connection's, not the caller's. Everyone in the shell who follows a unit takes one here and gives it back, so that the last
// to leave undoes it and none takes it from another.

import Gio from 'gi://Gio';

const SYSTEMD = 'org.freedesktop.systemd1';

let holders = 0;

function manager(method) {
    Gio.DBus.session.call(SYSTEMD, '/org/freedesktop/systemd1', `${SYSTEMD}.Manager`, method, null, null,
        Gio.DBusCallFlags.NO_AUTO_START, 2000, null, (connection, result) => {
            try {
                connection.call_finish(result);
            } catch {
                // Already subscribed by somebody else, or no manager to ask: nothing is lost
            }
        });
}

// Subscribes if nobody has; returns the function that gives it back
export function subscribeUnits() {
    if (holders++ === 0)
        manager('Subscribe');
    let released = false;
    return () => {
        if (released)
            return;
        released = true;
        if (--holders === 0)
            manager('Unsubscribe');
    };
}

// For the checks
export function subscribers() {
    return holders;
}
