// The logic of the truth group (plans/ui-ideas/r7/spec-truth.md), checked under node and gjs alike:
//   node build_files/shared/test-truth.mjs
// Where a connection breaks, what a removable disk's numbers say, and what to say of a night's battery. A failure throws.

const UI = '../../system_files/shared/usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io/modules';
const {arpAnswers, hostOf, NAME_FLAGS, nameFailure, verdict} = await import(`${UI}/tools/reachlogic.js`);
const {blockName, parseDirty, parseStat, provable, WriteWatch} = await import(`${UI}/pill/stickmodel.js`);
const {judge, listTimes, reading, remember, suspendedSeconds, wasSaid} = await import(`${UI}/power/sleptlogic.js`);

const say = globalThis.print ?? console.log;
let count = 0;
const failures = [];
function same(what, got, want) {
    count++;
    const a = JSON.stringify(got);
    const b = JSON.stringify(want);
    if (a !== b)
        failures.push(`${what}: got ${a}, wanted ${b}`);
}

// --- R7-37 -------------------------------------------------------------------------------------------------------------

same('never from the cache, DNS only', NAME_FLAGS, 4097);
same('no such name: a name server answered', nameFailure('org.freedesktop.resolve1.DnsError.NXDOMAIN'), 'ok');
same('its server failed', nameFailure('org.freedesktop.resolve1.DnsError.SERVFAIL'), 'fail');
same('no name server', nameFailure('org.freedesktop.resolve1.NoNameServers'), 'fail');
same('a timeout has no remote name', nameFailure(null), 'fail');
same('resolved is not there', nameFailure('org.freedesktop.DBus.Error.ServiceUnknown'), 'unknown');

const see = (router, name, connectivity = 'full', main = true) => {
    const {first, second, say: words} = verdict({router, name, connectivity, main});
    return `${first} ${second} ${words}`;
};
same('checking', see('wait', 'wait'), 'wait wait checking');
same('the router answered first', see('ok', 'wait'), 'ok wait checking');
same('all fine', see('ok', 'ok'), 'ok ok fine');
same('a router that ignores pings and a working name is fine', see('silent', 'ok'), 'ok ok fine');
same('a gateway-less network with a name that works is fine', see('none', 'ok'), 'ok ok fine');
same('beyond the router', see('ok', 'fail', 'limited'), 'ok bad beyond');
same('a router that answers at the link only is not blamed as silent', see('arp', 'fail', 'limited'), 'ok bad beyond');
same('the router', see('silent', 'fail', 'limited'), 'bad off router');
same('no gateway at all is not "the router does not answer"', see('none', 'fail', 'none'), 'bad off no-gateway');
same('a sign-in page', see('ok', 'ok', 'portal'), 'ok warn portal');
same('a sign-in page waits for the router alone', see('ok', 'wait', 'portal'), 'ok warn portal');
same('names work but the check does not', see('ok', 'ok', 'limited'), 'ok warn limited');
// The router answers and NetworkManager's check got through, but the lookup failed: encrypted DNS blocked, a VPN's DNS down
same('names fail while the internet answers', see('ok', 'fail', 'full'), 'ok warn dns');
same('and the router ignores pings', see('silent', 'fail', 'full'), 'ok warn dns');
same('no resolved, NetworkManager says full', see('silent', 'unknown', 'full'), 'ok ok fine');
same('no resolved, router silent: nobody knows', see('silent', 'unknown', 'unknown'), 'warn off silent');
same('no resolved, no gateway', see('none', 'unknown', 'unknown'), 'off off no-gateway');
same('no resolved, limited', see('ok', 'unknown', 'limited'), 'ok bad beyond');
same('no resolved, nothing known beyond', see('ok', 'unknown', 'unknown'), 'ok off router-only');
// The connectivity check is off: the name is not asked, and the router's silence is not a verdict
same('check off, router answers', see('ok', 'skip', 'unknown'), 'ok off router-only');
same('check off, router answers at the link', see('arp', 'skip', 'unknown'), 'ok off router-only');
same('check off, router silent', see('silent', 'skip', 'unknown'), 'warn off silent');
// A connection that is not the one the internet goes through: the first line alone
same('another connection, the router answers', see('ok', 'skip', 'unknown', false), 'ok off side-ok');
same('another connection, the router is silent', see('silent', 'skip', 'unknown', false), 'warn off side-silent');
same('another connection, no router', see('none', 'skip', 'unknown', false), 'off off side-none');
same('another connection, waiting', see('wait', 'skip', 'unknown', false), 'wait off checking');
same('another connection never gets a verdict about the internet', see('ok', 'fail', 'limited', false), 'ok off side-ok');
same('the name NetworkManager asks', hostOf('http://fedoraproject.org/static/hotspot.txt'), 'fedoraproject.org');
same('with a port and upper case', hostOf('https://Check.Example.ORG:8080/x'), 'check.example.org');
same('no name in a number', hostOf('http://192.0.2.1/check'), null);
same('not a web address', hostOf('file:///etc/passwd'), null);
same('a user in front is not a host', hostOf('http://user@evil.example/x'), null);
same('nothing', hostOf(''), null);
same('nothing at all', hostOf(null), null);
const ARP = 'IP address       HW type     Flags       HW address            Mask     Device\n' +
    '192.168.1.1      0x1         0x2         aa:bb:cc:dd:ee:ff     *        wlp2s0\n' +
    '192.168.1.7      0x1         0x0         00:00:00:00:00:00     *        wlp2s0\n' +
    '10.0.0.1         0x1         0x2         11:22:33:44:55:66     *        enp3s0\n';
