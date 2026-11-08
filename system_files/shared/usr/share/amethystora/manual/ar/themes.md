# السمات

السمة لوحة ألوان واحدة، يُلوَّن منها كل شيء. بدّل السمة فيتغيّر سطح المكتب كله معًا: النمط الفاتح أو
الداكن في GNOME ولون التمييز، والطرفية، ومحث الصدفة، والشريط العلوي، والمرسى، والخلفية، وVSCodium،
وهذا الدليل. وتمنح السمة الفاتحة المرسى مسحة داكنة خفيفة، كي تظل أيقوناته بارزة على خلفية شاحبة.

## التبديل

| المفتاح | ما يفعله |
| --- | --- |
| `Super+Ctrl+Shift+Space` | اختر سمة |
| `Super+Ctrl+D` | بدّل بين النسختين الفاتحة والداكنة من السمة |
| `Super+Ctrl+Space` | الخلفية التالية من السمة الحالية |

والأمر نفسه من الطرفية:

```bash
ame desktop theme                   # pick one from a list
amethystora-theme list              # the themes, with the current one marked
amethystora-theme set "Tokyo Night" # apply one by name
amethystora-theme toggle            # light or dark
amethystora-theme current           # which one is on
```

## السمات المتاحة

| السمة | الاسم المستخدم | النمط |
| --- | --- | --- |
| Amethystora | `amethystora` | داكنة، وهي المبدئية |
| Amethystora Light | `amethystora-light` | فاتحة |
| Catppuccin Mocha | `catppuccin` | داكنة |
| Catppuccin Latte | `catppuccin-latte` | فاتحة |
| Everforest | `everforest` | داكنة |
| Gruvbox | `gruvbox` | داكنة |
| Matte Black | `matte-black` | داكنة |
| Nord | `nord` | داكنة |
| Rosé Pine Dawn | `rose-pine-dawn` | فاتحة |
| Tokyo Night | `tokyo-night` | داكنة |

تجلب سمتا Amethystora أيضًا سمة GTK الخاصة بـAmethystora، وهي التي فيها الأضواء الملونة الثلاثة في زاوية
كل نافذة. أما السمات الأخرى فتُبقي على مظهر GNOME الأصلي بلون تمييزها. يبدّل `Super+Ctrl+D` السمة إلى
شريكتها (Catppuccin Mocha وLatte، وAmethystora وAmethystora Light)؛ والسمة التي لا شريكة لها تتحول إلى
Amethystora في الوضع الآخر.
