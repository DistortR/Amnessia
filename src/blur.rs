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
        pub tilt_x: Cell<f64>,
        pub tilt_y: Cell<f64>,
        pub target_x: Cell<f64>,
        pub target_y: Cell<f64>,
        pub ticking: Cell<bool>,
        pub lift: Cell<f64>,
        pub target_lift: Cell<f64>,
        pub hover_lift: Cell<f64>,
        pub hover_tilt: Cell<f64>,
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
                let width = self.obj().width() as f32;
                let height = self.obj().height() as f32;
                snapshot.save();
                snapshot.translate(&gtk4::graphene::Point::new(0.0, -self.lift.get() as f32));
                snapshot.translate(&gtk4::graphene::Point::new(width / 2.0, height / 2.0));
                snapshot.perspective(800.0);
                snapshot.rotate_3d(
                    self.tilt_x.get() as f32,
                    &gtk4::graphene::Vec3::new(1.0, 0.0, 0.0),
                );
                snapshot.rotate_3d(
                    self.tilt_y.get() as f32,
                    &gtk4::graphene::Vec3::new(0.0, 1.0, 0.0),
                );
                snapshot.translate(&gtk4::graphene::Point::new(-width / 2.0, -height / 2.0));
                let radius = self.radius.get();
                if radius > 0.0 {
                    snapshot.push_blur(radius);
                }
                self.obj().snapshot_child(child, snapshot);
                if radius > 0.0 {
                    snapshot.pop();
                }
                snapshot.restore();
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
    pub fn with_hover_tilt(self, degrees: f64) -> Self {
        self.imp().hover_tilt.set(degrees);
        self
    }

    pub fn with_hover_lift(self, pixels: f64) -> Self {
        self.imp().hover_lift.set(pixels);
        self
    }

    pub fn new(
        child: &impl IsA<gtk4::Widget>,
        animate_entrance: bool,
        max_tilt_degrees: f64,
    ) -> Self {
        let row: Self = glib::Object::new();
        child.set_parent(&row);
        row.imp().child.replace(Some(child.clone().upcast()));
        row.set_overflow(gtk4::Overflow::Visible);
        let motion = gtk4::EventControllerMotion::new();
        motion.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let weak = row.downgrade();
        motion.connect_enter(move |_, _, _| {
            if let Some(row) = weak.upgrade() {
                if gtk4::Settings::default().is_none_or(|s| s.is_gtk_enable_animations()) {
                    row.imp().target_lift.set(row.imp().hover_lift.get());
                    row.imp().target_x.set(-row.imp().hover_tilt.get());
                    row.imp().target_y.set(row.imp().hover_tilt.get() * 0.5);
                    animate_tilt(&row);
                }
            }
        });
        let weak = row.downgrade();
        motion.connect_motion(move |_, x, y| {
            let Some(row) = weak.upgrade() else {
                return;
            };
            if !gtk4::Settings::default().is_none_or(|s| s.is_gtk_enable_animations()) {
                return;
            }
            let (tx, ty) = tilt_for_pointer(
                x,
                y,
                row.width() as f64,
                row.height() as f64,
                max_tilt_degrees,
            );
            row.imp().target_x.set(tx - row.imp().hover_tilt.get());
            row.imp()
                .target_y
                .set(ty + row.imp().hover_tilt.get() * 0.5);
            animate_tilt(&row);
        });
        let weak = row.downgrade();
        motion.connect_leave(move |_| {
            if let Some(row) = weak.upgrade() {
                row.imp().target_lift.set(0.0);
                row.imp().target_x.set(0.0);
                row.imp().target_y.set(0.0);
                animate_tilt(&row);
            }
        });
        row.add_controller(motion);
        if !animate_entrance {
            return row;
        }
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

fn tilt_for_pointer(x: f64, y: f64, width: f64, height: f64, max_tilt_degrees: f64) -> (f64, f64) {
    if width <= 0.0 || height <= 0.0 {
        return (0.0, 0.0);
    }
    (
        (0.5 - y / height).clamp(-0.5, 0.5) * max_tilt_degrees * 2.0,
        (x / width - 0.5).clamp(-0.5, 0.5) * max_tilt_degrees * 2.0,
    )
}

fn animate_tilt(row: &BlurRow) {
    if row.imp().ticking.replace(true) {
        return;
    }
    let last = Cell::new(None);
    row.add_tick_callback(move |row, clock| {
        let imp = row.imp();
        let now = clock.frame_time();
        let dt = last.replace(Some(now)).map_or(1.0 / 60.0, |previous| {
            ((now - previous) as f64 / 1_000_000.0).clamp(0.0, 0.05)
        });
        let factor = 1.0 - (-dt * 16.0).exp();
        let x = imp.tilt_x.get() + (imp.target_x.get() - imp.tilt_x.get()) * factor;
        let y = imp.tilt_y.get() + (imp.target_y.get() - imp.tilt_y.get()) * factor;
        let lift = imp.lift.get() + (imp.target_lift.get() - imp.lift.get()) * factor;
        imp.lift.set(lift);
        if (x - imp.target_x.get()).abs() < 0.01
            && (y - imp.target_y.get()).abs() < 0.01
            && (lift - imp.target_lift.get()).abs() < 0.01
        {
            imp.lift.set(imp.target_lift.get());
            imp.tilt_x.set(imp.target_x.get());
            imp.tilt_y.set(imp.target_y.get());
            imp.ticking.set(false);
            row.queue_draw();
            glib::ControlFlow::Break
        } else {
            imp.tilt_x.set(x);
            imp.tilt_y.set(y);
            row.queue_draw();
            glib::ControlFlow::Continue
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tilt_points_toward_pointer() {
        assert_eq!(tilt_for_pointer(50.0, 50.0, 100.0, 100.0, 2.0), (0.0, 0.0));
        assert_eq!(tilt_for_pointer(100.0, 0.0, 100.0, 100.0, 2.0), (2.0, 2.0));
        assert_eq!(tilt_for_pointer(0.0, 0.0, 0.0, 0.0, 2.0), (0.0, 0.0));
    }
    #[test]
    fn focus_animation_starts_blurred_and_ends_sharp() {
        assert_eq!(blur_radius(0.0), 10.0);
        assert!(blur_radius(0.15) > 0.0);
        assert_eq!(blur_radius(0.3), 0.0);
        assert_eq!(blur_radius(1.0), 0.0);
    }
}
