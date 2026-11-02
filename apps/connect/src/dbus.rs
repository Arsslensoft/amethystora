//! The service on the session bus, as `dbus/org.amethystora.Connect.xml` says (the contract between this
//! program, the shell module and the app). Methods here only check what they were given and call the service;
//! properties are read from the devices; a change is a `PropertiesChanged` and the signals of the interface.

use std::cell::OnceCell;
use std::collections::HashMap;
use std::rc::Rc;

use glib::variant::{ObjectPath, ToVariant};

use crate::device::{DEVICE_INTERFACE, Device};
use crate::service::{ApiError, ApiResult, Service, Signal};

pub const ROOT_INTERFACE: &str = "org.amethystora.Connect";
pub const ROOT_PATH: &str = "/org/amethystora/Connect";
pub const BUS_NAME: &str = "org.amethystora.Connect";
const NOT_AVAILABLE: &str = "org.amethystora.Connect.Error.NotAvailable";
const INVALID_ARGS: &str = "org.freedesktop.DBus.Error.InvalidArgs";

const XML: &str = include_str!("../dbus/org.amethystora.Connect.xml");

/// The programs that may use the service, by the file they run (`/proc/PID/exe` of the caller). The app
/// (Phone) may do anything the interface offers; the shell's module (gnome-shell) the few things it needs: the
/// list of phones, the clipboard it pushes, a ping and a link. Anything else on the bus is refused: a program of the
/// account with the session bus (a Flatpak that was given it, an editor's extension) reads no texts, sends none,
/// and answers no pairing. Tests change the paths in `Config`.
pub const APP_EXE: &str = "/usr/lib/amethystora-phone/amethystora-phone";
pub const MODULE_EXE: &str = "/usr/bin/gnome-shell";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    App,
    Module,
    Other,
}

/// What the module may call, besides reading properties
const MODULE_METHODS: &[&str] = &[
    "GetDevices",
    "PushClipboard",
    "Ping",
    "SendUrl",
    // The live pill's chip for a phone ringing: it only silences the phone's ringer
    "MuteRinger",
    // The module says whether it shows the links phones share, which the program then does not notify itself
    "TakeLinks",
    // The phone's menu rings it; the live pill plays and pauses the phone's music; Present says while it presents
    // (plans/ui-ideas/spec-phone.md)
    "Ring",
    "MediaAction",
    "Presenting",
];

pub fn role_of_exe(exe: &str, app: &str, module: &str) -> Role {
    if exe == app {
        Role::App
    } else if exe == module {
        Role::Module
    } else {
        Role::Other
    }
}

pub fn method_allowed(role: Role, method: &str) -> bool {
    match role {
        Role::App => true,
        Role::Module => MODULE_METHODS.contains(&method),
        Role::Other => false,
    }
}

/// Finds out who a caller is: the process the bus says the connection belongs to, of this account, and the
/// file it runs. Asked of the bus once for each connection (the answer is kept until the connection goes). A
/// caller that cannot be found out is no one.
fn role_of_sender(service: &Service, bus: &gio::DBusConnection, sender: Option<&str>) -> Role {
    let Some(sender) = sender else { return Role::Other };
    if let Some(role) = service.callers.borrow().get(sender) {
        return *role;
    }
    let role = resolve_sender(service, bus, sender);
    service.callers.borrow_mut().insert(sender.to_string(), role);
    role
}

