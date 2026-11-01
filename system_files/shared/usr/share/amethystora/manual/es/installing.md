# Instalación

Descargue Amethystora, grábelo en una memoria USB, arranque el equipo desde la memoria y pruébelo: no
cambia nada en el equipo hasta que decida instalarlo. La instalación tarda unos diez minutos y no
necesita red.

## Descarga

| Descarga | Qué es |
| --- | --- |
| [amethystora.iso](https://download.amethystora.org/amethystora.iso) | Amethystora en una memoria USB: pruébelo y después instálelo desde ella. Empiece por esta. |
| [amethystora-non-uefi.iso](https://download.amethystora.org/amethystora-non-uefi.iso) | Solo el instalador, que también arranca equipos demasiado antiguos para UEFI |

Cada una ocupa varios gigabytes. Junto a cada ISO está su suma de comprobación, `.sha256`, firmada
con la misma clave con la que se comprueba cada actualización. Para comprobar que lo que ha
descargado es lo que se compiló:

```bash
curl -LO https://raw.githubusercontent.com/arsslensoft/amethystora/main/cosign.pub
curl -LO https://download.amethystora.org/amethystora.iso.sha256
curl -LO https://download.amethystora.org/amethystora.iso.sha256.bundle
cosign verify-blob --key cosign.pub --bundle amethystora.iso.sha256.bundle amethystora.iso.sha256
sha256sum -c amethystora.iso.sha256
```

En Windows, `certutil -hashfile amethystora.iso SHA256` muestra la suma de comprobación para
compararla con la de `amethystora.iso.sha256`.

En un equipo con tarjeta gráfica NVIDIA, instale desde cualquiera de las dos: una notificación tras el
primer arranque ofrece la imagen con el controlador de NVIDIA ([Hardware](hardware.md#graphics)).

## Grabarla en una memoria USB

Use una memoria de 16 GB o más: se borrará todo su contenido.

- **Desde Windows o macOS:** Fedora Media Writer (elija **Seleccionar archivo .iso**) o balenaEtcher.
  En Windows también sirve Rufus, en modo de imagen DD.
- **Desde Linux:** Impression, o `dd`:

  ```bash
  sudo dd if=amethystora.iso of=/dev/sdX bs=4M status=progress oflag=sync
  ```

  donde `/dev/sdX` es la memoria, tal como la muestra `lsblk`. Si se equivoca, `dd` borrará otro disco.

Una memoria preparada con Ventoy solo arranca la ISO en el modo GRUB2 de Ventoy.

## Probarlo

Conecte la memoria y arranque el equipo desde ella: mientras arranca, pulse su tecla de menú de
arranque (F12, F11, F10, F8 o Esc en la mayoría de los PC; mantenga pulsada Opción en un Mac con
Intel) y elija la memoria. Secure Boot puede seguir activado. Elija **Try or install Amethystora**, o
la entrada en **basic graphics mode** (modo de gráficos básicos) si la pantalla se queda en negro.

Amethystora arranca tal como lo hará una vez instalado, con la sesión ya iniciada. La wifi, el sonido,
la pantalla y el resto del hardware muestran si este equipo le va bien. No se escribe nada en el disco
del equipo, y todo lo que haga desaparece al apagarlo. Las aplicaciones de Flathub no están en la
memoria: se instalan en el primer arranque del sistema instalado.

La ISO en vivo solo arranca equipos con firmware UEFI, que tienen casi todos los equipos desde 2012.
Para uno más antiguo, use solo el instalador.

## Instalar

Elija **Install to Hard Drive** (Instalar en el disco duro) en el tablero. El instalador le pregunta
el idioma y el disco, y después se pone manos a la obra.

- **Elija el cifrado del disco.** No se puede activar después, y es lo que mantiene sus archivos a
  salvo si pierde el equipo o se lo roban. La frase de contraseña se pide en cada arranque, hasta que
  [el TPM lo desbloquee](security.md#unlock-the-disk-with-the-tpm), si así lo quiere.
- **Para conservar Windows,** lea antes [Junto a Windows](#next-to-windows).

Cuando termine, reinicie y retire la memoria. El primer arranque le pregunta el idioma, el teclado, la
cuenta y la contraseña, y las aplicaciones se instalan solas en cuanto el equipo tiene conexión.
Después siga los [Primeros pasos](getting-started.md).

## Junto a Windows

Amethystora se instala junto a Windows, y un menú en cada arranque le permite elegir uno de los dos.
En Windows, antes de instalar:

1. **Guarde su clave de recuperación de BitLocker.** La mayoría de los portátiles con Windows 11
   cifran su disco con BitLocker, también llamado cifrado de dispositivo. Encontrará la clave en
   **Configuración › Privacidad y seguridad › Cifrado de dispositivo**, o en
   [aka.ms/myrecoverykey](https://aka.ms/myrecoverykey); guárdela en un lugar que no sea este equipo:
   la instalación añade una entrada al menú de arranque del equipo, y Windows puede pedir la clave una
   vez después.
2. **Haga sitio.** Pulse con el botón derecho en **Inicio**, abra **Administración de discos**, pulse
   con el botón derecho en la partición de Windows (C:) y elija **Reducir volumen**. Deje al menos
   64 GB libres para Amethystora, más si va a guardar allí sus archivos.
3. **Desactive el inicio rápido:** **Panel de control › Opciones de energía › Elegir el comportamiento
   de los botones de inicio/apagado**, y desmarque **Activar inicio rápido**. Si está activado, Windows
   solo se apaga a medias y mantiene su disco bloqueado.

Después, en el instalador, instale en el espacio libre que ha creado, nunca en todo el disco. Mantenga
Secure Boot activado: Windows 11 lo necesita, y Amethystora arranca con él. Si el reloj de Windows se
desfasa unas horas después de usar Amethystora, active **Establecer la hora automáticamente** en la
configuración de fecha y hora de Windows.

## Desde otro escritorio Fedora Atomic

Un equipo que ya usa Fedora Silverblue, u otro escritorio creado como imagen sobre Fedora, puede
cambiar a Amethystora sin reinstalar. El cambio sustituye la imagen del sistema operativo y conserva
todo lo que hay en `/home`, `/etc` y `/var`.

- **Haga una copia de seguridad** de todo lo que no pueda perder. El cambio es seguro, pero una copia
  de seguridad siempre lo es.
- Los paquetes que superpuso al sistema anterior con `rpm-ostree install` no se conservan. Los
  Flatpak, Homebrew y los contenedores, sí.

```bash
sudo bootc switch ghcr.io/arsslensoft/amethystora:stable
```

Después reinicie. En el primer arranque, un servicio prepara por sí solo el paso a las actualizaciones
con firma comprobada, y a partir del reinicio siguiente solo se acepta una actualización si lleva la
firma de Amethystora.

## Elegir una imagen

| Imagen | Para |
| --- | --- |
| `ghcr.io/arsslensoft/amethystora` | El escritorio estándar |
| `ghcr.io/arsslensoft/amethystora-dx` | Desarrolladores: añade Docker, Incus, libvirt, VSCodium y más ([Modo desarrollador](developer.md)) |
| `ghcr.io/arsslensoft/amethystora-nvidia-open` | El escritorio estándar con el controlador de núcleo abierto de NVIDIA |
| `ghcr.io/arsslensoft/amethystora-dx-nvidia-open` | El modo desarrollador con el controlador de núcleo abierto de NVIDIA |

Las ISO instalan las imágenes sin el controlador de NVIDIA. En un equipo con tarjeta NVIDIA, el primer
arranque ofrece cambiar a la imagen NVIDIA correspondiente ([Hardware](hardware.md#graphics)).

## Elegir un canal

Cada imagen se ofrece en tres canales, que se indican con la etiqueta que va tras los dos puntos:

| Etiqueta | Para |
| --- | --- |
| `stable` | Todo el mundo. Se compila cada semana, con el núcleo retenido en el de Fedora CoreOS, que va un poco por detrás del de Fedora para que las regresiones del núcleo se detecten antes. |
| `latest` | Lo más reciente que tiene Fedora, recompilado cada vez que Amethystora cambia. |
| `beta` | Para probar lo que viene. Espere algunas asperezas. |

Las ISO instalan `stable`. Puede cambiar de imagen y de canal en cualquier momento;
[Actualizaciones](updates.md#switching-streams-and-images) explica cómo.

## Después del primer arranque

Siga los [Primeros pasos](getting-started.md). El único paso que no debe saltarse es registrar la
clave de Secure Boot, si Secure Boot está activado.