same('the router answered at the link', arpAnswers(ARP, '192.168.1.1', 'wlp2s0'), true);
same('an entry still being asked for', arpAnswers(ARP, '192.168.1.7', 'wlp2s0'), false);
same('on another device', arpAnswers(ARP, '10.0.0.1', 'wlp2s0'), false);
same('not there', arpAnswers(ARP, '192.168.1.2', 'wlp2s0'), false);
same('no file', arpAnswers(null, '192.168.1.1', 'wlp2s0'), false);

// --- R7-38 -------------------------------------------------------------------------------------------------------------

same('a stat line', parseStat('  4 0 12 0 9 8 7000 6 0 3 4 0 0 0 0 0 0\n'), {written: 7000});
same('a short stat line', parseStat('4 0 12'), null);
same('nothing', parseStat(''), null);
same('meminfo', parseDirty('MemTotal: 1 kB\nDirty:         100 kB\nWriteback:       20 kB\nAnonPages: 3 kB\n'), 120);
same('meminfo without them', parseDirty('MemTotal: 1 kB\n'), null);
same('a device', blockName('/dev/sdb1'), 'sdb1');
same('a mapped device', blockName('dm-3'), 'dm-3');
same('not a device', blockName('/dev/../etc/passwd'), null);

// Steps: a reading {written, dirty}, or 'flushed' (the sync came back), 'clean' / 'held' / 'unknown' (what holds the disk),
// 'timeout' (the sync never came back). Each gives the state after it, with ! when a sync is asked for, * when the holders are.
function run(steps, options) {
    const watch = new WriteWatch(options);
    return steps.map(step => {
        if (step === 'flushed') {
            watch.flushed(true);
        } else if (step === 'clean') {
            watch.verified(true);
        } else if (step === 'held') {
            watch.verified(false);
        } else if (step === 'unknown') {
            watch.verified(null);
        } else if (step === 'timeout') {
            watch.timedOut();
        } else {
            const told = watch.step(step);
            return told.flush ? `${watch.state}!` : told.verify ? `${watch.state}*` : told.nudge ? `${watch.state}?` : watch.state;
        }
        return watch.state;
    });
}
const r = (written, dirty = 1000) => ({written, dirty});
same('quiet stays quiet', run([r(0), r(0), r(0)]), ['quiet', 'quiet', 'quiet']);
same('a few sectors say nothing', run([r(0), r(8), r(16), r(16)]), ['quiet', 'quiet', 'quiet', 'quiet']);
same('a real write, settled, proved, held by nobody', run([r(0), r(4096), r(8192), r(8192), r(8192), 'flushed', r(8192), 'clean']),
    ['quiet', 'writing', 'writing', 'writing', 'flushing!', 'checking', 'verifying*', 'done']);
same('a sync that wrote is followed by another', run([r(0), r(4096), r(4096), r(4096), 'flushed', r(9000), 'flushed', r(9000), 'clean']),
    ['quiet', 'writing', 'writing', 'flushing!', 'checking', 'flushing!', 'checking', 'verifying*', 'done']);
