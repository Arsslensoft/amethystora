# The agent on the desktop

While the [agent](ai-agent.md)'s features are on, the live pill shows what it changes and reads, takes your questions
and its own, and a selection or a picture can be handed to it with one key.

## Its changes in the live pill

While the agent may make changes and runs, the pill in the middle of the top bar says so as soon as it changes
something, and every change has its own **Undo**.

![Undo the agent's change](media/agent-undo.webm)

The pill says **Agent · changing settings**, **Agent asks for root** while it waits for your password or your yes in a
terminal, and **Agent made 3 changes** for ten minutes after it ends. Click it to see each change (what it was, the
command or the app, and when) with its **Undo**; **Undo all** takes them back, the latest first; **History** opens the
whole record.

### Stopping it

**Stop the agent** ends its tools now (`ame agent stop` does the same). A change already under way, such as an
install, finishes, and some agents start their tools again, so close the agent itself to be sure it changes nothing
more.

### When an undo is refused

Undo is your own yes, and goes through the agent's own tools: an undo they refuse (a setting no longer offered, a
record whose way back does not match what it says it did) is refused, and the row says why. The agent's tools will not
turn this list off: `ame agent pill off` does, and is yours to run.

## What it is doing

While the agent's tools read, the pill says which in a few words (**Searching the manual**, **Checking the updates**,
**Reading the logs**) with its mark breathing, and a tick when the step is done. Click it for the last few steps, each
Done or Now, and **Stop the agent**. It shows only the tool's name, never what you asked or what it read, and is gone a
few seconds after the last step. `ame agent doing off` turns it off.

## Asking in the pill

Press `Super+/` (`Super+Shift+Space` is the next keyboard layout) and the pill's card opens with a field. Type your
question and press Enter: your agent answers there, with no terminal.

It looks at this computer only with Amethystora's tools that read (the manual, the machine's state, the log): it
cannot change anything or read your files. The card says whose answer it is, and that your question left this
computer for the maker of the agent you chose. The answer is plain text, kept ten minutes at most, and Amethystora
writes none of it to a file.

It works with Claude Code. The other agents have no way, checked by this image, to ask a question that changes
nothing, so for them the card offers to open the agent in a terminal instead; so does the key while the live pill is
off. `ame agent ask-here off` turns the key off.

## Questions in the pill

When the agent's tools ask to run a command for you, a setting that asks something, or a command as root in a
terminal, the pill says **Agent asks you**. Its card shows the exact command, the agent's reason in its own words, and
**Allow** and **Not now**. Allow opens the terminal that runs it and sends what it prints back to the agent; Not now
tells the agent no. A command too long for a card is asked in the terminal, printed whole, and a root command still
asks for your password there, from sudo.

### Your yes

Your yes is a click in the desktop, answered over the session bus: no file the agent could write stands for it, and
the command you read is checked against what the tools wrote. A question nobody answered (the screen locked, the
features went off, an hour passed) is told to the agent as not answered, never as a no. `ame agent ask-questions off`
makes them ask in a terminal again. Installing an app or changing a setting with the agent's own tools is still asked
in the agent itself.

## Asking about a picture or a file

Off until you turn it on with `ame agent ask-about on`. Then drop a picture or a text file on the pill's **Ask the
agent**, or copy a picture (a screenshot of a region, say) and press `Super+Shift+A` with no text selected.

A card shows the file and says what goes and to whom: "This picture goes to the maker of Claude Code. What is on it
goes with it." Nothing is read past a text file's first bytes, copied or sent until you press **Send and ask**; a
picture is copied, for the moment the question takes, into a folder only you can read, then taken away. It is refused
while your screen is shared or recorded.

## Asking about a selection

Select text in any window (an error in a terminal, a paragraph on a web page, a line in a log) and press
`Super+Shift+A`. A card shows the start of it, with a field for your question (empty, to have it explained),
**Cancel** and **Ask**.

![Ask about a selection](media/agent-selection.webm)

Ask opens the agent in a terminal with your question and the text as data: the agent is told someone else wrote it and
never to follow what it says, since selected text can carry instructions written to steer an agent. Up to 4,000
characters are passed on, each line at most 600; nothing is read before you press the keys, and nothing is kept.

It works only while the agentic features are on. `ame agent selection off` turns the key off.

## Notices that offer the agent

While the agentic features are on, the image's own notices of a failure (a security scan that could not run, a
device the driver check found without a driver) have an **Ask the agent** button. It opens the agent on the failed
service or the check's finding by its name, never on the notice's words. Other apps' notifications never get the
button: anyone can raise a notification, and its words could be written to steer the agent.
