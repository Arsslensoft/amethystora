// Hello: a new account's first login greets the person in the gem's light, the word for hello written as if by hand in
// each language Amethystora speaks, their own first, and then goes on to what a first login has always shown. Any key or
// click goes straight on. With reduced motion the words stand still together, and nothing is written.
//
// It is the shell's own layer rather than a window, so that it changes nothing the first look relies on: no window opens,
// no focus moves, and no token is asked for. Pick a look (modules/firstlook.js) waits for it as it waits for any modal
// dialog: while this holds the keyboard the desktop is not free, and giving it back moves the key focus, which is
// what firstlook listens for. GNOME's welcome dialog, opened when the session has started, is simply under it until then.
//
// Who is greeted is decided once a session, as user-setup.hooks.d/25-first-look.sh decides who gets Pick a look: a home
// folder less than a day old with no first-look.json. The mark hello.json is written as the greeting opens, and holds
// nothing else. Everyone else costs three file checks at login and nothing after them: nothing runs, no timer, no
// picture kept. While it shows, one picture is in memory at a time (branding/hello.mjs draws them) and each frame sets one
// number of one small shader.
//
// Keys (org.gnome.shell.extensions.amethystora-ui.hello, ame desktop hello, Control):
//   greet  true   greet a new account at its first login
// `ame desktop hello show` shows it again, through /org/amethystora/UI/Hello on the session bus.

import Clutter from 'gi://Clutter';
import Cogl from 'gi://Cogl';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

Gio._promisify(Gio.File.prototype, 'query_info_async');
Gio._promisify(Gio.File.prototype, 'make_directory_async');
Gio._promisify(Gio.File.prototype, 'replace_contents_bytes_async', 'replace_contents_finish');

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.hello';
const DAY = 86400;
const PATH = '/org/amethystora/UI/Hello';
// The layer's name: the transitions leave the screen alone while it is up (amethystora-transitions' _freeze). The reveal's
// sky is the transitions' too (reveal.js), and is waited for.
const NAME = 'amethystoraHello';
const REVEAL_COVER = 'amethystoraRevealCover';
// Read from the right
const RIGHT_TO_LEFT = ['ar', 'he', 'fa', 'ur'];
const XML = `
<node>
  <interface name="org.amethystora.UI.Hello">
    <method name="Show"/>
  </interface>
</node>`;

// Each language's picture and its word, for a screen reader. A new language is a line here and one in branding/hello.mjs.
const WORDS = [['en', 'hello'], ['fr', 'bonjour'], ['de', 'hallo'], ['es', 'hola'], ['ar', 'مرحبا']];
// A picture is drawn at this logical size, its own, or smaller on a small screen
const PLATE_WIDTH = 900;
const PLATE_HEIGHT = 420;
// The words standing still are drawn at 0.45 of that and never move, so their pictures are loaded at that size: a fifth
// of the memory, which keeps the five of them under the 2 MB a module may hold
const STILL_WIDTH = 405;
const STILL_HEIGHT = 189;
// Nominal, as in the transitions: the shell's slow-down factor (0.6 in the image) is applied on top. Writing, holding the
// word and letting it go come to two seconds a word.
const REVEAL_MS = 1500; // The gem's flight into the top bar comes first (amethystora-transitions' reveal.js)
const FADE_MS = 400;
const WRITE_MS = 1900;
const HOLD_MS = 800;
const LEAVE_MS = 500;
const RISE = 8;
// The words standing still stay as long as the moving ones take, and are not slowed or sped by anything
const STILL_MS = 10000;
// A picture that has not come by then never will
const LOAD_MS = 3000;