same('sectors moving while it syncs are not proof', run([r(0), r(4096), r(4096), r(4096), r(5000), 'flushed', r(5000)]),
    ['quiet', 'writing', 'writing', 'flushing!', 'flushing', 'checking', 'flushing!']);
// The review's case: the copy goes on, its data in memory, while the last sectors of a sync land
same('dirty data climbing during the sync is not done', run([r(0, 30000), r(4096, 30000), r(4096, 30000), r(4096, 30000),
    r(4096, 60000), 'flushed', r(4096, 120000), r(4096, 200000)]),
['quiet', 'writing', 'writing', 'flushing!', 'flushing', 'checking', 'writing', 'writing']);
same('dirty data climbing only after the sync is not done', run([r(0, 30000), r(4096, 30000), r(4096, 30000), r(4096, 30000),
    r(4096, 30000), 'flushed', r(4096, 60000)]),
['quiet', 'writing', 'writing', 'flushing!', 'flushing', 'checking', 'writing']);
same('dirty data rising while the holders are asked', run([r(0), r(4096), r(4096), r(4096), 'flushed', r(4096), r(4096, 9000), 'clean']),
    ['quiet', 'writing', 'writing', 'flushing!', 'checking', 'verifying*', 'writing', 'writing']);
same('a second copy while it says done starts it again', run([r(0, 30000), r(4096, 30000), r(4096, 30000), r(4096, 30000), 'flushed',
    r(4096, 30000), 'clean', r(4096, 30000), r(4096, 80000)]),
['quiet', 'writing', 'writing', 'flushing!', 'checking', 'verifying*', 'done', 'done', 'writing']);
same('a second copy that moves a sector starts it again', run([r(0), r(4096), r(4096), r(4096), 'flushed', r(4096), 'clean', r(6000)]),
    ['quiet', 'writing', 'writing', 'flushing!', 'checking', 'verifying*', 'done', 'writing']);
same('a few KiB of the machine\'s own noise does not undo it', run([r(0, 30000), r(4096, 30000), r(4096, 30000), r(4096, 30000), 'flushed',
    r(4096, 29000), 'clean', r(4096, 29500)]),
['quiet', 'writing', 'writing', 'flushing!', 'checking', 'verifying*', 'done', 'done']);
same('something holds the disk: not done', run([r(0), r(4096), r(4096), r(4096), 'flushed', r(4096), 'held', r(4096)]),
    ['quiet', 'writing', 'writing', 'flushing!', 'checking', 'verifying*', 'held', 'held']);
same('what holds it cannot be read: nothing is claimed', run([r(0), r(4096), r(4096), r(4096), 'flushed', r(4096), 'unknown', r(4096)]),
    ['quiet', 'writing', 'writing', 'flushing!', 'checking', 'verifying*', 'unproven', 'unproven']);
same('no reading of the machine\'s dirty data: nothing is claimed', run([r(0), r(4096), r(4096), r(4096), 'flushed', {written: 4096, dirty: null}]),
    ['quiet', 'writing', 'writing', 'flushing!', 'checking', 'unproven']);
same('a sync that never comes back is not waited for', run([r(0), r(4096), r(4096), r(4096), r(4096), 'timeout', r(4096), r(4096)]),
    ['quiet', 'writing', 'writing', 'flushing!', 'flushing', 'unproven', 'unproven', 'unproven']);
same('a FUSE file system never says done', run([r(0), r(4096), r(4096), r(4096), 'flushed', r(4096), r(4096), r(4096)], {proof: false}),
    ['quiet', 'writing', 'writing', 'flushing!', 'checking', 'unproven', 'unproven', 'unproven']);
same('and starts again when written to', run([r(0), r(4096), r(4096), r(4096), 'flushed', r(4096), r(9000)], {proof: false}),
    ['quiet', 'writing', 'writing', 'flushing!', 'checking', 'unproven', 'writing']);
same('a file system a sync reaches', [provable('exfat'), provable('vfat'), provable('ext4'), provable('btrfs')], [true, true, true, true]);
same('one it does not, or not known', [provable('fuseblk'), provable('fuse.ntfs-3g'), provable(''), provable(null)], [false, false, false, false]);
same('dirty data rising nudges once', run([r(0, 1000), r(0, 12000), r(0, 13000), r(0, 24000)]),
    ['quiet', 'quiet?', 'quiet', 'quiet?']);
