#![allow(dead_code)]
use std::{
    cell::RefCell,
    ffi::{c_char, c_double, c_int, c_uint, c_void},
    time::Instant,
};

type Ptr = *mut c_void;
#[link(name = "gtk-4")]
extern "C" {
    fn gtk_init();
    fn gtk_window_new() -> Ptr;
    fn gtk_window_set_title(window: Ptr, title: *const c_char);
    fn gtk_window_set_default_size(window: Ptr, width: c_int, height: c_int);
    fn gtk_window_set_child(window: Ptr, child: Ptr);
    fn gtk_window_present(window: Ptr);
    fn gtk_drawing_area_new() -> Ptr;
    fn gtk_drawing_area_set_draw_func(
        area: Ptr,
        callback: extern "C" fn(Ptr, Ptr, c_int, c_int, Ptr),
        data: Ptr,
        destroy: Ptr,
    );
    fn gtk_widget_add_controller(widget: Ptr, controller: Ptr);
    fn gtk_widget_add_tick_callback(
        widget: Ptr,
        callback: extern "C" fn(Ptr, Ptr, Ptr) -> c_int,
        data: Ptr,
        destroy: Ptr,
    ) -> c_uint;
    fn gtk_widget_queue_draw(widget: Ptr);
    fn gtk_event_controller_motion_new() -> Ptr;
    fn gtk_gesture_click_new() -> Ptr;
}
#[link(name = "gobject-2.0")]
extern "C" {
    fn g_signal_connect_data(
        instance: Ptr,
        signal: *const c_char,
        callback: Ptr,
        data: Ptr,
        destroy: Ptr,
        flags: c_uint,
    ) -> u64;
}
#[link(name = "glib-2.0")]
extern "C" {
    fn g_main_loop_new(context: Ptr, running: c_int) -> Ptr;
    fn g_main_loop_run(main_loop: Ptr);
    fn g_main_loop_quit(main_loop: Ptr);
}
#[link(name = "cairo")]
extern "C" {
    fn cairo_set_source_rgb(cr: Ptr, r: c_double, g: c_double, b: c_double);
    fn cairo_set_source_rgba(cr: Ptr, r: c_double, g: c_double, b: c_double, a: c_double);
    fn cairo_paint(cr: Ptr);
    fn cairo_set_line_width(cr: Ptr, width: c_double);
    fn cairo_move_to(cr: Ptr, x: c_double, y: c_double);
    fn cairo_line_to(cr: Ptr, x: c_double, y: c_double);
    fn cairo_stroke(cr: Ptr);
    fn cairo_select_font_face(cr: Ptr, family: *const c_char, slant: c_int, weight: c_int);
    fn cairo_set_font_size(cr: Ptr, size: c_double);
    fn cairo_show_text(cr: Ptr, text: *const c_char);
}

struct State {
    started: Instant,
    pointer: Option<(f64, f64)>,
    peak: (f64, f64),
    strength: f64,
    ripple: Option<(f64, f64, f64)>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State { started: Instant::now(), pointer: None, peak: (0.0, 0.0), strength: 0.0, ripple: None }); }

fn field(
    x: f64,
    y: f64,
    t: f64,
    peak: (f64, f64),
    strength: f64,
    ripple: Option<(f64, f64, f64)>,
) -> f64 {
    let base = (x * 0.009 + t * 0.16).sin() * 0.55
        + (y * 0.011 - t * 0.11).cos() * 0.45
        + ((x + y) * 0.006 + t * 0.08).sin() * 0.55;
    let d = ((x - peak.0).powi(2) + (y - peak.1).powi(2)).sqrt();
    let hill = strength * 2.0 * (-(d / 130.0).powi(2)).exp();
    let wave = ripple.map_or(0.0, |(rx, ry, age)| {
        let radius = ((x - rx).powi(2) + (y - ry).powi(2)).sqrt();
        let front = radius - age * 260.0;
        0.8 * (-(front / 32.0).powi(2)).exp() * (front / 18.0).sin() * (1.0 - age / 2.6).max(0.0)
    });
    base + hill + wave
}

