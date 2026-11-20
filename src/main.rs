mod vpn;

use ksni::{blocking::TrayMethods, menu::StandardItem, MenuItem};
use std::{
    collections::{BTreeSet, HashMap},
    error::Error,
    sync::mpsc::{self, RecvTimeoutError, Sender},
    time::Duration,
};
use vpn::{State, Target, Tunnel};
use zbus::{
    blocking::{Connection, Proxy},
    zvariant::Value,
};

#[derive(Default)]
struct Snapshot {
    tunnels: Vec<Tunnel>,
    warnings: Vec<String>,
}

impl Snapshot {
    fn read(bus: &Connection, remembered: &mut BTreeSet<String>) -> Self {
        let mut snapshot = Self::default();
        match vpn::network_manager(bus) {
            Ok(profiles) => snapshot.tunnels = profiles,
            Err(e) => snapshot.warnings.push(format!("NetworkManager: {e}")),
        }
        match vpn::wg_quick(
            bus,
            remembered,
            snapshot.tunnels.is_empty() && snapshot.warnings.is_empty(),
        ) {
            Ok(units) => snapshot.tunnels.extend(units),
            Err(e) => snapshot.warnings.push(format!("wg-quick: {e}")),
        }
        snapshot
    }
}

enum Request {
    Set(Target, bool),
    Quit,
}

struct Tray {
    snapshot: Snapshot,
    request: Sender<Request>,
    pending: bool,
    last_error: Option<String>,
}

fn message(label: String) -> MenuItem<Tray> {
    StandardItem {
        label,
        enabled: false,
        ..Default::default()
    }
    .into()
}

impl ksni::Tray for Tray {
    const MENU_ON_ACTIVATE: bool = true;

    fn id(&self) -> String {
        "wireceptor".into()
    }
    fn title(&self) -> String {
        "Wireceptor".into()
    }
    fn icon_name(&self) -> String {
        let tunnels = &self.snapshot.tunnels;
        if self.pending || tunnels.iter().any(|t| t.state == State::Changing) {
            "wireceptor-loading"
        } else if tunnels.iter().any(|t| t.state == State::On) {
            "wireceptor-on"
        } else if !self.snapshot.warnings.is_empty()
            || tunnels
                .iter()
                .any(|t| matches!(t.state, State::Unavailable | State::Failed))
        {
            "wireceptor-loading"
        } else {
            "wireceptor-off"
        }
        .into()
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let mut menu = Vec::new();
        for tunnel in &self.snapshot.tunnels {
            let owner = match tunnel.target {
                Target::NetworkManager(_) => "NetworkManager",
                Target::WgQuick(_) => "wg-quick",
            };
            let label = match tunnel.state {
                State::On => "Disconnect",
                State::Off => "Connect",
                State::Failed => "Retry",
                State::Changing => "Changing connection",
                State::Unavailable => "Unavailable",
            };
            let target = tunnel.target.clone();
            let action = tunnel.state.action();
            menu.push(
                StandardItem {
                    label: format!("{label} {} ({owner})", tunnel.name.replace('_', "__")),
                    enabled: !self.pending && action.is_some(),
                    activate: Box::new(move |tray: &mut Self| {
                        if let Some(on) = action.filter(|_| !tray.pending) {
                            tray.pending =
                                tray.request.send(Request::Set(target.clone(), on)).is_ok();
                            tray.last_error = None;
                        }
                    }),
                    ..Default::default()
                }
                .into(),
            );
        }
        if self.pending {
            menu.push(message("Applying change…".into()));
        } else if self.snapshot.tunnels.is_empty() && self.snapshot.warnings.is_empty() {
            menu.push(message("No WireGuard profiles found".into()));
        }
        for text in self.last_error.iter().chain(&self.snapshot.warnings) {
            menu.push(message(text.lines().next().unwrap_or(text).into()));
        }
        menu.push(MenuItem::Separator);
        menu.push(
            StandardItem {
                label: "Quit Wireceptor".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.request.send(Request::Quit);
                }),
                ..Default::default()
            }
            .into(),
        );
        menu
    }
}

