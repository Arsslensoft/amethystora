// The lab's half of live.sh (.claude/skills/run-tests/live-fix.md): a kept VM of a run's image, its /usr made
// writable for this boot, files of the checkout put in and what reads them reloaded, one suite run against it.
// live.sh copies it to /vm/live (with lab.mjs and agent.py) and runs it there, beside a folder for each VM (/vm/live/VM): files/ and push.new
// (what live.sh sent), push.list (every file pushed since the last reset), suites/, runs/ and disk.
//
//   node live.mjs VM start DISK [--fresh] | ready | reset | unlock | push | relogin | suite NAME | app | exec ... | shot | stop
//
// It uses the lab's tools (lab.mjs) as a test run does, and never a run's own VMs, disks or source tree.
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

import { spawn } from 'node:child_process';

// The checkout's lab.mjs, which live.sh copies beside this file with agent.py: not a run's copy in /opt
import { agent, call, decode, qmp, refusal, screendump } from './lab.mjs';

const [vm, command, ...args] = process.argv.slice(2);
if (!/^[a-z0-9][a-z0-9-]{0,30}$/.test(vm || '')) throw new Error(`live.mjs: no VM name ${vm}`);
// A run's own VM (run.sh test --keep) can be pushed into and tested, but never started, reset or deleted from here
if (/^test(-installer|-screens)?$/.test(vm) && ['start', 'ready', 'reset', 'delete'].includes(command)) {
  throw new Error(`live.mjs: ${vm} is a test run's VM: ${command} it with run.sh, or start one of your own`);
}
const DIR = `/vm/live/${vm}`;
const GUEST = '/run/amethystora-tests';
const PASSPHRASE = 'amethystora-lab-passphrase';
const TPM_PIN = '246813579';
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const began = Date.now();
const say = (line) => console.log(`[${((Date.now() - began) / 1000).toFixed(0).padStart(4)} s] ${line}`);
fs.mkdirSync(DIR, { recursive: true });

async function sh(cmd, timeout = 300, extra = {}) {
  const a = await agent(vm, { op: 'exec', command: cmd, timeout, ...extra }, timeout);
  return { rc: a.rc, out: decode(a.stdout), err: `${decode(a.stderr)}${a.error || ''}` };
}
async function root(cmd, timeout = 300) {
  const r = await sh(cmd, timeout);
  if (r.rc !== 0) throw new Error(`in ${vm}: ${cmd.split('\n')[0]}: exit ${r.rc}\n${r.out}${r.err}`);
  return r.out;
}
async function write(target, data, mode = '0644') {
  const a = await agent(vm, { op: 'write', path: target, data: data.toString('base64'), mode });
  if (a.rc !== 0) throw new Error(`writing ${target}: ${a.error}`);
}

// /usr writable until the next restart, which throws it away: bootc's development mode. The image's own list of
// extensions, as the build left it, kept first for reload()
async function unlock() {
  await root(`[ "$(findmnt -n -o FSTYPE /usr)" = overlay ] || bootc usr-overlay >/dev/null
    [ -e /run/live-enabled ] || sed -n "s/^enabled-extensions *= *//p" /usr/share/glib-2.0/schemas/zz0-amethystora-modifications.gschema.override |
      grep -o "'[^']*'" | tr -d "'" >/run/live-enabled`);
}

// The test account's session, a new one when its shell was BEFORE, once the shell answers on the bus
async function session(before = '', wait = 300000) {
  for (const until = Date.now() + wait; Date.now() < until; await sleep(3000)) {
    const r = await sh('[ "$(loginctl show-user tester -p State --value 2>/dev/null)" = active ] && pgrep -u tester -x gnome-shell', 20);
    const pid = r.out.trim();
    if (r.rc === 0 && pid && pid !== before) {
      await sh(`for i in $(seq 60); do runuser -u tester -- env DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$(id -u tester)/bus \
        gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.freedesktop.DBus.Properties.Get \
        org.gnome.Shell OverviewActive >/dev/null 2>&1 && break; sleep 1; done; sleep 5`, 90);
      return;
    }
  }
  throw new Error(`the test account got no session within ${Math.round(wait / 60000)} minutes`);
}
const hasAccount = async () => (await sh('grep -qs "^AutomaticLogin=tester" /etc/gdm/custom.conf', 20)).rc === 0;
async function relogin() {
  const before = (await sh('pgrep -u tester -x gnome-shell', 20)).out.trim();
  await root('systemctl restart gdm');
  // GDM 51 now and then shows its greeter instead of signing the account in again (seen on Fedora 45): once more
  try {
    await session(before, 90000);
  } catch {
    say('GDM showed its greeter, not the session: restarting it once more');
    await root('systemctl restart gdm');
    await session(before);
  }
}

