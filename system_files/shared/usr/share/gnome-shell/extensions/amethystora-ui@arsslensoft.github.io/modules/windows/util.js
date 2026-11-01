// What the parts of the windows module share: motion that honours the slow-down factor, the Animations switch and
// reduce motion; labels that never read text as markup; and shortcuts written as the key caps a person presses.

import Clutter from 'gi://Clutter';
import Graphene from 'gi://Graphene';
import St from 'gi://St';

import {gettext as _, pgettext as C_} from 'resource:///org/gnome/shell/extensions/extension.js';

// The boards' curve for things that glide: cubic-bezier(.2, .8, .2, 1)
export const GLIDE = [0.2, 0.8, 0.2, 1];
// Reduce motion turns a move into a crossfade no longer than this, before the slow-down factor
const CROSSFADE = 150;

export const animationsOn = () => St.Settings.get().enable_animations;

// Whether the person asked for less motion (the comfort module, which also counts GNOME's switches)
export const reduced = ext => !!ext.modules?.get('comfort')?.reduceMotion;

// A plain label: what it shows is text, whatever a window's title or an app's name says
export function label(text, styleClass) {
    const widget = new St.Label({text: text ?? '', style_class: styleClass ?? ''});
    widget.clutter_text.use_markup = false;
    widget.clutter_text.ellipsize = 3; // Pango.EllipsizeMode.END
    return widget;
}

// Runs step(progress) on every frame for `duration` ms on a cubic curve, then done(). The duration follows the
// slow-down factor; with animations off it jumps to the end at once. Returns the timeline (stop() it to cancel; done
// is not called then), or null when it already finished.
export function glide(actor, duration, curve, step, done) {
    const time = Math.round(duration * St.Settings.get().slow_down_factor);
    if (!animationsOn() || time <= 0) {
        step(1);
        done?.();
        return null;
    }
    const timeline = new Clutter.Timeline({actor, duration: time});
    timeline.set_cubic_bezier_progress(new Graphene.Point({x: curve[0], y: curve[1]}),
        new Graphene.Point({x: curve[2], y: curve[3]}));
    timeline.connect('new-frame', () => step(timeline.get_progress()));
    timeline.connect('stopped', (t, finished) => {
        if (!finished)
            return;
        step(1);
        done?.();
    });
    timeline.start();
    return timeline;
}

// An actor's fade and move in one call: with reduce motion, everything but the opacity goes to its end at once and
// the opacity crossfades in at most 150 ms
export function ease(ext, actor, props, {duration, mode = Clutter.AnimationMode.EASE, onComplete} = {}) {
    if (reduced(ext)) {
        const {opacity, ...rest} = props;
        actor.remove_all_transitions();
        actor.set(rest);
        if (opacity === undefined || actor.opacity === opacity) {
            onComplete?.();
            return;
        }
        actor.ease({opacity, duration: Math.min(duration, CROSSFADE), mode: Clutter.AnimationMode.EASE, onComplete});
        return;
    }
    actor.ease({...props, duration, mode, onComplete});
}

// The words on the key caps of an accelerator as GSettings stores it ('<Super><Shift>v'), modifiers first in the
// order Super, Ctrl, Shift, Alt; null for an empty or unreadable one
export function caps(accelerator) {
    if (!accelerator)
        return null;
    const mods = new Set();
    const key = accelerator.replace(/<([^>]+)>/g, (m, name) => {
        const lower = name.toLowerCase();
        if (lower === 'super' || lower === 'mod4')
            mods.add('super');
        else if (lower === 'control' || lower === 'ctrl' || lower === 'primary')
            mods.add('ctrl');
        else if (lower === 'shift')
            mods.add('shift');
        else if (lower === 'alt' || lower === 'mod1')
            mods.add('alt');
        return '';
    });
    if (!key)
        return null;
    // The names on the key caps, told apart from the same words elsewhere by their context
    const names = {
        super: C_('key', 'Super'), ctrl: C_('key', 'Ctrl'),
        shift: C_('key', 'Shift'), alt: C_('key', 'Alt'),
    };
    const result = ['super', 'ctrl', 'shift', 'alt'].filter(m => mods.has(m)).map(m => names[m]);
    result.push(keyName(key));
    return result;
}

function keyName(key) {
    if (key === 'space')
        return C_('key', 'Space');
    if (key.length === 1)
        return key.toUpperCase();
    const keysym = Clutter[`KEY_${key}`];
    const unicode = keysym ? Clutter.keysym_to_unicode(keysym) : 0;
    if (unicode > 32 && unicode !== 127)
        return String.fromCodePoint(unicode).toUpperCase();
    return key;
}

// A row of key caps, each in its own box
export function capRow(words, styleClass) {
    const row = new St.BoxLayout({style_class: `${styleClass}-keys`, y_align: Clutter.ActorAlign.CENTER});
    for (const word of words) {
        const cap = label(word, styleClass);
        cap.clutter_text.ellipsize = 0; // Pango.EllipsizeMode.NONE: a key name is never cut
        row.add_child(cap);
    }
    return row;
}

// Case and accents set aside, for matching what is typed with a name
export const fold = text => (text ?? '').normalize('NFD').replace(/\p{M}+/gu, '').toLocaleLowerCase();
