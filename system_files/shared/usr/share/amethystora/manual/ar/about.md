# حول

أنشأ Amethystora ويصونه Arsslen Idadi ([@arsslensoft](https://github.com/arsslensoft)). وهو برمجية حرة بموجب
Apache License 2.0، تُبنى علنًا على [github.com/arsslensoft/amethystora](https://github.com/arsslensoft/amethystora).
والبلاغات والأفكار وطلبات الدمج مرحَّب بها هناك.

يستند Amethystora إلى [Universal Blue](https://universal-blue.org)، حقوق النشر © للمساهمين في Universal Blue، ومرخّص بموجب Apache License 2.0.

## هذا الدليل

الصفحات ملفات Markdown في `/usr/share/amethystora/manual`، ملف لكل صفحة، وصفحة مفاتيح الاختصار هي
`/usr/share/amethystora/keybindings.md`، وهو الملف نفسه الذي يعرضه `ame desktop keybindings`. وهي تأتي مع
الصورة، لذا يصف الدليل دائمًا النظام الذي تشغّله، ويعمل دون اتصال بالشبكة.

```bash
amethystora-manual                 # open it where you left off
amethystora-manual updates         # open a page by its file name
amethystora-manual --list          # the pages there are
```

داخل الدليل: يبحث `Ctrl+K` أو `/`، وينقلك `Alt+Left` و`Alt+Right` إلى الخلف والأمام، ويغيّر `Ctrl+=`
و`Ctrl+-` حجم النص، ويعيده `Ctrl+0` إلى أصله.

## على أكتاف العمالقة

بُني Amethystora على [Fedora](https://fedoraproject.org) و[GNOME](https://www.gnome.org)، وعلى عمل مشاريع
أخرى كثيرة: نواة Linux وsystemd، وbootc وrpm-ostree، وFlatpak وFlathub، وHomebrew، وDistrobox وPodman، وكل
امتداد GNOME مذكور في [التنقّل في سطح المكتب](desktop.md#extensions). والأيقونات هي
[candy-icons](https://github.com/EliverLara/candy-icons) من Eliver Lara، وسمة GTK الخاصة بـAmethystora معاد
تلوينها من سمته Sweet. أما خلفيات السمات فمرسومة خصيصًا لـAmethystora.

إن Fedora وشعار Fedora علامتان تجاريتان لشركة Red Hat, Inc. ولا يقدّم Fedora Project ولا Red Hat نظام
Amethystora ولا يدعمانه ولا يؤيّدانه. ويوجد Fedora نفسه على [fedoraproject.org](https://fedoraproject.org).

رخص كل ما في الصورة موجودة في `/usr/share/licenses`. وبعض الأجزاء ليست برمجيات حرة وتخضع لشروط صانعيها:
البرامج الثابتة للأجهزة، ومشغّل DisplayLink لمحطات الإرساء، ومشغّل NVIDIA في صور NVIDIA. ويسردها الملف
`/usr/share/licenses/amethystora/NOTICE`.

## الشيفرة المصدرية

كل حزمة في الصورة مُدرجة في `/usr/share/licenses/amethystora/SOURCES`، مع رخصتها، والحزمة المصدرية التي
بُنيت منها، ومكان ذلك المصدر. ومصدر حزمة Fedora موجود في نظام البناء لدى Fedora على العنوان المذكور هناك،
أو على بُعد أمر واحد:

```bash
dnf download --srpm <package>    # the source of the version Fedora has now
```

وبعض الحزم يبنيها آخرون (RPM Fusion، وnegativo17، وFedora Copr)، ولا ينشرون إلا بنيتهم الحالية من كل منها.
وحيث تطلب رخصة حزمة كهذه مصدرها، تكون الحزمة المصدرية التي بُنيت منها هذه الصورة داخل الصورة نفسها، في
`/usr/src/amethystora`.

وكل ما عدا ذلك يُبنى من [مستودع Amethystora](https://github.com/arsslensoft/amethystora) عند الإيداع المسمّى
`BUILD_ID` في `/usr/lib/os-release`. ولمدة ثلاث سنوات على الأقل بعد آخر نشر لصورة، تُعطى الشيفرة المصدرية
الكاملة لكل ما فيها مما تطلب رخصته إتاحة مصدره (GNU GPL وLGPL، وMPL، وCDDL، ورخصة مرماز Fraunhofer AAC) لكل
من يطلبها عبر [متتبّع البلاغات](https://github.com/arsslensoft/amethystora/issues)؛ والعرض المكتوب موجود في
`/usr/share/licenses/amethystora/NOTICE`.
