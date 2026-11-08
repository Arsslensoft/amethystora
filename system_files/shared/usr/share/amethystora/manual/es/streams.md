# Canales

| Etiqueta | Qué obtiene |
| --- | --- |
| `stable` | Se compila cada semana, con el núcleo retenido en el de Fedora CoreOS, que va un poco por detrás del de Fedora para que las regresiones del núcleo se detecten antes. Para todo el mundo. |
| `stable-daily` | Lo mismo, recompilado cada vez que Amethystora cambia en lugar de cada semana |
| `latest` | Lo más reciente de Fedora y del núcleo, recompilado cada vez que Amethystora cambia |
| `beta` | Lo que viene. Espere algunas asperezas. |

## Cambiar de canal y de imagen

Para pasar a otro canal o a otra imagen, haga el cambio y reinicie:

```bash
sudo bootc switch --enforce-container-sigpolicy ghcr.io/arsslensoft/amethystora:latest
```

Sustituya `amethystora` por `amethystora-dx` para el [modo desarrollador](developer.md), y `latest`
por el canal que quiera.

Si tiene [paquetes superpuestos](software.md#layering-the-last-resort), cambie con `rpm-ostree` en su
lugar, porque `bootc switch` construye el sistema nuevo solo a partir de la imagen y los descartaría:

```bash
sudo rpm-ostree rebase ostree-image-signed:docker://ghcr.io/arsslensoft/amethystora:latest
```

## Actualizaciones firmadas

Una actualización solo se acepta si lleva la firma de Amethystora, comprobada con la clave de
`/etc/pki/containers/amethystora.pub`. Una imagen manipulada, o una de cualquier otro, se rechaza en
lugar de instalarse. Un servicio comprueba en cada arranque que la comprobación de firmas sigue
activada, y la vuelve a activar si un cambio de imagen la desactivó.
