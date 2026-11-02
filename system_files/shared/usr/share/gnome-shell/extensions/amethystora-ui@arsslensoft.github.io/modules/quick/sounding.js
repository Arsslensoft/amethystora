// What the sound server says of each app's stream, as data: nothing of the shell here, so that
// build_files/shared/test-devices.mjs reads the same code under node. From `pactl -f json list sink-inputs` it keeps three
// things per stream and drops the rest, what is playing above all: whether it is paused, the sandboxed app it belongs to
// (the id the portal writes for a sandboxed client; a program outside a sandbox can set any property of its own, so it
// goes by its own say-so, as it did by the id it sets) and the program's name. Never a level: nothing here listens.

const APP_ID = /^[A-Za-z][A-Za-z0-9_-]*(\.[A-Za-z0-9_-]+)+$/;
const BINARY = /^[A-Za-z0-9._+-]{1,64}$/;

// A Map from the stream's number to {corked, app, binary}; empty for anything that is not pactl's list
export function parseSinkInputs(text) {
    const said = new Map();
    let list;
    try {
        list = JSON.parse(text);
    } catch {
        return said;
    }
    if (!Array.isArray(list))
        return said;
    for (const stream of list) {
        if (!Number.isInteger(stream?.index))
            continue;
        const properties = stream.properties ?? {};
        const app = properties['pipewire.access.portal.app_id'];
        const binary = properties['application.process.binary'];
        said.set(stream.index, {
            corked: stream.corked === true,
            app: APP_ID.test(app ?? '') ? app : null,
            binary: BINARY.test(binary ?? '') ? binary : null,
        });
    }
    return said;
}

// The apps as the rows show them: with the key on, what is sounding first; each part by name, as they always were
export function sorted(apps, sounding) {
    const byName = (a, b) => a.name.localeCompare(b.name) || a.key.localeCompare(b.key);
    return [...apps].sort(sounding ? (a, b) => Number(!!a.paused) - Number(!!b.paused) || byName(a, b) : byName);
}
