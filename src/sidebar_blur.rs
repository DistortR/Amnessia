use gtk4::{glib, prelude::*, subclass::prelude::*};
use std::cell::RefCell;

mod imp {
    use super::*;
    #[derive(Default)]
    pub struct SidebarBlur {
        pub background: RefCell<Option<gtk4::WidgetPaintable>>,
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
            let width = background.intrinsic_width();
            let height = background.intrinsic_height();
            if width <= 0 || height <= 0 {
                return;
            }
            let bounds = gtk4::graphene::Rect::new(
                0.0,
                0.0,
                self.obj().width() as f32,
                self.obj().height() as f32,
            );
            snapshot.push_clip(&bounds);
            snapshot.push_blur(12.0);
            background.snapshot(snapshot, width as f64, height as f64);
            snapshot.pop();
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
    pub fn new(background: &impl IsA<gtk4::Widget>) -> Self {
        let widget: Self = glib::Object::new();
        widget.set_can_target(false);
        let paintable = gtk4::WidgetPaintable::new(Some(background));
        let weak = widget.downgrade();
        paintable.connect_invalidate_contents(move |_| {
            if let Some(widget) = weak.upgrade() {
                widget.queue_draw();
            }
        });
        widget.imp().background.replace(Some(paintable));
        widget
    }
}
