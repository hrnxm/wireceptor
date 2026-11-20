# <img src="assets/logo.png" alt="Wireceptor Logo" width="30" height="30"> Wireceptor

Wireceptor is a simple WireGuard VPN indicator designed for Ubuntu/Debian systems. It places an icon in the Ubuntu top bar or Debian toolbar, displaying the current status of WireGuard.

## Features

- Displays WireGuard status in the system tray
- Green icon when WireGuard is ON
- Grey icon when WireGuard is OFF
- Left or right click on the icon to open the menu and toggle a connection

## Installation

### 2.0.0

1. Download the `.deb` for your architecture (`dpkg --print-architecture`) from the artifacts of a successful Rust build in [GitHub Actions](https://github.com/hrnxm/wireceptor/actions/workflows/ci.yml).
1. From the download directory, run `sudo apt install "./wireceptor_2.0.0_$(dpkg --print-architecture).deb"`.
1. Launch **Wireceptor** from the applications menu. After an upgrade, quit the running indicator first.

The package lets members of the `sudo` group start and stop wg-quick tunnels without a password in an active local desktop session. NetworkManager uses its own authorization policy.

If you are upgrading from the AppImage, quit it and move its old `~/.config/autostart/wireceptor.desktop` aside so the packaged login entry takes effect. If you used `bypassword.sh`, you can retire its Wireceptor-specific rules with `sudo visudo`.

To build the package locally, run `./packaging/build-deb.sh` on Ubuntu or Debian with Rust 1.90 or later installed through rustup, a C linker and `dpkg-deb`. The installer is written to `dist/`.

### [DEPRECATED] 1.x (AppImage)

1. Download the artifact
1. Unzip
1. Run the `install.sh` script (superuser privileges are needed)
1. (optional) Run the `bypassword.sh` script (superuser privileges are needed) to avoid entering password for every action

## Usage

The app starts automatically at desktop login. Once the application is running, you will see the Wireceptor icon in your system tray. The icon will change color based on the status of WireGuard.

In 2.0, each NetworkManager WireGuard profile and each discovered wg-quick service has its own menu item. To switch between tunnels while keeping only one active, disconnect the current tunnel before connecting the next one. If a stopped wg-quick tunnel is hidden by the permissions on `/etc/wireguard`, you can specify its name when launching the app, for example `WIRECEPTOR_INTERFACE=office wireceptor`.

### 1.x scripts

Scripts for managing the WireGuard status are located in `Downloads/Wireceptor/wireceptor/scripts` when you download the artifacts, and in `/opt/wireceptor/scripts` when you install the app. These scripts can be updated to whatever WireGuard managing method you use, but keep in mind that changing them affects the `bypassword.sh` script (`SUDOERS_RULES` variable). If you rely on it, you should probably also update it accordingly.

## Changelog

### [2.0.0] - Rust rewrite

### [1.1.1] - Bypass password fix

- Fix the format in `bypassword.sh` script.

### [1.1.0] - Bypass password

- Run the `bypassword.sh` script if you want to avoid password prompt for turning WireGuard ON/OFF.

### [1.0.0] - Initial release

- Basic functionality to display and toggle WireGuard status in the system tray.
