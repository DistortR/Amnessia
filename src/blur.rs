use gtk4::{glib, prelude::*, subclass::prelude::*};
use std::cell::{Cell, RefCell};

const DURATION: f64 = 0.3;

fn blur_radius(elapsed: f64) -> f64 {
    let t = (elapsed / DURATION).clamp(0.0, 1.0);
    10.0 * (1.0 - t).powi(3)
}

mod imp {
    use super::*;
    #[derive(Default)]
    pub struct BlurRow {
        pub child: RefCell<Option<gtk4::Widget>>,
        pub radius: Cell<f64>,
    }
    #[glib::object_subclass]
    impl ObjectSubclass for BlurRow {
        const NAME: &'static str = "AmnessiaBlurRow";
        type Type = super::BlurRow;
        type ParentType = gtk4::Widget;
    }
    impl ObjectImpl for BlurRow {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().set_layout_manager(Some(gtk4::BinLayout::new()));
        }
        fn dispose(&self) {
            if let Some(child) = self.child.borrow_mut().take() {
                child.unparent();
            }
        }
    }
    impl WidgetImpl for BlurRow {
        fn snapshot(&self, snapshot: &gtk4::Snapshot) {
            if let Some(child) = self.child.borrow().as_ref() {
                let radius = self.radius.get();
                if radius > 0.0 {
                    snapshot.push_blur(radius);
                }
                self.obj().snapshot_child(child, snapshot);
                if radius > 0.0 {
                    snapshot.pop();
                }
            }
        }
    }
}

glib::wrapper! {
    pub struct BlurRow(ObjectSubclass<imp::BlurRow>)
        @extends gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
}

impl BlurRow {
    pub fn new(child: &impl IsA<gtk4::Widget>) -> Self {
        let row: Self = glib::Object::new();
        child.set_parent(&row);
        row.imp().child.replace(Some(child.clone().upcast()));
        row.imp().radius.set(10.0);
        let start = Cell::new(None);
        row.add_tick_callback(move |row, clock| {
            let now = clock.frame_time();
            let first = start.get().unwrap_or_else(|| {
                start.set(Some(now));
                now
            });
            let elapsed = (now - first) as f64 / 1_000_000.0;
            let radius = blur_radius(elapsed);
            row.imp().radius.set(radius);
            row.queue_draw();
            if elapsed >= DURATION {
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
        row
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn focus_animation_starts_blurred_and_ends_sharp() {
        assert_eq!(blur_radius(0.0), 10.0);
        assert!(blur_radius(0.15) > 0.0);
        assert_eq!(blur_radius(0.3), 0.0);
        assert_eq!(blur_radius(1.0), 0.0);
    }
}
