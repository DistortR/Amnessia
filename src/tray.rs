use ksni::{blocking::TrayMethods, menu::StandardItem, MenuItem, Tray};
use std::sync::mpsc::Sender;

pub enum Action {
    Open,
    Quit,
}

pub struct AmnessiaTray {
    pub sender: Sender<Action>,
}

impl Tray for AmnessiaTray {
    fn id(&self) -> String {
        "com.amnessia.App".into()
    }
    fn title(&self) -> String {
        "Amnessia".into()
    }
    fn icon_name(&self) -> String {
        "com.amnessia.App-triangle".into()
    }
    fn activate(&mut self, _: i32, _: i32) {
        let _ = self.sender.send(Action::Open);
    }
    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Open Amnessia".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.sender.send(Action::Open);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.sender.send(Action::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub fn start(sender: Sender<Action>) -> Result<ksni::blocking::Handle<AmnessiaTray>, ksni::Error> {
    AmnessiaTray { sender }
        .assume_sni_available(true)
        .spawn()
}
