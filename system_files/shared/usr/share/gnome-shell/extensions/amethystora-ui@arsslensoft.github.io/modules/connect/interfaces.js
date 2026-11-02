// The part of the phone link's D-Bus interface that the shell uses. The whole contract is
// apps/connect/dbus/org.amethystora.Connect.xml, which the program and the Phone app are written against; the module
// has no copy of the rest because it neither calls nor listens to it. Anything in the contract may be added to, so the
// proxies here are made from this subset and ignore what they do not name.
//
// The two clipboard switches of the contract are not here on purpose: the module reads them from GSettings, where
// the person's own commands write them, and never takes them from whoever answers on the bus.

import Gio from 'gi://Gio';

export const BUS_NAME = 'org.amethystora.Connect';
// The program's own object. The devices are at /org/amethystora/Connect/Device/ID, which GetDevices and DeviceAdded
// give whole, so only this one path is known here.
export const ROOT_PATH = '/org/amethystora/Connect';

const ROOT = `
<node>
  <interface name="org.amethystora.Connect">
    <method name="GetDevices">
      <arg name="devices" type="ao" direction="out"/>
    </method>
    <method name="PushClipboard">
      <arg name="text" type="s" direction="in"/>
    </method>
    <method name="TakeLinks">
      <arg name="take" type="b" direction="in"/>
    </method>
    <method name="Presenting">
      <arg name="presenting" type="b" direction="in"/>
    </method>
    <signal name="DeviceAdded">
      <arg name="device" type="o"/>
    </signal>
    <signal name="DeviceRemoved">
      <arg name="device" type="o"/>
    </signal>
    <signal name="ClipboardReceived">
      <arg name="device_id" type="s"/>
      <arg name="text" type="s"/>
    </signal>
    <signal name="UrlReceived">
      <arg name="device_id" type="s"/>
      <arg name="url" type="s"/>
    </signal>
    <signal name="Clicker">
      <arg name="device_id" type="s"/>
      <arg name="action" type="s"/>
    </signal>
    <signal name="Code">
      <arg name="code" type="s"/>
    </signal>
  </interface>
</node>`;

const DEVICE = `
<node>
  <interface name="org.amethystora.Connect.Device">
    <property name="Id" type="s" access="read"/>
    <property name="Name" type="s" access="read"/>
    <property name="Type" type="s" access="read"/>
    <property name="Paired" type="b" access="read"/>
    <property name="Connected" type="b" access="read"/>
    <property name="BatteryLevel" type="i" access="read"/>
    <property name="BatteryCharging" type="b" access="read"/>
    <property name="SignalStrength" type="i" access="read"/>
    <property name="NetworkType" type="s" access="read"/>
    <property name="Features" type="as" access="read"/>
    <property name="Players" type="aa{sv}" access="read"/>
    <method name="Ping"/>
    <method name="Ring"/>
    <method name="MediaAction">
      <arg name="player" type="s" direction="in"/>
      <arg name="action" type="s" direction="in"/>
    </method>
    <method name="SendUrl">
      <arg name="url" type="s" direction="in"/>
    </method>
    <method name="MuteRinger"/>
    <signal name="Call">
      <arg name="state" type="s"/>
      <arg name="number" type="s"/>
      <arg name="name" type="s"/>
    </signal>
  </interface>
</node>`;

const RootProxy = Gio.DBusProxy.makeProxyWrapper(ROOT);
const DeviceProxy = Gio.DBusProxy.makeProxyWrapper(DEVICE);

// A proxy for the program's object or one of its devices, once it has read the properties. It talks to the connection
// that was checked, by its unique name, and never to whoever holds the well-known name later. It never starts the
// program: only the person's switch does.
function makeProxy(Wrapper, connection, owner, path, cancellable) {
    return new Promise((resolve, reject) => {
        new Wrapper(connection, owner, path, (proxy, error) => {
            if (error)
                reject(error);
            else
                resolve(proxy);
        }, cancellable, Gio.DBusProxyFlags.DO_NOT_AUTO_START);
    });
}

export function rootProxy(connection, owner, cancellable) {
    return makeProxy(RootProxy, connection, owner, ROOT_PATH, cancellable);
}

export function deviceProxy(connection, owner, path, cancellable) {
    return makeProxy(DeviceProxy, connection, owner, path, cancellable);
}
