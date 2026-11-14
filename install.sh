#!/usr/bin/env bash
set -Eeuo pipefail

fail() {
  printf 'Amnessia installer: %s\n' "$*" >&2
  exit 1
}

[[ "$(uname -s)" == Linux ]] || fail 'This installer supports Linux only.'
(( EUID != 0 )) || fail 'Run this as your desktop user, without sudo.'
[[ -n "${HOME:-}" && -d "$HOME" ]] || fail 'Your home directory could not be found.'

for tool in install ln mv cp grep sed readlink; do
  command -v "$tool" >/dev/null 2>&1 || fail "Install the '$tool' command first."
done

find_missing_requirements() {
  missing=()
  command -v cargo >/dev/null 2>&1 && command -v rustc >/dev/null 2>&1 || missing+=("Rust and Cargo")
  command -v pkg-config >/dev/null 2>&1 || missing+=("pkg-config")
  if command -v pkg-config >/dev/null 2>&1; then
    if ! pkg-config --atleast-version=4.10 gtk4 2>/dev/null; then
      gtk_version=$(pkg-config --modversion gtk4 2>/dev/null || printf 'not installed')
      missing+=("GTK 4.10+ development files (found $gtk_version)")
    fi
    pkg-config --exists sqlite3 2>/dev/null || missing+=("SQLite development files")
  else
    missing+=("GTK 4.10+ development files" "SQLite development files")
  fi
  command -v notify-send >/dev/null 2>&1 || missing+=("notify-send")
}