// Three measures a pixel (branding/hello.mjs): red, when the pen gets here; green, the ink; blue, its glow. The ink is white
// with a little lilac; the glow is the gem's, stronger at the pen's point and settling behind it. Premultiplied, as Cogl
// blends by default.
const DECLARATIONS = `
uniform sampler2D map;
uniform float progress;
uniform float fade;
`;
const CODE = `
vec3 m = texture2D(map, cogl_tex_coord_in[0].st).rgb;
float done = smoothstep(m.r - 0.012, m.r + 0.004, progress);
float tip = exp(-max(progress - m.r, 0.0) * 22.0);
float ink = m.g * done;
float glow = m.b * smoothstep(m.r - 0.05, m.r + 0.03, progress) * (0.45 + 0.5 * tip);
float halo = (1.0 - ink) * glow;
cogl_color_out = vec4(vec3(0.98, 0.956, 1.0) * ink + vec3(0.702, 0.49, 0.976) * halo, ink + halo) * fade;
`;

// Draws the actor's picture through the shader, in place of its own paint. A plain effect with a pipeline of its own, as
// the transitions' reveal is: it copies nothing, so a frame costs what one textured rectangle costs.
const WritingEffect = GObject.registerClass(
class AmethystoraHelloEffect extends Clutter.Effect {
    _init(progress, width, height) {
        super._init();
        this._progress = progress;
        this._box = new Clutter.ActorBox({x1: 0, y1: 0, x2: width, y2: height});
        this._context = null;
        this._pipeline = null;
    }

    set progress(value) {
        this._progress = value;
        this.get_actor()?.queue_redraw();
    }

    // The pipeline holds the picture from here: let it go with the word, not whenever the collector comes
    release() {
        this._pipeline = null;
        this._context = null;
    }

    vfunc_paint_node(node, paintContext, _flags) {
        const actor = this.get_actor();
        const texture = actor.content?.get_texture?.();
        if (!texture)
            return;
        const context = paintContext.get_framebuffer().get_context();
        if (this._context !== context) {
            this._context = context;
            const pipeline = Cogl.Pipeline.new(context);
            pipeline.set_layer_null_texture(0);
            pipeline.set_layer_filters(0, Cogl.PipelineFilter.LINEAR, Cogl.PipelineFilter.LINEAR);
            pipeline.set_layer_wrap_mode(0, Cogl.PipelineWrapMode.CLAMP_TO_EDGE);
            pipeline.add_snippet(Cogl.Snippet.new(Cogl.SnippetHook.FRAGMENT, DECLARATIONS, CODE));
            pipeline.set_uniform_1i(pipeline.get_uniform_location('map'), 0);
            pipeline.set_layer_texture(0, texture);
            this._pipeline = pipeline;
            this._progressAt = pipeline.get_uniform_location('progress');
            this._fadeAt = pipeline.get_uniform_location('fade');
        }
        this._pipeline.set_uniform_1f(this._progressAt, this._progress);
        this._pipeline.set_uniform_1f(this._fadeAt, actor.get_paint_opacity() / 255);
        const pipelineNode = new Clutter.PipelineNode(this._pipeline);
        node.add_child(pipelineNode);
        pipelineNode.add_texture_rectangle(this._box, 0, 0, 1, 1);
    }
});

