use gtk4::{glib, prelude::*, Button, DrawingArea, Overlay};
use std::{cell::Cell, rc::Rc};

fn sweep_position(progress: f64, width: f64) -> f64 {
    (-0.4 + 1.8 * progress.clamp(0.0, 1.0)) * width
}

pub fn attach(button: &impl IsA<Button>) {
    let button = button.as_ref();
    if button.has_css_class("shine-attached") || button.has_css_class("flash-edit") {
        return;
    }
    let Some(child) = button.child() else {
        return;
    };
    button.add_css_class("shine-attached");
    button.set_child(gtk4::Widget::NONE);
    let overlay = Overlay::new();
    overlay.set_child(Some(&child));
    let area = DrawingArea::new();
    area.set_hexpand(true);
    area.set_vexpand(true);
    area.set_can_target(false);
    let progress = Rc::new(Cell::new(1.0_f64));
    let drawn = progress.clone();
    area.set_draw_func(move |_, cr, width, height| {
        let t = drawn.get();
        if t >= 1.0 {
            return;
        }
        let w = width as f64;
        let center = sweep_position(t, w);
        let stripe = (w * 0.35).max(20.0);
        let gradient = gtk4::cairo::LinearGradient::new(
            center - stripe,
            0.0,
            center + stripe,
            height as f64 * 0.25,
        );
        gradient.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.0);
        gradient.add_color_stop_rgba(0.5, 1.0, 1.0, 1.0, 0.32);
        gradient.add_color_stop_rgba(1.0, 1.0, 1.0, 1.0, 0.0);
        let _ = cr.set_source(&gradient);
        let _ = cr.paint();
    });
    overlay.add_overlay(&area);
    button.set_child(Some(&overlay));
    let motion = gtk4::EventControllerMotion::new();
    motion.set_propagation_phase(gtk4::PropagationPhase::Capture);
    let generation = Rc::new(Cell::new(0_u64));
    motion.connect_enter(move |_, _, _| {
        if !gtk4::Settings::default().is_none_or(|s| s.is_gtk_enable_animations()) {
            return;
        }
        let serial = generation.get().wrapping_add(1);
        generation.set(serial);
        progress.set(0.0);
        let start = Cell::new(None);
        let progress = progress.clone();
        let generation = generation.clone();
        area.add_tick_callback(move |area, clock| {
            if generation.get() != serial {
                return glib::ControlFlow::Break;
            }
            let now = clock.frame_time();
            let first = start.get().unwrap_or_else(|| {
                start.set(Some(now));
                now
            });
            let time = ((now - first) as f64 / 1_200_000.0).clamp(0.0, 1.0);
            progress.set(time * time * (3.0 - 2.0 * time));
            area.queue_draw();
            if time >= 1.0 {
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
    });
    button.add_controller(motion);
}

pub fn attach_tree(widget: &impl IsA<gtk4::Widget>) {
    let widget = widget.as_ref();
    if let Some(button) = widget.downcast_ref::<Button>() {
        attach(button);
        return;
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        attach_tree(&current);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sweep_crosses_the_button() {
        assert!(sweep_position(0.0, 100.0) < 0.0);
        assert!((sweep_position(0.5, 100.0) - 50.0).abs() < 0.001);
        assert!(sweep_position(1.0, 100.0) > 100.0);
    }
}