same('a failed sync goes on writing', (() => {
    const watch = new WriteWatch();
    for (const step of [r(0), r(4096), r(4096), r(4096)])
        watch.step(step);
    watch.flushed(false);
    return watch.state;
})(), 'writing');
{
    // The holders went: all of it is proved again, not taken as done
    const watch = new WriteWatch();
    for (const step of [r(0), r(4096), r(4096), r(4096)])
        watch.step(step);
    watch.flushed(true);
    watch.step(r(4096));
    watch.verified(false);
    same('not released while not held', [new WriteWatch().release(), watch.state], [false, 'held']);
    same('released: a sync again', [watch.release(), watch.state], [true, 'flushing']);
    watch.flushed(true);
    same('and then the question again', [watch.step(r(4096)).verify, watch.state], [true, 'verifying']);
    watch.verified(true);
    same('and done', watch.state, 'done');
}
{
    // A slow source: data reaches memory at 3 MB a second and the kernel writes it late. Every sync that writes is followed
    // by another and the rising dirty data undoes each proof: it is never "all on" while the copy goes on.
    const watch = new WriteWatch();
    let written = 0;
    let dirty = 1000;
    const seen = new Set();
    watch.step(r(written, dirty));
    for (let tick = 0; tick < 60; tick++) {
        dirty += 3000;
        if (tick === 0)
            written = 4096;
        const told = watch.step(r(written, dirty));
        if (told.flush) {
            written += (dirty - 1000) * 2;
            dirty = 1000;
            watch.flushed(true);
        }
        if (told.verify)
            watch.verified(true);
        seen.add(watch.state);
    }
    same('a slow source is never all on', [seen.has('done'), seen.has('verifying')], [false, false]);
}
{
    // A dirty reading that falls and rises: the percent never goes down, within one copy
    const watch = new WriteWatch();
    watch.step(r(0, 1000));
    watch.step(r(4096, 1000 + 4 * 1024));
    const percents = [watch.progress.percent];
    for (const dirty of [1000 + 100 * 1024, 1000 + 2 * 1024, 1000, 1000 + 200 * 1024, 1000 + 1024]) {
        watch.step(r(4096, dirty));
        percents.push(watch.progress.percent);
    }
    same('the percent never goes back', percents.every((p, i) => i === 0 || p >= percents[i - 1]), true);
    same('it moved', percents.at(-1) > percents[1], true);
}
{
    const watch = new WriteWatch();
    watch.step(r(0, 1000));
    watch.step(r(2048, 1000 + 3 * 1024));
    const {written, left, percent} = watch.progress;
    same('written', written, 2048 * 512);
    same('left is the rise since it was quiet', left, 3 * 1024 * 1024);
    same('its percent', percent, 25);
}
{
    const watch = new WriteWatch();
    watch.step(r(0, 1000));
    watch.step(r(2048, 500));
    same('never below nothing', watch.progress.left, 0);
    same('never 100 before it is proved', watch.progress.percent, 99);
}
{
    // A quiet disk is read only when the machine's dirty data may be for it
    const watch = new WriteWatch();
    same('the first reading is always taken', watch.peek(1000), true);
    watch.step(r(0, 1000));
    same('nothing has risen', watch.peek(1100), false);
    same('a quarter of a MiB has', watch.peek(1000 + 256), true);
    same('what fell is the new lowest', [watch.peek(500), watch.peek(600)], [false, false]);
    same('and a rise past it is seen', watch.peek(500 + 300), true);
}

// --- R7-39 -------------------------------------------------------------------------------------------------------------

same('seconds asleep', suspendedSeconds('5000.50 1.0\n', 1000.5e6), 4000);
same('unreadable', suspendedSeconds('nothing', 1), 0);
const night = judge({percent: 90, suspended: 0}, {percent: 72, suspended: 8 * 3600});
same('18 points overnight', [night.large, night.drop], [true, 18]);
same('a night at a normal rate', judge({percent: 90, suspended: 0}, {percent: 80, suspended: 10 * 3600}).large, false);
same('a small drop', judge({percent: 90, suspended: 0}, {percent: 85, suspended: 3600}).large, false);
same('a short sleep', judge({percent: 90, suspended: 0}, {percent: 70, suspended: 600}).large, false);
same('it charged', judge({percent: 50, suspended: 0}, {percent: 90, suspended: 36000}).large, false);
same('no sleep', judge({percent: 90, suspended: 100}, {percent: 50, suspended: 100}).large, false);