// Whether a file is there, without waiting for the disk
async function exists(file, cancellable) {
    try {
        await file.query_info_async('standard::type', Gio.FileQueryInfoFlags.NONE, GLib.PRIORITY_LOW, cancellable);
        return true;
    } catch (e) {
        if (e.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
            throw e;
        return false;
    }
}

// The mark: only that the greeting has been shown
async function mark(file) {
    const folder = file.get_parent();
    for (const each of [folder.get_parent().get_parent(), folder.get_parent(), folder]) {
        try {
            await each.make_directory_async(GLib.PRIORITY_LOW, null);
        } catch {}
    }
    await file.replace_contents_bytes_async(new GLib.Bytes(new TextEncoder().encode('{"version": 1}\n')), null, false,
        Gio.FileCreateFlags.PRIVATE, null);
}

// The words, with the session's own language first
function words() {
    const own = GLib.get_language_names()[0]?.slice(0, 2);
    return [...WORDS.filter(([language]) => language === own), ...WORDS.filter(([language]) => language !== own)];
}

// Who has been decided on, this shell: the module is started again at every unlock
let decided = false;

export class Hello {
    constructor(ext) {
        this._ext = ext;
        // The owner of the wait for the start of the session, apart from the layer's own connections to the same object
        this._start = {};
    }

    enable() {
        this._settings = this._ext.getSettings(SCHEMA);
        this._settings.connectObject('changed::greet', () => this._apply(), this);
        this._apply();
    }

    disable() {
        this._close(true);
        this._fading?.destroy();
        this._fading = null;
        this._stopWaiting();
        this._dbus?.unexport();
        this._dbus = null;
        this._settings?.disconnectObject(this);
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
    }

    // For a test: off, waiting (for the start of the session), or showing
    get state() {
        return this._overlay ? 'showing' : this._waitId || this._cancellable || this._cover ? 'waiting' : 'off';
    }

    _apply() {
        if (!this._settings.get_boolean('greet')) {
            this._close();
            this._stopWaiting();
            this._dbus?.unexport();
            this._dbus = null;
            return;
        }
        if (!this._dbus) {
            this._dbus = Gio.DBusExportedObject.wrapJSObject(XML, {Show: () => this._show()});
            this._dbus.export(Gio.DBus.session, PATH);
        }
        if (!decided)
            this._decide();
    }

    // Asked for over the bus: not over a password prompt or any dialog that is already up, which a program of the account
    // could otherwise cover and take keys from. The overview is not a dialog.
    _show() {
        if (Main.modalCount === 0 || Main.actionMode === Shell.ActionMode.OVERVIEW)
            this._open();
    }

    _stopWaiting() {
        this._cancellable?.cancel();
        this._cancellable = null;
        if (this._waitId)
            GLib.source_remove(this._waitId);
        this._waitId = 0;
        this._cover?.disconnectObject(this._start);
        this._cover = null;
        Main.layoutManager.disconnectObject(this._start);
    }

    async _decide() {
        if (this._cancellable || this._waitId)
            return;
        const cancellable = new Gio.Cancellable();
        this._cancellable = cancellable;
        const state = Gio.File.new_for_path(GLib.get_user_state_dir()).get_child('amethystora');
        try {
            const seen = await exists(state.get_child('hello.json'), cancellable) ||
                await exists(state.get_child('first-look.json'), cancellable);
            const info = await Gio.File.new_for_path(GLib.get_home_dir()).query_info_async('time::created',
                Gio.FileQueryInfoFlags.NONE, GLib.PRIORITY_LOW, cancellable);
            const born = info.get_attribute_uint64('time::created');
            if (seen || born === 0 || GLib.get_real_time() / 1e6 - born >= DAY) {
                decided = true;
                return;
            }
        } catch (e) {
            // Stopped (a lock, the key): the next start decides. Otherwise the account is left alone, as an old one.
            if (!e.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                decided = true;
            return;
        } finally {
            if (this._cancellable === cancellable)
                this._cancellable = null;
        }

        if (cancellable.is_cancelled())
            return;
        if (Main.layoutManager._startingUp)
            Main.layoutManager.connectObject('startup-complete', () => this._waitForReveal(), this._start);
        else
            this._waitForReveal();
    }

    _waitForReveal() {
        Main.layoutManager.disconnectObject(this._start);
        const wait = Math.round(REVEAL_MS * St.Settings.get().slow_down_factor);
        this._waitId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, wait, () => {
            this._waitId = 0;
            // The gem's flight draws a sky over everything that is added while it lasts: on a slow first login it can last
            // longer than this wait, and the greeting then comes when the sky has gone
            const cover = Main.layoutManager.uiGroup.get_children().find(actor => actor.name === REVEAL_COVER);
            if (cover) {
                this._cover = cover;
                cover.connectObject('destroy', () => {
                    this._cover = null;
                    this._begin();
                }, this._start);
            } else {
                this._begin();
            }
            return GLib.SOURCE_REMOVE;
        });
    }

    _begin() {
        decided = true;
        mark(Gio.File.new_for_path(GLib.get_user_state_dir()).get_child('amethystora').get_child('hello.json'))
            .catch(e => console.warn(`Amethystora UI: the greeting's mark: ${e.message}`));
        this._open();
    }

    get _still() {
        return this._ext.modules?.get('comfort')?.reduceMotion ?? !St.Settings.get().enable_animations;
    }

    // ---- The layer ----

    _open() {
        const monitor = Main.layoutManager.primaryMonitor;
        if (this._overlay || !monitor)
            return;
        const uiGroup = Main.layoutManager.uiGroup;
        const still = this._still;
        this._overlay = new St.Widget({
            name: NAME,
            style_class: 'amethystora-hello',
            reactive: true,
            x: 0,
            y: 0,
            width: global.stage.width,
            height: global.stage.height,
            opacity: still ? 255 : 0,
        });
        uiGroup.add_child(this._overlay);
        uiGroup.set_child_above_sibling(this._overlay, null);

        // Whatever goes wrong while it opens, the keyboard is never left held
        try {
            this._build(monitor, still);
        } catch (e) {
            console.error(`Amethystora UI: the greeting did not open: ${e.message}`);
            this._close(true);
        }
    }

    _build(monitor, still) {
        // Every key and click goes on, whichever it is; the button is there to be seen, and for a screen reader
        this._grab = Main.pushModal(this._overlay, {actionMode: Shell.ActionMode.SYSTEM_MODAL});
        this._overlay.connect('captured-event', (actor, event) => {
            const type = event.type();
            if (type === Clutter.EventType.KEY_PRESS || type === Clutter.EventType.BUTTON_PRESS ||
                type === Clutter.EventType.TOUCH_BEGIN) {
                this._close();
                return Clutter.EVENT_STOP;
            }
            return Clutter.EVENT_PROPAGATE;
        });
        Main.layoutManager.connectObject('monitors-changed', () => this._close(), this);
        // A dialog that opens over it (a password prompt) takes the keyboard from under it, where no key would end it
        global.stage.connectObject('notify::key-focus', () => {
            const focus = global.stage.key_focus;
            if (focus && focus !== global.stage && !this._overlay?.contains(focus))
                this._close();
        }, this);

        this._scale = Math.min(1, monitor.width / (PLATE_WIDTH * 1.3));
        this._frame = new St.Widget({x: monitor.x, y: monitor.y, width: monitor.width, height: monitor.height});
        this._overlay.add_child(this._frame);
        const start = new St.Button({style_class: 'amethystora-hello-start', label: _('Start'), can_focus: true});
        start.connect('clicked', () => this._close());
        start.add_constraint(new Clutter.AlignConstraint({source: this._frame, align_axis: Clutter.AlignAxis.X_AXIS, factor: 0.5}));
        start.y = Math.round(monitor.height * 0.78);
        this._frame.add_child(start);
        start.grab_key_focus();

        if (still) {
            this._stand();
        } else {
            this._overlay.ease({opacity: 255, duration: FADE_MS, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
            this._queue = words();
            this._next();
        }
    }

    // A word's picture, at its place on the screen; its effect is the one thing a frame touches
    _plate(language, word, x, y, scale, progress, loaded, small = false) {
        const file = this._ext.dir.get_child('hello').get_child(`${language}.png`);
        const plate = small
            ? St.TextureCache.get_default().load_file_async(file, STILL_WIDTH, STILL_HEIGHT, 1, 1)
            : St.TextureCache.get_default().load_file_async(file, -1, -1, 1, 1);
        const [width, height] = [Math.round(PLATE_WIDTH * scale), Math.round(PLATE_HEIGHT * scale)];
        plate.set({x: Math.round(x - width / 2), y: Math.round(y - height / 2), accessible_name: word});
        // The actor keeps the picture's own size, which the cache gives it as it hands the picture over; the effect
        // draws it at the size wanted
        const effect = new WritingEffect(progress, width, height);
        plate.add_effect(effect);
        plate.connect('notify::content', () => loaded?.(plate));
        plate.connect('destroy', () => {
            effect.release();
            plate.content = null;
        });
        this._frame.add_child(plate);
        return plate;
    }

    // ---- Reduced motion: every word, standing still ----

    _stand() {
        const list = words();
        const {width, height} = this._frame;
        const scale = this._scale * 0.45;
        // The first word is where the person starts reading: on the right, for a language read from the right
        const side = RIGHT_TO_LEFT.includes(GLib.get_language_names()[0]?.slice(0, 2)) ? -1 : 1;
        list.forEach(([language, word], index) => {
            const row = index < 3 ? 0 : 1;
            const inRow = row === 0 ? Math.min(3, list.length) : list.length - 3;
            const column = row === 0 ? index : index - 3;
            const x = width / 2 + side * (column - (inRow - 1) / 2) * PLATE_WIDTH * scale;
            const y = height * 0.42 + (row - 0.5) * PLATE_HEIGHT * scale;
            this._plate(language, word, x, y, scale, 2, null, true);
        });
        this._timerId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, STILL_MS, () => {
            this._timerId = 0;
            this._close();
            return GLib.SOURCE_REMOVE;
        });
    }

    // ---- The words, one after another ----

    _next() {
        this._word?.destroy();
        this._word = null;
        const next = this._queue?.shift();
        if (!next) {
            this._close();
            return;
        }
        const [language, word] = next;
        const {width, height} = this._frame;
        this._word = this._plate(language, word, width / 2, height * 0.42, this._scale, -0.02, plate => this._write(plate));
        // A picture that does not come ends the greeting rather than holding the keyboard
        this._timerId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, LOAD_MS, () => {
            this._timerId = 0;
            this._close();
            return GLib.SOURCE_REMOVE;
        });
    }

    _write(plate) {
        if (plate !== this._word || this._writing)
            return;
        if (this._timerId)
            GLib.source_remove(this._timerId);
        this._timerId = 0;
        const factor = St.Settings.get().slow_down_factor;
        const effect = plate.get_effects()[0];
        this._writing = new Clutter.Timeline({actor: plate, duration: Math.max(1, Math.round(WRITE_MS * factor))});
        this._writing.set_progress_mode(Clutter.AnimationMode.EASE_IN_OUT_SINE);
        this._writing.connect('new-frame', timeline => {
            // Past 1 so that the light at the pen's point has settled by the time the word is held
            effect.progress = -0.02 + timeline.get_progress() * 1.22;
        });
        this._writing.connect('stopped', (timeline, finished) => {
            this._writing = null;
            if (!finished || plate !== this._word)
                return;
            plate.ease({
                opacity: 0,
                translation_y: -RISE * this._scale,
                delay: HOLD_MS,
                duration: LEAVE_MS,
                mode: Clutter.AnimationMode.EASE_IN_QUAD,
                onComplete: () => {
                    if (plate === this._word)
                        this._next();
                },
            });
        });
        this._writing.start();
    }

    // Gives the keyboard back at once, so that a key goes straight on, and lets the layer fade away behind it. `now`: a lock
    // or the extension stopping, which leaves nothing behind.
    _close(now = false) {
        if (this._timerId)
            GLib.source_remove(this._timerId);
        this._timerId = 0;
        this._queue = null;
        this._writing?.stop();
        this._writing = null;
        Main.layoutManager.disconnectObject(this);
        global.stage.disconnectObject(this);
        if (this._grab) {
            Main.popModal(this._grab);
            this._grab = null;
        }
        const overlay = this._overlay;
        this._overlay = null;
        this._word = null;
        this._frame = null;
        if (!overlay)
            return;
        overlay.reactive = false;
        if (now || this._still) {
            overlay.destroy();
            return;
        }
        this._fading = overlay;
        overlay.ease({
            opacity: 0,
            duration: FADE_MS,
            mode: Clutter.AnimationMode.EASE_OUT_QUAD,
            onStopped: () => {
                if (this._fading === overlay)
                    this._fading = null;
                overlay.destroy();
            },
        });
    }
}
