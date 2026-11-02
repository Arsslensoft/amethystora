// What is happening now, in the order the pill shows it: only the first that runs is in the pill, the next three are
// rows of its card. Things that wait for the person come before things that run in the background, so that music
// playing all afternoon never hides a call, a link or the battery's question.
//
// An activity is an object the part that runs it makes and keeps:
//   id            one of ORDER
//   pill()        its content in the pill (an actor the activity owns, made once and updated in place)
//   text          the pill's words, as shown (for the suites)
//   accessible    the pill in words, for a screen reader
//   row           {icon, text}: its row among the others in a card
//   card(api)     a new actor with its part of the card; api.close() closes the card
//   plate         '' or the plate's own look: 'link', 'battery', 'saver', 'quiet', 'focus', 'agent', 'agent-ask',
//                 'agent-done'
//   activate()    optional: what a click on the pill's body does instead of opening the card
//   short         optional: a few characters for the bubble ("4:12"), beside its row's icon
//   progress      optional: 0 to 1, or null: a ring round its place, drawn by the pill
//   beside        optional: false keeps it out of the bubble
//   showing(on)   optional: told when it comes into or leaves what can be seen (the pill or the bubble, on a mapped bar)
//
// Two at once (R7-16): the first is in the pill, and `second`, the next that is no mark of how the desktop is set, sits
// as a small bubble beside it; the others stay rows of the card. A call and a recording come before everything else and
// keep the place alone, as they always did: nothing added after them (a plugin's, a flat battery's pill/flat.js, which
// goes right after them) ever comes before them. Present keeps the place alone when it is the first; behind a flat
// battery, which is no ALONE, it may be the bubble beside it.
//
// Present mode's mark (modules/present.js) follows a recording: it says what the screen is for now.
// The agent's changes (modules/agent/changes.js) come after the battery's question: the agent may be waiting for the
// person. Reading aloud (modules/access/speech.js) comes after what waits for the person and before what runs by
// itself: the person has just asked for it. A focus profile's mark (modules/focus/mark.js) comes last: it says what the
// desktop is set to, not what happens. A meeting about to start (pill/meeting.js) comes before a link: it waits for
// the person, at a time the person set. A disk still being written to (pill/stick.js) comes after what waits for the
// person: unplugging it now would lose what is on its way.
// Reopening the apps after an update's restart (pill/resume.js) asks the person, so it comes before what the update
// brought; saving data (pill/saving.js) only says how the connection is, after an update ready. A flat battery about to
// close the computer (pill/flat.js) comes after a call and a recording, whose controls (Mute ringer, Stop) are only in the
// pill: its card is on the screen whatever the pill says, and it is listed in the card of whoever comes first.
// A code from the phone (pill/code.js) lives a minute and waits for the person, as a link does. What a drag can be dropped
// on (pill/drop.js) and a device arriving (pill/hello.js) are the person's own moment, for as long as it lasts. A second
// timer comes after the first. What is moving (pill/moving.js) runs by itself, after music: the music stays in the pill
// and it sits beside it.
// A print job
// (pill/printing.js) runs by itself but may wait for paper, so it comes before what only reads; getting through on the
// battery (pill/through.js) only says how things are, beside saving data. A print job that has stood still for ten minutes
// steps back behind what plays and what is ready (printing-stuck), still in the card.

export const ORDER = ['call', 'recording', 'flat', 'present', 'meeting', 'link', 'code', 'battery', 'agent', 'waiting', 'drop', 'hello', 'stick',
    'reading', 'timer', 'timer2', 'printing', 'resume', 'news', 'media', 'moving', 'update', 'printing-stuck', 'through', 'saving', 'quiet', 'focus'];

// At most this many others are listed in a card
const OTHERS = 3;
// These keep the place alone
const ALONE = new Set(['call', 'recording', 'present']);
// These say how the desktop is set, not what is happening: never the bubble
const MARKS = new Set(['saving', 'quiet', 'focus']);

export class Activities {
    constructor(changed) {
        this._changed = changed;
        this._running = new Map();
    }

    // Starts, replaces or (with null) ends the activity of this id
    set(id, activity) {
        if (!ORDER.includes(id))
            throw new Error(`no activity ${id}`);
        const before = this._running.get(id) ?? null;
        if (activity)
            this._running.set(id, activity);
        else
            this._running.delete(id);
        if (before !== activity)
            this._changed?.();
    }

    get(id) {
        return this._running.get(id) ?? null;
    }

    // Something an activity shows changed (its words, and so perhaps its width)
    update(id) {
        if (this._running.has(id))
            this._changed?.(id);
    }

    get list() {
        return ORDER.filter(id => this._running.has(id)).map(id => this._running.get(id));
    }

    get top() {
        return this.list[0] ?? null;
    }

    get others() {
        return this.list.slice(1, 1 + OTHERS);
    }

    // What sits beside the first, or null
    get second() {
        const [top, ...rest] = this.list;
        if (!top || ALONE.has(top.id))
            return null;
        return rest.find(activity => !MARKS.has(activity.id) && activity.beside !== false) ?? null;
    }

    clear() {
        this._running.clear();
        this._changed = null;
    }
}
