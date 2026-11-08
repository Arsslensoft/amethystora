# Elegir una imagen

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
[Actualizaciones](streams.md#switching-streams-and-images) explica cómo.