find_missing_requirements
if ((${#missing[@]})); then
  printf 'Amnessia needs these packages before it can build:\n' >&2
  printf '  - %s\n' "${missing[@]}" >&2

  . /etc/os-release 2>/dev/null || true
  distro="${ID:-} ${ID_LIKE:-}"
  if [[ "$distro" == *arch* ]] && command -v pacman >/dev/null 2>&1; then
    package_manager=pacman
  elif [[ "$distro" == *debian* || "$distro" == *ubuntu* ]] && command -v apt-get >/dev/null 2>&1; then
    package_manager=apt-get
  elif [[ "$distro" == *fedora* || "$distro" == *rhel* ]] && command -v dnf >/dev/null 2>&1; then
    package_manager=dnf
  else
    package_manager=''
  fi

  if [[ -z "$package_manager" ]]; then
    fail "No supported package manager was detected. Install Rust/Cargo, pkg-config, GTK 4.10+ development files, SQLite development files, and notify-send with your distribution's package manager, then rerun ./install.sh."
  fi

  if command -v sudo >/dev/null 2>&1; then
    privilege=(sudo)
  elif command -v doas >/dev/null 2>&1; then
    privilege=(doas)
  else
    privilege=()
  fi

  update_command=()
  install_command=()
  case "$package_manager" in
    pacman)
      install_command=("${privilege[@]}" pacman -S --needed base-devel rust pkgconf gtk4 sqlite libnotify)
      ;;
    apt-get)
      update_command=("${privilege[@]}" apt-get update)
      install_command=("${privilege[@]}" apt-get install build-essential cargo pkg-config libgtk-4-dev libsqlite3-dev libnotify-bin)
      ;;
    dnf)
      install_command=("${privilege[@]}" dnf install gcc gcc-c++ make cargo pkgconf-pkg-config gtk4-devel sqlite-devel libnotify)
      ;;
  esac

  printf '\nThe installer can run these system package commands:\n' >&2
  if ((${#update_command[@]})); then
    printf '  ' >&2
    printf '%q ' "${update_command[@]}" >&2
    printf '\n' >&2
  fi
  printf '  ' >&2
  printf '%q ' "${install_command[@]}" >&2
  printf '\n' >&2
  if ((${#privilege[@]} == 0)); then
    fail 'Neither sudo nor doas is installed. Run the commands above as an administrator, then rerun ./install.sh.'
  fi
  if [[ ! -t 0 ]]; then
    fail 'No interactive terminal is available to approve system package installation. Run the commands above, then rerun ./install.sh.'
  fi
  read -r -p 'Install these missing dependencies now? [Y/n] ' answer
  case "${answer,,}" in
    n|no) fail 'Dependency installation declined. Run the commands above, then rerun ./install.sh.' ;;
  esac

  if ((${#update_command[@]})); then "${update_command[@]}"; fi
  "${install_command[@]}"
  find_missing_requirements
  ((${#missing[@]} == 0)) || fail "Dependencies are still missing after package installation: ${missing[*]}"
fi

project_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
source_binary="$project_dir/target/release/amnessia-demo"
command_dir="$HOME/.local/bin"
command_path="$command_dir/amnessia"
install_dir="$HOME/.local/lib/amnessia"
installed_binary="$install_dir/amnessia"

if [[ -L "$command_path" ]]; then
  old_target=$(readlink -m -- "$command_path")
  case "$old_target" in
    "$installed_binary"|*/target/release/amnessia-demo) ;;
    *) fail "Refusing to replace the existing command at $command_path" ;;
  esac
elif [[ -e "$command_path" ]]; then
  fail "Refusing to replace the existing file at $command_path"
fi

printf 'Building Amnessia from %s…\n' "$project_dir"
CARGO_TARGET_DIR="$project_dir/target" cargo build --release --locked --manifest-path "$project_dir/Cargo.toml"
[[ -x "$source_binary" ]] || fail 'Cargo finished without producing the Amnessia binary.'

mkdir -p -- "$install_dir" "$command_dir"
temporary_binary="$install_dir/.amnessia.$$"
temporary_link="$command_dir/.amnessia.$$"
trap 'rm -f -- "$temporary_binary" "$temporary_link"' EXIT
install -m 755 -- "$source_binary" "$temporary_binary"
mv -f -- "$temporary_binary" "$installed_binary"
ln -s -- "$installed_binary" "$temporary_link"
mv -f -- "$temporary_link" "$command_path"

data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
[[ "$data_home" == /* ]] || data_home="$HOME/.local/share"
config_home="${XDG_CONFIG_HOME:-$HOME/.config}"
[[ "$config_home" == /* ]] || config_home="$HOME/.config"

icon_path="$data_home/icons/hicolor/scalable/apps/com.amnessia.App-triangle.svg"
desktop_dir="$data_home/applications"
desktop_path="$desktop_dir/com.amnessia.App.desktop"
install -D -m 644 -- "$project_dir/assets/com.amnessia.App.svg" "$icon_path"
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache --force --ignore-theme-index --quiet "$data_home/icons/hicolor" ||
    printf 'Amnessia installer: warning: could not refresh the user icon cache.\n' >&2
fi
mkdir -p -- "$desktop_dir"

escaped_command=$(printf '%s' "$command_path" | sed 's/\\/\\\\/g; s/"/\\"/g; s/%/%%/g')
cat > "$desktop_path" <<EOF
[Desktop Entry]
Version=1.0
Type=Application
Name=Amnessia
Comment=Keep track of your Flashes
Exec="$escaped_command"
Icon=com.amnessia.App-triangle
Terminal=false
Categories=Utility;
StartupWMClass=com.amnessia.App
EOF

if [[ -f /usr/share/omarchy/default/hypr/helpers.lua ]]; then
  hypr_autostart="$config_home/hypr/autostart.lua"
  mkdir -p -- "$(dirname -- "$hypr_autostart")"
  [[ -e "$hypr_autostart" ]] || printf '%s\n' '-- Extra autostart processes.' > "$hypr_autostart"
  startup_line='o.exec_on_start("amnessia")'
  if ! grep -Fqx -- "$startup_line" "$hypr_autostart"; then
    if [[ -e "$hypr_autostart" ]]; then
      backup="$hypr_autostart.bak.$(date +%Y%m%d%H%M%S)"
      [[ ! -e "$backup" ]] || backup="$backup.$$"
      cp -p -- "$hypr_autostart" "$backup"
      printf 'Backed up Hyprland startup config: %s\n' "$backup"
    fi
    printf '\n%s\n' "$startup_line" >> "$hypr_autostart"
  fi
  rm -f -- "$config_home/autostart/com.amnessia.App.desktop"
  autostart_location="$hypr_autostart (Omarchy)"
else
  autostart_dir="$config_home/autostart"
  autostart_path="$autostart_dir/com.amnessia.App.desktop"
  mkdir -p -- "$autostart_dir"
  cat > "$autostart_path" <<EOF
[Desktop Entry]
Type=Application
Name=Amnessia
Comment=Run Flash reminders after login
Exec="$escaped_command"
Icon=com.amnessia.App-triangle
Terminal=false
X-GNOME-Autostart-enabled=true
EOF
  autostart_location="$autostart_path"
fi

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$desktop_dir" >/dev/null 2>&1 || true
fi

printf 'Installed command: %s\nInstalled app launcher: %s\nAutostart: %s\n' \
  "$command_path" "$desktop_path" "$autostart_location"
case ":${PATH:-}:" in
  *":$command_dir:"*) ;;
  *) printf 'Add %s to PATH to run Amnessia from a terminal.\n' "$command_dir" ;;
esac
printf 'Run `amnessia` or open it from your app menu. Rerun ./install.sh after code changes to update it.\n'
