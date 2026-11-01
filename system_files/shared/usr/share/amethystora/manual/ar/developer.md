# وضع المطوّر

الصورة `amethystora-dx` هي سطح المكتب نفسه وفوقه ورشة مطوّر: محركات حاويات، وآلات افتراضية، وVSCodium،
وأدوات التحليل لمعرفة سبب بطء شيء ما.

الفكرة من ورائها: لا ينبغي أن تكون بيئة تطويرك هي نظام تشغيلك. تعيش سلاسل الأدوات وبيئات تشغيل اللغات
وقواعد البيانات في حاويات موصوفة في مستودع المشروع نفسه، فيُبنى المشروع بالطريقة نفسها على جهازك، وعلى
جهاز Mac لزميلك، وفي التكامل المستمر (CI)، ويبقى النظام من تحتها نظيفًا بما يكفي لتحديثه دون تردد.

## تفعيله

انتقل إلى صورة dx، وأعد التشغيل:

```bash
sudo bootc switch --enforce-container-sigpolicy ghcr.io/arsslensoft/amethystora-dx:stable
```

إن كانت لديك [حزم مضافة كطبقات](software.md#layering-the-last-resort)، فاستخدم
`sudo rpm-ostree rebase ostree-image-signed:docker://ghcr.io/arsslensoft/amethystora-dx:stable` بدلًا من
ذلك، فهو يحتفظ بها. والعودة إلى `amethystora` تتم بالطريقة نفسها.

في إقلاعها الأول، تضيف صورة dx كل حساب مدير إلى المجموعات `docker` و`incus-admin` و`libvirt`
و`wireshark`. وإن قال `docker ps` إن الإذن مرفوض، فسجّل الخروج ثم لِج مجددًا مرة واحدة.

## ما تضيفه

| الأداة | الغرض |
| --- | --- |
| VSCodium | المحرر: VS Code مبنيًّا من شيفرته مفتوحة المصدر، دون قياس Microsoft عن بُعد أو رخصتها. تأتي الامتدادات من [Open VSX](https://open-vsx.org)؛ وOpen Remote SSH وContainer Tools مثبّتان. وألوانه تتبع [السمة](themes.md). |
| Docker Engine | مع buildx وcompose. وهو المبدئي لحاويات التطوير. |
| Podman | مع `podman-compose` و`podman machine`. موجود دائمًا، ويعمل دون صلاحيات الجذر، ومقبس Podman مفعّل. |
| Incus | حاويات نظام وآلات افتراضية، تُدار مثل مثيلات السحابة |
| libvirt وvirt-manager | آلات افتراضية بـQEMU وKVM |
| Sysprof، perf، bcc، bpftrace، bpftop، bpftool، trace-cmd | التحليل والتتبع، من النظام كله وصولًا إلى دالة واحدة |
| gdb، strace، ltrace، Valgrind | التنقيح: تتبّع برنامج خطوة بخطوة، وراقب استدعاءاته للنواة ولمكتباته، واعثر على أخطاء الذاكرة. ويتطلب ربط gdb ببرنامج لم يبدأه هو `sudo` ([لماذا](security.md#on-from-the-start)). |
| Wireshark | التقاط حركة مرور الشبكة وقراءتها، دون `sudo` |
| android-tools | `adb` و`fastboot` |
| ROCm | الحوسبة على معالج الرسوميات في بطاقات AMD |
| flatpak-builder | بناء حزم Flatpak |

## حاويات التطوير

حاوية التطوير ملف `.devcontainer/devcontainer.json` في المشروع: الصورة، والأدوات، وامتدادات المحرر التي
يحتاج إليها المشروع. وكل ما يثبّته المشروع يبقى هناك.

لا يعمل امتداد Dev Containers من Microsoft إلا في VS Code الخاص بـMicrosoft، لذا تعمل حاويات التطوير على
Amethystora عبر سطر أوامر `devcontainer`، وهو التطبيق المرجعي [للمواصفة](https://containers.dev)، التي
تتبعها بيئات JetBrains التطويرية أيضًا:

```bash
brew install devcontainer
devcontainer up --workspace-folder .
devcontainer exec --workspace-folder . bash
```

لاستخدام Podman بدلًا من Docker، أضف `--docker-path podman --docker-compose-path podman-compose` إلى
`devcontainer up`.

إن عجزت حاوية عن قراءة مجلد موصول بسبب SELinux، فأعد وسم المجلد بدلًا من تعطيل SELinux:
`restorecon -R -v ~/code/myproject`.

## محررات أخرى

- **JetBrains:** يثبّت `ame apps jetbrains-toolbox` أداة JetBrains Toolbox في مجلد المنزل، وهي بدورها تثبّت
  بيئات التطوير وتحدّثها. ولا يُنصح بنسخ Flatpak من هذه البيئات.
- **Neovim وHelix وأمثالهما:** `brew install neovim`، و`brew install devcontainer` لسطر أوامر حاويات
  التطوير.

## Kubernetes والسحابة

ثبّت أدوات سطر الأوامر بـHomebrew، فتبقى محدَّثة دون أن تمسّ الصورة:

```bash
brew install kubectl helm k9s kind
```

يشغّل `kind` عنقود Kubernetes كاملًا في حاويات Docker، وهي أسرع طريقة لتجربة شيء على عنقود حقيقي.