fn notify_error(session: &Connection, text: &str) {
    eprintln!("{text}");
    if let Ok(proxy) = Proxy::new(
        session,
        "org.freedesktop.Notifications",
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    ) {
        let _ = proxy.call::<_, _, u32>(
            "Notify",
            &(
                "Wireceptor",
                0u32,
                "wireceptor",
                "Wireceptor",
                text,
                Vec::<String>::new(),
                HashMap::<String, Value>::new(),
                -1i32,
            ),
        );
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut remembered = BTreeSet::new();
    if let Some(name) = std::env::var_os("WIRECEPTOR_INTERFACE") {
        let name = name
            .to_str()
            .filter(|s| vpn::valid_interface(s))
            .ok_or("Invalid WIRECEPTOR_INTERFACE")?;
        remembered.insert(name.to_owned());
    }
    let session = Connection::session()?;
    match session.request_name_with_flags(
        "io.github.hrnxm.Wireceptor",
        zbus::fdo::RequestNameFlags::DoNotQueue.into(),
    ) {
        Err(zbus::Error::NameTaken) => return Ok(()),
        result => {
            result?;
        }
    }
    let bus = Connection::system()?;
    let (request, requests) = mpsc::channel();
    let tray = Tray {
        snapshot: Snapshot::read(&bus, &mut remembered),
        request,
        pending: false,
        last_error: None,
    }
    .assume_sni_available(true)
    .spawn()?;

    while !tray.is_closed() {
        let request = match requests.recv_timeout(Duration::from_secs(10)) {
            Ok(Request::Set(target, on)) => Some((target, on)),
            Ok(Request::Quit) => break,
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        let mut snapshot = Snapshot::read(&bus, &mut remembered);
        let acted = request.is_some();
        let mut error = None;
        if let Some((target, on)) = request {
            // A desktop action or another manager may have changed ownership since the menu opened.
            let current = snapshot.tunnels.iter().find(|t| t.target == target);
            if current.is_some_and(|t| t.state.action() == Some(on)) {
                error = target.set(on).err();
                if let Some(text) = &error {
                    notify_error(&session, text);
                }
                snapshot = Snapshot::read(&bus, &mut remembered);
            }
        }
        tray.update(|tray| {
            tray.snapshot = snapshot;
            if acted {
                tray.pending = false;
                tray.last_error = error;
            }
        });
    }
    tray.shutdown().wait();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ksni::Tray as _;

    #[test]
    fn click_requests_one_change_and_waits_for_the_manager() {
        let (request, received) = mpsc::channel();
        let target = Target::WgQuick("office".into());
        let mut tray = Tray {
            snapshot: Snapshot {
                tunnels: vec![Tunnel {
                    name: "office".into(),
                    target: target.clone(),
                    state: State::Off,
                }],
                warnings: vec![],
            },
            request,
            pending: false,
            last_error: None,
        };
        assert_eq!(tray.icon_name(), "wireceptor-off");
        let MenuItem::Standard(connect) = tray.menu().remove(0) else {
            panic!("missing toggle")
        };
        (connect.activate)(&mut tray);
        (connect.activate)(&mut tray);
        assert!(matches!(received.try_recv(), Ok(Request::Set(id, true)) if id == target));
        assert!(received.try_recv().is_err());
        assert_eq!(tray.icon_name(), "wireceptor-loading");

        tray.pending = false;
        tray.snapshot.tunnels[0].state = State::On;
        assert_eq!(tray.icon_name(), "wireceptor-on");
        tray.snapshot.tunnels[0].state = State::Unavailable;
        let MenuItem::Standard(item) = tray.menu().remove(0) else {
            panic!("missing status")
        };
        assert!(!item.enabled);
        (item.activate)(&mut tray);
        assert!(received.try_recv().is_err());
    }
}
