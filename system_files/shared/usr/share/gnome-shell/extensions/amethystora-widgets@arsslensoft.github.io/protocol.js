// What a widget is made of, read the same way by the top bar (extension.js) and by
// `amethystora-widgets run` (check.js), so that what the check accepts is exactly what the bar shows.
// Plain JavaScript with no GNOME imports. The agent skill amethystora-widgets documents every field
// here for the people and agents who write widgets: keep the two in step.
//
// widget.json: {"name", "description", "command", "interval", "position", "order", "place", "size"}
// "place" is "bar" (the top bar, the default) or "desktop" (a card on the wallpaper, drawn by Amethystora UI's
// deskwidgets module while widgets on the desktop are on); a desktop widget's "position" is a corner and its
// "size" says how large its first line is.
// What the command prints, each update on one line: plain text, or
//   {"text", "icon", "class", "click", "hidden", "menu": [{"label", "icon", "run", "open", "toggle"} | {"separator": true}]}

export const POSITIONS = ['left', 'center', 'right'];
export const PLACES = ['bar', 'desktop'];
export const CORNERS = ['top-left', 'top-right', 'bottom-left', 'bottom-right'];
export const SIZES = ['normal', 'large'];
export const CLASSES = ['accent', 'good', 'warning', 'critical'];

const MANIFEST_FIELDS = ['name', 'description', 'command', 'interval', 'position', 'order', 'place', 'size'];
const OUTPUT_FIELDS = ['text', 'icon', 'class', 'click', 'hidden', 'menu'];
const ITEM_FIELDS = ['label', 'icon', 'run', 'open', 'toggle', 'separator'];

const isObject = value => typeof value === 'object' && value !== null && !Array.isArray(value);
const isText = value => typeof value === 'string' && value.trim() !== '';
// A command is a command line, split as a shell would split it but run without one, or its arguments
const isCommand = value => isText(value) ||
    (Array.isArray(value) && value.every(arg => typeof arg === 'string') && isText(value[0]));

// A field nobody reads is most often a typo, but it may be one a newer image knows: a warning
function unknownFields(object, known, where, warnings) {
    for (const key of Object.keys(object)) {
        if (!known.includes(key))
            warnings.push(`unknown field "${key}" in ${where}, ignored`);
    }
}

export function parseManifest(text) {
    const errors = [], warnings = [];
    let data;
    try {
        data = JSON.parse(text);
    } catch (e) {
        return {errors: [`widget.json is not JSON: ${e.message}`], warnings};
    }
    if (!isObject(data))
        return {errors: ['widget.json has to hold one object'], warnings};
    unknownFields(data, MANIFEST_FIELDS, 'widget.json', warnings);

    const place = data.place ?? 'bar';
    const desktop = place === 'desktop';
    const manifest = {
        name: data.name,
        description: data.description ?? '',
        command: data.command,
        interval: data.interval ?? 60,
        position: data.position ?? (desktop ? 'top-right' : 'right'),
        order: data.order ?? 0,
        place,
        size: data.size ?? 'normal',
    };
    if (!isText(manifest.name))
        errors.push('"name" has to be the name of the widget');
    if (typeof manifest.description !== 'string')
        errors.push('"description" has to be a string');
    if (!isCommand(manifest.command))
        errors.push('"command" has to be a command line, or the list of its arguments');
    if (!Number.isInteger(manifest.interval) || manifest.interval < 0)
        errors.push('"interval" has to be a whole number of seconds, or 0 for a command that keeps running');
    if (!PLACES.includes(place))
        errors.push(`"place" has to be one of ${PLACES.join(', ')}`);
    else if (desktop && !CORNERS.includes(manifest.position))
        errors.push(`"position" of a widget on the desktop has to be one of ${CORNERS.join(', ')}`);
    else if (!desktop && !POSITIONS.includes(manifest.position))
        errors.push(`"position" has to be one of ${POSITIONS.join(', ')}`);
    if (!SIZES.includes(manifest.size))
        errors.push(`"size" has to be one of ${SIZES.join(', ')}`);
    else if (!desktop && data.size !== undefined)
        warnings.push('"size" is read only on the desktop ("place": "desktop"), ignored');
    if (!Number.isInteger(manifest.order))
        errors.push('"order" has to be a whole number');
    return {manifest: errors.length ? undefined : manifest, errors, warnings};
}

// The update in what one run printed: all of it when it is one JSON object over several lines, as jq
// prints without -c, and otherwise the last line, so that anything printed before it is ignored
export function lastUpdate(stdout) {
    const output = stdout.trim();
    try {
        if (isObject(JSON.parse(output)))
            return output;
    } catch {}
    return output.split('\n').filter(line => line.trim()).pop() ?? '';
}

export function parseOutput(text) {
    const errors = [], warnings = [];
    text = text.trim();
    if (!text)
        return {errors: ['it printed nothing; print {"hidden": true} to hide the widget'], warnings};
    if (!text.startsWith('{'))
        return {state: {text, icon: '', class: '', click: null, hidden: false, menu: []}, errors, warnings};

    let data;
    try {
        data = JSON.parse(text);
    } catch (e) {
        return {errors: [`what it printed is not JSON (${e.message}); print each update on one line, with jq -c`], warnings};
    }
    if (!isObject(data))
        return {errors: ['what it printed has to be one JSON object'], warnings};
    unknownFields(data, OUTPUT_FIELDS, 'the output', warnings);

    const state = {
        text: data.text ?? '',
        icon: data.icon ?? '',
        class: data.class ?? '',
        click: data.click ?? null,
        hidden: data.hidden ?? false,
        menu: data.menu ?? [],
    };
    if (typeof state.text !== 'string')
        errors.push('"text" has to be a string');
    if (typeof state.icon !== 'string')
        errors.push('"icon" has to be the name of an icon, or the path to a picture');
    if (state.class !== '' && !CLASSES.includes(state.class))
        errors.push(`"class" has to be one of ${CLASSES.join(', ')}`);
    if (state.click !== null && !isCommand(state.click))
        errors.push('"click" has to be a command');
    if (typeof state.hidden !== 'boolean')
        errors.push('"hidden" has to be true or false');
    if (!Array.isArray(state.menu))
        errors.push('"menu" has to be a list');
    else
        state.menu.forEach((item, i) => checkItem(item, `menu item ${i + 1}`, errors, warnings));
    if (!errors.length && !state.hidden && !state.text && !state.icon)
        warnings.push('there is neither "text" nor "icon", so the bar shows an empty button');
    return {state: errors.length ? undefined : state, errors, warnings};
}

function checkItem(item, where, errors, warnings) {
    if (!isObject(item)) {
        errors.push(`${where} has to be an object`);
        return;
    }
    unknownFields(item, ITEM_FIELDS, where, warnings);
    if (item.separator === true)
        return;
    if (typeof item.label !== 'string')
        errors.push(`${where} needs a "label", or "separator": true`);
    if (item.icon !== undefined && typeof item.icon !== 'string')
        errors.push(`"icon" of ${where} has to be the name of an icon, or the path to a picture`);
    if (item.run !== undefined && !isCommand(item.run))
        errors.push(`"run" of ${where} has to be a command`);
    if (item.open !== undefined && !isText(item.open))
        errors.push(`"open" of ${where} has to be an address or a path`);
    if (item.run !== undefined && item.open !== undefined)
        errors.push(`${where} has both "run" and "open": it can only do one`);
    if (item.toggle !== undefined && (typeof item.toggle !== 'boolean' || item.run === undefined))
        errors.push(`"toggle" of ${where} has to be true or false, with a "run" that flips it`);
}
