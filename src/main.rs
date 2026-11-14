mod background;
mod blur;
mod db;
mod filters;
mod highlight;
mod repeat;
mod shine;
mod sidebar_blur;
mod tray;
mod typing;

use gtk4::{
    prelude::*, Application, ApplicationWindow, Box as GtkBox, Button, Calendar, ComboBoxText,
    Dialog, DrawingArea, Entry, Label, Orientation, Overlay, ResponseType, ScrolledWindow,
};
use rusqlite::Connection;
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    rc::Rc,
};

const DEFAULT_EPISODE_COLOR: &str = "#7C4DFF";
const EPISODE_EMOJIS: [&str; 7] = ["💼", "🏠", "🧠", "❤️", "📚", "🎯", "💡"];
const EPISODE_COLORS: [&str; 5] = ["#7C4DFF", "#3B82F6", "#14B8A6", "#EC4899", "#F59E0B"];
const EARLY_REMINDER_OPTIONS: [(&str, i64); 5] = [
    ("5 min", 5),
    ("15 min", 15),
    ("30 min", 30),
    ("1 hour", 60),
    ("1 day", 1_440),
];

#[derive(Clone)]
struct Ui {
    db: Rc<Connection>,
    selected: Rc<Cell<Option<i64>>>,
    background: gtk4::Widget,
    episodes: GtkBox,
    flashes: GtkBox,
    title: Label,
    window: ApplicationWindow,
    filter: filters::Filters,
    error: Label,
    loaded_episodes: Rc<RefCell<HashSet<i64>>>,
}

fn show_error(ui: &Ui, error: impl std::fmt::Display) {
    ui.error.set_text(&error.to_string());
}
fn clear(container: &GtkBox) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}
fn refresh_flashes(ui: &Ui) {
    clear(&ui.flashes);
    let Some(id) = ui.selected.get() else {
        ui.title.set_text("Select an Episode");
        return;
    };
    let filter = ui.filter.selected();
    match db::flashes(&ui.db, id).map(|items| {
        items
            .into_iter()
            .filter(|f| filter == "all" || f.status == filter)
            .collect::<Vec<_>>()
    }) {
        Ok(items) => {
            let animate_entrance = ui.loaded_episodes.borrow_mut().insert(id);
            if items.is_empty() {
                ui.flashes.append(&typing::empty_message());
                return;
            }
            for flash in items {
                let db::Flash {
                    id,
                    title,
                    status,
                    intensity,
                    due_at,
                    ..
                } = flash.clone();
                let row = GtkBox::new(Orientation::Horizontal, 12);
                row.add_css_class("flash-row");
                row.set_hexpand(true);
                let label = Label::new(Some(&title));
                label.set_xalign(0.0);
                label.set_hexpand(true);
                label.set_valign(gtk4::Align::Center);
                label.set_wrap(true);
                if status == "resolved" {
                    label.add_css_class("resolved");
                }
                let importance = Label::new(Some(&"!".repeat(intensity as usize)));
                importance.add_css_class("intensity");
                importance.set_valign(gtk4::Align::Center);
                let button = Button::new();
                button.set_valign(gtk4::Align::Center);
                let resolved = status == "resolved";
                button.set_child(Some(&bulb(resolved)));
                button.add_css_class("bulb-button");
                button.set_tooltip_text(Some(if resolved {
                    "Mark as active"
                } else {
                    "Mark as resolved"
                }));
                button.update_property(&[gtk4::accessible::Property::Label(if resolved {
                    "Mark as active"
                } else {
                    "Mark as resolved"
                })]);
                if let Some(due) = due_at {
                    let local = gtk4::glib::DateTime::from_iso8601(&due, None)
                        .and_then(|d| d.to_local())
                        .and_then(|d| d.format("%Y-%m-%d %H:%M"))
                        .map(|v| v.to_string())
                        .unwrap_or(due);
                    label.set_tooltip_text(Some(&format!("Reminder: {local}")));
                }
                let details = GtkBox::new(Orientation::Horizontal, 12);
                details.set_valign(gtk4::Align::Center);
                details.append(&label);
                if filter == "all" && (status == "resolved" || status == "faded") {
                    let badge = Label::new(Some(if status == "resolved" {
                        "Resolved"
                    } else {
                        "Faded"
                    }));
                    badge.add_css_class("status");
                    badge.set_valign(gtk4::Align::Center);
                    badge.set_margin_bottom(2);
                    details.append(&badge);
                }
                details.append(&importance);
                let edit = Button::new();
                edit.add_css_class("flash-edit");
                edit.set_hexpand(true);
                edit.set_valign(gtk4::Align::Center);
                edit.set_child(Some(&details));
                edit.set_tooltip_text(Some("Edit Flash"));
                let edit_ui = ui.clone();
                let edit_flash = flash.clone();
                edit.connect_clicked(move |_| flash_dialog(&edit_ui, Some(edit_flash.clone())));
                let cloned = ui.clone();
                let status_flash = flash.clone();
                button.connect_clicked(move |_| {
                    let next = if status == "resolved" {
                        "active"
                    } else {
                        "resolved"
                    };
                    let next_due = (|| -> Result<_, gtk4::glib::BoolError> {
                        if next != "active" || !status_flash.repeat_rule.is_repeating() {
                            return Ok(None);
                        }
                        let Some(due) = status_flash.due_at.as_deref() else {
                            return Ok(None);
                        };
                        let due_date = gtk4::glib::DateTime::from_iso8601(due, None)?;
                        let now = gtk4::glib::DateTime::now_utc()?;
                        if due_date.to_unix() <= now.to_unix() {
                            repeat::next_due_at(&status_flash.repeat_rule, due, &now)
                        } else {
                            Ok(None)
                        }
                    })();
                    match next_due {
                        Ok(next_due) => {
                            match db::set_status(&cloned.db, id, next, next_due.as_deref()) {
                                Ok(()) => {
                                    cloned.error.set_text("");
                                    refresh_flashes(&cloned);
                                }
                                Err(e) => show_error(&cloned, e),
                            }
                        }
                        Err(e) => show_error(&cloned, e),
                    }
                });
                let bulb = shine::wrap(&button);
                bulb.set_valign(gtk4::Align::Center);
                bulb.set_margin_bottom(2);
                row.append(&bulb);
                row.append(&edit);
                let surface = Overlay::new();
                surface.add_css_class("flash-surface");
                surface.set_overflow(gtk4::Overflow::Hidden);
                surface.set_child(Some(&sidebar_blur::SidebarBlur::new(&ui.background, 8.0)));
                surface.add_overlay(&row);
                surface.set_measure_overlay(&row, true);
                ui.flashes
                    .append(&blur::BlurRow::new(&surface, animate_entrance, 1.0));
            }
        }
        Err(e) => show_error(ui, e),
    }
}
fn refresh_episodes(ui: &Ui) {
    clear(&ui.episodes);
    match db::episodes(&ui.db) {
        Ok(items) => {
            if ui.selected.get().is_none() {
                ui.selected.set(items.first().map(|episode| episode.id));
            }
            for episode in items {
                let id = episode.id;
                let button = Button::new();
                button.add_css_class("episode");
                button.set_hexpand(true);
                let metadata = GtkBox::new(Orientation::Horizontal, 8);
                if !episode.emoji.is_empty() {
                    metadata.append(&Label::new(Some(&episode.emoji)));
                }
                if !episode.is_default {
                    let color = Label::new(Some("●"));
                    color.set_markup(&format!("<span foreground=\"{}\">●</span>", episode.color));
                    metadata.append(&color);
                }
                let name = Label::new(Some(&episode.name));
                name.set_valign(gtk4::Align::Center);
                if episode.is_default {
                    name.set_xalign(0.5);
                    let details = gtk4::CenterBox::new();
                    details.set_start_widget(Some(&metadata));
                    details.set_center_widget(Some(&name));
                    button.set_child(Some(&details));
                } else {
                    name.set_xalign(0.0);
                    name.set_hexpand(true);
                    let details = GtkBox::new(Orientation::Horizontal, 8);
                    details.append(&metadata);
                    details.append(&name);
                    button.set_child(Some(&details));
                }
                if ui.selected.get() == Some(id) {
                    button.add_css_class("selected-episode");
                    let title = if episode.emoji.is_empty() {
                        episode.name.clone()
                    } else {
                        format!("{} {}", episode.emoji, episode.name)
                    };
                    ui.title.set_text(&title);
                }
                let cloned = ui.clone();
                button.connect_clicked(move |_| {
                    cloned.selected.set(Some(id));
                    refresh_episodes(&cloned);
                });

                let row = GtkBox::new(Orientation::Horizontal, 4);
                row.append(&button);
                if !episode.is_default {
                    let edit = Button::with_label("✎");
                    edit.add_css_class("episode-edit");
                    edit.set_tooltip_text(Some("Edit Episode emoji and color"));
                    edit.update_property(&[gtk4::accessible::Property::Label(
                        "Edit Episode emoji and color",
                    )]);
                    let edit_ui = ui.clone();
                    edit.connect_clicked(move |_| episode_dialog(&edit_ui, Some(episode.clone())));
                    row.append(&edit);
                }
                ui.episodes.append(&blur::BlurRow::new(&row, false, 10.0));
            }
            refresh_flashes(ui);
        }
        Err(e) => show_error(ui, e),
    }
}
fn bulb(on: bool) -> DrawingArea {
    let area = DrawingArea::builder()
        .content_width(26)
        .content_height(26)
        .build();
    area.set_draw_func(move |_, cr, _, _| {
        if on {
            cr.set_source_rgba(0.70, 0.48, 1.0, 0.18);
            cr.arc(13.0, 11.0, 12.0, 0.0, std::f64::consts::TAU);
            let _ = cr.fill();
        }
        if on {
            cr.set_source_rgb(0.76, 0.57, 1.0);
        } else {
            cr.set_source_rgb(0.62, 0.62, 0.70);
        }
        cr.set_line_width(1.8);
        cr.move_to(9.3, 17.2);
        cr.curve_to(9.3, 15.4, 6.7, 14.0, 6.7, 10.3);
        cr.arc(13.0, 10.3, 6.3, std::f64::consts::PI, 0.0);
        cr.curve_to(19.3, 14.0, 16.7, 15.4, 16.7, 17.2);
        cr.close_path();
        if on {
            let _ = cr.fill_preserve();
        }
        let _ = cr.stroke();
        cr.move_to(9.6, 19.4);
        cr.line_to(16.4, 19.4);
        cr.move_to(10.5, 22.0);
        cr.line_to(15.5, 22.0);
        let _ = cr.stroke();
    });
    area
}

