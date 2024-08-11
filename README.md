# <img src="assets/logo.png" alt="Wireceptor Logo" width="30" height="30"> Wireceptor

Wireceptor is a simple WireGuard VPN indicator designed for Ubuntu/Debian systems. It places an icon in the Ubuntu top bar or Debian toolbar, displaying the current status of WireGuard.

## Features

-   Displays WireGuard status in the system tray
-   Green icon when WireGuard is ON
-   Grey icon when WireGuard is OFF
-   Left or right click on the icon to toggle status

## Installation

1. Download the artifact
1. Unzip
1. Run the `install.sh` script (superuser privileges are needed)
1. (optional) Run the `bypassword.sh` script (superuser privileges are needed) to avoid entering password for every action

## Usage

The app starts automatically after installation and upon system boot. Once the application is running, you will see the Wireceptor icon in your system tray. The icon will change color based on the status of WireGuard.

Scripts for managing the WireGuard status are located in `Downloads/Wireceptor/wireceptor/scripts` when you download the artifacts, and in `/opt/wireceptor/scripts` when you install the app. These scripts can be updated to whatever WireGuard managing method you use, but keep in mind that changing them affects the `bypassword.sh` script (`SUDOERS_RULES` variable). If you rely on it, you should probably also update it accordingly.

## Changelog

### [[1.1.1](https://github.com/hrnxm/wireceptor/actions/runs/10338840193/artifacts/1799013553)] - Bypass password fix

-   Fix the format in `bypassword.sh` script.

### [1.1.0] - Bypass password

-   Run the `bypassword.sh` script if you want to avoid password prompt for turning WireGuard ON/OFF.

### [1.0.0] - Initial release

-   Basic functionality to display and toggle WireGuard status in the system tray.
