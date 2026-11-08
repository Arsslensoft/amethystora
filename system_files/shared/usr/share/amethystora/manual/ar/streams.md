# المسارات

| الوسم | ما تحصل عليه |
| --- | --- |
| `stable` | يُبنى كل أسبوع، مع إبقاء النواة على نواة Fedora CoreOS، التي تتأخر قليلًا عن نواة Fedora كي تُكتشف تراجعات النواة أولًا. للجميع. |
| `stable-daily` | الشيء نفسه، لكنه يُعاد بناؤه كلما تغيّر Amethystora بدلًا من كل أسبوع |
| `latest` | أحدث Fedora وأحدث نواة، ويُعاد بناؤه كلما تغيّر Amethystora |
| `beta` | ما هو قادم. توقّع بعض العثرات. |

## تبديل المسارات والصور

للانتقال إلى مسار أو صورة أخرى، بدّل إليها وأعد التشغيل:

```bash
sudo bootc switch --enforce-container-sigpolicy ghcr.io/arsslensoft/amethystora:latest
```

استبدل `amethystora` بـ`amethystora-dx` من أجل [وضع المطوّر](developer.md)، و`latest` بالمسار الذي تريده.

وإن كانت لديك [حزم مضافة كطبقات](software.md#layering-the-last-resort)، فبدّل بـ`rpm-ostree` بدلًا من ذلك،
لأن `bootc switch` يبني النظام الجديد من الصورة وحدها وسيُسقطها:

```bash
sudo rpm-ostree rebase ostree-image-signed:docker://ghcr.io/arsslensoft/amethystora:latest
```

## التحديثات الموقّعة

لا يُقبل تحديث إلا إذا حمل توقيع Amethystora، الذي يُتحقق منه مقابل المفتاح في
`/etc/pki/containers/amethystora.pub`. والصورة المعبوث بها، أو الآتية من أي جهة أخرى، تُرفض بدلًا من أن
تُثبَّت. وتتحقق خدمة عند كل إقلاع من أن التحقق من التوقيع لا يزال مفعّلًا، وتعيد تفعيله إن عطّله تبديل ما.
