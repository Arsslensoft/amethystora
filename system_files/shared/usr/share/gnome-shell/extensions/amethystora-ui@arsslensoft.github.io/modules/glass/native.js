// The glass's paint in native code (apps/glass, AmethystoraGlass-1.typelib), when the image has it: effect.js then
// hands each pane's paint to it and keeps only what changes seldom. Without it, or with AMETHYSTORA_GLASS_JS set in
// GNOME Shell's environment, the glass is drawn by effect.js alone, as before, pixel for pixel the same.

import GLib from 'gi://GLib';

let Native = null;
if (!GLib.getenv('AMETHYSTORA_GLASS_JS')) {
    try {
        Native = (await import('gi://AmethystoraGlass?version=1')).default;
    } catch (e) {
        console.log(`Amethystora UI: the glass is drawn in JavaScript (${e.message})`);
    }
}

export default Native;
