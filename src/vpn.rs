use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    ffi::OsStr,
    fs,
    path::Path,
    process::Command,
};
use zbus::{
    blocking::{proxy::Builder, Connection, Proxy},
    proxy::CacheProperties,
    zvariant::{OwnedObjectPath, OwnedValue},
};

const NM: &str = "org.freedesktop.NetworkManager";
const SYSTEMD: &str = "org.freedesktop.systemd1";
const TEMPLATE: &str = "wg-quick@.service";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    On,
    Off,
    Changing,
    Failed,
    Unavailable,
}

impl State {
    pub fn action(self) -> Option<bool> {
        match self {
            Self::On => Some(false),
            Self::Off | Self::Failed => Some(true),
            Self::Changing | Self::Unavailable => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    NetworkManager(String),
    WgQuick(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tunnel {
    pub name: String,
    pub target: Target,
    pub state: State,
}

impl Target {
    fn command(&self, on: bool) -> Command {
        match self {
            Self::NetworkManager(uuid) => {
                let mut cmd = Command::new("nmcli");
                cmd.args(["connection", if on { "up" } else { "down" }, "uuid", uuid]);
                cmd
            }
            Self::WgQuick(name) => {
                let mut cmd = Command::new("/usr/bin/pkexec");
                cmd.args([
                    "--disable-internal-agent",
                    "/usr/libexec/wireceptor/wg-quick-control",
                    if on { "start" } else { "stop" },
                    name,
                ]);
                cmd
            }
        }
    }

    pub fn set(&self, on: bool) -> Result<(), String> {
        let mut cmd = self.command(on);
        let program = cmd.get_program().to_string_lossy().into_owned();
        let output = cmd.output().map_err(|e| format!("{program}: {e}"))?;
        if output.status.success() {
            Ok(())
        } else {
            let reason = String::from_utf8_lossy(&output.stderr);
            Err(format!("{program} ({}): {}", output.status, reason.trim()))
        }
    }
}

fn proxy<'a>(
    bus: &Connection,
    service: &'a str,
    path: &'a str,
    interface: &'a str,
) -> zbus::Result<Proxy<'a>> {
    Builder::new(bus)
        .destination(service)?
        .path(path)?
        .interface(interface)?
        .cache_properties(CacheProperties::No)
        .build()
}

pub fn network_manager(bus: &Connection) -> zbus::Result<Vec<Tunnel>> {
    let dbus = zbus::blocking::fdo::DBusProxy::new(bus)?;
    if !dbus.name_has_owner(NM.try_into()?)? {
        return Ok(Vec::new());
    }
    let manager = proxy(bus, NM, "/org/freedesktop/NetworkManager", NM)?;
    let active: Vec<OwnedObjectPath> = manager.get_property("ActiveConnections")?;
    let mut states = HashMap::new();
    for path in active {
        let conn = proxy(
            bus,
            NM,
            path.as_str(),
            "org.freedesktop.NetworkManager.Connection.Active",
        )?;
        if conn.get_property::<String>("Type")? == "wireguard" {
            let state = match conn.get_property::<u32>("State")? {
                1 | 3 => State::Changing,
                2 => State::On,
                4 => State::Off,
                _ => State::Unavailable,
            };
            states.insert(conn.get_property::<OwnedObjectPath>("Connection")?, state);
        }
    }

    let settings = proxy(
        bus,
        NM,
        "/org/freedesktop/NetworkManager/Settings",
        "org.freedesktop.NetworkManager.Settings",
    )?;
    let profiles: Vec<OwnedObjectPath> = settings.call("ListConnections", &())?;
    let mut tunnels = Vec::new();
    for path in profiles {
        let conn = proxy(
            bus,
            NM,
            path.as_str(),
            "org.freedesktop.NetworkManager.Settings.Connection",
        )?;
        let settings: HashMap<String, HashMap<String, OwnedValue>> =
            conn.call("GetSettings", &())?;
        let field = |key| {
            settings
                .get("connection")?
                .get(key)?
                .downcast_ref::<&str>()
                .ok()
        };
        if field("type") != Some("wireguard") {
            continue;
        }
        // NetworkManager also creates placeholder profiles for interfaces owned by wg-quick.
        const EXTERNAL: u32 = 0x08;
        if conn.get_property::<u32>("Flags")? & EXTERNAL != 0 {
            continue;
        }
        if let (Some(name), Some(uuid)) = (field("id"), field("uuid")) {
            tunnels.push(Tunnel {
                name: name.into(),
                target: Target::NetworkManager(uuid.into()),
                state: states.get(&path).copied().unwrap_or(State::Off),
            });
        }
    }
    tunnels.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(tunnels)
}

pub fn valid_interface(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 15
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
}

fn instance(unit: &str) -> Option<&str> {
    let name = unit.strip_prefix("wg-quick@")?.strip_suffix(".service")?;
    valid_interface(name).then_some(name)
}

pub fn wg_quick(
    bus: &Connection,
    remembered: &mut BTreeSet<String>,
    default_wg0: bool,
) -> zbus::Result<Vec<Tunnel>> {
    let manager = proxy(
        bus,
        SYSTEMD,
        "/org/freedesktop/systemd1",
        "org.freedesktop.systemd1.Manager",
    )?;
    let filter = (&[] as &[&str], &["wg-quick@*.service"][..]);
    let files: Vec<(String, String)> = manager.call("ListUnitFilesByPatterns", &filter)?;
    let mut names = remembered.clone();
    let mut masked = BTreeSet::new();
    let mut template = false;
    for (path, state) in files {
        let Some(unit) = Path::new(&path).file_name().and_then(OsStr::to_str) else {
            continue;
        };
        if unit == TEMPLATE {
            template = !state.starts_with("masked");
        } else if let Some(name) = instance(unit) {
            names.insert(name.to_owned());
            if state.starts_with("masked") {
                masked.insert(name.to_owned());
            }
        }
    }
    // Enabling a template creates a wants/ symlink that ListUnitFilesByPatterns omits.
    for dir in [
        "/etc/systemd/system/multi-user.target.wants",
        "/run/systemd/system/multi-user.target.wants",
    ] {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str().and_then(instance) {
                    names.insert(name.to_owned());
                }
            }
        }
    }
    let mut configs_hidden = false;
    if template {
        match fs::read_dir("/etc/wireguard") {
            Ok(entries) => {
                for entry in entries.flatten() {
                    if let Some(name) = entry
                        .file_name()
                        .to_str()
                        .and_then(|f| f.strip_suffix(".conf"))
                        .filter(|s| valid_interface(s))
                    {
                        names.insert(name.into());
                    }
                }
            }
            Err(e) => configs_hidden = e.kind() == std::io::ErrorKind::PermissionDenied,
        }
    }