fn icon_text(text: &str, icon: &str) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 8);
    row.set_valign(gtk4::Align::Center);
    let image = gtk4::Image::from_icon_name(icon);
    image.set_pixel_size(16);
    row.append(&image);
    let label = Label::new(Some(text));
    label.set_xalign(0.0);
    row.append(&label);
    row
}

fn button_icon(button: &Button, text: &str, icon: &str) {
    let row = icon_text(text, icon);
    row.set_halign(gtk4::Align::Center);
    button.set_child(Some(&row));
}

fn popup_label(text: &str) -> GtkBox {
    let icon = match text {
        "NAME" => "document-edit-symbolic",
        "ICON" => "face-smile-symbolic",
        "COLOR" => "applications-graphics-symbolic",
        "INTENSITY" => "weather-storm-symbolic",
        _ => "alarm-symbolic",
    };
    let row = icon_text(text, icon);
    row.add_css_class("popup-label");
    row
}

fn dismiss_popup(dialog: &Dialog) {
    dialog.set_modal(false);
    dialog.close();
}

fn present_popup(dialog: &Dialog) {
    let Some(parent) = dialog.transient_for() else {
        return;
    };
    let Some(overlay) = parent
        .child()
        .and_then(|child| child.downcast::<Overlay>().ok())
    else {
        return;
    };
    // The scene excludes popup overlays, so a live backdrop cannot capture itself.
    let Some(scene) = overlay.child() else {
        return;
    };
    let glass = sidebar_blur::SidebarBlur::new(&scene, 11.0).with_blur_radius(24.0);
    let Some(frame) = dialog.child() else {
        return;
    };
    dialog.set_child(gtk4::Widget::NONE);
    frame.add_css_class("reference-dialog");
    let card = Overlay::new();
    card.set_halign(gtk4::Align::Center);
    card.set_valign(gtk4::Align::Center);
    card.set_size_request(dialog.default_size().0, -1);
    card.set_child(Some(&glass));
    card.add_overlay(&frame);
    card.set_measure_overlay(&frame, true);
    let shield = GtkBox::new(Orientation::Vertical, 0);
    shield.add_css_class("popup-scrim");
    shield.set_hexpand(true);
    shield.set_vexpand(true);
    overlay.add_overlay(&shield);
    overlay.add_overlay(&card);
    let click = gtk4::GestureClick::new();
    let weak_card = card.downgrade();
    click.connect_pressed(move |_, _, _, _| {
        if let Some(card) = weak_card.upgrade() {
            card.child_focus(gtk4::DirectionType::TabForward);
        }
    });
    shield.add_controller(click);
    let keys = gtk4::EventControllerKey::new();
    keys.set_propagation_phase(gtk4::PropagationPhase::Capture);
    let controller_dialog = dialog.clone();
    let weak_parent = parent.downgrade();
    let weak_card = card.downgrade();
    let handler = keys.connect_key_pressed(move |_, key, _, _| {
        if key == gtk4::gdk::Key::Escape {
            controller_dialog.response(ResponseType::Cancel);
            return gtk4::glib::Propagation::Stop;
        }
        if let (Some(parent), Some(card)) = (weak_parent.upgrade(), weak_card.upgrade()) {
            if gtk4::prelude::GtkWindowExt::focus(&parent)
                .is_some_and(|focus| focus.is_ancestor(&card))
            {
                return gtk4::glib::Propagation::Proceed;
            }
            card.child_focus(gtk4::DirectionType::TabForward);
        }
        gtk4::glib::Propagation::Stop
    });
    parent.add_controller(keys.clone());
    let handler = RefCell::new(Some(handler));
    let weak_parent = parent.downgrade();
    let weak_overlay = overlay.downgrade();
    dialog.connect_modal_notify(move |dialog| {
        if dialog.is_modal() {
            return;
        }
        if let Some(overlay) = weak_overlay.upgrade() {
            overlay.remove_overlay(&card);
            overlay.remove_overlay(&shield);
        }
        if let Some(parent) = weak_parent.upgrade() {
            parent.remove_controller(&keys);
        }
        if let Some(handler) = handler.borrow_mut().take() {
            keys.disconnect(handler);
        }
    });
    let mut widgets = vec![frame];
    while let Some(widget) = widgets.pop() {
        if let Some(entry) = widget.downcast_ref::<Entry>() {
            let weak_dialog = dialog.downgrade();
            entry.set_activates_default(false);
            entry.connect_activate(move |_| {
                if let Some(dialog) = weak_dialog.upgrade() {
                    dialog.response(ResponseType::Accept);
                }
            });
        }
        let mut child = widget.first_child();
        while let Some(next) = child {
            child = next.next_sibling();
            widgets.push(next);
        }
    }
    // The dialog remains an unmapped response controller; GTK paints the popup in the main window.
    if let Some(card) = overlay.last_child() {
        card.child_focus(gtk4::DirectionType::TabForward);
    }
}

