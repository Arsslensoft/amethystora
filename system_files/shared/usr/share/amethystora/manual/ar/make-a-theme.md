# عدّل سمة، أو اصنع سمتك

تأتي السمات للقراءة فقط في `/usr/share/amethystora/themes`. والمجلد الذي يحمل الاسم نفسه في
`~/.config/amethystora/themes` يُوضع فوقها، فلا تكتب إلا الملفات التي تريد تغييرها. والمجلد الذي يحمل
اسمًا خاصًا به سمة جديدة. كل ملف صغير واختياري، باستثناء لوحة الألوان:

| الملف | محتواه |
| --- | --- |
| `colors.toml` | لوحة الألوان: `accent` و`foreground` و`background` و`cursor` و`selection_foreground` و`selection_background`، و`color0` إلى `color15` |
| `light.mode` | موجود، وفارغ، حين تكون السمة فاتحة |
| `pair.theme` | السمة التي يبدّل إليها `Super+Ctrl+D` |
| `accent.theme` | لون التمييز في GNOME: blue أو teal أو green أو yellow أو orange أو red أو pink أو purple أو slate. ومن دونه يُختار أقرب لون |
| `gtk.theme` | اسم سمة GTK، بدلًا من مظهر GNOME الأصلي |
| `icons.theme` | سمة أيقونات، بدلًا من candy-icons |
| `cursor.theme` | سمة مؤشر |
| `vscode.theme` | سمة ألوان VSCodium التي يُبدَّل إليها |
| `tophat.theme` | لون مقاييس الشريط العلوي، حين لا يظهر لون التمييز جيدًا في خط رفيع |
| `backgrounds/` | الخلفيات |
| `backgrounds.list` | خلفيات محفوظة في مكان آخر من النظام، مسار في كل سطر. الأولى هي التي تبدأ بها السمة |

السمة التي ليس لها صور خاصة تحصل على أربع صور مرسومة بألوانها: توهّج ناعم، وتلال في ضباب، وخريطة
كنتورية، وأشرطة منسابة.

تحتاج سمتك الخاصة إلى ثلاثة أوامر:

```bash
mkdir -p ~/.config/amethystora/themes/sunset
cp /usr/share/amethystora/themes/amethystora/colors.toml ~/.config/amethystora/themes/sunset/
amethystora-theme set sunset
```

عدّل الألوان، ونفّذ `amethystora-theme set sunset` مجددًا، وانظر إلى النتيجة.

## للتعمّق أكثر

- **كيف تُكتب الإعدادات.** لوحة ألوان الطرفية، والمحث، وألوان btop، والخلفيات المرسومة كلها قوالب في
  `/usr/share/amethystora/themed`. انسخ أحدها إلى `~/.config/amethystora/themed/` وعدّل النسخة لتغيّر
  طريقة إخراج كل السمات له.
- **شغّل شيئًا عند كل تبديل.** ضع ملفًا تنفيذيًا في `~/.config/amethystora/hooks/theme-set.d/`. وسيتلقى
  اسم السمة في `AMETHYSTORA_THEME` ومجلدها في `AMETHYSTORA_THEME_DIR`.
- **محثّك الخاص يبقى لك.** إن كنت كتبت `~/.config/starship.toml` بنفسك، فلن تمسّه السمة.
- **لا تعدّل `~/.config/amethystora/current/` أبدًا.** فهو يُعاد كتابته عند كل تبديل.

## قائمة الإقلاع

يمكن لقائمة الإقلاع أن تحمل رسومات Amethystora أيضًا. وهذا معطّل مبدئيًا، لأن قائمة الإقلاع تقع خارج
الصورة:

```bash
ame desktop boot-menu
```

نفّذه مجددًا لإزالة الرسومات. وما إن تُفعَّل، حتى تُبقيها تحديثات الصورة محدَّثة.
