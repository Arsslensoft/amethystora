# Wi-Fi, Bluetooth and phones

Wi-Fi and Bluetooth are in the menu at the top right. An Android phone pairs through the phone link, an iPhone through
its cable.

## Phones

- **Android:** the phone link works with the KDE Connect app on the phone: its notifications, files and links sent
  either way, texts and calls, and a clipboard you can share ([Phone](phone.md)). It is off until you turn it on, with
  `ame desktop phone on` or the **Phone pairing** switch on [Control](control.md)'s **Desktop** page: the program
  listens only while it is on, and `ame desktop phone off` stops it.
- **iPhone:** plug it in and trust the computer on the phone; its photos and files show up in Files.

### Pairing an Android phone

Install KDE Connect on the phone and pair them from the **Phone** app, on a network you have named home: elsewhere the
firewall keeps the phone out ([Security](security.md#on-from-the-start)). Name it with `ame security home-networks`. A
phone paired with GSConnect, which the phone link replaced, has to be paired again, which a notification says once.

## Turn off Bluetooth

The menu at the top right has a Bluetooth button. [Show me](guide:turn-off-bluetooth)

## Wi-Fi

To join a network, open the menu at the top right and press the arrow beside Wi-Fi. [Show me](guide:join-wifi)

![Join a network](media/join-wifi.webm)

Nearly every Wi-Fi card works without anything to install.

### Broadcom cards without a driver

A few of Broadcom's (the BCM4360, 4352 and 43142, in older MacBooks and some laptops) have no driver on Amethystora. The
only one there is, Broadcom's own `wl`, has security holes anyone in Wi-Fi range can reach (CVE-2019-9501 and
CVE-2019-9502), its licence forbids anyone but Broadcom to fix them, and Broadcom stopped updating it in 2015. A USB
Wi-Fi adapter works instead.
