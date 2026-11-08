# Sound and captions

The desktop's own sounds, what happens to your sound and microphone, and captions and dictation made on this computer.

## Sounds

The desktop's own sounds are Amethystora's **Soft** set: lower and rounder, for a quiet room. Three more sets play the
same events in other voices: **Glass**, bright, short chimes; **Wood**, warm knocks and taps; **Retro**, eight-bit
blips. Each is made from arithmetic by the image's own script, nothing recorded.

You hear them for a message from a chat app that asks for one (GNOME plays no sound for an ordinary notification), a text message from
your [phone](wireless.md#phones), plugging in the power, a screenshot, each step of the volume, an error and the
bell, and a slow chord once the desktop has started. Any other event an app asks for comes from the standard sound
theme.

### Choosing a set

`ame desktop sounds soft|glass|wood|retro` picks a set, and `ame desktop sounds off` makes the desktop silent: it
makes no sound of its own, and apps can still make theirs (a call, music, a game). Control has the same choice under
Appearance, in the **Sound** group. Choosing an alert sound in **Settings**, **Sound**, gives the bell that sound;
Control then shows none of the sets as chosen, and `ame desktop sounds soft` goes back to the whole Soft set.

### Pause when the headphones come off

When the headphones or the headset you are listening on are unplugged or go away (a Bluetooth headset out of range or
switched off), every player that is playing pauses, so that a podcast never bursts out of the laptop in a quiet room. A
notice says **Your headphones went away**, with **Play on speakers** to play on; when the headphones come back while it
shows, it offers **Play**. Picking the speakers yourself while the headphones stay plugged in pauses nothing. Players
are paused through their own media controls, the same ones the top bar's media card uses. It is on;
`ame desktop pause-on-unplug on|off`, also in Control.

### Clear voice

**Clear voice** takes background noise and echo out of your microphone: the fan, the keyboard, the street and the
room's echo. Calls hear your voice and little else, in every app, because the apps simply get a cleaner microphone:
PipeWire's own echo cancel and noise suppression (its webrtc processor), as an input of its own named **Clear voice**,
made the one in use while it is on. It works only while an app records, and costs some processor time then; nothing is
sent or saved. It is off; turn it on with the switch at the end of the microphone's menu in quick settings (shown
while an app records), with `ame desktop clear-voice on|off`, or in Control. Off puts back the microphone you had.

### Sound presets

**Sound presets** shape what an output plays, kept for each output: **Laptop** fills out thin laptop speakers,
**Speech** makes voices and podcasts clearer, **Bass** is for music, **Flat** is as it comes. Each is PipeWire's own
equaliser placed before that output, and only that output: headphones keep Flat until you pick otherwise. They are
off, since a preset changes every app's sound; `ame desktop sound-presets on` turns them on, and the output's menu in
quick settings (the arrow beside the volume) then shows the four with the curve of the one chosen. `ame desktop
sound-preset flat|laptop|speech|bass` sets the output in use, also in Control.

## Live captions

Live captions put the words of whatever is being said in a video, a call or a podcast in a dark box near the bottom
of the screen, as they are spoken. They are made on this computer from the sound it plays, by the Vosk speech
recognizer.

![Live captions](media/live-captions.webm)

They are **off**: turn them on with

```bash
ame desktop captions on
```

or **Live captions** on [Control](control.md)'s **Appearance** page. The first time, it says what it downloads and
asks: a speech model of 39 MiB (68 MiB once unpacked), fetched once from alphacephei.com, the makers of Vosk, and
checked against the copy the image knows. After that, **Live captions** in [quick settings](quick-settings.md) turns
them on and off.

### Languages

Captions hear **English** unless you choose French, Spanish, German, Italian, Turkish, Portuguese, Dutch, Russian or
Polish, with **Captions language** in Control or `ame desktop captions language fr` (`es`, `de`, `it`, `tr`, `pt`,
`nl`, `ru`, `pl`; `en` is English). Each language's speech model, 31 to 51 MiB, is downloaded once from
alphacephei.com when you choose it, after it says so and asks, and checked against the copy the image knows; then that
language works offline like English. [What this machine sends](what-leaves.md) names every language downloaded.
[Dictation](#dictation) hears the same language.

### Good to know

- The sound is listened to only while captions are on and an app plays something, and only in memory: it is never
  recorded, written down or sent anywhere, and neither are the words. They are gone a few seconds after they are
  shown.
- Your microphone is never used. The **Live captions** toggle in quick settings, checked, is what shows they are on.
- The box is drawn over every window, a video in full screen too, and clicks go through it. It is part of the
  screen, so a screenshot taken or a screen shared while words show has them in it.
- They cost a part of one processor core while speech plays and hold about 230 MB of memory while they are on.
  Silence, and an app that is paused, cost next to nothing.
- Recognition is good for clear speech and makes mistakes with music, accents and several voices at once.
- `ame desktop captions off` turns them off; `ame desktop captions remove` turns them and dictation off and deletes
  every model; `ame desktop captions status` says how things stand.

## Dictation

Dictation types what you say into the text field you are in: a note, a message, a search. It is made on this computer
by the same recognizer as [live captions](#live-captions), in their language, and it is **off**: turn it on with

```bash
ame desktop dictation on
```

or **Dictation** on [Control](control.md)'s **Appearance** page. If the speech model of the captions' language is not
on this computer yet, it says what it downloads and asks first, as captions do.

### Dictating

Click into a text field, press **Super+Ctrl+Shift+D** and speak. A small pill near the bottom of the screen says
*Listening on this computer only* and shows the words as they come; each sentence you finish is typed into the field.
Press **Super+Ctrl+Shift+D** again to stop. It also stops by itself after 15 seconds without a word, after two
minutes, or when you go to another window.

### What dictation hears and types

- The microphone is listened to only while the pill shows, and GNOME's microphone indicator shows in the top bar
  meanwhile. Nothing is recorded or written down, and nothing is sent anywhere.
- It never types into a password field.
- Most apps take the words as typed text. In a terminal, and in an app that does not take text from the input method,
  each sentence is pasted instead, so it also lands on the [clipboard](clipboard.md#the-clipboard) and in its history.
- Dictation writes what it hears without punctuation, and makes the mistakes captions make with noise and accents.
- `ame desktop dictation off` turns it off, and its shortcut with it.