fn popup_content(dialog: &Dialog, width: i32, heading: &str) -> GtkBox {
    dialog.add_css_class("reference-dialog");
    dialog.set_decorated(false);
    dialog.set_default_size(width, -1);
    let root = dialog.content_area();
    root.set_spacing(0);
    root.set_margin_top(16);
    root.set_margin_bottom(16);
    root.set_margin_start(16);
    root.set_margin_end(16);
    let content = GtkBox::new(Orientation::Vertical, 0);
    let header = icon_text(
        heading,
        if heading.contains("Episode") {
            "folder-symbolic"
        } else {
            "weather-storm-symbolic"
        },
    );
    header.add_css_class("popup-heading");
    content.append(&header);
    root.append(&content);
    content
}

fn dialog_actions(dialog: &Dialog, shine_create: bool) {
    let footer = GtkBox::new(Orientation::Horizontal, 16);
    footer.add_css_class("popup-footer");
    footer.set_halign(gtk4::Align::End);
    for (response, class) in [
        (ResponseType::Cancel, "dialog-cancel"),
        (ResponseType::Accept, "dialog-primary"),
    ] {
        if let Some(widget) = dialog.widget_for_response(response) {
            if let Ok(button) = widget.downcast::<Button>() {
                if let Some(parent) = button.parent().and_then(|p| p.downcast::<GtkBox>().ok()) {
                    parent.remove(&button);
                    if parent.first_child().is_none() {
                        parent.set_visible(false);
                    }
                    let text = button.label().unwrap_or_default();
                    button_icon(
                        &button,
                        &text,
                        if response == ResponseType::Cancel {
                            "window-close-symbolic"
                        } else if shine_create {
                            "list-add-symbolic"
                        } else {
                            "document-save-symbolic"
                        },
                    );
                    button.add_css_class(class);
                    button.set_size_request(96, 38);
                    if response == ResponseType::Accept && shine_create {
                        footer.append(&shine::wrap(&button));
                    } else {
                        footer.append(&button);
                    }
                }
            }
        }
    }
    let divider = gtk4::Separator::new(Orientation::Horizontal);
    divider.add_css_class("popup-divider");
    dialog.content_area().append(&divider);
    dialog.content_area().append(&footer);
}

fn rgba_hex(color: &gtk4::gdk::RGBA) -> String {
    format!(
        "#{:02X}{:02X}{:02X}",
        (color.red() * 255.0).round() as u8,
        (color.green() * 255.0).round() as u8,
        (color.blue() * 255.0).round() as u8
    )
}

fn calendar_label(calendar: &Calendar) -> String {
    calendar
        .date()
        .format("%m/%d/%Y")
        .map(|date| date.to_string())
        .unwrap_or_else(|_| "Choose date".into())
}

fn format_custom_reminder(minutes: i64) -> String {
    if minutes % 1_440 == 0 {
        format!("Custom ({}d)", minutes / 1_440)
    } else if minutes % 60 == 0 {
        format!("Custom ({}h)", minutes / 60)
    } else {
        format!("Custom ({minutes}m)")
    }
}

fn custom_reminder_parts(minutes: i64) -> (i64, u32) {
    if minutes % 1_440 == 0 {
        (minutes / 1_440, 2)
    } else if minutes % 60 == 0 {
        (minutes / 60, 1)
    } else {
        (minutes, 0)
    }
}

fn episode_dialog(ui: &Ui, existing: Option<db::Episode>) {
    let editing = existing.is_some();
    let dialog = Dialog::with_buttons(
        Some(if editing {
            "Customize Episode"
        } else {
            "New Episode"
        }),
        Some(&ui.window),
        gtk4::DialogFlags::MODAL,
        &[
            ("Cancel", ResponseType::Cancel),
            (
                if editing { "Save" } else { "Create" },
                ResponseType::Accept,
            ),
        ],
    );
    let content = popup_content(
        &dialog,
        320,
        if editing {
            "Edit Episode"
        } else {
            "New Episode"
        },
    );
    content.append(&popup_label("NAME"));
    let name = Entry::new();
    name.set_placeholder_text(Some("What's this episode about?"));
    name.set_activates_default(true);
    if let Some(episode) = existing.as_ref() {
        name.set_text(&episode.name);
    }
    content.append(&name);

    let icons = GtkBox::new(Orientation::Vertical, 8);
    icons.add_css_class("popup-section");
    icons.append(&popup_label("ICON"));
    let icon_grid = gtk4::Grid::new();
    icon_grid.set_column_spacing(12);
    icon_grid.set_row_spacing(8);
    icon_grid.set_column_homogeneous(true);
    let selected_emoji = Rc::new(RefCell::new(
        existing
            .as_ref()
            .map(|episode| episode.emoji.clone())
            .unwrap_or_default(),
    ));
    let mut icon_buttons: Vec<gtk4::ToggleButton> = Vec::new();
    for (index, emoji) in EPISODE_EMOJIS.iter().enumerate() {
        let button = gtk4::ToggleButton::with_label(emoji);
        button.add_css_class("episode-icon");
        button.set_size_request(48, 48);
        if let Some(first) = icon_buttons.first() {
            button.set_group(Some(first));
        }
        button.set_active(selected_emoji.borrow().as_str() == *emoji);
        let choice = (*emoji).to_string();
        let selection = selected_emoji.clone();
        button.connect_toggled(move |button| {
            if button.is_active() {
                *selection.borrow_mut() = choice.clone();
            }
        });
        icon_grid.attach(&button, index as i32 % 4, index as i32 / 4, 1, 1);
        icon_buttons.push(button);
    }
    let custom_emoji = gtk4::ToggleButton::with_label("…");
    custom_emoji.add_css_class("episode-icon");
    custom_emoji.set_size_request(48, 48);
    if let Some(first) = icon_buttons.first() {
        custom_emoji.set_group(Some(first));
    }
    let initial_emoji = selected_emoji.borrow().clone();
    let custom_icon_selected =
        !initial_emoji.is_empty() && !EPISODE_EMOJIS.contains(&initial_emoji.as_str());
    if custom_icon_selected {
        custom_emoji.set_label(&initial_emoji);
        custom_emoji.set_active(true);
    }
    let chooser = gtk4::EmojiChooser::new();
    chooser.set_parent(&custom_emoji);
    let custom_emoji_ref = custom_emoji.downgrade();
    let selected_emoji_ref = selected_emoji.clone();
    chooser.connect_emoji_picked(move |chooser, picked| {
        if let Some(button) = custom_emoji_ref.upgrade() {
            button.set_label(picked);
            button.set_active(true);
        }
        *selected_emoji_ref.borrow_mut() = picked.to_string();
        chooser.popdown();
    });
    let chooser_ref = chooser.downgrade();
    custom_emoji.connect_clicked(move |button| {
        button.set_active(true);
        if let Some(chooser) = chooser_ref.upgrade() {
            chooser.popup();
        }
    });
    icon_grid.attach(&custom_emoji, 3, 1, 1, 1);
    icons.append(&icon_grid);
    content.append(&icons);

    let color_section = GtkBox::new(Orientation::Vertical, 8);
    color_section.add_css_class("popup-section");
    color_section.append(&popup_label("COLOR"));
    let color_row = GtkBox::new(Orientation::Horizontal, 10);
    let color_dialog = gtk4::ColorDialog::builder().with_alpha(false).build();
    let color_button = gtk4::ColorDialogButton::new(Some(color_dialog));
    color_button.add_css_class("episode-color-picker");
    color_button.set_size_request(36, 30);
    color_button.set_tooltip_text(Some("Choose a custom color"));
    color_button.update_property(&[gtk4::accessible::Property::Label(
        "Choose a custom Episode color",
    )]);
    let custom_color = Overlay::new();
    custom_color.add_css_class("episode-color-custom");
    custom_color.set_overflow(gtk4::Overflow::Hidden);
    custom_color.set_child(Some(&color_button));
    let brush = gtk4::Image::from_icon_name("draw-brush-symbolic");
    brush.set_can_target(false);
    custom_color.add_overlay(&brush);
    custom_color.set_size_request(36, 30);
    let initial_color = existing
        .as_ref()
        .map(|episode| episode.color.as_str())
        .unwrap_or(DEFAULT_EPISODE_COLOR);
    let initial_rgba = gtk4::gdk::RGBA::parse(initial_color).unwrap_or(gtk4::gdk::RGBA::new(
        124.0 / 255.0,
        77.0 / 255.0,
        255.0 / 255.0,
        1.0,
    ));
    color_button.set_rgba(&initial_rgba);
    let mut color_buttons: Vec<gtk4::ToggleButton> = Vec::new();
    for (index, color) in EPISODE_COLORS.iter().enumerate() {
        let button = gtk4::ToggleButton::new();
        button.add_css_class("episode-color-swatch");
        button.add_css_class(&format!("episode-color-{index}"));
        button.set_size_request(36, 30);
        button.set_tooltip_text(Some(color));
        if let Some(first) = color_buttons.first() {
            button.set_group(Some(first));
        }
        button.set_active(color.eq_ignore_ascii_case(initial_color));
        color_row.append(&button);
        color_buttons.push(button);
    }
    let custom_color_selected = !EPISODE_COLORS
        .iter()
        .any(|color| color.eq_ignore_ascii_case(initial_color));
    if custom_color_selected {
        custom_color.add_css_class("selected-color");
    }
    let changing_preset = Rc::new(Cell::new(false));
    for (button, color) in color_buttons.iter().zip(EPISODE_COLORS) {
        let picker = color_button.clone();
        let custom = custom_color.downgrade();
        let changing_preset = changing_preset.clone();
        button.connect_toggled(move |button| {
            if !button.is_active() {
                return;
            }
            if let Some(custom) = custom.upgrade() {
                custom.remove_css_class("selected-color");
            }
            changing_preset.set(true);
            if let Ok(color) = gtk4::gdk::RGBA::parse(color) {
                picker.set_rgba(&color);
            }
            changing_preset.set(false);
        });
    }
    let weak_presets = color_buttons
        .iter()
        .map(|button| button.downgrade())
        .collect::<Vec<_>>();
    let custom_color_ref = custom_color.downgrade();
    color_button.connect_rgba_notify(move |_| {
        if changing_preset.get() {
            return;
        }
        for button in &weak_presets {
            if let Some(button) = button.upgrade() {
                button.set_active(false);
            }
        }
        if let Some(custom) = custom_color_ref.upgrade() {
            custom.add_css_class("selected-color");
        }
    });
    color_row.append(&custom_color);
    color_section.append(&color_row);
    content.append(&color_section);

    let error = Label::new(None);
    error.add_css_class("error");
    error.set_wrap(true);
    content.append(&error);
    dialog.set_default_response(ResponseType::Accept);
    dialog_actions(&dialog, !editing);
    let ui = ui.clone();
    dialog.connect_response(move |dialog, response| {
        if response != ResponseType::Accept {
            dismiss_popup(dialog);
            return;
        }
        let emoji = selected_emoji.borrow().clone();
        let color = rgba_hex(&color_button.rgba());
        let name = name.text();
        if name.trim().is_empty() {
            error.set_text("Enter an Episode name.");
            return;
        }
        let result = if let Some(episode) = existing.as_ref() {
            db::update_episode(&ui.db, episode.id, &name, &emoji, &color)
        } else {
            db::add_episode(&ui.db, &name, &emoji, &color)
        };
        match result {
            Ok(()) => {
                if !editing {
                    ui.selected.set(Some(ui.db.last_insert_rowid()));
                }
                refresh_episodes(&ui);
                dismiss_popup(dialog);
            }
            Err(e) => error.set_text(&e.to_string()),
        }
    });
    present_popup(&dialog);
}