fn resolve_sender(service: &Service, bus: &gio::DBusConnection, sender: &str) -> Role {
    // A call to the bus's own daemon is answered at once; it is made here, in the call that needs the answer
    let reply = bus.call_sync(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
        "GetConnectionCredentials",
        Some(&(sender,).to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        2000,
        None::<&gio::Cancellable>,
    );
    let Ok(reply) = reply else { return Role::Other };
    let credentials = glib::VariantDict::new(Some(&reply.child_value(0)));
    let pid = credentials.lookup::<u32>("ProcessID").ok().flatten();
    let user = credentials.lookup::<u32>("UnixUserID").ok().flatten();
    // SAFETY: getuid has no arguments and cannot fail
    let ours = unsafe { libc::getuid() };
    let (Some(pid), Some(user)) = (pid, user) else { return Role::Other };
    if user != ours {
        return Role::Other;
    }
    match std::fs::read_link(format!("/proc/{pid}/exe")) {
        Ok(exe) => role_of_exe(&exe.to_string_lossy(), &service.cfg.app_exe, &service.cfg.module_exe),
        Err(_) => Role::Other,
    }
}

/// A connection that called as the app or the module: the signals are sent to it. A program that never called, or
/// called and was refused, is sent nothing, whatever it listens for.
fn note_client(service: &Service, sender: Option<&str>, role: Role) {
    if let (true, Some(sender)) = (role != Role::Other, sender) {
        service.clients.borrow_mut().insert(sender.to_string(), role);
    }
}

/// Which signals each of the two is sent: the module follows the phones, the clipboard it carries and the files going
/// out (the dock shows how far a drop on the phone got), a phone ringing and a shared link (the live pill); the app
/// everything but the clipboard and the link. A file coming in is the
/// app's alone: its name is the phone's to say, and the shell has no use for it. `args` are the signal's.
pub fn audience(role: Role, signal: &str, args: Option<&glib::Variant>) -> bool {
    match role {
        // The clipboard is the shell's to set, and a shared link the shell's to show (the live pill)
        Role::App => !matches!(signal, "ClipboardReceived" | "UrlReceived" | "Clicker" | "Code"),
        Role::Module => match signal {
            "DeviceAdded" | "DeviceRemoved" | "PropertiesChanged" | "ClipboardReceived" => true,
            // The live pill: a phone ringing, and a link shared from one
            "Call" => true,
            "UrlReceived" => true,
            // The phone as a slide clicker, which only the shell turns into a key
            "Clicker" => true,
            // A one-time code, the digits alone (R7-23): never the text, who sent it or which phone. The module never reads
            // texts: ListMessages and MessagesChanged stay the app's
            "Code" => true,
            // (transfer_id, name, incoming, done, total, state)
            "Transfer" => args.filter(|a| a.n_children() > 2).and_then(|a| a.child_value(2).get::<bool>()) == Some(false),
            _ => false,
        },
        Role::Other => false,
    }
}

/// The value of a property for a caller that may not read it: the empty one of its kind
fn redact(value: &glib::Variant) -> glib::Variant {
    match value.type_().as_str() {
        "s" => "".to_variant(),
        "b" => false.to_variant(),
        "i" => (-1i32).to_variant(),
        "x" => 0i64.to_variant(),
        "as" => Vec::<String>::new().to_variant(),
        "aa{sv}" => Vec::<HashMap<String, glib::Variant>>::new().to_variant(),
        _ => false.to_variant(),
    }
}

/// Forgets a caller when its connection goes
fn watch_callers(service: &Rc<Service>, bus: &gio::DBusConnection) {
    let weak = Rc::downgrade(service);
    #[allow(deprecated)] // the stream form would need the program to poll it for a callback this small
    // The subscription lasts as long as the connection: its id is not needed
    let _ = bus.signal_subscribe(Some("org.freedesktop.DBus"), Some("org.freedesktop.DBus"), Some("NameOwnerChanged"), Some("/org/freedesktop/DBus"), None, gio::DBusSignalFlags::NONE, move |_, _, _, _, _, params| {
        let (Some(name), Some(new_owner)) = (params.child_value(0).get::<String>(), params.child_value(2).get::<String>()) else { return };
        if new_owner.is_empty() {
            if let Some(service) = weak.upgrade() {
                service.callers.borrow_mut().remove(&name);
                service.clients.borrow_mut().remove(&name);
                service.link_takers.borrow_mut().remove(&name);
                service.presenters.borrow_mut().remove(&name);
            }
        }
    });
}

thread_local! {
    static INTERFACES: OnceCell<(gio::DBusInterfaceInfo, gio::DBusInterfaceInfo)> = const { OnceCell::new() };
}

fn interfaces() -> (gio::DBusInterfaceInfo, gio::DBusInterfaceInfo) {
    INTERFACES.with(|cell| {
        cell.get_or_init(|| {
            let node = gio::DBusNodeInfo::for_xml(XML).expect("the contract is valid XML");
            (node.lookup_interface(ROOT_INTERFACE).expect("the contract has the program"), node.lookup_interface(DEVICE_INTERFACE).expect("the contract has the device"))
        })
        .clone()
    })
}

fn object_path(path: &str) -> ObjectPath {
    ObjectPath::try_from(path.to_string()).expect("paths are made of letters, digits and underscores")
}

// --- Signals -------------------------------------------------------------------------------------------------

/// Sends a signal of the program (`dev` is `None`) or of a device, to the app and the module, each by its own
/// name: a signal sent to the whole bus would be heard by any program that listens, the phone's number and the names
/// of its files included
pub fn emit(service: &Service, dev: Option<&Rc<Device>>, name: &str, args: Option<glib::Variant>) {
    if service.cfg.record_signals {
        service.recorded.borrow_mut().push(Signal { device: dev.map(|d| d.id.clone()), name: name.to_string(), args: args.clone() });
    }
    let (path, interface) = match dev {
        Some(d) => (d.path.as_str(), DEVICE_INTERFACE),
        None => (ROOT_PATH, ROOT_INTERFACE),
    };
    send_to_clients(service, path, interface, name, args.as_ref());
}

fn send_to_clients(service: &Service, path: &str, interface: &str, signal: &str, args: Option<&glib::Variant>) {
    let Some(bus) = service.bus.borrow().clone() else { return };
    let clients: Vec<(String, Role)> = service.clients.borrow().iter().map(|(n, r)| (n.clone(), *r)).collect();
    for (name, role) in clients {
        if !audience(role, signal, args) {
            continue;
        }
        if let Err(e) = bus.emit_signal(Some(&name), path, interface, signal, args) {
            eprintln!("amethystora-connect: cannot send {signal}: {}", e.message());
        }
    }
}

pub fn device_added(service: &Service, dev: &Rc<Device>) {
    emit(service, None, "DeviceAdded", Some((object_path(&dev.path),).to_variant()));
}

pub fn device_removed(service: &Service, dev: &Rc<Device>) {
    emit(service, None, "DeviceRemoved", Some((object_path(&dev.path),).to_variant()));
}

/// The progress or the end of a transfer
pub fn transfer(service: &Service, dev: &Rc<Device>, id: &str, name: &str, incoming: bool, done: u64, total: u64, state: &str) {
    emit(service, Some(dev), "Transfer", Some((id, name, incoming, done as i64, total as i64, state).to_variant()));
}

fn properties_changed(service: &Service, dev: Option<&Rc<Device>>, path: &str, interface: &str, changed: HashMap<String, glib::Variant>) {
    if changed.is_empty() {
        return;
    }
    let invalidated: Vec<String> = Vec::new();
    let args = (interface, changed, invalidated).to_variant();
    if service.cfg.record_signals {
        service.recorded.borrow_mut().push(Signal { device: dev.map(|d| d.id.clone()), name: "PropertiesChanged".to_string(), args: Some(args.clone()) });
    }
    send_to_clients(service, path, "org.freedesktop.DBus.Properties", "PropertiesChanged", Some(&args));
}

/// Says which properties of a device changed since they were last said
pub fn props_changed(service: &Service, dev: &Rc<Device>) {
    let now = dev.properties();
    let mut changed = HashMap::new();
    {
        let mut published = dev.st.borrow_mut();
        for (name, value) in now {
            if published.published.get(name) != Some(&value) {
                published.published.insert(name, value.clone());
                changed.insert(name.to_string(), value);
            }
        }
    }
    properties_changed(service, Some(dev), &dev.path, DEVICE_INTERFACE, changed);
}

fn root_properties(service: &Service) -> Vec<(&'static str, glib::Variant)> {
    vec![
        ("Name", service.name().to_variant()),
        ("ClipboardSend", service.clip_send.get().to_variant()),
        ("ClipboardReceive", service.clip_receive.get().to_variant()),
        ("Computers", service.computers.get().to_variant()),
        ("ReceiveFolder", service.receive_root().to_string_lossy().to_string().to_variant()),
        ("PhotoFolder", service.photo_folder_path().unwrap_or_else(|| service.receive_root()).to_string_lossy().to_string().to_variant()),
    ]
}

pub fn root_props_changed(service: &Service) {
    let mut changed = HashMap::new();
    {
        let mut published = service.root_published.borrow_mut();
        for (name, value) in root_properties(service) {
            if published.get(name) != Some(&value) {
                published.insert(name, value.clone());
                changed.insert(name.to_string(), value);
            }
        }
    }
    properties_changed(service, None, ROOT_PATH, ROOT_INTERFACE, changed);
}

// --- Registering -----------------------------------------------------------------------------------------

/// Puts the program on a connection: its own object, and every device known so far
pub fn export(service: &Rc<Service>, bus: &gio::DBusConnection) -> Result<(), glib::Error> {
    *service.bus.borrow_mut() = Some(bus.clone());
    let (root, _) = interfaces();
    let weak = Rc::downgrade(service);
    let methods = weak.clone();
    let props = weak.clone();
    watch_callers(service, bus);
    bus.register_object(ROOT_PATH, &root)
        .method_call(move |connection, sender, _, _, method, params, invocation| match methods.upgrade() {
            Some(service) => {
                let role = role_of_sender(&service, &connection, sender);
                if !method_allowed(role, method) {
                    return invocation.return_dbus_error(NOT_AVAILABLE, "caller not allowed");
                }
                note_client(&service, sender, role);
                if method == "TakeLinks" {
                    // Only a client of the module's role takes them, and only for its own connection
                    return nothing(invocation, arg::<bool>(&params, 0).map(|on| set_link_taker(&service, sender, role, on)));
                }
                if method == "Presenting" {
                    return nothing(invocation, arg::<bool>(&params, 0).map(|on| set_presenter(&service, sender, role, on)));
                }
                root_method(&service, method, &params, invocation)
            }
            None => invocation.return_dbus_error(NOT_AVAILABLE, "the program is stopping"),
        })
        .property(move |connection, sender, _, _, name| {
            let Some(service) = props.upgrade() else { return false.to_variant() };
            let role = role_of_sender(&service, &connection, sender);
            note_client(&service, sender, role);
            let value = root_properties(&service).into_iter().find(|(n, _)| *n == name).map(|(_, v)| v).unwrap_or_else(|| false.to_variant());
            if role == Role::Other { redact(&value) } else { value }
        })
        .build()?;
    let devices: Vec<Rc<Device>> = service.devices.borrow().values().cloned().collect();
    for dev in devices {
        register_device(service, &dev);
    }
    root_props_changed(service);
    Ok(())
}

pub fn register_device(service: &Rc<Service>, dev: &Rc<Device>) {
    let Some(bus) = service.bus.borrow().clone() else { return };
    let (_, interface) = interfaces();
    let weak = Rc::downgrade(service);
    let methods = weak.clone();
    let device_id = dev.id.clone();
    let props_device = dev.clone();
    let props_service = weak.clone();
    let result = bus
        .register_object(&dev.path, &interface)
        .method_call(move |connection, sender, _, _, method, params, invocation| match methods.upgrade() {
            Some(service) => {
                let role = role_of_sender(&service, &connection, sender);
                if !method_allowed(role, method) {
                    return invocation.return_dbus_error(NOT_AVAILABLE, "caller not allowed");
                }
                note_client(&service, sender, role);
                device_method(&service, &device_id, method, &params, invocation)
            }
            None => invocation.return_dbus_error(NOT_AVAILABLE, "the program is stopping"),
        })
        .property(move |connection, sender, _, _, name| {
            let Some(service) = props_service.upgrade() else { return false.to_variant() };
            let role = role_of_sender(&service, &connection, sender);
            note_client(&service, sender, role);
            let value = props_device.properties().into_iter().find(|(n, _)| *n == name).map(|(_, v)| v).unwrap_or_else(|| false.to_variant());
            if role == Role::Other { redact(&value) } else { value }
        })
        .build();
    match result {
        Ok(id) => dev.st.borrow_mut().registration = Some(id),
        Err(e) => eprintln!("amethystora-connect: cannot put {} on the bus: {}", dev.id, e.message()),
    }
}

pub fn unregister_device(service: &Rc<Service>, dev: &Rc<Device>) {
    let registration = dev.st.borrow_mut().registration.take();
    if let (Some(bus), Some(id)) = (service.bus.borrow().clone(), registration) {
        let _ = bus.unregister_object(id);
    }
}

/// Whether a connection shows the links phones share (TakeLinks): only the module's, and only while it is a client
pub fn set_link_taker(service: &Service, sender: Option<&str>, role: Role, on: bool) {
    let Some(sender) = sender else { return };
    if on && role == Role::Module {
        service.link_takers.borrow_mut().insert(sender.to_string());
    } else {
        service.link_takers.borrow_mut().remove(sender);
    }
}

/// Whether a connection is presenting (Presenting): only the module's, and only while it is a client
pub fn set_presenter(service: &Service, sender: Option<&str>, role: Role, on: bool) {
    let Some(sender) = sender else { return };
    if on && role == Role::Module {
        service.presenters.borrow_mut().insert(sender.to_string());
    } else {
        service.presenters.borrow_mut().remove(sender);
    }
}

/// Whether the shell is presenting now: a connection of the module's that is still a client said so, and has not
/// said otherwise. Only then are the clicker's keys passed on
pub fn presenting(service: &Service) -> bool {
    let clients = service.clients.borrow();
    service.presenters.borrow().iter().any(|name| clients.get(name) == Some(&Role::Module))
}

/// Whether a web link goes to the shell's module now: a connection of the module's that is still a client has taken
/// them, and has not let go
pub fn links_taken(service: &Service) -> bool {
    let clients = service.clients.borrow();
    service.link_takers.borrow().iter().any(|name| clients.get(name) == Some(&Role::Module))
}

// --- Methods ---------------------------------------------------------------------------------------------

fn fail(invocation: gio::DBusMethodInvocation, error: ApiError) {
    match error {
        ApiError::NotAvailable(why) => invocation.return_dbus_error(NOT_AVAILABLE, &why),
        ApiError::InvalidArgs(why) => invocation.return_dbus_error(INVALID_ARGS, &why),
    }
}

fn nothing(invocation: gio::DBusMethodInvocation, result: ApiResult<()>) {
    match result {
        Ok(()) => invocation.return_value(None),
        Err(e) => fail(invocation, e),
    }
}

fn arg<T: glib::variant::FromVariant>(params: &glib::Variant, at: usize) -> ApiResult<T> {
    params.child_value(at).get::<T>().ok_or_else(|| ApiError::InvalidArgs(format!("argument {at}")))
}

fn root_method(service: &Rc<Service>, method: &str, params: &glib::Variant, invocation: gio::DBusMethodInvocation) {
    match method {
        "GetDevices" => {
            let paths: Vec<ObjectPath> = service.devices.borrow().values().map(|d| object_path(&d.path)).collect();
            invocation.return_value(Some(&(paths,).to_variant()));
        }
        "Announce" => {
            service.look_again();
            invocation.return_value(None);
        }
        "ConnectTo" => nothing(invocation, arg::<String>(params, 0).and_then(|a| service.connect_to(&a))),
        "PushClipboard" => nothing(
            invocation,
            arg::<String>(params, 0).map(|text| {
                service.push_clipboard(&text);
            }),
        ),
        _ => invocation.return_dbus_error(INVALID_ARGS, "no such method"),
    }
}

fn device_method(service: &Rc<Service>, id: &str, method: &str, params: &glib::Variant, invocation: gio::DBusMethodInvocation) {
    let result = (|| -> Result<Option<glib::Variant>, ApiError> {
        Ok(match method {
            "Pair" => service.pair(id).map(|_| None)?,
            "AcceptPairing" => service.accept_pairing(id).map(|_| None)?,
            "RejectPairing" => service.reject_pairing(id).map(|_| None)?,
            "Unpair" => service.unpair(id).map(|_| None)?,
            "Ping" => service.ping(id).map(|_| None)?,
            "SendFiles" => {
                let ids = service.send_files(id, &arg::<Vec<String>>(params, 0)?)?;
                Some((ids,).to_variant())
            }
            "SendUrl" => service.send_url(id, &arg::<String>(params, 0)?).map(|_| None)?,
            "SendText" => service.send_text(id, &arg::<String>(params, 0)?).map(|_| None)?,
            "CancelTransfer" => service.cancel_transfer(id, &arg::<String>(params, 0)?).map(|_| None)?,
            "ReplyToNotification" => service.reply_to_notification(id, &arg::<String>(params, 0)?, &arg::<String>(params, 1)?).map(|_| None)?,
            "DismissNotification" => service.dismiss_notification(id, &arg::<String>(params, 0)?).map(|_| None)?,
            "GetRepliable" => {
                let list: Vec<HashMap<String, glib::Variant>> = service.repliable(id)?;
                Some((list,).to_variant())
            }
            "RequestConversations" => service.request_conversations(id).map(|_| None)?,
            "ListConversations" => {
                let list = service.list_conversations(id, arg::<i32>(params, 0)?)?;
                Some((list,).to_variant())
            }
            "RequestMessages" => service.request_messages(id, arg::<i64>(params, 0)?, arg::<i64>(params, 1)?, arg::<i32>(params, 2)?).map(|_| None)?,
            "ListMessages" => {
                let list = service.list_messages(id, arg::<i64>(params, 0)?, arg::<i32>(params, 1)?)?;
                Some((list,).to_variant())
            }
            "SendMessage" => service.send_message(id, &arg::<Vec<String>>(params, 0)?, &arg::<String>(params, 1)?, arg::<i64>(params, 2)?).map(|_| None)?,
            "MuteRinger" => service.mute_ringer(id).map(|_| None)?,
            "Ring" => service.ring(id).map(|_| None)?,
            "MediaAction" => service.media_action(id, &arg::<String>(params, 0)?, &arg::<String>(params, 1)?).map(|_| None)?,
            _ => return Err(ApiError::InvalidArgs("no such method".into())),
        })
    })();
    match result {
        Ok(value) => invocation.return_value(value.as_ref()),
        Err(e) => fail(invocation, e),
    }
}

