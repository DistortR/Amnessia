# Amnessia

A Rust and GTK4 app for organizing Flashes in Episodes, with a Letter Glitch background inspired by [React Bits](https://reactbits.dev/backgrounds/letter-glitch).

Run `cargo run --release`. GTK 4.10 or newer, SQLite development libraries, and `notify-send` are required. Add Flashes to the default Static Episode or create another Episode. Select a Flash to edit its name, reminder, and Intensity. Use the bulb to resolve or reopen a Flash; the filter shows All, Resolved, or Faded. The exclamation marks show Intensity.

A Flash can have one reminder date and time, several earlier reminders, and a repeat schedule by interval or selected weekdays. Times follow the computer's local time and are stored in UTC. When its due time passes unresolved, a Flash becomes Faded and Amnessia sends a desktop notification with its Intensity and scheduled time. Repeating Flashes keep scheduling future occurrences until marked Resolved; reactivating one resumes at its next future occurrence. Closing the window keeps Amnessia running in the background. The Quit button stops reminders; Amnessia must be running for reminders to be delivered. Installing it adds the `amnessia` command and starts the app after graphical login; it checks overdue reminders when started again.

Data is stored in `$XDG_DATA_HOME/amnessia/amnessia.db`, or `~/.local/share/amnessia/amnessia.db` if unset. Existing databases are upgraded in place without replacing Episodes or Flashes. The sidebar can be collapsed; the theme switch lasts for the current session. Run `cargo build --release` followed by `bin/install-desktop` to install the icon, launcher, and `amnessia` command, and add the app to Omarchy's Hyprland login startup. The launcher uses the release binary in this project, so rebuild it after changes.