fn flash_dialog(ui: &Ui, existing: Option<db::Flash>) {
    let Some(episode) = ui.selected.get() else {
        show_error(ui, "Select an Episode first.");
        return;
    };
    let editing = existing.is_some();
    let dialog = Dialog::with_buttons(
        Some(if editing { "Edit Flash" } else { "New Flash" }),
        Some(&ui.window),
        gtk4::DialogFlags::MODAL,
        &[
            ("Cancel", ResponseType::Cancel),
            (
                if editing { "Save" } else { "Create" },
                ResponseType::Accept,
            ),
        ],
    );
    let content = popup_content(
        &dialog,
        500,
        if editing { "Edit Flash" } else { "New Flash" },
    );
    content.append(&popup_label("NAME"));

    let title = Entry::new();
    title.set_placeholder_text(Some("I swear i wont forget"));
    title.set_activates_default(true);
    content.append(&title);

    let intensity_label = popup_label("INTENSITY");
    intensity_label.add_css_class("popup-section");
    content.append(&intensity_label);
    let intensity = highlight::HighlightSelector::new(
        &["Dim", "Bright", "Storbe"],
        existing
            .as_ref()
            .map_or(0, |flash| usize::from(flash.intensity - 1)),
    );
    intensity.set_icons(&[
        "display-brightness-symbolic",
        "weather-clear-symbolic",
        "weather-storm-symbolic",
    ]);
    intensity.connect_changed(|| {});
    intensity.widget.add_css_class("popup-selector");
    content.append(&intensity.widget);

    let calendar = Calendar::new();
    let date_picker = Button::with_label(&calendar_label(&calendar));
    button_icon(
        &date_picker,
        &calendar_label(&calendar),
        "x-office-calendar-symbolic",
    );
    date_picker.add_css_class("date-picker");
    date_picker.set_hexpand(false);
    date_picker.set_size_request(204, 38);
    let calendar_popover = gtk4::Popover::new();
    calendar_popover.set_child(Some(&calendar));
    calendar_popover.set_parent(&date_picker);
    let calendar_popover_ref = calendar_popover.downgrade();
    date_picker.connect_clicked(move |_| {
        if let Some(popover) = calendar_popover_ref.upgrade() {
            popover.popup();
        }
    });
    let date_picker_ref = date_picker.downgrade();
    let calendar_popover_ref = calendar_popover.downgrade();
    calendar.connect_day_selected(move |calendar| {
        if let Some(button) = date_picker_ref.upgrade() {
            button_icon(
                &button,
                &calendar_label(calendar),
                "x-office-calendar-symbolic",
            );
        }
        if let Some(popover) = calendar_popover_ref.upgrade() {
            popover.popdown();
        }
    });

    let time = Entry::new();
    time.set_placeholder_text(Some("Time (HH:MM)"));
    time.set_text("09:00");
    time.set_icon_from_icon_name(
        gtk4::EntryIconPosition::Secondary,
        Some("preferences-system-time-symbolic"),
    );
    gtk4::prelude::EntryExt::set_alignment(&time, 0.5);
    time.set_hexpand(false);
    time.set_size_request(204, 38);

    let date_switch = gtk4::Switch::new();
    date_switch.add_css_class("reminder-switch");
    let date_name = Label::new(Some("Date"));
    let date_left = GtkBox::new(Orientation::Horizontal, 8);
    date_left.append(&date_switch);
    date_left.append(&date_name);
    date_left.set_hexpand(true);
    date_switch.set_valign(gtk4::Align::Center);
    let date_row = GtkBox::new(Orientation::Horizontal, 14);
    date_row.add_css_class("date-time-row");
    date_row.append(&date_left);
    date_row.append(&date_picker);
    let date_time = GtkBox::new(Orientation::Vertical, 6);
    date_time.set_margin_top(12);
    date_time.add_css_class("date-time-controls");
    date_time.append(&date_row);

    let time_switch = gtk4::Switch::new();
    time_switch.add_css_class("reminder-switch");
    let time_name = Label::new(Some("Time"));
    let time_left = GtkBox::new(Orientation::Horizontal, 8);
    time_left.append(&time_switch);
    time_left.append(&time_name);
    time_left.set_hexpand(true);
    time_switch.set_valign(gtk4::Align::Center);
    let time_row = GtkBox::new(Orientation::Horizontal, 14);
    time_row.add_css_class("date-time-row");
    time_row.append(&time_left);
    time_row.append(&time);
    date_time.append(&time_row);
    content.append(&date_time);

    if let Some(flash) = existing.as_ref() {
        title.set_text(&flash.title);
        if let Some(due) = &flash.due_at {
            match gtk4::glib::DateTime::from_iso8601(due, None).and_then(|d| d.to_local()) {
                Ok(local) => {
                    date_switch.set_active(true);
                    time_switch.set_active(true);
                    calendar.select_day(&local);
                    button_icon(
                        &date_picker,
                        &calendar_label(&calendar),
                        "x-office-calendar-symbolic",
                    );
                    if let Ok(formatted) = local.format("%H:%M") {
                        time.set_text(&formatted);
                    }
                }
                Err(_) => {
                    show_error(ui, format!("Cannot edit invalid reminder date: {due}"));
                    return;
                }
            }
        }
    }
    date_picker.set_sensitive(date_switch.is_active());
    time.set_sensitive(time_switch.is_active());
    let time_switch_ref = time_switch.downgrade();
    date_switch.connect_active_notify(move |date_switch| {
        if !date_switch.is_active() {
            if let Some(time_switch) = time_switch_ref.upgrade() {
                time_switch.set_active(false);
            }
        }
    });
    let date_switch_ref = date_switch.downgrade();
    time_switch.connect_active_notify(move |time_switch| {
        if time_switch.is_active() {
            if let Some(date_switch) = date_switch_ref.upgrade() {
                date_switch.set_active(true);
            }
        }
    });
    let date_picker_ref = date_picker.downgrade();
    date_switch.connect_active_notify(move |date_switch| {
        if let Some(button) = date_picker_ref.upgrade() {
            button.set_sensitive(date_switch.is_active());
        }
    });
    let time_ref = time.downgrade();
    time_switch.connect_active_notify(move |time_switch| {
        if let Some(entry) = time_ref.upgrade() {
            entry.set_sensitive(time_switch.is_active());
        }
    });

    let earlier = GtkBox::new(Orientation::Vertical, 8);
    earlier.add_css_class("popup-section");
    let earlier_label = popup_label("REMIND ME EARLIER");
    earlier.append(&earlier_label);
    let earlier_grid = gtk4::Grid::new();
    earlier_grid.set_column_spacing(8);
    earlier_grid.set_row_spacing(8);
    earlier_grid.set_column_homogeneous(true);
    let saved_early = existing
        .as_ref()
        .map(|flash| flash.early_reminders.as_slice())
        .unwrap_or(&[]);
    let mut early_buttons = Vec::new();
    for (index, (label, offset)) in EARLY_REMINDER_OPTIONS.iter().enumerate() {
        let button = gtk4::ToggleButton::with_label(label);
        button.add_css_class("reminder-option");
        button.set_size_request(80, 32);
        button.set_active(saved_early.contains(offset));
        let tilted = blur::BlurRow::new(&button, false, 6.0)
            .with_hover_lift(4.0)
            .with_hover_tilt(10.0);
        tilted.set_overflow(gtk4::Overflow::Visible);
        earlier_grid.attach(&tilted, index as i32 % 4, index as i32 / 4, 1, 1);
        early_buttons.push(button);
    }
    let saved_custom = saved_early.iter().copied().find(|offset| {
        !EARLY_REMINDER_OPTIONS
            .iter()
            .any(|(_, standard)| standard == offset)
    });
    let custom_offset = Rc::new(Cell::new(saved_custom));
    let custom_selected = Rc::new(Cell::new(saved_custom.is_some()));
    let custom_button = Button::with_label(
        &saved_custom
            .map(format_custom_reminder)
            .unwrap_or_else(|| "Custom".into()),
    );
    custom_button.add_css_class("reminder-option");
    custom_button.add_css_class("reminder-custom");
    if saved_custom.is_some() {
        custom_button.add_css_class("reminder-custom-selected");
    }
    custom_button.set_size_request(80, 32);
    let custom_revealer = gtk4::Revealer::new();
    custom_revealer.set_transition_type(gtk4::RevealerTransitionType::SlideDown);
    custom_revealer.set_transition_duration(140);
    custom_revealer.set_reveal_child(false);
    let custom_form = GtkBox::new(Orientation::Horizontal, 8);
    custom_form.add_css_class("custom-reminder-form");
    custom_form.set_margin_top(8);
    custom_form.set_margin_bottom(8);
    custom_form.set_margin_start(8);
    custom_form.set_margin_end(8);
    let custom_value = gtk4::SpinButton::with_range(1.0, 525_600.0, 1.0);
    let custom_unit = ComboBoxText::new();
    for unit in ["minutes", "hours", "days"] {
        custom_unit.append_text(unit);
    }
    let (initial_custom_value, initial_custom_unit) =
        saved_custom.map(custom_reminder_parts).unwrap_or((30, 0));
    custom_value.set_value(initial_custom_value as f64);
    custom_unit.set_active(Some(initial_custom_unit));
    let custom_value_ref = custom_value.downgrade();
    custom_unit.connect_changed(move |unit| {
        let maximum = match unit.active() {
            Some(1) => 8_760.0,
            Some(2) => 365.0,
            _ => 525_600.0,
        };
        if let Some(value) = custom_value_ref.upgrade() {
            value.set_range(1.0, maximum);
        }
    });
    let apply_custom = Button::with_label("Set");
    let remove_custom = Button::with_label("Remove");
    custom_form.append(&custom_value);
    custom_form.append(&custom_unit);
    custom_form.append(&remove_custom);
    custom_form.append(&apply_custom);
    custom_revealer.set_child(Some(&custom_form));
    let custom_button_ref = custom_button.downgrade();
    let custom_revealer_ref = custom_revealer.downgrade();
    let custom_offset_ref = custom_offset.clone();
    let custom_selected_ref = custom_selected.clone();
    apply_custom.connect_clicked(move |_| {
        let multiplier = match custom_unit.active() {
            Some(1) => 60,
            Some(2) => 1_440,
            _ => 1,
        };
        let offset = i64::from(custom_value.value_as_int()) * multiplier;
        if !(1..=525_600).contains(&offset) {
            return;
        }
        custom_offset_ref.set(Some(offset));
        custom_selected_ref.set(true);
        if let Some(button) = custom_button_ref.upgrade() {
            button.set_label(&format_custom_reminder(offset));
            button.add_css_class("reminder-custom-selected");
        }
        if let Some(revealer) = custom_revealer_ref.upgrade() {
            revealer.set_reveal_child(false);
        }
    });
    let custom_button_ref = custom_button.downgrade();
    let custom_revealer_ref = custom_revealer.downgrade();
    let custom_offset_ref = custom_offset.clone();
    let custom_selected_ref = custom_selected.clone();
    remove_custom.connect_clicked(move |_| {
        custom_offset_ref.set(None);
        custom_selected_ref.set(false);
        if let Some(button) = custom_button_ref.upgrade() {
            button.set_label("Custom");
            button.remove_css_class("reminder-custom-selected");
        }
        if let Some(revealer) = custom_revealer_ref.upgrade() {
            revealer.set_reveal_child(false);
        }
    });
    let custom_revealer_ref = custom_revealer.downgrade();
    custom_button.connect_clicked(move |_| {
        if let Some(revealer) = custom_revealer_ref.upgrade() {
            revealer.set_reveal_child(!revealer.reveals_child());
        }
    });
    let custom_tilt = blur::BlurRow::new(&custom_button, false, 6.0)
        .with_hover_lift(4.0)
        .with_hover_tilt(10.0);
    custom_tilt.set_overflow(gtk4::Overflow::Visible);
    earlier_grid.attach(&custom_tilt, 1, 1, 1, 1);
    earlier.append(&earlier_grid);
    earlier.append(&custom_revealer);
    content.append(&earlier);

    let repeat_switch = gtk4::Switch::new();
    repeat_switch.add_css_class("reminder-switch");
    let repeat_label = icon_text("Repeat", "media-playlist-repeat-symbolic");
    let repeat_header = GtkBox::new(Orientation::Horizontal, 8);
    repeat_header.set_margin_top(12);
    repeat_header.set_margin_bottom(10);
    repeat_switch.set_valign(gtk4::Align::Center);
    repeat_header.append(&repeat_switch);
    repeat_header.append(&repeat_label);
    content.append(&repeat_header);

    let existing_repeat = existing
        .as_ref()
        .map(|flash| flash.repeat_rule.clone())
        .unwrap_or(db::RepeatRule::None);
    let repeat_mode_index =
        usize::from(matches!(&existing_repeat, db::RepeatRule::Weekdays { .. }));
    let repeat_modes =
        highlight::HighlightSelector::new(&["By time", "By weekdays"], repeat_mode_index);
    repeat_modes.set_icons(&["alarm-symbolic", "x-office-calendar-symbolic"]);
    repeat_modes.widget.add_css_class("popup-selector");
    let repeat_stack = gtk4::Stack::new();
    repeat_stack.set_margin_top(10);
    repeat_stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
    repeat_stack.set_transition_duration(160);
    let every = GtkBox::new(Orientation::Horizontal, 8);
    every.set_valign(gtk4::Align::Center);
    every.append(&Label::new(Some("Every")));
    let interval = gtk4::SpinButton::with_range(1.0, 365.0, 1.0);
    interval.set_value(match &existing_repeat {
        db::RepeatRule::Interval { every, .. } => f64::from(*every),
        _ => 2.0,
    });
    interval.set_size_request(64, 32);
    let unit = ComboBoxText::new();
    for value in ["minutes", "hours", "days", "weeks"] {
        unit.append_text(value);
    }
    let repeat_unit_index = match &existing_repeat {
        db::RepeatRule::Interval { unit, .. } => match unit {
            db::RepeatUnit::Minutes => 0,
            db::RepeatUnit::Hours => 1,
            db::RepeatUnit::Days => 2,
            db::RepeatUnit::Weeks => 3,
        },
        _ => 1,
    };
    unit.set_active(Some(repeat_unit_index));
    every.append(&interval);
    every.append(&unit);
    repeat_stack.add_named(&every, Some("time"));

    let weekdays = GtkBox::new(Orientation::Horizontal, 8);
    weekdays.set_homogeneous(true);
    let day_names = [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ];
    let saved_weekdays = match &existing_repeat {
        db::RepeatRule::Weekdays { days } => *days,
        _ => 0,
    };
    let mut weekday_buttons = Vec::new();
    for (index, (day, name)) in ["M", "T", "W", "T", "F", "S", "S"]
        .iter()
        .zip(day_names)
        .enumerate()
    {
        let button = gtk4::ToggleButton::with_label(day);
        button.add_css_class("weekday-button");
        button.set_size_request(32, 32);
        button.set_tooltip_text(Some(name));
        button.update_property(&[gtk4::accessible::Property::Label(name)]);
        button.set_active(saved_weekdays & (1 << index) != 0);
        let tilted = blur::BlurRow::new(&button, false, 6.0).with_hover_tilt(8.0);
        weekdays.append(&tilted);
        weekday_buttons.push(button);
    }
    repeat_stack.add_named(&weekdays, Some("weekdays"));
    let repeat_controls = GtkBox::new(Orientation::Vertical, 8);
    repeat_controls.append(&repeat_modes.widget);
    repeat_controls.append(&repeat_stack);
    let repeat_enabled = existing_repeat.is_repeating();
    repeat_switch.set_active(repeat_enabled);
    repeat_controls.set_sensitive(repeat_enabled);
    repeat_stack.set_visible_child_name(if repeat_mode_index == 0 {
        "time"
    } else {
        "weekdays"
    });
    content.append(&repeat_controls);
    let repeat_stack_ref = repeat_stack.downgrade();
    let repeat_modes_ref = repeat_modes.clone();
    repeat_modes.connect_changed(move || {
        if let Some(stack) = repeat_stack_ref.upgrade() {
            stack.set_visible_child_name(if repeat_modes_ref.selected() == 0 {
                "time"
            } else {
                "weekdays"
            });
        }
    });
    let repeat_controls_ref = repeat_controls.downgrade();
    repeat_switch.connect_active_notify(move |switch| {
        if let Some(controls) = repeat_controls_ref.upgrade() {
            controls.set_sensitive(switch.is_active());
        }
    });
    let date_switch_ref = date_switch.downgrade();
    let time_switch_ref = time_switch.downgrade();
    repeat_switch.connect_active_notify(move |switch| {
        if switch.is_active() {
            if let Some(date) = date_switch_ref.upgrade() {
                date.set_active(true);
            }
            if let Some(time) = time_switch_ref.upgrade() {
                time.set_active(true);
            }
        }
    });
    let repeat_switch_ref = repeat_switch.downgrade();
    date_switch.connect_active_notify(move |switch| {
        if !switch.is_active() {
            if let Some(repeat) = repeat_switch_ref.upgrade() {
                repeat.set_active(false);
            }
        }
    });

    let error = Label::new(None);
    error.add_css_class("error");
    error.set_wrap(true);
    content.append(&error);
    dialog.set_default_response(ResponseType::Accept);
    dialog_actions(&dialog, !editing);
    let ui = ui.clone();
    dialog.connect_response(move |dialog, response| {
        if response != ResponseType::Accept {
            dismiss_popup(dialog);
            return;
        }
        let name = title.text();
        if name.trim().is_empty() {
            error.set_text("Enter a Flash name.");
            return;
        }
        let mut early_reminders = early_buttons
            .iter()
            .zip(EARLY_REMINDER_OPTIONS)
            .filter_map(|(button, (_, offset))| button.is_active().then_some(offset))
            .collect::<Vec<_>>();
        if custom_selected.get() {
            if let Some(offset) = custom_offset.get() {
                early_reminders.push(offset);
            }
        }
        early_reminders.sort_unstable();
        early_reminders.dedup();
        let repeat_rule = if repeat_switch.is_active() {
            if !date_switch.is_active() || !time_switch.is_active() {
                error.set_text("Repeating Flashes need a date and time.");
                return;
            }
            if repeat_modes.selected() == 0 {
                let unit = match unit.active() {
                    Some(0) => db::RepeatUnit::Minutes,
                    Some(2) => db::RepeatUnit::Days,
                    Some(3) => db::RepeatUnit::Weeks,
                    _ => db::RepeatUnit::Hours,
                };
                db::RepeatRule::Interval {
                    every: interval.value_as_int() as u32,
                    unit,
                }
            } else {
                let days = weekday_buttons
                    .iter()
                    .enumerate()
                    .filter(|(_, button)| button.is_active())
                    .fold(0u8, |mask, (index, _)| mask | (1 << index));
                if days == 0 {
                    error.set_text("Choose at least one weekday.");
                    return;
                }
                db::RepeatRule::Weekdays { days }
            }
        } else {
            db::RepeatRule::None
        };
        let due = if date_switch.is_active() {
            let text = if time_switch.is_active() {
                time.text().to_string()
            } else {
                "09:00".to_string()
            };
            let parts: Vec<_> = text.split(':').collect();
            let valid = parts.len() == 2
                && parts[0].len() == 2
                && parts[1].len() == 2
                && parts[0].bytes().all(|b| b.is_ascii_digit())
                && parts[1].bytes().all(|b| b.is_ascii_digit())
                && parts[0].parse::<u8>().unwrap_or(99) < 24
                && parts[1].parse::<u8>().unwrap_or(99) < 60;
            if !valid {
                error.set_text("Enter a valid time in 24-hour HH:MM format.");
                return;
            }
            let date = calendar.date();
            let when = match gtk4::glib::DateTime::from_local(
                date.year(),
                date.month(),
                date.day_of_month(),
                parts[0].parse().unwrap(),
                parts[1].parse().unwrap(),
                0.0,
            ) {
                Ok(when) => when,
                Err(_) => {
                    error.set_text("That local date and time is invalid.");
                    return;
                }
            };
            let now = match gtk4::glib::DateTime::now_utc() {
                Ok(now) => now,
                Err(e) => {
                    error.set_text(&e.to_string());
                    return;
                }
            };
            let unchanged = existing
                .as_ref()
                .and_then(|flash| flash.due_at.as_deref())
                .and_then(|due| gtk4::glib::DateTime::from_iso8601(due, None).ok())
                .is_some_and(|old| old.to_unix() == when.to_unix());
            let same_schedule = unchanged
                && existing
                    .as_ref()
                    .is_some_and(|flash| flash.repeat_rule == repeat_rule);
            if !same_schedule && when.to_unix() <= now.to_unix() {
                error.set_text("Choose a future reminder date and time.");
                return;
            }
            match when
                .to_utc()
                .and_then(|date| date.format("%Y-%m-%dT%H:%M:%SZ"))
            {
                Ok(date) => Some(date.to_string()),
                Err(e) => {
                    error.set_text(&e.to_string());
                    return;
                }
            }
        } else {
            None
        };
        if due.is_none() && (!early_reminders.is_empty() || repeat_rule.is_repeating()) {
            error.set_text("Choose a date and time for earlier or repeating reminders.");
            return;
        }
        let intensity = intensity.selected() as u8 + 1;
        let result = if let Some(flash) = &existing {
            db::update_flash(
                &ui.db,
                flash.id,
                &name,
                due.as_deref(),
                intensity,
                &early_reminders,
                &repeat_rule,
            )
        } else {
            db::add_flash(
                &ui.db,
                episode,
                &name,
                due.as_deref(),
                intensity,
                &early_reminders,
                &repeat_rule,
            )
        };
        match result {
            Ok(()) => {
                refresh_flashes(&ui);
                dismiss_popup(dialog);
            }
            Err(e) => error.set_text(&e.to_string()),
        }
    });
    present_popup(&dialog);
}

