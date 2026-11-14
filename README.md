<img src="assets/amnessia-logo.png" alt="Amnessia logo" width="480">

# Amnessia

A native Linux desktop app for keeping track of Flashes in Episodes.

## Install

### Install build dependencies manually (optional)

`install.sh` can install missing dependencies for you on these distributions. To install them yourself first, use the command for your system:

**Arch Linux / Omarchy**

```sh
sudo pacman -S --needed base-devel rust pkgconf gtk4 sqlite libnotify
```

**Debian / Ubuntu**

```sh
sudo apt update
sudo apt install build-essential cargo pkg-config libgtk-4-dev libsqlite3-dev libnotify-bin
```

**Fedora**

```sh
sudo dnf install gcc gcc-c++ make cargo pkgconf-pkg-config gtk4-devel sqlite-devel libnotify
```

GTK 4.10 or newer is required. If your distribution installs an older version, use a newer release or package source that provides GTK 4.10+.

### Install Amnessia

Clone the repository and run the installer as your regular desktop user:

```sh
git clone https://github.com/DistortR/Amnessia.git
cd Amnessia
./install.sh
```

The installer checks for Rust/Cargo, `pkg-config`, GTK 4.10 or newer development files, SQLite development files, and `notify-send`. If any are missing, it displays the package commands for Arch, Debian/Ubuntu, or Fedora and asks before running them with `sudo` or `doas`. On other distributions, install the equivalent packages with your package manager and rerun the script. The app itself installs in your home directory; do not run `install.sh` with `sudo`.

It builds the current source in release mode and installs a user-level app command, icon, menu entry, and login autostart. It uses Omarchy's Hyprland startup hook on Omarchy and XDG autostart on other desktop environments. GTK 4.10 or newer is required, so older distributions may need a newer GTK package source.

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