extern "C" fn draw(_: Ptr, cr: Ptr, width: c_int, height: c_int, _: Ptr) {
    STATE.with(|s| {
        let s = s.borrow();
        let t = s.started.elapsed().as_secs_f64();
        unsafe {
            cairo_set_source_rgb(cr, 0.035, 0.047, 0.075);
            cairo_paint(cr);
            let step = 9.0;
            let cols = (width as f64 / step).ceil() as usize + 1;
            let rows = (height as f64 / step).ceil() as usize + 1;
            let ripple = s
                .ripple
                .map(|(x, y, start)| (x, y, t - start))
                .filter(|v| v.2 < 2.6);
            let mut values = Vec::with_capacity(cols * rows);
            for y in 0..rows {
                for x in 0..cols {
                    values.push(field(
                        x as f64 * step,
                        y as f64 * step,
                        t,
                        s.peak,
                        s.strength,
                        ripple,
                    ));
                }
            }
            for level in -8..=8 {
                let threshold = level as f64 * 0.23;
                cairo_set_source_rgba(
                    cr,
                    0.40,
                    0.68,
                    0.88,
                    if level % 5 == 0 { 0.35 } else { 0.18 },
                );
                cairo_set_line_width(cr, if level % 5 == 0 { 1.3 } else { 0.8 });
                for y in 0..rows - 1 {
                    for x in 0..cols - 1 {
                        let px = x as f64 * step;
                        let py = y as f64 * step;
                        let corners = [
                            (px, py, values[y * cols + x]),
                            (px + step, py, values[y * cols + x + 1]),
                            (px + step, py + step, values[(y + 1) * cols + x + 1]),
                            (px, py + step, values[(y + 1) * cols + x]),
                        ];
                        let mut points = [(0.0, 0.0); 4];
                        let mut count = 0;
                        for i in 0..4 {
                            let a = corners[i];
                            let b = corners[(i + 1) % 4];
                            if (a.2 < threshold) != (b.2 < threshold) {
                                let f = (threshold - a.2) / (b.2 - a.2);
                                points[count] = (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f);
                                count += 1;
                            }
                        }
                        if count >= 2 {
                            cairo_move_to(cr, points[0].0, points[0].1);
                            cairo_line_to(cr, points[1].0, points[1].1);
                        }
                        if count == 4 {
                            cairo_move_to(cr, points[2].0, points[2].1);
                            cairo_line_to(cr, points[3].0, points[3].1);
                        }
                    }
                }
                cairo_stroke(cr);
            }
            /* label omitted */
            /*
            cairo_select_font_face(cr, c"Sans".as_ptr(), 0, 1);
            cairo_set_font_size(cr, 34.0);
            cairo_move_to(cr, 42.0, 65.0);
            cairo_show_text(cr, c"Amnessia".as_ptr());
            cairo_set_source_rgba(cr, 0.77, 0.85, 0.94, 0.8);
            cairo_select_font_face(cr, c"Sans".as_ptr(), 0, 0);
            cairo_set_font_size(cr, 14.0);
            cairo_move_to(cr, 44.0, 91.0);
            cairo_show_text(
                cr,
                c"Move your cursor to shape the landscape. Click to make a wave.".as_ptr(),
            );
            */
        }
    });
}
extern "C" fn tick(widget: Ptr, _: Ptr, _: Ptr) -> c_int {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if let Some((x, y)) = s.pointer {
            s.peak.0 += (x - s.peak.0) * 0.12;
            s.peak.1 += (y - s.peak.1) * 0.12;
            s.strength += (1.0 - s.strength) * 0.08;
        } else {
            s.strength *= 0.92;
        }
    });
    unsafe {
        gtk_widget_queue_draw(widget);
    }
    1
}
extern "C" fn motion(_: Ptr, x: f64, y: f64, _: Ptr) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.pointer.is_none() && s.strength < 0.01 {
            s.peak = (x, y);
        }
        s.pointer = Some((x, y));
    });
}
extern "C" fn leave(_: Ptr, _: Ptr) {
    STATE.with(|s| s.borrow_mut().pointer = None);
}
extern "C" fn click(_: Ptr, _: c_int, x: f64, y: f64, _: Ptr) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.ripple = Some((x, y, s.started.elapsed().as_secs_f64()));
    });
}
extern "C" fn close(_: Ptr, main_loop: Ptr) -> c_int {
    unsafe {
        g_main_loop_quit(main_loop);
    }
    0
}

pub fn create() -> gtk4::Widget {
    use gtk4::glib::translate::from_glib_none;
    unsafe {
        let area = gtk_drawing_area_new();
        gtk_drawing_area_set_draw_func(area, draw, std::ptr::null_mut(), std::ptr::null_mut());
        gtk_widget_add_tick_callback(area, tick, std::ptr::null_mut(), std::ptr::null_mut());
        from_glib_none(area.cast())
    }
}

// Track the overlay so the background still responds beneath the controls.
pub fn attach(widget: &impl gtk4::prelude::IsA<gtk4::Widget>) {
    use gtk4::prelude::*;
    let pointer = gtk4::EventControllerMotion::new();
    pointer.set_propagation_phase(gtk4::PropagationPhase::Capture);
    pointer.connect_motion(|_, x, y| motion(std::ptr::null_mut(), x, y, std::ptr::null_mut()));
    pointer.connect_leave(|_| leave(std::ptr::null_mut(), std::ptr::null_mut()));
    widget.add_controller(pointer);
    let gesture = gtk4::GestureClick::new();
    gesture.set_propagation_phase(gtk4::PropagationPhase::Capture);
    gesture
        .connect_pressed(|_, n, x, y| click(std::ptr::null_mut(), n, x, y, std::ptr::null_mut()));
    widget.add_controller(gesture);
}
