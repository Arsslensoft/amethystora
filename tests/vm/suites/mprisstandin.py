#!/usr/bin/python3
"""A stand-in media player for suite 64 (plans/ui-ideas/spec-phone.md 3.1): an MPRIS player on the session bus, written
from the MPRIS specification, that plays one song and writes down every method it is called with.

  mprisstandin.py LOG      owns org.mpris.MediaPlayer2.vmlab until killed; each call is a line of LOG
"""
import sys

from gi.repository import Gio, GLib

XML = """<node>
  <interface name='org.mpris.MediaPlayer2'><property name='Identity' type='s' access='read'/></interface>
  <interface name='org.mpris.MediaPlayer2.Player'>
    <method name='Play'/><method name='Pause'/><method name='PlayPause'/><method name='Stop'/>
    <method name='Next'/><method name='Previous'/>
    <method name='Seek'><arg type='x' direction='in'/></method>
    <property name='PlaybackStatus' type='s' access='read'/>
    <property name='Metadata' type='a{sv}' access='read'/>
    <property name='Position' type='x' access='read'/>
    <property name='CanPlay' type='b' access='read'/><property name='CanPause' type='b' access='read'/>
    <property name='CanGoNext' type='b' access='read'/><property name='CanGoPrevious' type='b' access='read'/>
    <property name='CanSeek' type='b' access='read'/>
  </interface>
</node>"""

LOG = sys.argv[1]
PROPERTIES = {
    "Identity": GLib.Variant("s", "VM lab player"),
    "PlaybackStatus": GLib.Variant("s", "Playing"),
    "Metadata": GLib.Variant("a{sv}", {"xesam:title": GLib.Variant("s", "Lab song"),
                                       "xesam:artist": GLib.Variant("as", ["The Lab"]),
                                       "mpris:length": GLib.Variant("x", 200_000_000)}),
    "Position": GLib.Variant("x", 5_000_000),
}


def called(connection, sender, path, interface, method, params, invocation):
    with open(LOG, "a") as out:
        out.write(f"{method}\n")
    invocation.return_value(None)


def get(connection, sender, path, interface, name):
    return PROPERTIES.get(name, GLib.Variant("b", True))


def acquired(connection, name):
    node = Gio.DBusNodeInfo.new_for_xml(XML)
    for interface in node.interfaces:
        connection.register_object("/org/mpris/MediaPlayer2", interface, called, get, None)


Gio.bus_own_name(Gio.BusType.SESSION, "org.mpris.MediaPlayer2.vmlab", Gio.BusNameOwnerFlags.NONE, acquired, None, None)
GLib.MainLoop().run()
