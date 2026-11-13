use gtk4::{glib, prelude::*, Box as GtkBox, DrawingArea, Orientation, Overlay, ToggleButton};
use std::{cell::Cell, rc::Rc};

const FILTERS: [(&str, &str); 3] = [("all", "All"), ("resolved", "Resolved"), ("faded", "Faded")];

#[derive(Clone)]
pub struct Filters {
    pub widget: Overlay,
    buttons: Vec<ToggleButton>,
    selected: Rc<Cell<usize>>,
    position: Rc<Cell<f64>>,
    generation: Rc<Cell<u64>>,
    backdrop: DrawingArea,
}

impl Filters {
    pub fn new() -> Self {
        let widget = Overlay::new();
        widget.add_css_class("filter-selector");
        let backdrop = DrawingArea::new();
        backdrop.set_content_width(276);
        backdrop.set_content_height(40);
        backdrop.set_can_target(false);
        let position = Rc::new(Cell::new(0.0));
        let current = position.clone();
        backdrop.set_draw_func(move |_, cr, width, height| {
            let segment = width as f64 / 3.0;
            cr.set_source_rgba(0.24, 0.25, 0.29, 0.8);
            let w = width as f64;
            let h = height as f64;
            let r = 14.0_f64.min(h / 2.0).min(w / 2.0);
            cr.new_sub_path();
            cr.arc(w - r, r, r, -std::f64::consts::FRAC_PI_2, 0.0);
            cr.arc(w - r, h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
            cr.arc(
                r,
                h - r,
                r,
                std::f64::consts::FRAC_PI_2,
                std::f64::consts::PI,
            );
            cr.arc(
                r,
                r,
                r,
                std::f64::consts::PI,
                3.0 * std::f64::consts::FRAC_PI_2,
            );
            cr.close_path();
            let _ = cr.fill();
            cr.set_source_rgba(0.78, 0.80, 0.84, 0.48);
            let x = current.get() * segment + 3.0;
            let y = 3.0;
            let w = (segment - 6.0).max(0.0);
            let h = (height as f64 - 6.0).max(0.0);
            let radius = (h / 2.0).min(12.0);
            cr.new_sub_path();
            cr.arc(
                x + w - radius,
                y + radius,
                radius,
                -std::f64::consts::FRAC_PI_2,
                0.0,
            );
            cr.arc(
                x + w - radius,
                y + h - radius,
                radius,
                0.0,
                std::f64::consts::FRAC_PI_2,
            );
            cr.arc(
                x + radius,
                y + h - radius,
                radius,
                std::f64::consts::FRAC_PI_2,
                std::f64::consts::PI,
            );
            cr.arc(
                x + radius,
                y + radius,
                radius,
                std::f64::consts::PI,
                3.0 * std::f64::consts::FRAC_PI_2,
            );
            cr.close_path();
            let _ = cr.fill();
        });
        widget.set_child(Some(&backdrop));
        let row = GtkBox::new(Orientation::Horizontal, 0);
        row.set_homogeneous(true);
        let mut buttons = Vec::new();
        for (_, label) in FILTERS {
            let button = ToggleButton::with_label(label);
            button.add_css_class("filter-button");
            if let Some(first) = buttons.first() {
                button.set_group(Some(first));
            }
            row.append(&button);
            buttons.push(button);
        }
        buttons[0].set_active(true);
        widget.add_overlay(&row);
        widget.set_measure_overlay(&row, true);
        Self {
            widget,
            buttons,
            selected: Rc::new(Cell::new(0)),
            position,
            generation: Rc::new(Cell::new(0)),
            backdrop,
        }
    }

    pub fn selected(&self) -> &'static str {
        FILTERS[self.selected.get()].0
    }

    pub fn connect_changed(&self, callback: impl Fn() + 'static) {
        let callback = Rc::new(callback);
        for (index, button) in self.buttons.iter().enumerate() {
            let this = self.clone();
            let callback = callback.clone();
            button.connect_toggled(move |button| {
                if !button.is_active() {
                    return;
                }
                this.selected.set(index);
                let generation = this.generation.get().wrapping_add(1);
                this.generation.set(generation);
                if !gtk4::Settings::default().is_none_or(|s| s.is_gtk_enable_animations()) {
                    this.position.set(index as f64);
                    this.backdrop.queue_draw();
                    callback();
                    return;
                }
                let start_position = this.position.get();
                let start = Cell::new(None);
                let position = this.position.clone();
                let animation_generation = this.generation.clone();
                this.backdrop.add_tick_callback(move |area, clock| {
                    if animation_generation.get() != generation {
                        return glib::ControlFlow::Break;
                    }
                    let now = clock.frame_time();
                    let first = start.get().unwrap_or_else(|| {
                        start.set(Some(now));
                        now
                    });
                    let t = ((now - first) as f64 / 250_000.0).clamp(0.0, 1.0);
                    position.set(
                        start_position
                            + (index as f64 - start_position) * (1.0 - (1.0 - t).powi(3)),
                    );
                    area.queue_draw();
                    if t >= 1.0 {
                        glib::ControlFlow::Break
                    } else {
                        glib::ControlFlow::Continue
                    }
                });
                callback();
            });
        }
    }
}