// Once the boot is over: /usr unlocked, the session there, and every file pushed since the last reset put back
async function boot() {
  say((await call('vm_wait', { name: vm, timeout_s: 900 })).content[0].text);
  await unlock();
  // A session that does not come still has what was pushed put back first, or every suite after would run on the
  // image's own files (seen after 91's restart)
  let late = null;
  if (await hasAccount()) await session().catch((e) => { late = e; });
  if (fs.existsSync(`${DIR}/push.list`)) {
    // Only what the restart threw away: /etc is on the disk and keeps what was pushed, and what a suite changed since
    // (security.conf put back over a profile a suite turned on before its restart)
    say('putting back the files pushed before the restart');
    await apply(readList(`${DIR}/push.list`), { usrOnly: true });
  }
  if (fs.existsSync(`${DIR}/apps.installed`)) {
    say('putting back the apps program live.sh app built');
    await installApps();
  }
  if (late) throw late;
}
// The signed-in state `ready` kept: a stopped VM of its own, VM-ready, which a fresh start of VM is cloned from
// (vm_start's from: an overlay on its overlay, its firmware variables and TPM copied), as run.sh --parallel does
const READY = `${vm}-ready`;
const hasReady = () => fs.existsSync(`/vm/vms/${READY}.qcow2`);
async function startVm(fresh = false) {
  const disk = fs.readFileSync(`${DIR}/disk`, 'utf8').trim();
  // VMLAB_MEM: a guest smaller than the lab's 4096 MB, so that a third VM fits beside two
  const memory = Number(process.env.VMLAB_MEM) || undefined;
  say((await call('vm_start', { name: vm, disk, fresh, ...(memory ? { memory_mb: memory } : {}), ...(fresh && hasReady() ? { from: READY } : {}) })).content[0].text);
  await boot();
}
// QEMU started by restore, waiting for its state: -incoming on its command line
const restored = () => {
  try {
    return fs.readFileSync(`/proc/${Number(fs.readFileSync(`/vm/vms/${vm}.pid`, 'utf8'))}/cmdline`, 'latin1').split('\0').includes('-incoming');
  } catch {
    return false;
  }
};
const running = () => {
  try {
    return fs.existsSync(`/proc/${Number(fs.readFileSync(`/vm/vms/${vm}.pid`, 'utf8'))}`);
  } catch {
    return false;
  }
};
// Powered off from inside, as the run's restarts are: vm_stop's power button has GNOME lock the session, and the
// next boot of that disk then came up locked behind a greeter. With force, the plug pulled.
async function stopVm(force) {
  if (!running()) return;
  if (!force) {
    // The agent answers one client at a time: while a suite runs it cannot hear this, and QEMU's power button does
    // (good enough to stop; ready, which keeps the state, runs with nothing else on the VM)
    const asked = await agent(vm, { op: 'exec', command: '(sleep 1; systemctl poweroff) >/dev/null 2>&1 &' }, 5).then(() => true, () => false);
    if (!asked) await qmp(vm, 'system_powerdown', {}).catch(() => {});
    for (let i = 0; i < 120 && running(); i++) await sleep(1000);
  }
  if (running()) await call('vm_stop', { name: vm, force: true });
}
// The apps' one program (live.sh app built it into DIR/amethystora-apps), hard-linked to each app's path as 13-apps.sh
// does, and the agent's tools when a build of them was left beside it; each written beside its place first, since a
// running program's file cannot be written over
async function installApps() {
  await write('/usr/lib/amethystora-apps/amethystora-apps.live', fs.readFileSync(`${DIR}/amethystora-apps`), '0755');
  await root(`mv -f /usr/lib/amethystora-apps/amethystora-apps.live /usr/lib/amethystora-apps/amethystora-apps
    for app in manual security update notes logs backups control phone shelf trim; do
      mkdir -p /usr/lib/amethystora-$app
      ln -f /usr/lib/amethystora-apps/amethystora-apps /usr/lib/amethystora-$app/amethystora-$app; done
    restorecon -F /usr/lib/amethystora-apps/amethystora-apps`);
  if (fs.existsSync(`${DIR}/amethystora-agent-tools`)) {
    await write('/usr/libexec/amethystora-agent-tools.live', fs.readFileSync(`${DIR}/amethystora-agent-tools`), '0755');
    await root('mv -f /usr/libexec/amethystora-agent-tools.live /usr/libexec/amethystora-agent-tools && restorecon -F /usr/libexec/amethystora-agent-tools');
  }
}
function forget() {
  for (const f of ['push.list', 'push.new', 'files', 'apps.installed']) fs.rmSync(`${DIR}/${f}`, { recursive: true, force: true });
}

