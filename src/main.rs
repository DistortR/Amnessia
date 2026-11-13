mod background;
mod db;
mod tray;

use gtk4::{
    prelude::*, Application, ApplicationWindow, Box as GtkBox, Button, Calendar, CheckButton,
    ComboBoxText, Dialog, DrawingArea, Entry, Label, Orientation, Overlay, ResponseType,
    ScrolledWindow,
};
use rusqlite::Connection;
use std::{cell::Cell, rc::Rc};

#[derive(Clone)]
struct Ui {
    db: Rc<Connection>,
    selected: Rc<Cell<Option<i64>>>,
    episodes: GtkBox,
    flashes: GtkBox,
    title: Label,
    window: ApplicationWindow,
    filter: ComboBoxText,
    error: Label,
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
    let filter = ui.filter.active_id().unwrap_or_else(|| "all".into());
    match db::flashes(&ui.db, id).map(|items| {
        items
            .into_iter()
            .filter(|f| filter == "all" || f.status == filter)
            .collect::<Vec<_>>()
    }) {
        Ok(items) if items.is_empty() => ui
            .flashes
            .append(&Label::new(Some("No Flashes yet. Add one above."))),
        Ok(items) => {
            for flash in items {
                let db::Flash {
                    id,
                    title,
                    status,
                    intensity,
                    due_at,
                } = flash.clone();
                let row = GtkBox::new(Orientation::Horizontal, 12);
                row.add_css_class("flash-row");
                let label = Label::new(Some(&title));
                label.set_xalign(0.0);
                label.set_hexpand(true);
                label.set_wrap(true);
                if status == "resolved" {
                    label.add_css_class("resolved");
                }
                let importance = Label::new(Some(&"!".repeat(intensity as usize)));
                importance.add_css_class("intensity");
                let button = Button::new();
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
                details.append(&label);
                if filter == "all" && (status == "resolved" || status == "faded") {
                    let badge = Label::new(Some(if status == "resolved" {
                        "Resolved"
                    } else {
                        "Faded"
                    }));
                    badge.add_css_class("status");
                    details.append(&badge);
                }
                details.append(&importance);
                let edit = Button::new();
                edit.add_css_class("flash-edit");
                edit.set_hexpand(true);
                edit.set_child(Some(&details));
                edit.set_tooltip_text(Some("Edit Flash"));
                let edit_ui = ui.clone();
                edit.connect_clicked(move |_| flash_dialog(&edit_ui, Some(flash.clone())));
                let cloned = ui.clone();
                button.connect_clicked(move |_| {
                    let next = if status == "resolved" {
                        "active"
                    } else {
                        "resolved"
                    };
                    match db::set_status(&cloned.db, id, next) {
                        Ok(()) => {
                            cloned.error.set_text("");
                            refresh_flashes(&cloned);
                        }
                        Err(e) => show_error(&cloned, e),
                    }
                });
                row.append(&button);
                row.append(&edit);
                ui.flashes.append(&row);
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
                ui.selected.set(items.first().map(|e| e.0));
            }
            for (id, name) in items {
                let button = Button::with_label(&name);
                button.add_css_class("episode");
                if ui.selected.get() == Some(id) {
                    button.add_css_class("selected-episode");
                    ui.title.set_text(&name);
                }
                let cloned = ui.clone();
                button.connect_clicked(move |_| {
                    cloned.selected.set(Some(id));
                    refresh_episodes(&cloned);
                    refresh_flashes(&cloned);
                });
                ui.episodes.append(&button);
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

fn episode_dialog(ui: &Ui) {
    let dialog = Dialog::with_buttons(
        Some("New Episode"),
        Some(&ui.window),
        gtk4::DialogFlags::MODAL,
        &[
            ("Cancel", ResponseType::Cancel),
            ("Create", ResponseType::Accept),
        ],
    );
    let content = dialog.content_area();
    content.set_spacing(12);
    content.set_margin_top(20);
    content.set_margin_bottom(20);
    content.set_margin_start(20);
    content.set_margin_end(20);
    let entry = Entry::new();
    entry.set_placeholder_text(Some("Episode name"));
    entry.set_activates_default(true);
    content.append(&entry);
    let error = Label::new(None);
    error.add_css_class("error");
    error.set_wrap(true);
    content.append(&error);
    dialog.set_default_response(ResponseType::Accept);
    let ui = ui.clone();
    dialog.connect_response(move |dialog, response| {
        if response != ResponseType::Accept {
            dialog.close();
            return;
        }
        let name = entry.text();
        if name.trim().is_empty() {
            error.set_text("Enter an Episode name.");
            return;
        }
        match db::add_episode(&ui.db, &name) {
            Ok(()) => {
                ui.selected.set(Some(ui.db.last_insert_rowid()));
                refresh_episodes(&ui);
                dialog.close();
            }
            Err(e) => error.set_text(&e.to_string()),
        }
    });
    dialog.present();
}

fn flash_dialog(ui: &Ui, existing: Option<db::Flash>) {
    let Some(episode) = ui.selected.get() else {
        show_error(ui, "Select an Episode first.");
        return;
    };
    let dialog = Dialog::with_buttons(
        Some(if existing.is_some() {
            "Edit Flash"
        } else {
            "New Flash"
        }),
        Some(&ui.window),
        gtk4::DialogFlags::MODAL,
        &[
            ("Cancel", ResponseType::Cancel),
            (
                if existing.is_some() { "Save" } else { "Create" },
                ResponseType::Accept,
            ),
        ],
    );
    let content = dialog.content_area();
    content.set_spacing(10);
    content.set_margin_top(20);
    content.set_margin_bottom(20);
    content.set_margin_start(20);
    content.set_margin_end(20);
    let title = Entry::new();
    title.set_placeholder_text(Some("Flash name"));
    title.set_activates_default(true);
    dialog.set_default_response(ResponseType::Accept);
    content.append(&title);
    let intensity = ComboBoxText::new();
    for text in ["! Dim", "!! Bright", "!!! Storbe"] {
        intensity.append_text(text);
    }
    intensity.set_active(Some(
        existing.as_ref().map_or(0, |f| u32::from(f.intensity - 1)),
    ));
    let label = Label::new(Some("Intensity"));
    label.set_xalign(0.0);
    content.append(&label);
    content.append(&intensity);
    let reminder = CheckButton::with_label("Set a reminder");
    content.append(&reminder);
    let calendar = Calendar::new();
    calendar.set_sensitive(false);
    content.append(&calendar);
    let time = Entry::new();
    time.set_placeholder_text(Some("Time (24-hour HH:MM)"));
    time.set_sensitive(false);
    content.append(&time);
    let cal = calendar.clone();
    let clock = time.clone();
    reminder.connect_toggled(move |check| {
        cal.set_sensitive(check.is_active());
        clock.set_sensitive(check.is_active());
    });
    if let Some(flash) = &existing {
        title.set_text(&flash.title);
        if let Some(due) = &flash.due_at {
            match gtk4::glib::DateTime::from_iso8601(due, None).and_then(|d| d.to_local()) {
                Ok(local) => {
                    reminder.set_active(true);
                    calendar.select_day(&local);
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
    let error = Label::new(None);
    error.add_css_class("error");
    error.set_wrap(true);
    content.append(&error);
    let ui = ui.clone();
    dialog.connect_response(move |dialog, response| {
        if response != ResponseType::Accept {
            dialog.close();
            return;
        }
        let name = title.text();
        if name.trim().is_empty() {
            error.set_text("Enter a Flash name.");
            return;
        }
        let due = if reminder.is_active() {
            let text = time.text();
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
                .and_then(|f| f.due_at.as_deref())
                .and_then(|due| gtk4::glib::DateTime::from_iso8601(due, None).ok())
                .is_some_and(|old| old.to_unix() == when.to_unix());
            if !unchanged && when.to_unix() <= now.to_unix() {
                error.set_text("Choose a future reminder date and time.");
                return;
            }
            match when.to_utc().and_then(|d| d.format("%Y-%m-%dT%H:%M:%SZ")) {
                Ok(date) => Some(date.to_string()),
                Err(e) => {
                    error.set_text(&e.to_string());
                    return;
                }
            }
        } else {
            None
        };
        let result = if let Some(flash) = &existing {
            db::update_flash(
                &ui.db,
                flash.id,
                &name,
                due.as_deref(),
                intensity.active().unwrap_or(0) as u8 + 1,
            )
        } else {
            db::add_flash(
                &ui.db,
                episode,
                &name,
                due.as_deref(),
                intensity.active().unwrap_or(0) as u8 + 1,
            )
        };
        match result {
            Ok(()) => {
                refresh_flashes(&ui);
                dialog.close();
            }
            Err(e) => error.set_text(&e.to_string()),
        }
    });
    dialog.present();
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
    match db::due_reminders(&ui.db) {
        Ok(items) => {
            let mut changed = false;
            for (flash, notified) in items {
                let id = flash.id;
                if let Err(e) = db::fade(&ui.db, id) {
                    show_error(ui, e);
                    continue;
                }
                changed = true;
                if !notified {
                    let message = gtk4::glib::DateTime::now_local()
                        .and_then(|now| notification_text(&flash, &now));
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
                            if let Err(e) = db::mark_notified(&ui.db, id) {
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
    let side = GtkBox::new(Orientation::Vertical, 12);
    side.add_css_class("sidebar");
    side.set_width_request(240);
    let brand = GtkBox::new(Orientation::Horizontal, 2);
    brand.add_css_class("brand");
    let loader = gtk4::gdk_pixbuf::PixbufLoader::new();
    loader.set_size(34, 34);
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
    logo.set_pixel_size(34);
    logo.set_margin_bottom(3);
    logo.update_property(&[gtk4::accessible::Property::Label("Amnessia logo")]);
    brand.append(&logo);
    let brand_name = Label::new(Some("mnessia"));
    brand_name.set_xalign(0.0);
    brand_name.set_valign(gtk4::Align::End);
    brand.append(&brand_name);
    side.append(&brand);
    let heading = Label::new(Some("EPISODES"));
    heading.set_xalign(0.0);
    heading.add_css_class("section-label");
    side.append(&heading);
    let add_episode = Button::with_label("+ Add Episode");
    side.append(&add_episode);
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
    let side_clone = side.clone();
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
    controls.append(&add_flash);
    let filter = ComboBoxText::new();
    filter.append(Some("all"), "All");
    filter.append(Some("resolved"), "Resolved");
    filter.append(Some("faded"), "Faded");
    filter.set_active_id(Some("all"));
    controls.append(&filter);
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
    root.append(&side);
    root.append(&content);
    overlay.add_overlay(&root);
    window.set_child(Some(&overlay));
    let ui = Ui {
        db: database,
        selected: Rc::new(Cell::new(None)),
        episodes,
        flashes,
        title,
        window: window.clone(),
        filter,
        error,
    };
    let u = ui.clone();
    add_episode.connect_clicked(move |_| episode_dialog(&u));
    let u = ui.clone();
    add_flash.connect_clicked(move |_| flash_dialog(&u, None));
    let u = ui.clone();
    ui.filter.connect_changed(move |_| refresh_flashes(&u));
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
fn main() {
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
