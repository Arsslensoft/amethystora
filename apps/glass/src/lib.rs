//! The glass's paint in native code: AmethystoraGlass.Pane, one for each glass surface of Amethystora UI
//! (modules/glass/effect.js). The effect gives it, whenever they change, what the pane is made of (set_values, in the
//! layout of the V_ constants below, the same as effect.js's VALUES), the two lens pipelines it copies (set_templates)
//! and each screen's pictures (set_slot); on every paint it calls paint(), which does what effect.js's _paintGlass does,
//! term for term: where the surface is on the stage, its corners, each screen's part of it, the lens's uniforms (sent
//! only when they change) and the paint nodes, the inside of a large pane drawn with glass_interior(). It then adds the
//! surface itself. This runs inside GNOME Shell, which a panic would take down: nothing here unwraps or indexes
//! without checking, and a pane that cannot be drawn is left undrawn.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CStr, c_char, c_float, c_int, c_uint, c_void};

use glib::ffi::{GType, gboolean, gpointer, gsize};
use glib::gobject_ffi::{g_object_ref, g_object_unref};
use glib::prelude::*;
use glib::subclass::prelude::*;
use glib::translate::*;

// ---- Mutter's Clutter and Cogl, as their headers declare them -----------------------------------------------------

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct ActorBox {
    x1: c_float,
    y1: c_float,
    x2: c_float,
    y2: c_float,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct GrapheneRect {
    x: c_float,
    y: c_float,
    width: c_float,
    height: c_float,
}

type Actor = c_void;
type PaintNode = c_void;
type PaintContext = c_void;
type Pipeline = c_void;
type Texture = c_void;


unsafe extern "C" {
    fn clutter_actor_get_width(actor: *mut Actor) -> c_float;
    fn clutter_actor_get_height(actor: *mut Actor) -> c_float;
    fn clutter_actor_get_transformed_extents(actor: *mut Actor, rect: *mut GrapheneRect);
    fn clutter_actor_get_paint_opacity(actor: *mut Actor) -> u8;
    fn clutter_actor_is_in_clone_paint(actor: *mut Actor) -> gboolean;
    fn clutter_paint_context_get_redraw_clip(context: *mut PaintContext) -> *const c_void;
    fn clutter_pipeline_node_new(pipeline: *mut Pipeline) -> *mut PaintNode;
    fn clutter_actor_node_new(actor: *mut Actor, opacity: c_int) -> *mut PaintNode;
    fn clutter_paint_node_add_child(node: *mut PaintNode, child: *mut PaintNode);
    fn clutter_paint_node_unref(node: *mut PaintNode);
    fn clutter_paint_node_add_texture_rectangle(node: *mut PaintNode, rect: *const ActorBox, x1: c_float,
        y1: c_float, x2: c_float, y2: c_float);
}


unsafe extern "C" {
    fn cogl_pipeline_copy(source: *mut Pipeline) -> *mut Pipeline;
    fn cogl_pipeline_get_uniform_location(pipeline: *mut Pipeline, name: *const c_char) -> c_int;
    fn cogl_pipeline_set_uniform_1f(pipeline: *mut Pipeline, location: c_int, value: c_float);
    fn cogl_pipeline_set_uniform_1i(pipeline: *mut Pipeline, location: c_int, value: c_int);
    fn cogl_pipeline_set_uniform_float(pipeline: *mut Pipeline, location: c_int, n_components: c_int, count: c_int,
        value: *const c_float);
    fn cogl_pipeline_set_layer_texture(pipeline: *mut Pipeline, layer: c_int, texture: *mut Texture);
}

// ---- What a pane is made of (effect.js's VALUES) -------------------------------------------------------------------

const V_SAT: usize = 0;
const V_DIM: usize = 1;
const V_TINT: usize = 2;
const V_GLINT: usize = 3;
const V_RIM: usize = 4; // 4: pull, reach, steep, fringe
const V_SHADE: usize = 8;
const V_LIFT: usize = 9; // 2
const V_TOP: usize = 11; // 2: alpha, width
const V_SIDES: usize = 13; // 2
const V_RING: usize = 15;
const V_GLOW: usize = 16; // 4: alpha, y, blur, grow
const V_SIDE_GLOW: usize = 20; // 4: alpha, x, blur, grow
const V_SHEEN: usize = 24; // 4
const V_GROUND: usize = 28; // 3
const V_EDGE: usize = 31; // 4
const V_GLOW_COLOR: usize = 35; // 3
const V_SHADOW: usize = 38; // 3: alpha, blur, offset (0 for none)
const V_LOOK_RING: usize = 41;
const V_REACH: usize = 42;
const V_DIAGONAL: usize = 43;
const V_RADIUS: usize = 44;
const V_CIRCULAR: usize = 45;
const V_MARGIN: usize = 46;
const V_CONTENT: usize = 47;
const V_SCALE: usize = 48;
const V_IN_CLONES: usize = 49;
const V_STAGE_ONLY: usize = 50;
const V_WIDGET: usize = 51;
pub const VALUES: usize = 52;

// The lens's uniforms, in the order the memo keeps them
const UNIFORMS: [&CStr; 22] = [
    c"u_origin", c"u_pixel", c"u_size", c"u_corner", c"u_opacity", c"u_frost", c"u_solid", c"u_tone", c"u_rim",
    c"u_shade", c"u_veil", c"u_edge", c"u_lift", c"u_light", c"u_ring", c"u_glow", c"u_glowshape", c"u_sideglow",
    c"u_sheen", c"u_shadow", c"u_picture", c"u_clear",
];
const SAMPLERS: usize = 20;

// A pane whose inside is no larger than this across, in device pixels, is drawn whole with glass_color()
const INTERIOR: f32 = 32.0;
const LN2: f32 = std::f32::consts::LN_2;

// One screen's pictures
struct Slot {
    monitor: u32,
    rect: [f32; 4],
    texture: *mut Texture,
    clear: *mut Texture,
    veil: f32,
    ready: bool,
}

// A pipeline of the pane's, copied from a template, with the textures it was last given and the uniforms it last had
struct Lens {
    pipeline: *mut Pipeline,
    textures: [*mut Texture; 2],
    memo: [[f32; 4]; UNIFORMS.len()],
    set: [bool; UNIFORMS.len()],
}

impl Drop for Lens {
    fn drop(&mut self) {
        unsafe { g_object_unref(self.pipeline as *mut _) };
    }
}

#[derive(Default)]
pub struct State {
    values: Vec<f32>,
    templates: [Option<*mut Pipeline>; 2],
    slots: Vec<Slot>,
    lenses: HashMap<(u32, u32, bool), Lens>,
    locations: Option<[c_int; UNIFORMS.len()]>,
    corner: [f32; 2],
}

impl Drop for State {
    fn drop(&mut self) {
        self.lenses.clear();
        for template in self.templates.iter().flatten() {
            unsafe { g_object_unref(*template as *mut _) };
        }
        for slot in &self.slots {
            release(slot.texture);
            release(slot.clear);
        }
    }
}

fn retain(object: *mut c_void) -> *mut c_void {
    if !object.is_null() {
        unsafe { g_object_ref(object as *mut _) };
    }
    object
}

fn release(object: *mut c_void) {
    if !object.is_null() {
        unsafe { g_object_unref(object as *mut _) };
    }
}

// Where the curve of a corner starts (s) and its exponent (n, 2 a circle), in device pixels, for a corner of radius r on a
// pane whose shorter half side is m: effect.js's corner()
fn corner(r: f32, m: f32, circular: bool, reach: f32, diagonal: f32) -> (f32, f32) {
    if r < 0.5 {
        return (0.0, 2.0);
    }
    if r >= m - 0.5 {
        return (m, 2.0);
    }
    if circular {
        return (r.min(m), 2.0);
    }
    let s = (reach * r).min(m);
    (s, (LN2 / -(1.0 - diagonal * r / s).ln()).max(2.0))
}

// effect.js's deep() and insets(): how far in from each side ([top, right, bottom, left]) glass_interior() draws what
// glass_color() would
fn insets(v: &[f32], unit: f32, s: f32) -> [f32; 4] {
    let (pull, reach, steep) = (v[V_RIM] * unit, (v[V_RIM + 1] * unit).max(1e-3), v[V_RIM + 2]);
    let bend = if pull > 0.01 { reach * (pull / 0.01).ln().powf(1.0 / steep) } else { 0.0 };
    let glint = if v[V_GLINT] > 0.0 { 6.0_f32.max(6.0 * (255.0 * v[V_GLINT]).ln()) } else { 0.0 };
    let d = bend.max(glint).max(unit + 0.5).max(s);
    let sigma = |blur: f32| (blur / 2.0 * unit).max(1e-3);
    let top = if v[V_TOP] > 0.0 { s + v[V_TOP + 1] * unit + unit } else { 0.0 };
    let left = if v[V_SIDES] > 0.0 { s + 2.0 * unit } else { 0.0 };
    let right = if v[V_SIDES + 1] > 0.0 { s + 2.0 * unit } else { 0.0 };
    let glow = if v[V_GLOW] > 0.0 {
        3.0 * sigma(v[V_GLOW + 2]) - v[V_GLOW + 1] * unit - v[V_GLOW + 3] * unit
    } else {
        0.0
    };
    let side = if v[V_SIDE_GLOW] > 0.0 {
        3.0 * sigma(v[V_SIDE_GLOW + 2]) + v[V_SIDE_GLOW + 1] * unit - v[V_SIDE_GLOW + 3] * unit
    } else {
        0.0
    };
    [d.max(top), d.max(right), d.max(glow), d.max(left).max(side)].map(|x| x.ceil() + 1.0)
}

impl Lens {
    fn new(template: *mut Pipeline, locations: &[c_int; UNIFORMS.len()]) -> Option<Self> {
        let pipeline = unsafe { cogl_pipeline_copy(template) };
        if pipeline.is_null() {
            return None;
        }
        unsafe {
            cogl_pipeline_set_uniform_1i(pipeline, locations[SAMPLERS], 0);
            cogl_pipeline_set_uniform_1i(pipeline, locations[SAMPLERS + 1], 1);
        }
        Some(Lens {
            pipeline,
            textures: [std::ptr::null_mut(); 2],
            memo: [[0.0; 4]; UNIFORMS.len()],
            set: [false; UNIFORMS.len()],
        })
    }

    fn textures(&mut self, texture: *mut Texture, clear: *mut Texture) {
        if texture.is_null() {
            return;
        }
        let clear = if clear.is_null() { texture } else { clear };
        for (layer, wanted) in [texture, clear].into_iter().enumerate() {
            if self.textures[layer] != wanted {
                unsafe { cogl_pipeline_set_layer_texture(self.pipeline, layer as c_int, wanted) };
                self.textures[layer] = wanted;
            }
        }
    }

    // One uniform of up to four floats, sent only when it changed
    fn uniform(&mut self, locations: &[c_int; UNIFORMS.len()], index: usize, values: &[f32]) {
        let (Some(memo), Some(&location)) = (self.memo.get_mut(index), locations.get(index)) else { return };
        let n = values.len().min(4);
        if self.set[index] && memo[..n] == values[..n] {
            return;
        }
        memo[..n].copy_from_slice(&values[..n]);
        self.set[index] = true;
        if location < 0 {
            return;
        }
        unsafe {
            if n == 1 {
                cogl_pipeline_set_uniform_1f(self.pipeline, location, values[0]);
            } else {
                cogl_pipeline_set_uniform_float(self.pipeline, location, n as c_int, 1, values.as_ptr());
            }
        }
    }
}

struct Part {
    monitor: [f32; 4],
    left: f32,
    top: f32,
    size: [f32; 2],
    s: f32,
    n: f32,
    unit: f32,
    opacity: f32,
    sharp: bool,
    veil: f32,
    solid: bool,
}

// effect.js's _set(): every uniform of one pane on one screen
fn uniforms(lens: &mut Lens, locations: &[c_int; UNIFORMS.len()], v: &[f32], p: &Part) {
    let (ux, uy) = (1.0 / p.monitor[2], 1.0 / p.monitor[3]);
    let unit = p.unit;
    let mut u = |index: usize, values: &[f32]| lens.uniform(locations, index, values);
    u(0, &[(p.left - p.monitor[0]) * ux, (p.top - p.monitor[1]) * uy]);
    u(1, &[ux, uy]);
    u(2, &p.size);
    u(3, &[p.s, p.n]);
    u(4, &[p.opacity]);
    u(5, &[if p.sharp { p.opacity } else { 1.0 }]);
    u(6, &[if p.solid { 0.92 } else { 0.0 }]);
    u(7, &[v[V_SAT], v[V_DIM], v[V_TINT], v[V_GLINT]]);
    u(8, &[v[V_RIM] * unit, (v[V_RIM + 1] * unit).max(1e-3), v[V_RIM + 2], v[V_RIM + 3]]);
    u(9, &[v[V_SHADE]]);
    u(10, &[v[V_GROUND], v[V_GROUND + 1], v[V_GROUND + 2], p.veil]);
    u(11, &v[V_EDGE..V_EDGE + 4]);
    u(12, &v[V_LIFT..V_LIFT + 2]);
    u(13, &[v[V_TOP], v[V_TOP + 1] * unit, v[V_SIDES], v[V_SIDES + 1]]);
    u(14, &[v[V_RING]]);
    u(15, &[v[V_GLOW_COLOR], v[V_GLOW_COLOR + 1], v[V_GLOW_COLOR + 2], v[V_GLOW]]);
    u(16, &[v[V_GLOW + 1] * unit, (v[V_GLOW + 2] / 2.0 * unit).max(1e-3), v[V_GLOW + 3] * unit, v[V_SIDE_GLOW]]);
    u(17, &[v[V_SIDE_GLOW + 1] * unit, (v[V_SIDE_GLOW + 2] / 2.0 * unit).max(1e-3), v[V_SIDE_GLOW + 3] * unit,
        unit]);
    u(18, &v[V_SHEEN..V_SHEEN + 4]);
    u(19, &[v[V_SHADOW], v[V_SHADOW + 1] * unit, v[V_SHADOW + 2] * unit, v[V_LOOK_RING]]);
}

// Adds a pipeline node for the lens with the rectangles given, each [x1, y1, x2, y2] on the stage
fn draw(node: *mut PaintNode, pipeline: *mut Pipeline, rects: &[[f32; 4]], to_box: &dyn Fn([f32; 4]) -> (ActorBox, [f32; 4])) {
    let rects: Vec<[f32; 4]> = rects.iter().copied().filter(|r| r[2] > r[0] && r[3] > r[1]).collect();
    if rects.is_empty() {
        return;
    }
    unsafe {
        let child = clutter_pipeline_node_new(pipeline);
        if child.is_null() {
            return;
        }
        clutter_paint_node_add_child(node, child);
        for rect in rects {
            let (actor_box, tex) = to_box(rect);
            clutter_paint_node_add_texture_rectangle(child, &actor_box, tex[0], tex[1], tex[2], tex[3]);
        }
        clutter_paint_node_unref(child);
    }
}

impl State {
    fn lens(&mut self, key: (u32, u32, bool)) -> Option<&mut Lens> {
        let locations = self.locations?;
        if !self.lenses.contains_key(&key) {
            let template = self.templates[usize::from(key.2)]?;
            let lens = Lens::new(template, &locations)?;
            self.lenses.insert(key, lens);
        }
        self.lenses.get_mut(&key)
    }

    // effect.js's _paintGlass(): whether any glass was drawn under the surface
    fn paint_glass(&mut self, node: *mut PaintNode, actor: *mut Actor, areas: &[f32]) -> bool {
        if self.values.len() < VALUES || self.templates[0].is_none() || self.templates[1].is_none() {
            return false;
        }
        if self.locations.is_none() {
            let Some(template) = self.templates[0] else { return false };
            let mut locations = [-1; UNIFORMS.len()];
            for (location, name) in locations.iter_mut().zip(UNIFORMS) {
                *location = unsafe { cogl_pipeline_get_uniform_location(template, name.as_ptr()) };
            }
            self.locations = Some(locations);
        }
        let v = self.values.clone();
        let (width, height) = unsafe { (clutter_actor_get_width(actor), clutter_actor_get_height(actor)) };
        if !(width >= 1.0 && height >= 1.0) {
            return false;
        }
        let mut extents = GrapheneRect::default();
        unsafe { clutter_actor_get_transformed_extents(actor, &mut extents) };
        let (sx, sy) = (extents.width / width, extents.height / height);
        let opacity = f32::from(unsafe { clutter_actor_get_paint_opacity(actor) }) / 255.0;
        if !(opacity > 0.0 && sx > 0.0 && sy > 0.0) {
            return false;
        }
        let unit = v[V_SCALE] * sx.min(sy);
        let margin = v[V_MARGIN];
        let widget = v[V_WIDGET] > 0.5;
        let whole = [0.0, 0.0, width, height];
        let areas: Vec<[f32; 4]> = if areas.len() >= 4 {
            areas.chunks_exact(4).map(|a| [a[0], a[1], a[2], a[3]]).collect()
        } else {
            vec![whole]
        };
        let mut drawn = false;
        let mut first = true;
        for (i, &[x, y, w, h]) in areas.iter().enumerate() {
            if !(w >= 1.0 && h >= 1.0) {
                continue;
            }
            let left = extents.x + x * sx;
            let top = extents.y + y * sy;
            let size = [w * sx, h * sy];
            let (s, n) = corner(v[V_RADIUS] * unit, size[0].min(size[1]) / 2.0, v[V_CIRCULAR] > 0.5, v[V_REACH],
                v[V_DIAGONAL]);
            let quad = [left - margin * sx, top - margin * sy, left + (w + margin) * sx, top + (h + margin) * sy];
            let slots: Vec<(u32, [f32; 4], *mut Texture, *mut Texture, f32, bool)> = self.slots.iter()
                .map(|slot| (slot.monitor, slot.rect, slot.texture, slot.clear, slot.veil, slot.ready))
                .collect();
            for (monitor, m, texture, clear, veil, ready) in slots {
                let x1 = quad[0].max(m[0]);
                let y1 = quad[1].max(m[1]);
                let x2 = quad[2].min(m[0] + m[2]);
                let y2 = quad[3].min(m[1] + m[3]);
                if x2 <= x1 || y2 <= y1 || m[2] <= 0.0 || m[3] <= 0.0 {
                    continue;
                }
                let ready = ready && !texture.is_null();
                // A window has nothing to stand in for its glass: it is drawn opaque until the picture is there
                if !ready && !widget {
                    continue;
                }
                let sharp = ready && !clear.is_null();
                let part = Part {
                    monitor: m, left, top, size, s, n, unit, opacity, sharp,
                    veil: if ready { veil } else { 0.0 }, solid: !ready,
                };
                if first {
                    self.corner = [s, n];
                    first = false;
                }
                let Some(locations) = self.locations else { return drawn };
                let to_box = |r: [f32; 4]| {
                    let (ux, uy) = (1.0 / m[2], 1.0 / m[3]);
                    (ActorBox {
                        x1: x - margin + (r[0] - quad[0]) / sx,
                        y1: y - margin + (r[1] - quad[1]) / sy,
                        x2: x - margin + (r[2] - quad[0]) / sx,
                        y2: y - margin + (r[3] - quad[1]) / sy,
                    }, [(r[0] - m[0]) * ux, (r[1] - m[1]) * uy, (r[2] - m[0]) * ux, (r[3] - m[1]) * uy])
                };
                let [it, ir, ib, il] = if ready { insets(&v, unit, s) } else { [0.0; 4] };
                let i1 = x1.max(left + il);
                let j1 = y1.max(top + it);
                let i2 = x2.min(left + size[0] - ir);
                let j2 = y2.min(top + size[1] - ib);
                let split = ready && i2 - i1 > INTERIOR && j2 - j1 > INTERIOR;
                let i = i as u32;
                let Some(ring) = self.lens((i, monitor, false)) else { continue };
                ring.textures(texture, clear);
                uniforms(ring, &locations, &v, &part);
                let ring_pipeline = ring.pipeline;
                if split {
                    let Some(inside) = self.lens((i, monitor, true)) else { continue };
                    inside.textures(texture, clear);
                    uniforms(inside, &locations, &v, &part);
                    let inside_pipeline = inside.pipeline;
                    draw(node, ring_pipeline, &[[x1, y1, x2, j1], [x1, j2, x2, y2], [x1, j1, i1, j2], [i2, j1, x2, j2]],
                        &to_box);
                    draw(node, inside_pipeline, &[[i1, j1, i2, j2]], &to_box);
                } else {
                    draw(node, ring_pipeline, &[[x1, y1, x2, y2]], &to_box);
                }
                drawn = true;
            }
        }
        drawn
    }
}

// ---- The GObject --------------------------------------------------------------------------------------------------

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Pane {
        pub state: RefCell<State>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Pane {
        const NAME: &'static str = "AmethystoraGlassPane";
        type Type = super::Pane;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for Pane {}
}

glib::wrapper! {
    pub struct Pane(ObjectSubclass<imp::Pane>);
}

// The pane behind a pointer GObject introspection handed over, or None for anything else
unsafe fn pane(pointer: *mut c_void) -> Option<Borrowed<Pane>> {
    if pointer.is_null() {
        return None;
    }
    let object: Borrowed<glib::Object> = unsafe { from_glib_borrow(pointer as *mut glib::gobject_ffi::GObject) };
    if !object.type_().is_a(Pane::static_type()) {
        return None;
    }
    Some(unsafe { from_glib_borrow(pointer as *mut <Pane as glib::object::ObjectType>::GlibType) })
}

fn with_state<T>(pointer: *mut c_void, default: T, work: impl FnOnce(&mut State) -> T) -> T {
    let Some(pane) = (unsafe { pane(pointer) }) else { return default };
    let Ok(mut state) = pane.imp().state.try_borrow_mut() else { return default };
    work(&mut state)
}

#[unsafe(no_mangle)]
pub extern "C" fn amethystora_glass_pane_get_type() -> GType {
    Pane::static_type().into_glib()
}

#[unsafe(no_mangle)]
pub extern "C" fn amethystora_glass_pane_new() -> gpointer {
    let pane: Pane = glib::Object::new();
    let raw: *mut <Pane as glib::object::ObjectType>::GlibType = pane.to_glib_full();
    raw as gpointer
}

/// The lens pipelines it copies: the whole lens, and the one that draws a pane's inside
#[unsafe(no_mangle)]
pub extern "C" fn amethystora_glass_pane_set_templates(pane: gpointer, full: *mut Pipeline, inner: *mut Pipeline) {
    with_state(pane, (), |state| {
        let wanted = [full, inner];
        if state.templates.iter().zip(wanted).all(|(had, new)| *had == Some(new)) {
            return;
        }
        state.lenses.clear();
        state.locations = None;
        for (slot, new) in state.templates.iter_mut().zip(wanted) {
            if let Some(old) = slot.take() {
                release(old);
            }
            *slot = if new.is_null() { None } else { Some(retain(new)) };
        }
    });
}

/// What the pane is made of, in effect.js's VALUES layout
#[unsafe(no_mangle)]
pub extern "C" fn amethystora_glass_pane_set_values(pane: gpointer, values: *const c_float, n: gsize) {
    if values.is_null() || n < VALUES {
        return;
    }
    let values = unsafe { std::slice::from_raw_parts(values, n) };
    if values.iter().any(|v| !v.is_finite()) {
        return;
    }
    with_state(pane, (), |state| state.values = values.to_vec());
}

/// Forgets every screen's pictures, before they are given again
#[unsafe(no_mangle)]
pub extern "C" fn amethystora_glass_pane_clear_slots(pane: gpointer) {
    with_state(pane, (), |state| {
        for slot in state.slots.drain(..) {
            release(slot.texture);
            release(slot.clear);
        }
    });
}

/// One screen's pictures: where the screen is, the frosted picture and the sharp one (or none), its veil and whether
/// the frosted picture is ready
#[unsafe(no_mangle)]
pub extern "C" fn amethystora_glass_pane_add_slot(pane: gpointer, monitor: c_uint, x: c_float, y: c_float, width: c_float,
    height: c_float, texture: *mut Texture, clear: *mut Texture, veil: c_float, ready: gboolean) {
    if ![x, y, width, height, veil].iter().all(|v| v.is_finite()) {
        return;
    }
    with_state(pane, (), |state| state.slots.push(Slot {
        monitor,
        rect: [x, y, width, height],
        texture: retain(texture),
        clear: retain(clear),
        veil,
        ready: ready != 0,
    }));
}

/// Lets go of every pipeline and picture, for an effect taken off its surface
#[unsafe(no_mangle)]
pub extern "C" fn amethystora_glass_pane_forget(pane: gpointer) {
    with_state(pane, (), |state| *state = State::default());
}

/// Where the corners of the first part drawn start, and their exponent, for the tests
#[unsafe(no_mangle)]
pub extern "C" fn amethystora_glass_pane_get_corner(pane: gpointer, s: *mut c_float, n: *mut c_float) {
    let [cs, cn] = with_state(pane, [0.0, 2.0], |state| state.corner);
    unsafe {
        if !s.is_null() {
            *s = cs;
        }
        if !n.is_null() {
            *n = cn;
        }
    }
}

/// The paint of the surface: its glass, unless it is painted by itself or as a copy it is not drawn in, then the
/// surface over it. Returns whether glass was drawn.
#[unsafe(no_mangle)]
pub extern "C" fn amethystora_glass_pane_paint(pane: gpointer, node: *mut PaintNode, context: *mut PaintContext,
    actor: *mut Actor, areas: *const c_float, n_areas: gsize) -> gboolean {
    if node.is_null() || actor.is_null() || context.is_null() {
        return false.into_glib();
    }
    let areas: &[f32] = if areas.is_null() || n_areas == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(areas, n_areas) }
    };
    let areas: Vec<f32> = if areas.iter().all(|a| a.is_finite()) { areas.to_vec() } else { Vec::new() };
    let (drawn, content) = with_state(pane, (false, -1.0), |state| {
        if state.values.len() < VALUES {
            return (false, -1.0);
        }
        let v = &state.values;
        let (stage_only, in_clones, content) = (v[V_STAGE_ONLY] > 0.5, v[V_IN_CLONES] > 0.5, v[V_CONTENT]);
        // Painted by itself (a screenshot of the window): the screen is painted with the region being redrawn
        let alone = stage_only && unsafe { clutter_paint_context_get_redraw_clip(context) }.is_null();
        let cloned = unsafe { clutter_actor_is_in_clone_paint(actor) } != 0;
        if alone || (cloned && !in_clones) {
            return (false, -1.0);
        }
        (state.paint_glass(node, actor, &areas), content)
    });
    let opacity = if drawn && content >= 0.0 {
        (f32::from(unsafe { clutter_actor_get_paint_opacity(actor) }) * content).round() as c_int
    } else {
        -1
    };
    unsafe {
        let child = clutter_actor_node_new(actor, opacity);
        if !child.is_null() {
            clutter_paint_node_add_child(node, child);
            clutter_paint_node_unref(child);
        }
    }
    drawn.into_glib()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corners_follow_effect_js() {
        // A circle, a straight corner and a continuous one (effect.js's CONTINUOUS: reach 1.4)
        assert_eq!(corner(0.2, 100.0, false, 1.4, 0.3), (0.0, 2.0));
        assert_eq!(corner(10.0, 100.0, true, 1.4, 0.3), (10.0, 2.0));
        let (s, n) = corner(10.0, 100.0, false, 1.4, 0.3);
        assert!((s - 14.0).abs() < 1e-4 && n > 2.0);
    }

    #[test]
    fn insets_cover_the_rim() {
        let mut v = vec![0.0; VALUES];
        v[V_RIM] = 12.8;
        v[V_RIM + 1] = 7.28;
        v[V_RIM + 2] = 1.59;
        v[V_GLINT] = 0.08;
        let [t, r, b, l] = insets(&v, 1.0, 20.0);
        assert!(t >= 26.0 && r == t && b == t && l == t);
    }
}
