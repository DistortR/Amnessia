use crate::highlight::HighlightSelector;
use gtk4::{prelude::*, Overlay};

const FILTERS: [(&str, &str); 3] = [("all", "All"), ("resolved", "Resolved"), ("faded", "Faded")];

#[derive(Clone)]
pub struct Filters {
    pub widget: Overlay,
    selector: HighlightSelector,
}

impl Filters {
    pub fn new() -> Self {
        let selector = HighlightSelector::new(&["All", "Resolved", "Faded"], 0);
        selector.widget.add_css_class("filter-selector");
        Self {
            widget: selector.widget.clone(),
            selector,
        }
    }

    pub fn selected(&self) -> &'static str {
        FILTERS[self.selector.selected()].0
    }

    pub fn connect_changed(&self, callback: impl Fn() + 'static) {
        self.selector.connect_changed(callback);
    }
}