// --- Pushing files ----------------------------------------------------------------------------------------------

// MODE PATH lines, MODE 0755, 0644 or - for a file the checkout no longer has
const readList = (file) => fs.readFileSync(file, 'utf8').split('\n').filter(Boolean).map((l) => {
  const [mode, ...rest] = l.split(' ');
  return { mode, file: rest.join(' ') };
});
// Where a file of the checkout is in the guest: system_files where the build copies them (/etc also in /usr/etc,
// where the image keeps its default), the suites beside lib.sh
function targets(file) {
  let m = file.match(/^system_files\/(?:shared|dx)\/(.+)$/);
  if (m) return m[1].startsWith('etc/') ? [`/${m[1]}`, `/usr/${m[1]}`] : [`/${m[1]}`];
  m = file.match(/^tests\/vm\/suites\/(.+)$/);
  if (m) return [`${GUEST}/${m[1]}`];
  throw new Error(`${file}: live.sh puts in only system_files and tests/vm/suites; the rest is build-time`);
}
// The checkout is CRLF on Windows: text (no NUL byte) goes in with LF endings
function lf(data) {
  return data.includes(0) ? data : Buffer.from(data.toString('latin1').replace(/\r\n/g, '\n'), 'latin1');
}

// Files into the guest ([{target, data, mode}]) as one tarball of plain files: no folder entries, so no folder's
// mode or label changes; each file labelled as the policy labels its path. In parts of 4 MB: the guest's helper
// reads a request whole, and a large one slowly.
async function send(files) {
  if (!files.length) return;
  const stage = fs.mkdtempSync('/tmp/live-');
  try {
    for (const { target, data, mode } of files) {
      const out = path.join(stage, target);
      fs.mkdirSync(path.dirname(out), { recursive: true });
      fs.writeFileSync(out, data);
      fs.chmodSync(out, Number.parseInt(mode, 8));
    }
    fs.writeFileSync(`${stage}.list`, files.map((f) => `${f.target.slice(1)}\n`).join(''));
    execFileSync('tar', ['-czf', `${stage}.tgz`, '--owner=0', '--group=0', '-C', stage, '-T', `${stage}.list`]);
    const tgz = fs.readFileSync(`${stage}.tgz`);
    const parts = Math.max(1, Math.ceil(tgz.length / (4 << 20)));
    for (let i = 0; i < parts; i++) await write(`/run/live-push.${i}`, tgz.subarray(i * (4 << 20), (i + 1) * (4 << 20)));
    await write('/run/live-push.list', Buffer.from(files.map((f) => `${f.target}\n`).join('')));
    await root(`cat $(seq -f /run/live-push.%g 0 ${parts - 1}) | tar -xz -C /
      rm -f /run/live-push.[0-9]*
      xargs restorecon -F </run/live-push.list`, 600);
  } finally {
    for (const f of [stage, `${stage}.list`, `${stage}.tgz`]) fs.rmSync(f, { recursive: true, force: true });
  }
}

