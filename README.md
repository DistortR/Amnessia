<img src="assets/amnessia-logo.png" alt="Amnessia logo" width="480">

# Amnessia

A native Linux desktop app for keeping track of Flashes in Episodes.

## Install

Install these build and runtime requirements with your distribution's package manager:

- Rust and Cargo
- `pkg-config`
- GTK 4.10 or newer development files
- SQLite development files
- `notify-send` (usually provided by `libnotify`)

Then clone the repository and run the installer as your regular desktop user. Do not use `sudo`:

```sh
git clone https://github.com/DistortR/Amnessia.git
cd Amnessia
./install.sh
```

The installer checks the requirements, builds the current source in release mode, and installs a user-level app command, icon, menu entry, and login autostart. It uses Omarchy's Hyprland startup hook on Omarchy and XDG autostart on other desktop environments.

## Run and update

Launch Amnessia from your app menu or run:

```sh
amnessia
```

To install newer code, update the checkout and rerun the installer:

```sh
git pull
./install.sh
```

For local code changes, just rerun `./install.sh`. Quit and reopen Amnessia to use the newly installed build. Closing its window keeps it running in the tray for reminders; the Quit button stops it. The app starts after graphical login so desktop notifications are available.

## Data

Episodes and Flashes are stored in SQLite at `$XDG_DATA_HOME/amnessia/amnessia.db`, or `~/.local/share/amnessia/amnessia.db` when `XDG_DATA_HOME` is unset. Schema upgrades run automatically and preserve existing data.
