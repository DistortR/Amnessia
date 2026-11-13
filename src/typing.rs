use gtk4::{glib, prelude::*, Box as GtkBox, Label, Orientation};
use std::cell::Cell;

const MESSAGE: &str = "Nothing to remember";

fn visible_chars(elapsed: f64) -> usize {
    ((elapsed.clamp(0.0, 1.0) * MESSAGE.len() as f64).floor() as usize).min(MESSAGE.len())
}

pub fn empty_message() -> GtkBox {
    let container = GtkBox::new(Orientation::Horizontal, 2);
    container.set_halign(gtk4::Align::Center);
    let label = Label::new(None);
    label.update_property(&[gtk4::accessible::Property::Label(MESSAGE)]);
    let cursor = Label::new(Some("▏"));
    cursor.update_state(&[gtk4::accessible::State::Hidden(true)]);
    container.append(&label);
    container.append(&cursor);
    if !gtk4::Settings::default().is_none_or(|s| s.is_gtk_enable_animations()) {
        label.set_text(MESSAGE);
        cursor.set_visible(false);
        return container;
    }
    let start = Cell::new(None);
    container.add_tick_callback(move |_, clock| {
        let now = clock.frame_time();
        let first = start.get().unwrap_or_else(|| {
            start.set(Some(now));
            now
        });
        let elapsed = (now - first) as f64 / 1_000_000.0;
        let length = visible_chars(elapsed);
        label.set_text(&MESSAGE[..length]);
        if elapsed >= 1.0 {
            cursor.add_css_class("typing-cursor");
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
    container
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn types_the_full_message_in_one_second() {
        assert_eq!(visible_chars(0.0), 0);
        assert!(visible_chars(0.5) > 0 && visible_chars(0.5) < MESSAGE.len());
        assert_eq!(visible_chars(1.0), MESSAGE.len());
        assert_eq!(visible_chars(2.0), MESSAGE.len());
    }
}