    // ListUnitsByPatterns returns id, description, load/active/sub states, following,
    // unit path and job information. Only loaded service instances have live state.
    type Unit = (
        String,
        String,
        String,
        String,
        String,
        String,
        OwnedObjectPath,
        u32,
        String,
        OwnedObjectPath,
    );
    let units: Vec<Unit> = manager.call("ListUnitsByPatterns", &filter)?;
    let mut states = BTreeMap::new();
    for (id, _, load, active, ..) in units {
        if let Some(name) = instance(&id) {
            if load == "not-found" {
                continue;
            }
            names.insert(name.to_owned());
            remembered.insert(name.to_owned());
            let state = match (load.as_str(), active.as_str()) {
                (_, "active") => State::On,
                ("loaded", "inactive") => State::Off,
                ("loaded", "failed") => State::Failed,
                (_, "activating" | "deactivating" | "reloading") => State::Changing,
                _ => State::Unavailable,
            };
            states.insert(name.to_owned(), state);
        }
    }
    if configs_hidden && default_wg0 && names.is_empty() {
        names.insert("wg0".into());
    }
    Ok(names
        .into_iter()
        .map(|name| {
            let mut state = states.get(&name).copied().unwrap_or(if template {
                State::Off
            } else {
                State::Unavailable
            });
            if matches!(state, State::Off | State::Failed)
                && (masked.contains(&name) || Path::new("/sys/class/net").join(&name).exists())
            {
                state = State::Unavailable;
            }
            Tunnel {
                target: Target::WgQuick(name.clone()),
                name,
                state,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_concrete_wg_quick_instances_are_targets() {
        assert_eq!(instance("wg-quick@office-1.service"), Some("office-1"));
        for unit in [
            TEMPLATE,
            "wg-quick@../etc.service",
            "wg-quick@*.service",
            "ssh.service",
            "wg-quick@1234567890123456.service",
        ] {
            assert_eq!(instance(unit), None);
        }
    }
    #[test]
    fn actions_address_one_profile_or_service() {
        let command = Target::NetworkManager("profile-id".into()).command(false);
        assert_eq!(command.get_program(), "nmcli");
        assert!(command
            .get_args()
            .eq(["connection", "down", "uuid", "profile-id"]));
        let command = Target::WgQuick("office".into()).command(true);
        assert_eq!(command.get_program(), "/usr/bin/pkexec");
        assert!(command.get_args().eq([
            "--disable-internal-agent",
            "/usr/libexec/wireceptor/wg-quick-control",
            "start",
            "office"
        ]));
    }
}