const usb = {id: 'usb:046d:c52b', kind: 'usb', name: 'Receiver', can_stop: true};
const radio = {id: 'usb:8087:0aaa', kind: 'bluetooth', name: 'Radio', can_stop: true};
const lid = {id: 'lid', kind: 'lid', name: 'PNP0C0D:00', can_stop: false};
const hub = {id: 'usb:05e3:0610', kind: 'hub', name: 'Hub', can_stop: false};
const controller = {id: 'pci:8086:a0ed', kind: 'controller', name: 'PCI 8086:a0ed', can_stop: false};
const timer = {id: 'timer', kind: 'timer', name: 'rtc0', can_stop: false};
const fresh = {readable: true, last_wake_ago_s: 4};
const read = (why, field = 'say') => reading(why)[field];
same('woken, named', read({...fresh, wakes: [1, 2, 3], sources: [usb]}), 'woken');
same('the wake that opened it is the persons own', reading({...fresh, wakes: [1, 2, 3], sources: [usb]}).times, [1, 2]);
same('woken, nothing named', read({...fresh, wakes: [1, 2], sources: []}), 'woken-unnamed');
same('the lid is never blamed', read({...fresh, wakes: [1, 2], sources: [lid]}), 'woken-unnamed');
same('a hub is never blamed', read({...fresh, wakes: [1, 2], sources: [hub, radio]}, 'culprit')?.id, 'usb:8087:0aaa');
same('nor a USB controller', read({...fresh, wakes: [1, 2], sources: [controller]}), 'woken-unnamed');
same('a timer is named, not stopped', [read({...fresh, wakes: [1, 2], sources: [timer]}), reading({...fresh, wakes: [1, 2], sources: [timer]}).culprit.can_stop], ['woken', false]);
same('only the person woke it', read({...fresh, wakes: [9], sources: [usb]}), 'asleep');
same('and nobody is blamed then', reading({...fresh, wakes: [9], sources: [usb]}).culprit, null);
// Nothing known is not "nothing woke it"
same('a journal that cannot be read', read({readable: false, wakes: [], sources: []}), 'silent');
same('and never names a device then', reading({readable: false, wakes: [], last_wake_ago_s: null, sources: [usb]}).culprit, null);
same('a journal without the record', read({...fresh, wakes: [], sources: []}), 'silent');
same('the last wake long ago may have been the computers own', read({readable: true, last_wake_ago_s: 1500, wakes: [9], sources: []}), 'silent');
same('and then nothing is named', reading({readable: true, last_wake_ago_s: 1500, wakes: [1, 9], sources: [usb]}).culprit, null);
same('a wake time that is not a number', read({readable: true, last_wake_ago_s: null, wakes: [9], sources: []}), 'silent');
same('nothing at all', read(null), 'silent');
same('few times are listed', listTimes([1, 2, 3, 4, 5, 6]), [1, 2, 3, 4, 5, 6]);
same('many are only counted', listTimes([1, 2, 3, 4, 5, 6, 7]), null);

// Across a lock: the readings are taken as it goes to sleep and as it wakes, so the time between them is seconds
same('a lock sitting 25 minutes awake before it slept is not charged to sleep (the reading is now taken at the sleep)',
    judge({percent: 90, suspended: 0, mono: 0}, {percent: 60, suspended: 2100, mono: 25 * 60 * 1e6}).large, false);
same('readings seconds apart', judge({percent: 90, suspended: 0, mono: 0}, {percent: 72, suspended: 8 * 3600, mono: 4 * 1e6}).large, true);

let state = {};
same('not said yet', wasSaid(state, 'a'), false);
state = remember(state, 'a');
same('said', wasSaid(state, 'a'), true);
same('said once', remember(state, 'a').said, ['a']);
same('the salt stays', remember({salt: 'x', said: []}, 'a').salt, 'x');
same('and is not made up', remember({said: []}, 'a').salt, undefined);
for (let i = 0; i < 40; i++)
    state = remember(state, `m${i}`);
same('a bounded list', state.said.length, 32);
same('a bad file is nothing', wasSaid('nonsense', 'a'), false);

if (failures.length > 0) {
    for (const failure of failures)
        say(failure);
    throw new Error(`${failures.length} of ${count} checks failed`);
}
say(`test-truth: ${count} checks`);
