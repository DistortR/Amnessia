#!/usr/bin/env bash
set -Eeuo pipefail

fail() {
  printf 'Amnessia installer: %s\n' "$*" >&2
  exit 1
}

[[ "$(uname -s)" == Linux ]] || fail 'This installer supports Linux only.'
(( EUID != 0 )) || fail 'Run this as your desktop user, without sudo.'
[[ -n "${HOME:-}" && -d "$HOME" ]] || fail 'Your home directory could not be found.'

for tool in cargo pkg-config install ln mv cp grep sed readlink; do
  command -v "$tool" >/dev/null 2>&1 || fail "Install the '$tool' command first."
done

pkg-config --atleast-version=4.10 gtk4 2>/dev/null || {
  gtk_version=$(pkg-config --modversion gtk4 2>/dev/null || printf 'not installed')
  fail "GTK 4.10 or newer development files are required (found $gtk_version)."
}
pkg-config --exists sqlite3 2>/dev/null || fail 'SQLite development files are required.'
command -v notify-send >/dev/null 2>&1 || fail "Install 'notify-send' (usually provided by libnotify)."

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
mkdir -p -- "$desktop_dir"

escaped_command=$(printf '%s' "$command_path" | sed 's/\\/\\\\/g; s/"/\\"/g; s/%/%%/g')
cat > "$desktop_path" <<EOF
[Desktop Entry]
Version=1.0
Type=Application
Name=Amnessia
Comment=Keep track of your Flashes
Exec="$escaped_command"
TryExec="$escaped_command"
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