// What live.sh sent, into the guest, then what reads it reloaded
async function apply(entries, { usrOnly = false } = {}) {
  await unlock();
  const files = [];
  const removed = [];
  for (const { mode, file } of entries) {
    for (const target of targets(file).filter((t) => !usrOnly || !t.startsWith('/etc/'))) {
      if (mode === '-') removed.push(target);
      else files.push({ target, data: lf(fs.readFileSync(`${DIR}/files/${file}`)), mode });
    }
  }
  await send(files);
  if (removed.length) {
    await write('/run/live-remove.list', Buffer.from(`${removed.join('\n')}\n`));
    await root('xargs -r rm -f </run/live-remove.list');
  }
  say(`${files.length} file(s) written, ${removed.length} removed`);
  await reload([...files.map((f) => f.target), ...removed]);
}

// What reads each kind of file, as the build or a boot would have it read
async function reload(files) {
  const any = (re) => files.some((f) => re.test(f));
  const steps = [];
  if (any(/^\/usr\/share\/(glib-2\.0|gnome-shell\/extensions\/[^/]+)\/schemas\//)) {
    // build-gnome-extensions.sh: the image's own extensions added to zz0's enabled-extensions (its source names one)
    // in the order the build added them, Amethystora UI's schemas system-wide too, each extension's own compiled with its sections of the image's
    // overrides (getSettings() reads those first), then the system's
    steps.push(`o=/usr/share/glib-2.0/schemas/zz0-amethystora-modifications.gschema.override
      for u in $(cat /run/live-enabled; cd /usr/share/gnome-shell/extensions && ls -d -- *@iarsslen.github.io); do
        [ ! -d "/usr/share/gnome-shell/extensions/$u" ] || grep -q "'$u'" "$o" || sed -i "/^enabled-extensions/ s/\\]/, '$u'&/" "$o"
      done
      ui=/usr/share/gnome-shell/extensions/amethystora-ui@iarsslen.github.io/schemas
      install -pm0644 -t /usr/share/glib-2.0/schemas "$ui"/*.gschema.xml
      for s in /usr/share/gnome-shell/extensions/*/schemas; do
        ids="$(cat "$s"/*.gschema.xml | tr -s '[:space:]' ' ' | grep -oE '<schema [^>]*id="[^"]+"' | sed -E 's/.*id="([^"]+)".*/\\1/' | tr '\\n' ' ')"
        awk -v ids="$ids" 'BEGIN { n = split(ids, l, " "); for (i = 1; i <= n; i++) own["[" l[i] "]"] = 1 } /^\\[/ { keep = ($0 in own) } keep' \\
          /usr/share/glib-2.0/schemas/zz0-amethystora-modifications.gschema.override \\
          /usr/share/glib-2.0/schemas/zz1-amethystora-modifications.gschema.override >"$s/zz-amethystora-modifications.gschema.override"
        glib-compile-schemas --strict "$s" || echo "schemas: $s did not compile"
        rm -f "$s/zz-amethystora-modifications.gschema.override"
      done
      glib-compile-schemas /usr/share/glib-2.0/schemas && echo "schemas compiled"`);
  }
  if (any(/^\/etc\/dconf\//)) steps.push('dconf update && echo "dconf updated"');
  for (const pack of new Set(files.map((f) => f.match(/^\/usr\/share\/icons\/([^/]+)\//)?.[1]).filter(Boolean))) {
    steps.push(`[ ! -e /usr/share/icons/${pack}/index.theme ] || { gtk-update-icon-cache -qf /usr/share/icons/${pack} && echo "icon cache ${pack}"; }`);
  }
  if (any(/^\/(usr\/lib|etc)\/systemd\/system\//)) steps.push('systemctl daemon-reload && echo "systemd reloaded: restart the unit"');
  if (any(/^\/(usr\/lib|etc)\/systemd\/user\//)) steps.push('echo "user units: read at the next login (live.sh relogin)"');
  // The file-watch sensor refuses a manual stop (RefuseManualStop): its process ended, systemd starts the new one
  if (any(/^\/usr\/libexec\/amethystora-sensor$/)) {
    steps.push(`old="$(systemctl show -P MainPID amethystora-sensor.service)"
      if [ "$old" != 0 ]; then kill "$old"
        for i in $(seq 30); do n="$(systemctl show -P MainPID amethystora-sensor.service)"; [ "$n" != 0 ] && [ "$n" != "$old" ] &&
          systemctl is-active -q amethystora-sensor.service && break; sleep 1; done
        echo "sensor restarted: $(systemctl is-active amethystora-sensor.service), pid $old -> $n"
      fi`);
  }
  if (steps.length) say((await root(steps.join('\n'), 600)).trim());
  const signedIn = await hasAccount();
  if (signedIn && any(/^\/usr\/(share\/amethystora\/(themed|themes)\/|lib\/amethystora\/theme\/|bin\/amethystora-theme$)/)) {
    const r = await sh('amethystora-theme reload', 300, { user: 'tester', session: true });
    say(`theme reloaded (exit ${r.rc})${r.rc ? `\n${r.out}${r.err}` : ''}`);
  }
  if (signedIn && any(/^\/usr\/share\/(gnome-shell\/extensions\/|glib-2\.0\/schemas\/)/)) {
    say('the session started again for the extensions');
    await relogin();
  }
  if (any(/^\/usr\/share\/applications\//)) say('launchers go in untranslated: translate-desktop.py is build-time');
}

// --- A suite ----------------------------------------------------------------------------------------------------

async function suite(name) {
  const names = fs.readdirSync(`${DIR}/suites`).filter((f) => /^\d\d-.+\.sh$/.test(f));
  const chosen = names.filter((f) => f === name || f === `${name}.sh`);
  if (!chosen.length) chosen.push(...names.filter((f) => f.startsWith(`${name}-`)));
  if (chosen.length !== 1) throw new Error(`suite ${name}: ${chosen.length ? `which one? ${chosen.join(' ')}` : 'none'}`);
  const file = chosen[0];
  // The checkout's suites and lib.sh, as run.mjs puts them in
  const install = async () => {
    await root(`rm -rf ${GUEST} && mkdir -p ${GUEST}`);
    await send(fs.readdirSync(`${DIR}/suites`).filter((f) => fs.statSync(`${DIR}/suites/${f}`).isFile())
      .map((f) => ({ target: `${GUEST}/${f}`, data: lf(fs.readFileSync(`${DIR}/suites/${f}`)), mode: '0755' })));
  };
  await install();
  fs.mkdirSync(`${DIR}/runs`, { recursive: true });
  const log = `${DIR}/runs/${file}.log`;
  fs.writeFileSync(log, '');
  let output = '';
  let rc = 0;
  const t = Date.now();
  for (let phase = 1; phase <= 16; phase++) {
    const env = `SUITE_PHASE=${phase} VMLAB_PASSPHRASE=${PASSPHRASE} VMLAB_TPM_PIN=${TPM_PIN}`;
    const a = await agent(vm, { op: 'exec', command: `${env} bash ${GUEST}/${file}`, timeout: 1800 }, 1800);
    output += decode(a.stdout);
    fs.appendFileSync(log, `--- phase ${phase}, exit ${a.rc}${a.error ? ` (${a.error})` : ''}\n${decode(a.stdout)}${a.stderr ? `--- stderr\n${decode(a.stderr)}` : ''}`);
    rc = a.rc;
    if (rc === 101) continue;
    if (rc !== 100) break;
    // A restart throws the unlocked /usr away: boot() unlocks it again and puts the pushed files back. A plain
    // restart only: a suite asking next_boot for a power cycle with disks or credentials needs run.mjs
    if ((await sh('test -e /var/lib/vmlab-tests/next-boot.json', 20)).rc === 0) {
      throw new Error(`${file} asked for a power cycle (next_boot) before phase ${phase + 1}: run it with run.sh test --suites`);
    }
    say(`${file}: restarting for phase ${phase + 1}`);
    // QEMU's uefi-vars device does not live through the firmware running again on a reset: a QEMU that took its state
    // from a restore (-incoming) dies in it (signal 11 in hw-uefi-vars.so), on a power-off from inside too, and a
    // fresh one of 3 GB stopped in the firmware (#GP in VirtMmCommunication) as it restarted. So every restart is QEMU
    // quitting (which writes the variables out) and starting anew: a fresh one powered off from inside, which runs the
    // shutdown a restart runs, a restored one after a sync, as a power cut
    const restore = restored();
    if (restore) await sh('sync', 120);
    await stopVm(restore);
    // Its memory may take a while to be given back, or another VM to leave room: up to 20 minutes
    for (let i = 0; ; i++) {
      try {
        await startVm(false);
        break;
      } catch (error) {
        if (i >= 20 || !/not enough memory/.test(error.message)) throw error;
        say(`${error.message}: trying again in a minute`);
        await sleep(60000);
      }
    }
    await install();
  }
  fs.writeFileSync(`${DIR}/runs/${file}.png`, await screendump(vm));
  const lines = output.split('\n');
  const failures = [];
  lines.forEach((line, i) => {
    if (!line.startsWith('not ok')) return;
    const detail = [];
    for (let j = i + 1; j < lines.length && lines[j].startsWith('  '); j++) detail.push(lines[j]);
    failures.push([line, ...detail].join('\n'));
  });
  const count = (p) => lines.filter((l) => l.startsWith(p)).length;
  say(`${file}: ${rc === 0 && !failures.length ? 'passed' : 'FAILED'} (${count('ok')} ok, ${failures.length} not ok, ` +
    `${count('skip')} skipped, exit ${rc}, ${((Date.now() - t) / 60000).toFixed(1)} min)`);
  for (const f of failures) console.log(f);
  for (const l of lines.filter((x) => x.startsWith('skip') || x.startsWith('#'))) console.log(l);
  say(`log ${log}, screenshot ${DIR}/runs/${file}.png`);
  return rc === 0 && !failures.length;
}

// --- Commands ---------------------------------------------------------------------------------------------------

let passed = true;
switch (command) {
  case 'start':
    if (!args[0]) throw new Error('start DISK [--fresh]');
    fs.writeFileSync(`${DIR}/disk`, args[0]);
    if (args.includes('--fresh')) forget();
    await startVm(args.includes('--fresh'));
    break;
  case 'ready':
    // The test account signed in as a run's 10-account.sh does, then kept as VM-ready for reset. Not over an existing
    // VM-ready: this VM's overlay may be on top of it
    if (hasReady()) throw new Error(`${READY} exists: VMLAB_LIVE=${READY} live.sh delete, then live.sh start DISK --fresh`);
    if (!(await suite('10'))) throw new Error('10-account.sh failed: no ready state kept');
    // ...once the first boot's Flatpaks are in: kept half done, every start after a reset installs them again, and
    // the account's shell then starts so slowly that GDM puts its greeter in front and the session locks
    say('waiting for flatpak-preinstall to finish');
    // In short calls: QEMU's socket takes one client at a time, and a long one would shut every other out
    for (const until = Date.now() + 3600000; Date.now() < until; await sleep(15000)) {
      // It stays "active (exited)" once done (RemainAfterExit): its sub-state says when it has ended
      if (!/^(running|start|auto-restart)$/.test((await sh('systemctl show -P SubState flatpak-preinstall.service', 20)).out.trim())) break;
    }
    say(`flatpak-preinstall: ${(await sh('systemctl show -P SubState flatpak-preinstall.service; flatpak list --app --columns=application | wc -l', 30)).out.trim().replace('\n', ', apps: ')}`);
    await stopVm(false);
    // VM's files become VM-ready's (the disk under its overlay stays where it is), never started again
    await call('vm_delete', { name: READY }).catch(() => {});
    for (const f of fs.readdirSync('/vm/vms').filter((x) => x.startsWith(`${vm}.`) && /\.(qcow2|vars\.json|tpm)$/.test(x))) {
      fs.renameSync(`/vm/vms/${f}`, `/vm/vms/${READY}${f.slice(vm.length)}`);
    }
    say(`${READY} kept`);
    await startVm(true);
    break;
  case 'reset':
    await stopVm(true);
    forget();
    await call('vm_delete', { name: vm });
    await startVm(true);
    break;
  case 'unlock':
    await unlock();
    say('/usr is writable until the VM restarts');
    break;
  case 'push': {
    const fresh = readList(`${DIR}/push.new`);
    await apply(fresh);
    // Every file pushed since the reset, the last word on each, for after a restart
    const kept = new Map(fs.existsSync(`${DIR}/push.list`) ? readList(`${DIR}/push.list`).map((e) => [e.file, e]) : []);
    for (const e of fresh) kept.set(e.file, e);
    fs.writeFileSync(`${DIR}/push.list`, [...kept.values()].map((e) => `${e.mode} ${e.file}\n`).join(''));
    fs.rmSync(`${DIR}/push.new`);
    break;
  }
  case 'switch': {
    // The VM onto an image of the lab's registry (live.sh delta), finished with a soft reboot when the update allows
    // one (amethystora-soft-reboot), else a restart. The image holds what was pushed, so push.list is forgotten.
    let t = Date.now();
    await root(`mkdir -p /etc/containers/registries.conf.d
      printf '[[registry]]\\nlocation = "10.0.2.2:5000"\\ninsecure = true\\n' >/etc/containers/registries.conf.d/50-vmlab-live.conf
      bootc switch --transport registry ${args[0]} >/dev/null`, 3600);
    say(`bootc switch: ${((Date.now() - t) / 1000).toFixed(0)} s`);
    forget();
    t = Date.now();
    const quick = (await sh('/usr/libexec/amethystora-soft-reboot status', 60)).out.includes('"quick":true');
    const id = decode((await agent(vm, { op: 'read', path: '/proc/sys/kernel/random/boot_id' })).data).trim();
    const soft = (await sh('systemctl show -P SoftRebootsCount', 20)).out.trim();
    await agent(vm, { op: 'exec', command: quick ? '/usr/libexec/amethystora-soft-reboot prepare >/dev/null && (sleep 2; systemctl soft-reboot) >/dev/null 2>&1 &'
      : '(sleep 2; systemctl reboot) >/dev/null 2>&1 &' }, 60);
    for (;;) {
      await sleep(3000);
      try {
        const now = decode((await agent(vm, { op: 'read', path: '/proc/sys/kernel/random/boot_id' }, 5)).data).trim();
        if (now !== id || (await sh('systemctl show -P SoftRebootsCount', 5)).out.trim() !== soft) break;
      } catch {
        // Restarting
      }
    }
    await boot();
    say(`${quick ? 'soft reboot' : 'restart'} into ${args[0]}: ${((Date.now() - t) / 1000).toFixed(0)} s`);
    break;
  }
  case 'save': {
    // The running VM kept whole, memory too (QEMU's migration into a file), with its disk as an internal snapshot
    // "ram" and its firmware variables and TPM beside: `restore` brings it back running, session and all, without a
    // boot. QEMU's command line is kept to start it the same way.
    const pid = Number(fs.readFileSync(`/vm/vms/${vm}.pid`, 'utf8'));
    fs.writeFileSync(`${DIR}/qemu.args`, JSON.stringify(fs.readFileSync(`/proc/${pid}/cmdline`, 'utf8').split('\0').filter(Boolean)));
    const t = Date.now();
    await qmp(vm, 'stop', {});
    fs.rmSync(`${DIR}/ram.mig`, { force: true });
    await qmp(vm, 'migrate', { uri: `file:${DIR}/ram.mig` });
    for (;;) {
      const state = (await qmp(vm, 'query-migrate', {})).status;
      if (state === 'completed') break;
      if (state === 'failed' || state === 'cancelled') throw new Error(`migration ${state}`);
      await sleep(500);
    }
    await qmp(vm, 'quit', {}).catch(() => {});
    for (let i = 0; i < 40 && running(); i++) await sleep(250);
    try {
      process.kill(Number(fs.readFileSync(`/vm/vms/${vm}.swtpm.pid`, 'utf8')));
    } catch {
      // Gone with QEMU
    }
    for (let i = 0; i < 5; i++) await call('vm_snapshot', { name: vm, action: 'delete', tag: 'ram' }).catch(() => (i = 5));
    await call('vm_snapshot', { name: vm, action: 'save', tag: 'ram' });
    // What was pushed by then is in the saved state: restore takes the list back with it
    fs.rmSync(`${DIR}/ram.push.list`, { force: true });
    if (fs.existsSync(`${DIR}/push.list`)) fs.copyFileSync(`${DIR}/push.list`, `${DIR}/ram.push.list`);
    say(`saved: memory ${(fs.statSync(`${DIR}/ram.mig`).size / 2 ** 30).toFixed(1)} GiB, ${((Date.now() - t) / 1000).toFixed(1)} s`);
    break;
  }
  case 'restore': {
    const t = Date.now();
    await stopVm(true);
    const argv = JSON.parse(fs.readFileSync(`${DIR}/qemu.args`, 'utf8'));
    // As much memory as the saved guest has (VMLAB_MEM at its start), not the lab's default
    const no = refusal(Number(argv[argv.indexOf('-m') + 1]) || 4096);
    if (no) throw new Error(`${vm} not restored: ${no}`);
    for (const f of ['push.list', 'push.new']) fs.rmSync(`${DIR}/${f}`, { force: true });
    if (fs.existsSync(`${DIR}/ram.push.list`)) fs.copyFileSync(`${DIR}/ram.push.list`, `${DIR}/push.list`);
    await call('vm_snapshot', { name: vm, action: 'load', tag: 'ram' });
    // Its TPM as the VM started it, then QEMU waiting for the state, which carries the TPM's too
    const tpm = argv.find((a) => a.includes('.swtpm'))?.match(/path=([^,]+)/)?.[1];
    if (tpm) {
      await new Promise((resolve, reject) => spawn('swtpm', ['socket', '--tpm2', '--tpmstate', `dir=/vm/vms/${vm}.tpm`, '--ctrl',
        `type=unixio,path=${tpm}`, '--pid', `file=/vm/vms/${vm}.swtpm.pid`, '--daemon']).on('exit', (c) => (c ? reject(new Error(`swtpm ${c}`)) : resolve())));
    }
    fs.rmSync(`/vm/vms/${vm}.pid`, { force: true });
    const log = fs.openSync(`/vm/vms/${vm}.log`, 'w');
    spawn(argv[0], [...argv.slice(1), '-incoming', 'defer'], { detached: true, stdio: ['ignore', log, log] }).unref();
    for (let i = 0; i < 50 && !running(); i++) await sleep(200);
    if (!running()) throw new Error(`QEMU did not start: ${fs.readFileSync(`/vm/vms/${vm}.log`, 'utf8')}`);
    await qmp(vm, 'migrate-incoming', { uri: `file:${DIR}/ram.mig` });
    for (;;) {
      const status = (await qmp(vm, 'query-status', {})).status;
      if (status === 'paused' || status === 'running') break;
      if (!running()) throw new Error(`QEMU ended loading the state: ${fs.readFileSync(`/vm/vms/${vm}.log`, 'utf8')}`);
      await sleep(250);
    }
    await qmp(vm, 'cont', {}).catch(() => {});
    await agent(vm, { op: 'ping' }, 30);
    // The guest's clock is the saved one: set from the host's, as chrony would after a while
    await root(`date -u -s @${Math.floor(Date.now() / 1000)} >/dev/null; hwclock -w 2>/dev/null || true`);
    await unlock();
    say(`restored, running: ${((Date.now() - t) / 1000).toFixed(1)} s`);
    break;
  }
  case 'relogin':
    await relogin();
    say('the test account has a new session');
    break;
  case 'suite':
    passed = await suite(args[0]);
    break;
  case 'app':
    await installApps();
    // Put back after each restart, as the pushed files are, until a reset
    fs.writeFileSync(`${DIR}/apps.installed`, '');
    say('the apps program replaced: windows opened from now on run it');
    break;
  case 'exec': {
    let user = 'root';
    let inSession = false;
    while (args[0]?.startsWith('--')) {
      const flag = args.shift();
      if (flag === '--user') user = args.shift();
      else if (flag === '--session') [user, inSession] = [user === 'root' ? 'tester' : user, true];
      else throw new Error(`exec: no option ${flag}`);
    }
    const r = await sh(args.join(' '), 1800, { user, session: inSession });
    process.stdout.write(r.out);
    process.stderr.write(r.err);
    process.exitCode = r.rc;
    break;
  }
  case 'shot':
    fs.writeFileSync(`${DIR}/shot.png`, await screendump(vm, Number(args[0] || 1)));
    say(`${DIR}/shot.png`);
    break;
  case 'stop':
    await stopVm(args.includes('--force'));
    say(`${vm} stopped; its next start unlocks /usr and puts the pushed files back (push.list)`);
    break;
  case 'delete':
    await stopVm(true);
    say((await call('vm_delete', { name: vm })).content[0].text);
    fs.rmSync(DIR, { recursive: true, force: true });
    break;
  default:
    throw new Error(`live.mjs: no command ${command}`);
}
if (!passed) process.exitCode = 1;
