use gtk4::{glib, prelude::*, Box as GtkBox, DrawingArea, Orientation, Overlay, ToggleButton};
use std::{cell::Cell, rc::Rc};

#[derive(Clone)]
pub struct HighlightSelector {
    pub widget: Overlay,
    buttons: Vec<ToggleButton>,
    selected: Rc<Cell<usize>>,
    position: Rc<Cell<f64>>,
    generation: Rc<Cell<u64>>,
    backdrop: DrawingArea,
}

impl HighlightSelector {
    pub fn new(labels: &[&str], selected: usize) -> Self {
        let selected = selected.min(labels.len() - 1);
        let widget = Overlay::new();
        widget.add_css_class("highlight-selector");
        let backdrop = DrawingArea::new();
        backdrop.set_content_width(labels.len() as i32 * 92);
        backdrop.set_content_height(40);
        backdrop.set_can_target(false);
        let position = Rc::new(Cell::new(selected as f64));
        let current = position.clone();
        let count = labels.len() as f64;
        backdrop.set_draw_func(move |area, cr, width, height| {
            let popup = area
                .parent()
                .is_some_and(|parent| parent.has_css_class("popup-selector"));
            let inset = if popup { 7.0 } else { 3.0 };
            let w = width as f64;
            let h = height as f64;
            let radius = 14.0_f64.min(h / 2.0).min(w / 2.0);
            rounded_rect(cr, 0.0, 0.0, w, h, radius);
            if popup {
                cr.set_source_rgba(17.0 / 255.0, 17.0 / 255.0, 27.0 / 255.0, 0.48);
            } else {
                cr.set_source_rgba(0.24, 0.25, 0.29, 0.8);
            }
            let _ = cr.fill();
            let segment = w / count;
            let x = current.get() * segment + inset;
            let y = inset;
            let width = (segment - inset * 2.0).max(0.0);
            let height = (h - inset * 2.0).max(0.0);
            let radius = (height / 2.0).min(12.0);
            rounded_rect(cr, x, y, width, height, radius);
            if popup {
                cr.set_source_rgb(93.0 / 255.0, 58.0 / 255.0, 156.0 / 255.0);
            } else {
                cr.set_source_rgba(0.27, 0.13, 0.42, 0.94);
            }
            let _ = cr.fill();
        });
        widget.set_child(Some(&backdrop));
        let row = GtkBox::new(Orientation::Horizontal, 0);
        row.set_homogeneous(true);
        let mut buttons: Vec<ToggleButton> = Vec::new();
        for label in labels {
            let button = ToggleButton::with_label(label);
            button.add_css_class("highlight-button");
            if let Some(first) = buttons.first() {
                button.set_group(Some(first));
            }
            row.append(&button);
            buttons.push(button);
        }
        buttons[selected].set_active(true);
        widget.add_overlay(&row);
        widget.set_measure_overlay(&row, true);
        Self {
            widget,
            buttons,
            selected: Rc::new(Cell::new(selected)),
            position,
            generation: Rc::new(Cell::new(0)),
            backdrop,
        }
    }

    pub fn set_icons(&self, icons: &[&str]) {
        for (button, icon) in self.buttons.iter().zip(icons) {
            let text = button.label().unwrap_or_default();
            let row = crate::icon_text(&text, icon);
            row.set_halign(gtk4::Align::Center);
            button.set_child(Some(&row));
        }
    }

    pub fn selected(&self) -> usize {
        self.selected.get()
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

fn rounded_rect(cr: &gtk4::cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
    cr.arc(
        x + r,
        y + h - r,
        r,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
    );
    cr.arc(
        x + r,
        y + r,
        r,
        std::f64::consts::PI,
        3.0 * std::f64::consts::FRAC_PI_2,
    );
    cr.close_path();
}
