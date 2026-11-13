use gtk4::prelude::*;
use std::{cell::RefCell, collections::VecDeque, rc::Rc, time::Instant};

const CHARACTERS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ!@#$&*()-_+=/[]{};:<>.,0123456789";
const CELL_WIDTH: f64 = 8.0;
const CELL_HEIGHT: f64 = 12.0;
const DENSITY: f64 = 0.2;
const BASE_OPACITY: f64 = 0.1;
const GLITCH_AMOUNT: f64 = 0.35;
const SPEED: f64 = 0.3;
const FLOW: f64 = 0.4;
const CURSOR_RADIUS: f64 = 100.0;
const EDGE_FADE: f64 = 0.15;

struct State {
    start: Instant,
    last_frame: f64,
    pointer: Option<(f64, f64)>,
    trail: VecDeque<(f64, f64, f64)>,
    tear: Option<(f64, f64, f64)>,
}

thread_local! {
    static STATE: Rc<RefCell<State>> = Rc::new(RefCell::new(State {
        start: Instant::now(), last_frame: 0.0, pointer: None, trail: VecDeque::new(), tear: None,
    }));
}

fn hash(x: i32, y: i32, seed: u32) -> u32 {
    let mut value = (x as u32).wrapping_mul(0x9e3779b1)
        ^ (y as u32).wrapping_mul(0x85ebca6b)
        ^ seed.wrapping_mul(0xc2b2ae35);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846ca68b);
    value ^ (value >> 16)
}

fn fraction(value: u32) -> f64 {
    value as f64 / u32::MAX as f64
}
fn smooth(value: f64) -> f64 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn edge_mask(x: f64, y: f64, width: f64, height: f64) -> f64 {
    let edge = width.min(height) * EDGE_FADE;
    if edge <= 0.0 {
        return 1.0;
    }
    smooth(x.min(width - x) / edge) * smooth(y.min(height - y) / edge)
}

fn animations_enabled() -> bool {
    gtk4::Settings::default().is_none_or(|settings| settings.is_gtk_enable_animations())
}

