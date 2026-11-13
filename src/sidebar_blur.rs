use gtk4::{glib, prelude::*, subclass::prelude::*};
use std::cell::{Cell, RefCell};

mod imp {
    use super::*;
    #[derive(Default)]
    pub struct SidebarBlur {
        pub background: RefCell<Option<gtk4::gdk::Paintable>>,
        pub source: RefCell<Option<gtk4::Widget>>,
        pub corner_radius: Cell<f32>,
        pub blur_radius: Cell<f64>,
    }
    #[glib::object_subclass]
    impl ObjectSubclass for SidebarBlur {
        const NAME: &'static str = "AmnessiaSidebarBlur";
        type Type = super::SidebarBlur;
        type ParentType = gtk4::Widget;
    }
    impl ObjectImpl for SidebarBlur {}
    impl WidgetImpl for SidebarBlur {
        fn snapshot(&self, snapshot: &gtk4::Snapshot) {
            let Some(background) = self.background.borrow().as_ref().cloned() else {
                return;
            };
            let Some(source) = self.source.borrow().as_ref().cloned() else {
                return;
            };
            let width = background.intrinsic_width();
            let height = background.intrinsic_height();
            if width <= 0 || height <= 0 {
                return;
            }
            let Some(origin) = self
                .obj()
                .compute_point(&source, &gtk4::graphene::Point::new(0.0, 0.0))
            else {
                return;
            };
            let bounds = gtk4::graphene::Rect::new(
                0.0,
                0.0,
                self.obj().width() as f32,
                self.obj().height() as f32,
            );
            let clip = gtk4::gsk::RoundedRect::from_rect(bounds, self.corner_radius.get());
            snapshot.push_rounded_clip(&clip);
            snapshot.save();
            snapshot.translate(&gtk4::graphene::Point::new(-origin.x(), -origin.y()));
            snapshot.push_blur(self.blur_radius.get());
            background.snapshot(snapshot, width as f64, height as f64);
            snapshot.pop();
            snapshot.restore();
            snapshot.pop();
        }
    }
}

glib::wrapper! {
    pub struct SidebarBlur(ObjectSubclass<imp::SidebarBlur>)
        @extends gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
}

impl SidebarBlur {
    pub fn with_blur_radius(self, radius: f64) -> Self {
        self.imp().blur_radius.set(radius);
        self
    }

    pub fn new(background: &impl IsA<gtk4::Widget>, corner_radius: f32) -> Self {
        let widget: Self = glib::Object::new();
        widget.set_can_target(false);
        let background = background.as_ref();
        let paintable = gtk4::WidgetPaintable::new(Some(background));
        let weak = widget.downgrade();
        paintable.connect_invalidate_contents(move |_| {
            if let Some(widget) = weak.upgrade() {
                widget.queue_draw();
            }
        });
        widget.imp().background.replace(Some(paintable.upcast()));
        widget.imp().source.replace(Some(background.clone()));
        widget.imp().blur_radius.set(12.0);
        widget.imp().corner_radius.set(corner_radius);
        widget
    }
}