thread_local! { static HOLD: std::cell::RefCell<Option<gtk4::gio::ApplicationHoldGuard>> = const { std::cell::RefCell::new(None) }; }

fn notification_text(
    flash: &db::Flash,
    now: &gtk4::glib::DateTime,
) -> Result<(String, String), gtk4::glib::BoolError> {
    let due = gtk4::glib::DateTime::from_iso8601(flash.due_at.as_deref().unwrap_or(""), None)?
        .to_local()?;
    let now = now.to_local()?;
    let same_day = (due.year(), due.month(), due.day_of_month())
        == (now.year(), now.month(), now.day_of_month());
    let when = due.format(if same_day { "%H:%M" } else { "%Y-%m-%d %H:%M" })?;
    let intensity = match flash.intensity {
        1 => "Dim",
        2 => "Bright",
        _ => "Storbe",
    };
    Ok((
        format!("Flash - {intensity}"),
        format!("{} - {when}", flash.title),
    ))
}

fn check_reminders(ui: &Ui) {
    let now = match gtk4::glib::DateTime::now_utc() {
        Ok(now) => now,
        Err(e) => {
            show_error(ui, e);
            return;
        }
    };
    let local_now = match now.to_local() {
        Ok(now) => now,
        Err(e) => {
            show_error(ui, e);
            return;
        }
    };
    let now_text = match now.format("%Y-%m-%dT%H:%M:%SZ") {
        Ok(now) => now.to_string(),
        Err(e) => {
            show_error(ui, e);
            return;
        }
    };
    match db::due_reminders(&ui.db, &now_text) {
        Ok(items) => {
            let mut changed = false;
            for reminder in items {
                let is_due = reminder.offset_minutes == 0;
                if is_due {
                    if let Err(e) = db::fade(&ui.db, reminder.flash.id) {
                        show_error(ui, e);
                        continue;
                    }
                    changed = true;
                }
                let message = notification_text(&reminder.flash, &local_now);
                let (title, body) = match message {
                    Ok(message) => message,
                    Err(e) => {
                        show_error(ui, e);
                        continue;
                    }
                };
                match std::process::Command::new("notify-send")
                    .args([
                        "--app-name=Amnessia",
                        "--icon=com.amnessia.App-triangle",
                        "--",
                        &title,
                        &body,
                    ])
                    .status()
                {
                    Ok(status) if status.success() => {
                        let next_due = if is_due {
                            match repeat::next_due_at(
                                &reminder.flash.repeat_rule,
                                &reminder.occurrence_at,
                                &now,
                            ) {
                                Ok(next) => next,
                                Err(e) => {
                                    show_error(ui, e);
                                    continue;
                                }
                            }
                        } else {
                            None
                        };
                        if let Err(e) =
                            db::complete_reminder(&ui.db, &reminder, next_due.as_deref())
                        {
                            show_error(ui, e);
                        }
                    }
                    Ok(status) => show_error(
                        ui,
                        format!("Could not send reminder: notify-send exited with {status}"),
                    ),
                    Err(e) => show_error(ui, format!("Could not send reminder: {e}")),
                }
            }
            if changed && ui.window.is_visible() {
                refresh_flashes(ui);
            }
        }
        Err(e) => show_error(ui, e),
    }
}

