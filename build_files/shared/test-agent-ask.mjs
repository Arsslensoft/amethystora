// The pill's questions to the agent (R7-11, R7-12, R7-14), the parts that need no screen: how an answer is made fit to show, how a command
// is read, what a hand-off's question file may hold, what is sent to the launcher, and what may be asked about. No screen:
//   node build_files/shared/test-agent-ask.mjs
// A failure throws, which makes node exit with an error.

const UI = '../../system_files/shared/usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io/modules';
const {ANSWER_SHOWN, COMMAND_SHOWN, PICTURE_MAX, aboutFiles, askInput, commandLine, isId, kindOfName, outcome, parseQuestion, pictureEnd, tidy} =
    await import(`${UI}/agent/askmodel.js`);

const say = globalThis.print ?? console.log;
const failures = [];
let count = 0;

function same(what, got, want) {
    count++;
    if (JSON.stringify(got) !== JSON.stringify(want))
        failures.push(`${what}: expected ${JSON.stringify(want)}, got ${JSON.stringify(got)}`);
}

const chr = code => String.fromCodePoint(code);

// --- An answer is somebody else's words: shown as text, whole lines, no characters that hide or turn ------------------------------
same('plain text stays', tidy('Firefox is busy.\nOne tab plays video.'), 'Firefox is busy.\nOne tab plays video.');
same('control characters go', tidy(`a${chr(0)}b${chr(7)}c${chr(0x1b)}[31md`), 'abc[31md');
same('direction marks and overrides go', tidy(`a${chr(0x202e)}b${chr(0x2066)}c${chr(0x200f)}d`), 'abcd');
same('line separators go', tidy(`a${chr(0x2028)}b${chr(0x2029)}c`), 'abc');
same('carriage returns and tabs', tidy('a\r\nb\rc\td'), 'a\nb\nc d');
same('blank lines are one', tidy('a\n\n\n\n\nb'), 'a\n\nb');
same('trailing space goes', tidy('a   \nb  '), 'a\nb');
same('a long answer is cut with a mark', [...tidy('x'.repeat(ANSWER_SHOWN + 50))].length, ANSWER_SHOWN);
same('and says so', tidy('x'.repeat(ANSWER_SHOWN + 50)).endsWith('…'), true);
same('not a string is nothing', tidy(null), '');

// --- A command is read whole -----------------------------------------------------------------------------------------------------
same('plain words stay', commandLine(['flatpak', 'install', '--user', 'org.inkscape.Inkscape']), 'flatpak install --user org.inkscape.Inkscape');
same('a space is quoted', commandLine(['echo', 'a b']), "echo 'a b'");
same('a quote is closed and opened', commandLine(['echo', "it's"]), "echo 'it'\\''s'");
same('a semicolon is quoted, so it is plainly one word', commandLine(['echo', 'a;rm']), "echo 'a;rm'");

// --- What a hand-off's question file may hold ------------------------------------------------------------------------------------
const ask = more => JSON.stringify({kind: 'run', command: ['ls', '-l'], root: false, reason: 'to look', server: 4, ...more});
same('a command', parseQuestion(ask({}))?.line, 'ls -l');
same('its reason is cleaned', parseQuestion(ask({reason: `a${chr(0x202e)}b`}))?.reason, 'ab');
same('a setting', parseQuestion(ask({kind: 'change', title: 'Layout'}))?.title, 'Layout');
same('root is only true when it says true', parseQuestion(ask({root: 'yes'}))?.root, false);
same('another kind is refused', parseQuestion(ask({kind: 'install'})), null);
same('no command is refused', parseQuestion(ask({command: []})), null);
same('a command that is not words is refused', parseQuestion(ask({command: ['a', 1]})), null);
same('a string for a command is refused', parseQuestion(ask({command: 'rm -rf /'})), null);
same('too many words are refused', parseQuestion(ask({command: Array(65).fill('a')})), null);
same('an empty word is refused', parseQuestion(ask({command: ['a', '']})), null);
same('not JSON is refused', parseQuestion('{nope'), null);
same('a command too long for a card says so', parseQuestion(ask({command: ['echo', 'x'.repeat(COMMAND_SHOWN)]}))?.tooLong, true);
same('a short one does not', parseQuestion(ask({}))?.tooLong, false);
same('a command with a direction override is not shown in a card', parseQuestion(ask({command: ['echo', `a${chr(0x202e)}b`]}))?.tooLong, true);
same('nor one with a line break in a word', parseQuestion(ask({command: ['echo', 'a\nb']}))?.tooLong, true);
same('nor a zero-width character', parseQuestion(ask({command: ['echo', `a${chr(0x200b)}b`]}))?.tooLong, true);
same('a plain one is', parseQuestion(ask({command: ['echo', 'a b']}))?.tooLong, false);
same('an id is eight hex digits', [isId('ab12cd34'), isId('AB12CD34'), isId('../../etc'), isId('ab12cd3'), isId(12345678)], [true, false, false, false, false]);

// --- What goes to the launcher ----------------------------------------------------------------------------------------------------
same('a question alone', JSON.parse(askInput('why is it slow?')), {question: 'why is it slow?'});
same('with text it is about', JSON.parse(askInput('what is this', {quoted: 'a\nb'})), {question: 'what is this', quoted: 'a\nb'});
same('with a picture', JSON.parse(askInput('what is this', {picture: '/run/user/1000/amethystora/ask/pic-1.png'})).picture, '/run/user/1000/amethystora/ask/pic-1.png');
same('a long question is cut', [...JSON.parse(askInput('q'.repeat(900))).question].length, 600);

// --- How it ended -------------------------------------------------------------------------------------------------------------------
same('an answer', outcome(0, 'Hello\n'), {kind: 'answer', text: 'Hello'});
same('nothing is no answer', outcome(0, '  \n'), {kind: 'failed', why: 'empty'});
same('off', outcome(2, '').why, 'off');
same('not installed', outcome(3, '').why, 'missing');
same('an agent with no mode that only looks', outcome(4, '').why, 'headless');
same('slow', [outcome(124, '').why, outcome(137, '').why], ['slow', 'slow']);
same('anything else', outcome(1, 'Not logged in').why, 'error');
same('and says what it said', outcome(1, 'Not logged in').detail, 'Not logged in');

// --- What may be asked about (R7-14) ------------------------------------------------------------------------------------------------
same('a picture', kindOfName('Screenshot 1.PNG'), 'picture');
same('a log', kindOfName('boot.log'), 'text');
same('a program is not text to send', kindOfName('setup.exe'), null);
same('no ending', kindOfName('README'), null);
same('a picture ends in one of the list', [pictureEnd('a.JPG'), pictureEnd('a.exe'), pictureEnd('a')], ['jpg', 'png', 'png']);
same('one file at a time', aboutFiles([{name: 'a.png', size: 1}, {name: 'b.png', size: 1}]).why, 'one');
same('a kind the agent cannot read', aboutFiles([{name: 'a.exe', size: 1}]).why, 'kind');
same('a picture too large', aboutFiles([{name: 'a.png', size: PICTURE_MAX + 1}]).why, 'big');
same('a picture', aboutFiles([{name: 'a.png', size: 10}]).kind, 'picture');
same('a large text file is read only at its start, so it is allowed', aboutFiles([{name: 'a.log', size: PICTURE_MAX * 4}]).kind, 'text');

if (failures.length) {
    for (const failure of failures)
        say(`not ok: ${failure}`);
    throw new Error(`${failures.length} of ${count} checks of the pill's questions failed`);
}
say(`ok: ${count} checks of the pill's questions`);
