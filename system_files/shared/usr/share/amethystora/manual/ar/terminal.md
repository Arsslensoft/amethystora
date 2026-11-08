# الطرفية

صُنع Amethystora لمن يحبون سطر الأوامر بقدر ما صُنع لمن لا يفتحونه أبدًا. وحين تفتحه، تجده جاهزًا لك.

## فتح طرفية

يفتح `Super+Return` طرفية، وكذلك `Ctrl+Alt+T` و`Ctrl+Alt+Return`. الطرفية هي Ptyxis، وهي تعرف
الحاويات: فالقائمة المجاورة لزر اللسان الجديد تفتح لسانًا على المضيف أو داخل أيٍّ من
[حاويات Distrobox وToolbox](containers.md) لديك.

ألوانها تتبع [السمة](themes.md)، وكذلك المحث.

## التحية

تحيّيك كل طرفية جديدة بشعار Amethystora، وملخص عن الجهاز، والصورة التي تشغّلها، وبعض الأوامر المفيدة،
ونصيحة. يعطّلها `ame desktop greeting`، ويعيد تفعيلها.

يعرض `fetch` الشعار وملخص النظام مجددًا متى شئت. يعبر بريقٌ أوجه الشعار قبل ظهور الملخص؛ وتحصل على
الشعار ثابتًا بدلًا من ذلك عبر SSH، وفي الطرفية النصية، وفي طرفية لا تدعم الألوان الكاملة، ومع تعيين
`NO_COLOR`، أو حين يكون **تقليل الحركة** مفعّلًا في **الإعدادات ← الإتاحة**. وكل ما تضيفه بعد `fetch`
يُمرَّر إلى fastfetch، الذي يشغّله.

## الصدفات

الصدفة هي bash، مع محث Starship. وfish وzsh مثبّتتان أيضًا. والتبديل يكون في الطرفية لا على مستوى النظام
كله، كي لا يمنعك إعداد صدفة معطوب من الولوج أبدًا:

1. افتح **التفضيلات** في الطرفية وعدّل ملف التعريف الخاص بك.
2. فعّل **استخدم أمرًا مخصصًا** وأدخل `/usr/bin/fish` أو `/usr/bin/zsh`.

## ame

يشغّل `ame` الأوامر التي تأتي مع النظام: نصوص برمجية صغيرة ومختبَرة للمهام التي كانت ستتطلب لولاها
بحثًا على الويب، مقسّمة في مجموعات مثل `desktop` و`security` و`system`. يذكرها هذا الدليل حيث تفيد؛
ولرؤيتها كلها:

```bash
ame                           # the groups, and the commands outside them
ame security                  # the commands in one group, with a line about what each does
ame --list --list-submodules  # every command, in every group
ame -n security status        # print what a command would run, without running it
```

الأمر `ame pkg` هو [amethystora-pkg](containers.md#packages-from-other-distributions). ويشغّل `ujust`، الاسم
الأقدم لـ`ame`، الأوامر نفسها، ولا تزال أسماؤها السابقة لتقسيمها في مجموعات، مثل `ujust setup-backup`،
تعمل.

هذه الأوامر وصفات `just`، و`just` نفسه متاح لك لمشاريعك: فملف `justfile` في أي مجلد يحوّل أوامره إلى
وصفات بالطريقة نفسها.

## أدوات سطر الأوامر

تأتي أدوات سطر الأوامر من Homebrew، الذي يثبّت في مجلد المنزل دون `sudo` ودون أن يمسّ النظام:

```bash
brew install ripgrep
brew search <name>
brew upgrade
```

تصفّح المتاح على [formulae.brew.sh](https://formulae.brew.sh). يُحدَّث Homebrew مع النظام. لا تشغّله أبدًا
بـ`sudo`: فهو لك، لا للمستخدم الجذر.

موجود سلفًا في الصورة: `just` و`gum` و`glow` و`tmux` و`fastfetch` و`git` وبقية الأدوات المعتادة. وتشمل
الخطوط Inter وJetBrains Mono ورموز Nerd Fonts، فترسم المحثات ومديرو الملفات في الطرفية أيقوناتها.