fn build(app: &Application) {
    if let Some(window) = app.windows().first() {
        window.present();
        return;
    }
    let path = match db::path() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return;
        }
    };
    let database = match db::open(&path) {
        Ok(c) => Rc::new(c),
        Err(e) => {
            let window = ApplicationWindow::builder()
                .application(app)
                .title("Amnessia")
                .default_width(500)
                .default_height(200)
                .build();
            let label = Label::new(Some(&format!("Could not open database:\n{e}")));
            label.set_wrap(true);
            window.set_child(Some(&label));
            window.present();
            return;
        }
    };
    gtk4::Window::set_default_icon_name("com.amnessia.App-triangle");
    let window = ApplicationWindow::builder()
        .application(app)
        .icon_name("com.amnessia.App-triangle")
        .title("Amnessia")
        .default_width(960)
        .default_height(640)
        .build();
    let css = gtk4::CssProvider::new();
    css.load_from_data(include_str!("style.css"));
    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().unwrap(),
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    let overlay = Overlay::new();
    let bg = background::create();
    bg.set_hexpand(true);
    bg.set_vexpand(true);
    overlay.set_child(Some(&bg));
    let root = GtkBox::new(Orientation::Horizontal, 0);
    background::attach(&root);
    root.set_hexpand(true);
    root.set_vexpand(true);
    let side_overlay = Overlay::new();
    side_overlay.set_width_request(240);
    side_overlay.set_vexpand(true);
    side_overlay.set_child(Some(&sidebar_blur::SidebarBlur::new(&bg, 0.0)));
    let side = GtkBox::new(Orientation::Vertical, 12);
    side.add_css_class("sidebar");
    side.set_width_request(240);
    let brand = GtkBox::new(Orientation::Horizontal, 2);
    brand.add_css_class("brand");
    brand.set_margin_top(8);
    brand.set_margin_start(8);
    let loader = gtk4::gdk_pixbuf::PixbufLoader::new();
    loader.set_size(42, 42);
    let logo = match loader
        .write(include_bytes!("../assets/com.amnessia.App.svg"))
        .and_then(|_| loader.close())
    {
        Ok(()) => gtk4::Image::from_pixbuf(loader.pixbuf().as_ref()),
        Err(error) => {
            eprintln!("Could not load Amnessia logo: {error}");
            gtk4::Image::new()
        }
    };
    logo.set_pixel_size(42);
    logo.set_margin_bottom(3);
    logo.update_property(&[gtk4::accessible::Property::Label("Amnessia logo")]);
    brand.append(&logo);
    let brand_name = Label::new(Some("mnessia"));
    brand_name.set_xalign(0.0);
    brand_name.set_valign(gtk4::Align::End);
    brand_name.set_margin_bottom(1);
    brand.append(&brand_name);
    side.append(&brand);
    let heading = Label::new(Some("EPISODES"));
    heading.set_xalign(0.0);
    heading.add_css_class("section-label");
    side.append(&heading);
    let add_episode = Button::with_label("+ Add Episode");
    side.append(&shine::wrap(&add_episode));
    let episodes = GtkBox::new(Orientation::Vertical, 6);
    let scroll = ScrolledWindow::builder()
        .vexpand(true)
        .child(&episodes)
        .build();
    side.append(&scroll);
    let content = GtkBox::new(Orientation::Vertical, 16);
    content.add_css_class("content");
    content.set_hexpand(true);
    let top = GtkBox::new(Orientation::Horizontal, 12);
    let toggle = Button::with_label("☰");
    toggle.set_tooltip_text(Some("Show or hide Episodes"));
    side_overlay.add_overlay(&side);
    let side_clone = side_overlay.clone();
    toggle.connect_clicked(move |_| side_clone.set_visible(!side_clone.is_visible()));
    let title = Label::new(Some("Select an Episode"));
    title.set_hexpand(true);
    title.set_xalign(0.0);
    title.add_css_class("title");
    top.append(&toggle);
    top.append(&title);
    let theme = Button::with_label("☀ Light");
    let theme_root = root.clone();
    theme.connect_clicked(move |button| {
        if theme_root.has_css_class("light") {
            theme_root.remove_css_class("light");
            button.set_label("☀ Light");
        } else {
            theme_root.add_css_class("light");
            button.set_label("☾ Dark");
        }
    });
    top.append(&theme);
    let quit = Button::with_label("Quit");
    quit.set_tooltip_text(Some("Quit Amnessia and stop reminders"));
    let quit_app = app.clone();
    quit.connect_clicked(move |_| quit_app.quit());
    top.append(&quit);
    content.append(&top);
    let controls = GtkBox::new(Orientation::Horizontal, 8);
    let add_flash = Button::with_label("+ Add Flash");
    controls.append(&shine::wrap(&add_flash));
    let filter = filters::Filters::new();
    controls.append(&filter.widget);
    content.append(&controls);
    let error = Label::new(None);
    error.add_css_class("error");
    error.set_xalign(0.0);
    error.set_wrap(true);
    content.append(&error);
    let flashes = GtkBox::new(Orientation::Vertical, 8);
    let flash_scroll = ScrolledWindow::builder()
        .vexpand(true)
        .child(&flashes)
        .build();
    content.append(&flash_scroll);
    root.append(&side_overlay);
    root.append(&content);
    overlay.add_overlay(&root);
    let popup_layer = Overlay::new();
    popup_layer.set_child(Some(&overlay));
    window.set_child(Some(&popup_layer));
    let ui = Ui {
        db: database,
        selected: Rc::new(Cell::new(None)),
        background: bg.clone().upcast(),
        episodes,
        flashes,
        title,
        window: window.clone(),
        filter,
        error,
        loaded_episodes: Rc::new(RefCell::new(HashSet::new())),
    };
    let u = ui.clone();
    add_episode.connect_clicked(move |_| episode_dialog(&u, None));
    let u = ui.clone();
    add_flash.connect_clicked(move |_| flash_dialog(&u, None));
    let u = ui.clone();
    ui.filter.connect_changed(move || refresh_flashes(&u));
    refresh_episodes(&ui);
    HOLD.with(|hold| {
        if hold.borrow().is_none() {
            *hold.borrow_mut() = Some(app.hold());
        }
    });
    window.connect_close_request(|window| {
        window.set_visible(false);
        gtk4::glib::Propagation::Stop
    });
    let (tray_sender, tray_receiver) = std::sync::mpsc::channel();
    match tray::start(tray_sender) {
        Ok(tray_handle) => {
            let tray_window = window.clone();
            let tray_app = app.clone();
            gtk4::glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
                let _keep_tray_alive = &tray_handle;
                while let Ok(action) = tray_receiver.try_recv() {
                    match action {
                        tray::Action::Open => tray_window.present(),
                        tray::Action::Quit => tray_app.quit(),
                    }
                }
                gtk4::glib::ControlFlow::Continue
            });
        }
        Err(e) => show_error(&ui, format!("Could not start the system tray icon: {e}")),
    }
    check_reminders(&ui);
    gtk4::glib::timeout_add_seconds_local(5, move || {
        check_reminders(&ui);
        gtk4::glib::ControlFlow::Continue
    });
    window.present();
}
fn wayland_disabled_protocols(existing: Option<std::ffi::OsString>) -> std::ffi::OsString {
    let mut disabled = existing.unwrap_or_default();
    if !disabled.is_empty() {
        disabled.push(",");
    }
    disabled.push("ext_background_effect_manager_v1");
    disabled
}

