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
//
// Present mode's mark (modules/present.js) follows a recording: it says what the screen is for now.
// The agent's changes (modules/agent/changes.js) come after the battery's question: the agent may be waiting for the
// person. Reading aloud (modules/access/speech.js) comes after what waits for the person and before what runs by
// itself: the person has just asked for it. A focus profile's mark (modules/focus/mark.js) comes last: it says what the
// desktop is set to, not what happens. A meeting about to start (pill/meeting.js) comes before a link: it waits for
// the person, at a time the person set.
// Reopening the apps after an update's restart (pill/resume.js) asks the person, so it comes before what the update
// brought; saving data (pill/saving.js) only says how the connection is, after an update ready.

export const ORDER = ['call', 'recording', 'present', 'meeting', 'link', 'battery', 'agent', 'waiting', 'reading', 'timer',
    'resume', 'news', 'media', 'update', 'saving', 'quiet', 'focus'];

// At most this many others are listed in a card
const OTHERS = 3;

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

    clear() {
        this._running.clear();
        this._changed = null;
    }
}
