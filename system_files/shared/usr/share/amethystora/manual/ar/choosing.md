# اختيار صورة النظام

## اختر صورة

| الصورة | لمن |
| --- | --- |
| `ghcr.io/arsslensoft/amethystora` | سطح المكتب القياسي |
| `ghcr.io/arsslensoft/amethystora-dx` | للمطورين: تضيف Docker وIncus وlibvirt وVSCodium وغيرها ([وضع المطوّر](developer.md)) |
| `ghcr.io/arsslensoft/amethystora-nvidia-open` | سطح المكتب القياسي مع مشغّل النواة المفتوح من NVIDIA |
| `ghcr.io/arsslensoft/amethystora-dx-nvidia-open` | وضع المطوّر مع مشغّل النواة المفتوح من NVIDIA |

تثبّت ملفات ISO الصور التي لا تتضمن مشغّل NVIDIA. وعلى حاسوب فيه بطاقة NVIDIA، يعرض الإقلاع الأول
الانتقال إلى صورة NVIDIA المطابقة ([العتاد](hardware.md#graphics)).

## اختر مسارًا

تأتي كل صورة في ثلاثة مسارات، يحددها الوسم الذي يلي النقطتين:

| الوسم | لمن |
| --- | --- |
| `stable` | للجميع. يُبنى أسبوعيًا، مع إبقاء النواة على نواة Fedora CoreOS، التي تتأخر قليلًا عن نواة Fedora كي تُكتشف تراجعات النواة أولًا. |
| `latest` | أحدث ما لدى Fedora، ويُعاد بناؤه كلما تغيّر Amethystora. |
| `beta` | لتجربة ما هو قادم. توقّع بعض العثرات. |

تثبّت ملفات ISO المسار `stable`. يمكنك التنقّل بين الصور والمسارات في أي وقت؛ وتبيّن صفحة
[التحديثات](streams.md#switching-streams-and-images) الطريقة.
