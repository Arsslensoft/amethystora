// The pill's questions to the agent (R7-11, R7-12, R7-14), the parts that need no screen: the words an answer is shown in, a
// command as the person reads it, and the checks of what a hand-off asks. No imports: Node reads this file in
// build_files/shared/test-agent-ask.mjs.

// The most of an answer the card shows, and of a typed question
export const ANSWER_SHOWN = 1200;
export const QUESTION_MAX = 600;
// A command longer than this is never shown in a card: a person has to read all of a command to say yes to it, so the question
// falls back to the terminal, which prints it whole
export const COMMAND_SHOWN = 1200;
// The answer stays in the pill this many seconds, and no longer
export const ANSWER_KEPT = 600;
// A question nobody answered in this many seconds is dropped
export const QUESTION_KEPT = 3600;

// C0 and C1 control characters but the newline, and the characters that turn text round or hide part of it
// eslint-disable-next-line no-control-regex
const UNSEEN = /[\u0000-\u0009\u000b-\u001f\u007f-\u009f\u200b-\u200f\u202a-\u202e\u2066-\u2069\u2028\u2029\ufeff]/g;

// Text somebody else wrote (an agent's answer), made fit to show: no control or direction characters, tabs as spaces, no
// more than one blank line in a row, at most `max` characters, with … where it was cut
export function tidy(text, max = ANSWER_SHOWN) {
    if (typeof text !== 'string')
        return '';
    const lines = text.replace(/\r\n?|\u0085/g, '\n').replace(/\t/g, ' ').replace(UNSEEN, '').split('\n')
        .map(line => line.replace(/\s+$/, ''));
    const body = lines.join('\n').replace(/\n{3,}/g, '\n\n').trim();
    const letters = [...body];
    return letters.length > max ? `${letters.slice(0, max - 1).join('').trimEnd()}…` : body;
}

// A command as the person reads it: each word quoted when it needs to be, as the tools' own history does
export function commandLine(words) {
    return words.map(word => (/^[A-Za-z0-9_./:=@+,%-]+$/.test(word) ? word : `'${word.replace(/'/g, "'\\''")}'`)).join(' ');
}

export function isId(id) {
    return typeof id === 'string' && /^[0-9a-f]{8}$/.test(id);
}

// What a hand-off's question file holds (apps/agent-tools/src/handoff.rs writes it), checked: null for anything else. `reason`
// is the agent's own sentence, a person's yes is for the command; `title` names a setting for a change.
export function parseQuestion(text) {
    let body;
    try {
        body = JSON.parse(text);
    } catch {
        return null;
    }
    if (!body || typeof body !== 'object' || !['run', 'change'].includes(body.kind))
        return null;
    const command = body.command;
    if (!Array.isArray(command) || command.length === 0 || command.length > 64 ||
        !command.every(word => typeof word === 'string' && word.length > 0 && word.length <= 4096 && !word.includes('\0')))
        return null;
    const line = commandLine(command);
    return {
        kind: body.kind,
        command,
        line,
        tooLong: [...line].length > COMMAND_SHOWN,
        root: body.root === true,
        reason: tidy(typeof body.reason === 'string' ? body.reason : '', 600),
        title: tidy(typeof body.title === 'string' ? body.title : '', 120),
        server: Number.isInteger(body.server) ? body.server : 0,
    };
}

// What one pill question sends to `amethystora-agent ask-here`, as the JSON on its standard input
export function askInput(question, {quoted = '', picture = ''} = {}) {
    const input = {question: tidy(question, QUESTION_MAX)};
    if (quoted)
        input.quoted = tidy(quoted, 4000);
    if (picture)
        input.picture = picture;
    return JSON.stringify(input);
}

// How `ask-here` ended, as what the card says: `answer` with the text, or `failed` with a reason key the module words
export function outcome(status, stdout) {
    if (status === 0) {
        const text = tidy(stdout);
        return text ? {kind: 'answer', text} : {kind: 'failed', why: 'empty'};
    }
    return {kind: 'failed', why: {2: 'off', 3: 'missing', 4: 'headless', 124: 'slow', 137: 'slow'}[status] ?? 'error',
        detail: tidy(stdout, 160)};
}
