# Amnessia

A Rust and GTK4 app for organizing Flashes in Episodes, with a Letter Glitch background inspired by [React Bits](https://reactbits.dev/backgrounds/letter-glitch).

Run `cargo run --release`. GTK 4.10 or newer, SQLite development libraries, and `notify-send` are required. Add Flashes to the default Static Episode or create another Episode. Select a Flash to edit its name, reminder, and Intensity. Use the bulb to resolve or reopen a Flash; the filter shows All, Resolved, or Faded. The exclamation marks show Intensity.

A Flash can have one reminder date and time. Times are entered in the computer's local time and stored in UTC. When an unresolved Flash reaches its reminder time, it becomes Faded and Amnessia sends a desktop notification with its Intensity and scheduled local time (and date if it is not today). Closing the window keeps Amnessia running in the background. The Quit button stops reminders; Amnessia must be running for reminders to be delivered. It does not start automatically on login, but checks overdue reminders when started again.

Data is stored in `$XDG_DATA_HOME/amnessia/amnessia.db`, or `~/.local/share/amnessia/amnessia.db` if unset. Existing databases are upgraded in place without replacing Episodes or Flashes. The sidebar can be collapsed; the theme switch lasts for the current session. Run `cargo build --release` followed by `bin/install-desktop` to install the icon and development launcher for the current user. The launcher uses the release binary in this project, so rebuild it after changes.