pub fn create() -> gtk4::Widget {
    let area = gtk4::DrawingArea::new();
    area.set_can_target(false);
    area.set_draw_func(|_, cr, width, height| {
        STATE.with(|state| {
            let state = state.borrow();
            let animated = animations_enabled();
            let time = if animated {
                state.start.elapsed().as_secs_f64()
            } else {
                100.0
            };
            cr.set_source_rgb(0.035, 0.047, 0.075);
            let _ = cr.paint();
            cr.select_font_face(
                "monospace",
                gtk4::cairo::FontSlant::Normal,
                gtk4::cairo::FontWeight::Normal,
            );
            cr.set_font_size(10.0);
            let cols = (width as f64 / CELL_WIDTH).ceil() as i32;
            let rows = (height as f64 / CELL_HEIGHT).ceil() as i32;
            let edge_width = width as f64;
            let edge_height = height as f64;
            let epoch_time = time * SPEED;
            let epoch = epoch_time.floor() as u32;
            let phase = epoch_time.fract();
            let transition = (std::f64::consts::PI * phase).sin().max(0.0);
            let intro = if animated { smooth(time / 1.4) } else { 1.0 };
            for row in 0..rows {
                for col in 0..cols {
                    let x = (col as f64 + 0.5) * CELL_WIDTH;
                    let y = (row as f64 + 0.5) * CELL_HEIGHT;
                    let seed = hash(col, row, 0);
                    let occupied = fraction(seed) < DENSITY;
                    let change = hash(col, row, epoch.wrapping_add(1));
                    let glitch = animated && fraction(change) < GLITCH_AMOUNT * 0.05;
                    let mut heat = if glitch { transition * 0.72 } else { 0.0 };
                    if animated && (occupied || fraction(change.rotate_left(13)) < 0.15) {
                        if let Some((px, py)) = state.pointer {
                            let distance = (x - px).hypot(y - py);
                            heat = heat.max(smooth(1.0 - distance / CURSOR_RADIUS) * 0.75);
                        }
                        for &(px, py, then) in &state.trail {
                            let age = time - then;
                            let distance = (x - px).hypot(y - py);
                            let trail = smooth(1.0 - distance / (CURSOR_RADIUS * 0.55))
                                * (1.0 - age / 0.8).max(0.0)
                                * 0.6;
                            heat = heat.max(trail);
                        }
                        if let Some((tx, ty, when)) = state.tear {
                            let age = time - when;
                            let distance = (x - tx).abs();
                            let front = age * 1500.0;
                            let row_fade = (1.0 - (y - ty).abs() / (CELL_HEIGHT * 3.0)).max(0.0);
                            let line = (1.0 - (distance - front).abs() / 80.0).max(0.0);
                            heat = heat.max(line * row_fade * (1.0 - age / 1.3).max(0.0));
                        }
                    }
                    if !occupied && heat < 0.04 {
                        continue;
                    }
                    let wave = 1.0 + FLOW * ((x * 0.012 + y * 0.008 + time * 0.12).sin() * 0.5);
                    let base = if occupied { BASE_OPACITY * wave } else { 0.0 };
                    let alpha = (base + heat * 0.8).min(1.0)
                        * edge_mask(x, y, edge_width, edge_height)
                        * intro;
                    if alpha < 0.004 {
                        continue;
                    }
                    let glyph = if heat > 0.18 { change } else { seed };
                    let ch = CHARACTERS[glyph as usize % CHARACTERS.len()] as char;
                    if heat > 0.08 && change & 1 == 0 {
                        cr.set_source_rgba(124.0 / 255.0, 58.0 / 255.0, 237.0 / 255.0, alpha);
                    } else {
                        cr.set_source_rgba(1.0, 1.0, 1.0, alpha);
                    }
                    cr.move_to(x - 3.0, y + 3.5);
                    let _ = cr.show_text(&ch.to_string());
                }
            }
        });
    });
    area.add_tick_callback(|area, clock| {
        if !animations_enabled() {
            return gtk4::glib::ControlFlow::Continue;
        }
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            let now = clock.frame_time() as f64 / 1_000_000.0;
            if now - state.last_frame >= 1.0 / 30.0 {
                state.last_frame = now;
                let time = state.start.elapsed().as_secs_f64();
                while state
                    .trail
                    .front()
                    .is_some_and(|point| time - point.2 > 0.8)
                {
                    state.trail.pop_front();
                }
                if state.tear.is_some_and(|tear| time - tear.2 > 1.3) {
                    state.tear = None;
                }
                area.queue_draw();
            }
        });
        gtk4::glib::ControlFlow::Continue
    });
    area.upcast()
}

pub fn attach(widget: &impl IsA<gtk4::Widget>) {
    let motion = gtk4::EventControllerMotion::new();
    motion.set_propagation_phase(gtk4::PropagationPhase::Capture);
    motion.connect_motion(|_, x, y| {
        if !animations_enabled() {
            return;
        }
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            let time = state.start.elapsed().as_secs_f64();
            if state
                .pointer
                .is_none_or(|(px, py)| (px - x).hypot(py - y) > 20.0)
            {
                state.trail.push_back((x, y, time));
                if state.trail.len() > 12 {
                    state.trail.pop_front();
                }
            }
            state.pointer = Some((x, y));
        });
    });
    motion.connect_leave(|_| STATE.with(|state| state.borrow_mut().pointer = None));
    widget.add_controller(motion);
    let click = gtk4::GestureClick::new();
    click.set_propagation_phase(gtk4::PropagationPhase::Capture);
    click.connect_pressed(|_, _, x, y| {
        if !animations_enabled() {
            return;
        }
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.tear = Some((x, y, state.start.elapsed().as_secs_f64()));
        });
    });
    widget.add_controller(click);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hash_density_and_edge_fade_are_bounded() {
        let occupied = (0..100)
            .flat_map(|y| (0..100).map(move |x| fraction(hash(x, y, 0)) < DENSITY))
            .filter(|occupied| *occupied)
            .count();
        assert!((1800..2200).contains(&occupied));
        assert_eq!(edge_mask(0.0, 50.0, 100.0, 100.0), 0.0);
        assert_eq!(edge_mask(50.0, 50.0, 100.0, 100.0), 1.0);
    }
}
