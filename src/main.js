const { app, Menu, Tray, Notification } = require("electron");
const { exec } = require("child_process");
const path = require("path");

if (!app.requestSingleInstanceLock()) {
    console.error("Wireceptor already running!");
    app.quit();
}

const ICON_ON = path.join(__dirname, "..", "assets", "on.png");
const ICON_OFF = path.join(__dirname, "..", "assets", "off.png");
const ICON_LOADING = path.join(__dirname, "..", "assets", "loading.png");

const SCRIPTS_PATH = app.isPackaged
    ? path.join("/", "opt", "wireceptor", "scripts")
    : path.join(__dirname, "..", "scripts");

const START_SCRIPT = path.join(SCRIPTS_PATH, "start.sh");
const STOP_SCRIPT = path.join(SCRIPTS_PATH, "stop.sh");
const STATUS_SCRIPT = path.join(SCRIPTS_PATH, "status.sh");

let tray = null;

app.whenReady().then(() => {
    tray = new Tray(ICON_LOADING);
    tray.setToolTip("Wireceptor for WireGuard VPN");
    setLoadingState();
    checkVpnStatus();
    setInterval(checkVpnStatus, 10000);
});

app.on("window-all-closed", () => {
    app.quit();
});

function checkVpnStatus() {
    getStatusAndThen((status) => {
        if (status === "on") {
            setOnState();
        } else if (status === "off") {
            setOffState();
        }
    });
}

function toggleVpn() {
    setLoadingState();
    getStatusAndThen((status) => {
        if (status === "on") {
            turnOff();
        } else if (status === "off") {
            turnOn();
        }
    });
}

function getStatusAndThen(callback) {
    exec(STATUS_SCRIPT, (error, stdout) => {
        // Exit code 3 just means the service is not running
        if (error && error.code !== 3) {
            return handleError(error);
        }
        if (error?.code === 3 || stdout.includes("inactive")) {
            callback("off");
        } else {
            callback("on");
        }
    });
}

function turnOn() {
    exec(START_SCRIPT, (error) => {
        if (error) {
            handleError(error);
            setOffState();
            return;
        }
        setOnState();
    });
}

function turnOff() {
    exec(STOP_SCRIPT, (error) => {
        if (error) {
            handleError(error);
            setOnState();
            return;
        }
        setOffState();
    });
}

function setOnState() {
    tray.setContextMenu(
        Menu.buildFromTemplate([
            { label: "Turn WireGuard OFF", type: "normal", click: toggleVpn },
            { label: "Quit Wireceptor", type: "normal", click: () => app.quit() },
        ])
    );
    tray.setImage(ICON_ON);
}

function setOffState() {
    tray.setContextMenu(
        Menu.buildFromTemplate([
            { label: "Turn WireGuard ON", type: "normal", click: toggleVpn },
            { label: "Quit Wireceptor", type: "normal", click: () => app.quit() },
        ])
    );
    tray.setImage(ICON_OFF);
}

function setLoadingState() {
    tray.setContextMenu(
        Menu.buildFromTemplate([{ label: "Quit Wireceptor", type: "normal", click: () => app.quit() }])
    );
    tray.setImage(ICON_LOADING);
}

function handleError(error) {
    console.error(error);
    new Notification({ title: "Wireceptor error", body: error.message }).show();
}

process.on("uncaughtException", (err) => {
    handleError(err);
    app.quit();
});
