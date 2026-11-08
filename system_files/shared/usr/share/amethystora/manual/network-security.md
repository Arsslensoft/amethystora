# On the network

What your browsers may install and send, who sees the names you look up, how networks tell this machine apart, and the
inspection of its connections. Each section says whether its part is on or off as the image ships it.

## Browser protection

An extension sees every page you open, and that is how most password stealers arrive. With browser protection on,
browsers install only the extensions on a list, and web pages are kept from USB, serial, HID and Bluetooth devices. It is
off by default: Firefox comes exactly as Fedora builds it, and what it may install is your decision, not the image's.

```bash
ame security browser                 # the menu
ame security browser on|off
ame security browser allow <id>      # add an extension to the list
ame security browser disallow <id>   # take it back
```

### What the list holds

The Bitwarden, 1Password, Proton Pass and KeePassXC password managers (the Vault works with Bitwarden's, see
[In your browser](vault-secrets.md#in-your-browser)), and the ones you add. What you allowed is in
`/etc/amethystora/browser-extensions`, one ID a line.

Allow the extensions you use before you turn it on. Firefox removes the ones that are not on the list, with their
settings, the next time it starts; the other browsers turn theirs off. Firefox shows each extension's ID in
`about:support`, under Add-ons, and the others on their extensions page with developer mode on. Themes, dictionaries and
language packs in Firefox are left alone. Turned off, browsers install any extension again, but the ones Firefox removed
do not come back by themselves.

### Which browsers

Firefox, Brave, Chrome, Chromium, ungoogled-chromium and Edge, as packages or Flatpaks, whether installed now or later,
for every account on the machine. Other browsers are not covered, nor is Firefox while it has a policy file of somebody
else's, which is left alone.

### Site checking

`ame security browser osprey` allows Osprey: Browser Protection, which checks each site you open against more than twenty
threat-intelligence providers, and warns you or blocks the site.

- Amethystora does not make it, and never installs it: you do, from your browser's own store, which the command opens,
  and it can be removed there like any other extension.
- It sends the address of every site you open to Osprey Project LLC, in California, which passes it on to its providers
  without the query string and without your address. That is why it is off until you choose it, and why no profile
  turns it on.
- Which providers it asks is set in Osprey's own settings, and Amethystora sets none of them.
- `ame security browser osprey off` takes it off the list again: with browser protection on, browsers then turn it off,
  and Firefox removes it; with protection off, remove it yourself.

### Data collection off

`ame security browser privacy` is a part of its own, independent of the extension list: either can be on without the
other. It is off by default for the same reason as protection, and no security profile turns it on.
`ame security browser privacy off` lets the browsers send data again, from their next start.

- **What they send without it.** Firefox, as Fedora builds it, sends Mozilla usage data (Telemetry and its usage ping),
  takes part in studies, and shows sponsored shortcuts and stories on its new tab page; where Firefox Suggest exists, it
  can send what you type in the address bar. Chrome, Chromium, Brave and Edge send their makers usage and crash
  reports, URLs, survey answers and personalisation data; Brave adds its own usage statistics.
- **With it on,** each of them is told not to, through the same policy files. It installs nothing, sets no home page and
  changes nothing about what a site can do, and what you search for still goes to the search engine you chose.
- Firefox then says it is managed by your organization, and greys out its Telemetry and studies settings; the sponsored
  shortcuts, stories and suggestions are off but not locked, so you can turn them back on in Firefox's settings.
- Edge sends only the data it needs to work (`DiagnosticData` is set to required data, not off), and whether Edge on
  Linux honours its two policies is Microsoft's to say: it documents them for Windows and macOS.

## Encrypted DNS

Every site you open and every server an app talks to is first a name looked up with a DNS server. By default that is
whichever one the network hands out, asked in plain text: the network sees every name, and can answer with any address
it likes. With encrypted DNS on, every lookup goes to one resolver you choose, over DNS-over-TLS, and to nobody else.

```bash
ame security dns                                     # the menu: it says who runs each resolver
ame security dns quad9                               # pick one straight away
ame security dns custom 192.0.2.1#dns.example.net    # servers of your own
ame security dns pause|resume|off
```

### Choosing a resolver

Quad9, Mullvad, Cloudflare, Google, or servers of your own: for those, an address, not a name, then the name its
certificate is for; several can be given, separated by spaces. Quad9 also refuses to look up sites known for malware,
and Google keeps the address each lookup came from for a day or two. The resolver sees every name in the network's
place, so choose one you trust.

### What still goes to the network

- Names on the network itself, such as a printer's, still go to the network's own servers, and so does the check of
  whether a network reaches the internet.
- Which names count as the network's own is the network's to say (its search domains): one that claims a whole domain,
  such as `com`, has every name in it asked of its own servers, in plain text. `resolvectl domain` shows what each
  network claimed.
- A VPN's own names keep working, and Tailscale's.
- Firefox asks the same resolver, since its own DNS-over-HTTPS is off as Fedora builds it. Turned on in Firefox's
  settings (**Privacy & Security**, **DNS over HTTPS**), it sends Firefox's lookups to the provider chosen there
  instead, encrypted too.

### Login pages

A network that wants you to log in first, in a hotel or on a train, cannot show its login page while this is on. A
notification offers to pause it: for ten minutes, the network's own servers answer, in plain text.
`ame security dns pause` does the same, `ame security dns resume` ends the pause early, and `ame security dns off` turns
encrypted DNS off.

## A different address on each network

A wired network knows a computer by its hardware (MAC) address, which is the same on every network, and every network
is told the machine's name, and over IPv6 an identity made from it, so a café, an office and an airport can each tell it
is the same machine. Wi-Fi networks already see an address of their own, one for each network name, without this.

```bash
ame security mac        # the menu
ame security mac on     # every network
ame security mac away   # only away from home
ame security mac off    # off everywhere
```

### What each network sees

With this on, each Wi-Fi and wired network sees an address and an identity of its own for this machine, the same each
time it comes back, so your router keeps what it reserved for it and a login page remembers it, and no network is told
the machine's name. It applies from the next time each network connects.

- A wired network cannot be told apart before it connects, so every wired network without a saved connection of its own
  sees the same made-up address: not the hardware's, but the same in an office and in a hotel.
- Temporary IPv6 addresses, which change over time, are on already (`ame security ipv6-privacy`).

### Only away from home

`away` leaves out the networks you named home with `ame security home-networks`: they keep the machine's own hardware
address and are told its name, so that your router's reservation and Wake-on-LAN keep working, and every other network,
a new one included, sees none of it.

- A wired connection named home is the one used on every wired network.
- Away from home, the machine also stops asking networks about single-word names over LLMNR.
- Naming a network home, or taking it off the list, changes the connection's own settings, and only the ones this
  command set: an address you set to Permanent by hand stays.

### Networks that register devices

A network that lets in only devices it has registered, as some universities and offices do, then refuses the machine.
For that one network, open it in **Settings**, then **Identity**, and set **Cloned Address** to **Permanent**, or name it
home with `away`.

Avahi, which lets printers and other devices find this machine by name, still announces the machine's name on every
network, as it can be told about interfaces and not networks. `ame security network-daemons` turns it off
([Stricter settings](security-switches.md#stricter-settings)).

## Network protection

Suricata inspects every connection this machine makes and receives against the Emerging Threats Open rules, which it
downloads every day, as `amethystora-ips.service`. Its setting, `NETWORK`, is `off` by default: blocking anything is a
decision about your own traffic, and yours to make.

```bash
ame security settings network
```

- **`watch`** says what it recognises and blocks nothing. Start here: a week of it shows what blocking would get in the
  way of.
- **`block`** also cuts off connections to known malware and its command servers, exploits, phishing pages and
  cryptominers, and to addresses on the Spamhaus and DShield block lists. What the rules recognise with less certainty
  is only reported.

### What it blocked

Each blocked connection is a notification. **Network** in the **Security** app lists what was blocked and noticed, and
so does `ame security connections`. If a connection was yours and wanted, **Allow** (which asks for an administrator's
password) or `ame security connections allow <rule>` leaves that one rule out on this machine, and
`ame security connections block <rule>` puts it back.

### What it never does

- Whatever goes wrong with it, your connections keep working: while Suricata starts, restarts, falls behind or stops,
  they pass uninspected rather than not at all.
- An encrypted connection is inspected up to its handshake, the server name and certificate, because what follows
  cannot be read.
- Its log keeps what it recognised and nothing else, never a list of the sites you visit.
- It inspects this machine's own connections, not what it forwards for virtual machines or containers run as root, and
  takes a few hundred megabytes of memory.

### More rules

Two more sets of rules can join it, each off until you turn it on:

- **Faster command-server lists** (`ame security settings network-feeds daily` or `hourly`): abuse.ch's lists of botnet
  servers, of the certificates command servers use, and of addresses that served malware, which know a new campaign
  within hours, where the rules above take days. Under `block`, the first two block; the malware addresses only report,
  since one is often a shared host. They are downloaded on this machine, not on a connection marked as metered, and
  take more memory: the malware addresses are some forty thousand rules.
- **Hunting rules** (`ame security settings network-hunting record`): Emerging Threats' rules for what is suspicious but
  uncertain, which is where new attacks first show. They are recorded, for the
  [chains of steps](new-threats.md), and never said or blocked.

### Odd names in DNS

`ame security settings dns-names` (`record` or `on`, and off by default) needs none of the above. It follows what the
system's resolver is asked, encrypted DNS or not, and keeps only a name that looks made by a program, as malware makes
the names of its command servers, or one long enough to carry data, as DNS tunnelling does.

- Every name passes through it, and only those are kept, for up to 15 minutes, where only root can read them: it keeps
  no history of the sites you visit.
- A browser that sends its own encrypted DNS goes past it; Firefox does not unless you turn that on in its settings.
- CDNs, which name their servers at random, are allowed already.