fn main() {
    // GTK otherwise asks Wayland to disable compositor blur for this window.
    std::env::set_var(
        "GDK_WAYLAND_DISABLE",
        wayland_disabled_protocols(std::env::var_os("GDK_WAYLAND_DISABLE")),
    );
    let app = Application::builder()
        .application_id("com.amnessia.App")
        .build();
    app.connect_activate(build);
    app.run();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_existing_disabled_wayland_protocols() {
        assert_eq!(
            wayland_disabled_protocols(None),
            "ext_background_effect_manager_v1"
        );
        assert_eq!(
            wayland_disabled_protocols(Some("existing".into())),
            "existing,ext_background_effect_manager_v1"
        );
    }
    #[test]
    fn notification_includes_intensity_and_date_only_for_another_day() {
        let now = gtk4::glib::DateTime::from_local(2030, 5, 12, 14, 0, 0.0).unwrap();
        let later = gtk4::glib::DateTime::from_local(2030, 5, 12, 16, 30, 0.0)
            .unwrap()
            .to_utc()
            .unwrap()
            .format("%Y-%m-%dT%H:%M:%SZ")
            .unwrap()
            .to_string();
        let mut flash = db::Flash {
            id: 1,
            title: "Drink water".into(),
            status: "active".into(),
            intensity: 2,
            due_at: Some(later),
            early_reminders: Vec::new(),
            repeat_rule: db::RepeatRule::None,
        };
        assert_eq!(
            notification_text(&flash, &now).unwrap(),
            ("Flash - Bright".into(), "Drink water - 16:30".into())
        );
        flash.due_at = Some(
            gtk4::glib::DateTime::from_local(2030, 5, 13, 8, 15, 0.0)
                .unwrap()
                .to_utc()
                .unwrap()
                .format("%Y-%m-%dT%H:%M:%SZ")
                .unwrap()
                .to_string(),
        );
        assert_eq!(
            notification_text(&flash, &now).unwrap().1,
            "Drink water - 2030-05-13 08:15"
        );
    }
}
