# Certificates and signing

A certificate says who signed something. **Certificates** makes them for signing PDFs and programs, for mail, for logging in to
a service of your own, for naming this computer, and for local websites.

## Who vouches

Every computer makes its own **Amethystora OS Root CA** the first time anything needs it, kept where only the service can reach
it. It never leaves the computer and is in no backup. Under it, each account gets its own **Code Signing CA** with the account's
name in it, and only that signs your certificates.

```text
Amethystora OS Root CA          one for each computer, never leaves it
  +- <you> Code Signing CA      one for each account on it
       +- a PDF certificate     signs documents
       +- a code certificate    signs programs
```

The subject says `OU=Amethystora Vault`, and the root names its computer by a number from its key (`Machine 3FA107C2`), never
the computer's name, since signed files travel. Your CA comes with your first certificate, and only at the screen: a session
over SSH gets none, and neither does a guest account.

> **Only computers that trust this machine's root, or your CA, accept these. Other computers call the signer unknown until
> then.** The Vault says so beside every certificate and before every PDF it signs.

### Not a public certificate

No public authority issues a signing certificate for a key that lives in software, and Let's Encrypt issues none at all. A
certificate from here proves that this computer's Vault signed, to whoever chose to trust it. For files the whole world should
accept, use a signing service, with its key on a card or in its own hardware.

## Making a certificate

**Make a certificate** asks which kind, and shows how the subject will read before it is made.

| Kind | For | Lasts |
| --- | --- | --- |
| **PDF** | Signing documents | 460 days |
| **Code** | Signing programs (Windows programs, Java archives) | 460 days |
| **Mail** (S/MIME) | A mail program to sign and encrypt with. It needs your address | 460 days |
| **Local TLS** | A website or a device on your own network | 397 days |
| **Logging in to a service** | A client certificate for a VPN, Wi-Fi or a server of your own | 397 days |
| **This computer's identity** | Naming this computer. Needs an administrator | 397 days |

- A certificate never outlasts its CA: yours lasts five years, the root twenty.
- **A signer's name that looks like a host name** (`j.smith`) is refused, since other programs would refuse it. `J. Smith` is fine.
- A signing key is RSA 3072 by default, or RSA 4096, ECDSA P-256 or P-384, made by the service and signing inside it.
- **Renew** makes a new certificate under the same name and marks the old one, which stays until you remove it. The window warns
  from 30 days before a certificate ends, whenever the vault is open.

## Signing a PDF

`ame vault pdf sign FILE` (or **Sign a PDF**) signs a copy, `FILE (signed).pdf` beside it, never over anything. It asks once for
each PDF, and the key never leaves the vault: the file's fingerprint goes in, a signature comes out. In Files, select a PDF and
pick **Scripts › Sign a PDF** ([Files](files.md#sign-a-pdf)); a card says first what a reader makes of it.

### What a signed PDF tells

Whoever opens it sees **the signer's name, your name in the CA, and a number that identifies this computer.** That lets them
check it, and lets two documents be seen to come from one computer.

There is no timestamp, since that would send a fingerprint of the document to someone else's server: **a reader may call the
signature expired once the certificate is**, 460 days on.

## Signing programs

**Export for signing (PFX)** gives a PKCS#12 file, locked with a password you type, for `osslsigncode` (in this image),
`signtool` and `jarsigner`. The key then lives outside the vault, and the window says so first. In a terminal:
`ame vault cert export NAME`. Signing inside the vault is for PDFs and SSH only.

## Mail, your own services and websites

- **Mail.** The address is typed and not checked. It is exported as a PFX for your mail program, which keeps its own copy; no mail
  program asks the Vault.
- **Logging in to a service** and **This computer's identity** are issued by the computer's *Device Identity CA*, for a VPN, a Wi-Fi
  network or a server of your own. They are exported as PFX or PEM with a locked key, asking every time, since NetworkManager and
  the like read files. Tell the service to trust that CA (`ame vault cert ca device`). They prove which computer, not who you are.

### Local websites

**Local TLS** covers local names (`.local`, `.home.arpa`, `.internal`, `localhost`) and private addresses. The root and the
*Local Services CA* under it can speak for nothing else: a public website's name is refused, and a stolen key could not stand in
for one.

- `ame vault cert export NAME pem` gives a server its key and chain, asking every time.
- A browser trusts the *Local Services CA* by your choice, never the root: `ame vault tls trust browsers on` adds it to the
  browsers that read your own certificate folder. The window explains Firefox and Flatpak browsers, which keep their own.
- `curl` and the like take `--cacert`.

## Trusting a certificate

Whoever should accept your signatures has to trust this computer's root, or your CA. **Trusting the root means trusting
everything this computer ever issues, to any account, for every purpose**, and a stolen root key could sign in any name an
administrator here could. Give people your CA where their program can trust just that, and the root only where it cannot.

`ame vault root > root.pem` writes the root, showing its number and fingerprint; `ame vault cert ca > ca.pem` writes your CA.
Compare the fingerprint with the person you give it to, by another route.

### On Linux, Windows and Java

- **Linux**, for a signed PDF: `certutil -d sql:$HOME/.pki/nssdb -A -n "Amethystora" -t ",C," -i ca.pem`, then
  `pdfsig -nssdir sql:$HOME/.pki/nssdb signed.pdf`. Make the database first with
  `certutil -d sql:$HOME/.pki/nssdb -N --empty-password` if it is not there. Always give `pdfsig` that folder: once Firefox has
  started it looks in Firefox's profile instead. Papers (a Flatpak with an older NSS) may still say "untrusted issuer" where
  `pdfsig` accepts: believe `pdfsig`.
- **Windows**: import into the account's *Trusted Root Certification Authorities* store (`certutil -user -addstore Root root.pem`),
  which Windows's programs and `signtool` use. Adobe's readers keep their own list or can use Windows's: not tried here.
- **Java**: `keytool -importcert -alias amethystora -file ca.pem -keystore truststore.jks`, then
  `jarsigner -verify -certs -keystore truststore.jks app.jar`. A Windows program checked with `osslsigncode` needs the root, not the CA.
- **Never put the root in the system's trust store.** It would become a trust anchor for every program that checks websites. The
  Vault never does.

### If the root's key is stolen

`ame vault root rotate` (asking an administrator) makes a new root, and every account gets a new CA at its next certificate.
Nothing revokes the old one, since nobody runs a server for it: **everyone who trusted the old root has to stop by hand.** An
administrator of this computer can, as the service's user, issue in any name; the root vouches for this computer and no more.

The same command is the way on when the Vault says the root **can no longer be opened**: its key is sealed to this computer's chip
for the Vault's service, and a new chip, or a service that is no longer the one that sealed it, cannot open it again.
