# Modo desarrollador

`amethystora-dx` es el mismo escritorio con un taller de desarrollo encima: motores de contenedores,
máquinas virtuales, VSCodium y las herramientas de perfilado para averiguar por qué algo va lento.

La idea de fondo: su entorno de desarrollo no debería ser su sistema operativo. Las herramientas de
compilación, los entornos de ejecución de los lenguajes y las bases de datos viven en contenedores
descritos en el propio repositorio del proyecto, así que el proyecto se compila igual en su equipo, en
el Mac de un compañero y en la CI, y el sistema de debajo se mantiene lo bastante limpio para
actualizarlo sin pensárselo dos veces.

## Activarlo

Cambie a la imagen dx y reinicie:

```bash
sudo bootc switch --enforce-container-sigpolicy ghcr.io/arsslensoft/amethystora-dx:stable
```

Si tiene [paquetes superpuestos](software.md#layering-the-last-resort), use en su lugar
`sudo rpm-ostree rebase ostree-image-signed:docker://ghcr.io/arsslensoft/amethystora-dx:stable`, que los
conserva. Volver a `amethystora` funciona igual.

En su primer arranque, la imagen dx añade todas las cuentas de administrador a los grupos `docker`,
`incus-admin`, `libvirt` y `wireshark`. Si `docker ps` dice que se deniega el permiso, cierre la sesión
y vuelva a iniciarla una vez.

## Qué añade

| Herramienta | Para |
| --- | --- |
| VSCodium | El editor: VS Code compilado a partir de su código abierto, sin la telemetría ni la licencia de Microsoft. Las extensiones vienen de [Open VSX](https://open-vsx.org); Open Remote SSH y Container Tools vienen instaladas. Sus colores siguen el [tema](themes.md). |
| Docker Engine | Con buildx y compose. El predeterminado para los contenedores de desarrollo. |
| Podman | Con `podman-compose` y `podman machine`. Siempre disponible, sin root, y con el socket de Podman activado. |
| Incus | Contenedores de sistema y máquinas virtuales, gestionados como instancias en la nube |
| libvirt y virt-manager | Máquinas virtuales con QEMU y KVM |
| Sysprof, perf, bcc, bpftrace, bpftop, bpftool, trace-cmd | Perfilado y trazas, desde todo el sistema hasta una sola función |
| gdb, strace, ltrace, Valgrind | Depuración: recorrer un programa paso a paso, ver las llamadas que hace al núcleo y a sus bibliotecas, encontrar errores de memoria. Para conectar gdb a un programa que no inició él hace falta `sudo` ([por qué](security.md#on-from-the-start)). |
| Wireshark | Capturar y leer el tráfico de red, sin `sudo` |
| android-tools | `adb` y `fastboot` |
| ROCm | Cálculo en GPU con gráficos AMD |
| flatpak-builder | Compilar Flatpak |

## Contenedores de desarrollo

Un contenedor de desarrollo es un `.devcontainer/devcontainer.json` en el proyecto: la imagen, las
herramientas y las extensiones del editor que necesita el proyecto. Todo lo que instala el proyecto se
queda ahí dentro.

La extensión Dev Containers de Microsoft solo funciona en el propio VS Code de Microsoft, así que en
Amethystora los contenedores de desarrollo se ejecutan con la línea de comandos `devcontainer`, la
implementación de referencia de [la especificación](https://containers.dev), que también siguen los
IDE de JetBrains:

```bash
brew install devcontainer
devcontainer up --workspace-folder .
devcontainer exec --workspace-folder . bash
```

Para usar Podman en lugar de Docker, añada `--docker-path podman --docker-compose-path podman-compose`
a `devcontainer up`.

Si un contenedor no puede leer una carpeta montada por culpa de SELinux, vuelva a etiquetar la carpeta
en lugar de desactivar SELinux: `restorecon -R -v ~/code/myproject`.

## Otros editores

- **JetBrains:** `ame apps jetbrains-toolbox` instala JetBrains Toolbox en su carpeta personal, y este
  instala y actualiza los IDE. No se recomiendan los Flatpak de los IDE.
- **Neovim, Helix y compañía:** `brew install neovim`, y `brew install devcontainer` para la línea de
  comandos de los contenedores de desarrollo.

## Kubernetes y la nube

Instale las herramientas de línea de comandos con Homebrew, para que se mantengan al día sin tocar la
imagen:

```bash
brew install kubectl helm k9s kind
```

`kind` ejecuta todo un clúster de Kubernetes en contenedores Docker, que es la forma más rápida de
probar algo con un clúster real.
