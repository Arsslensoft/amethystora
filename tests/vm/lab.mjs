// The VM lab: starts Amethystora in QEMU with KVM, UEFI Secure Boot and a TPM, and looks at it, types into it,
// runs commands in it and keeps checkpoints of it. Its tools are what the MCP server (server.mjs) offers and
// what a test run (run.mjs) calls; it runs in the lab container (README.md) and needs nothing but Node.
//
// Commands reach the guest through agent.py, which goes in as a systemd credential at boot and runs from /run:
// nothing of the lab is in the image. The screen, keyboard and mouse go through QEMU's QMP socket.
import { execFile, spawn } from 'node:child_process';
import fs from 'node:fs';
import net from 'node:net';
import path from 'node:path';
import { promisify } from 'node:util';

const run = promisify(execFile);
const ROOT = process.env.VMLAB_DIR || '/vm';
const DISKS = `${ROOT}/disks`;
const VMS = `${ROOT}/vms`;
const HERE = path.dirname(new URL(import.meta.url).pathname);
const OVMF = '/usr/share/edk2/ovmf';
const NAME = /^[a-z0-9][a-z0-9._-]{0,40}$/;
const TEXT_LIMIT = 60000;

fs.mkdirSync(DISKS, { recursive: true });
fs.mkdirSync(VMS, { recursive: true });

const file = (name, ext) => `${VMS}/${name}.${ext}`;
// A process that has exited and not been reaped yet is not alive
const alive = (pidFile) => {
  try {
    const stat = fs.readFileSync(`/proc/${Number(fs.readFileSync(pidFile, 'utf8'))}/stat`, 'utf8');
    return !/^\d+ \(.*\) Z/.test(stat);
  } catch {
    return false;
  }
};
const running = (name) => alive(file(name, 'pid'));
const checkName = (name, what = 'name') => {
  if (!NAME.test(name || '')) throw new Error(`${what} must be lowercase letters, digits, . _ - (got ${JSON.stringify(name)})`);
  return name;
};
const needRunning = (name) => {
  checkName(name);
  if (!running(name)) throw new Error(`VM ${name} is not running`);
  return name;
};
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const clip = (text) => (text.length > TEXT_LIMIT ? `${text.slice(0, TEXT_LIMIT / 2)}\n[... ${text.length - TEXT_LIMIT} characters cut ...]\n${text.slice(-TEXT_LIMIT / 2)}` : text);

// The lab shares one Linux VM (WSL 2's on Windows) with every other container, and it hung when a build and a VM
// ran beside other containers and its memory ran out (2026-10-29): the whole of Docker stopped answering. So a VM
// starts only while no image is being built here, and only when the memory it will take is free: what the kernel
// calls available, less what the VMs running already have yet to take (QEMU takes a guest's memory as it touches
// it), less a margin for the lab itself.
export const MARGIN_MB = 1536;
const procs = () => fs.readdirSync('/proc').filter((p) => /^\d+$/.test(p) && Number(p) !== process.pid).map((pid) => {
  try {
    return { pid, argv: fs.readFileSync(`/proc/${pid}/cmdline`, 'utf8').split('\0') };
  } catch {
    return null;
  }
}).filter(Boolean);
// What builds an image here, if anything: buildah, or podman building
export function building() {
  const p = procs().find(({ argv }) => /(^|\/)buildah$/.test(argv[0]) || (/(^|\/)podman$/.test(argv[0]) && argv.includes('build')));
  return p ? `${p.argv.slice(0, 3).join(' ')} (process ${p.pid})` : '';
}
// MB free for one more VM
export function headroom() {
  const available = Number(fs.readFileSync('/proc/meminfo', 'utf8').match(/^MemAvailable:\s+(\d+)/m)[1]) / 1024;
  let owed = 0;
  for (const { pid, argv } of procs().filter(({ argv }) => /qemu-system/.test(argv[0]))) {
    const size = Number(argv[argv.indexOf('-m') + 1]) || 4096;
    let rss = 0;
    try {
      rss = Number(fs.readFileSync(`/proc/${pid}/status`, 'utf8').match(/^VmRSS:\s+(\d+)/m)?.[1] || 0) / 1024;
    } catch {
      // Gone since
    }
    owed += Math.max(0, size - rss);
  }
  return Math.floor(available - owed - MARGIN_MB);
}
// Why a VM of SIZE MB may not start now, or nothing
export function refusal(size) {
  const build = building();
  if (build) return `an image is being built in the lab (${build}): no VM starts beside a build`;
  const free = headroom();
  return free < size ? `not enough memory: ${size} MB wanted, ${Math.max(free, 0)} MB free beyond a ${MARGIN_MB} MB margin` : '';
}

// One request at a time on each socket: QEMU's socket chardevs take one client
const queues = new Map();
function serial(key, task) {
  const next = (queues.get(key) || Promise.resolve()).then(task, task);
  queues.set(key, next.catch(() => {}));
  return next;
}

// A line-based exchange on a unix socket: send each message, and resolve with the first line `done` accepts
function exchange(socketPath, messages, done, timeoutMs) {
  return new Promise((resolve, reject) => {
    const socket = net.createConnection(socketPath);
    let buffer = '';
    const timer = setTimeout(() => {
      socket.destroy();
      reject(new Error(`no answer on ${path.basename(socketPath)} within ${Math.round(timeoutMs / 1000)} s`));
    }, timeoutMs);
    const finish = (error, value) => {
      clearTimeout(timer);
      socket.destroy();
      error ? reject(error) : resolve(value);
    };
    socket.on('error', (error) => finish(error));
    socket.on('connect', () => messages.forEach((m) => socket.write(`${JSON.stringify(m)}\n`)));
    socket.on('data', (chunk) => {
      buffer += chunk;
      let newline;
      while ((newline = buffer.indexOf('\n')) >= 0) {
        const line = buffer.slice(0, newline);
        buffer = buffer.slice(newline + 1);
        let message;
        try {
          message = JSON.parse(line);
        } catch {
          continue;
        }
        const result = done(message);
        if (result !== undefined) return finish(result instanceof Error ? result : null, result);
      }
    });
  });
}

// QMP: the greeting, capabilities, then one command
export function qmp(name, execute, args) {
  return serial(`${name}.qmp`, () => {
    let step = 0;
    return exchange(file(name, 'qmp'), [{ execute: 'qmp_capabilities' }, { execute, arguments: args }], (m) => {
      if (m.QMP || m.event) return undefined;
      if (m.error) return new Error(`${execute}: ${m.error.desc}`);
      if ('return' in m && ++step === 2) return m.return;
      return undefined;
    }, 120000);
  });
}

let requestId = 0;
// Text typed on the serial console, as at a prompt there (a disk's passphrase on a machine without Plymouth)
export function serialWrite(name, text) {
  return new Promise((resolve, reject) => {
    const socket = net.createConnection(file(name, 'console'));
    socket.on('error', reject);
    socket.on('connect', () => socket.end(text, resolve));
  });
}

export function agent(name, request, timeoutS = 120) {
  const id = ++requestId;
  return serial(`${name}.agent`, () =>
    exchange(file(name, 'agent'), [{ ...request, id }], (m) => (m.id === id ? m : undefined), (timeoutS + 15) * 1000));
}

// --- The guest's helper, as systemd credentials over SMBIOS ---------------------------------------------------

// The helper, and any other credentials asked for by name (cryptsetup.passphrase, cryptsetup.tpm2-pin), which
// systemd-cryptsetup imports at boot, in the initramfs too
function credentials(extra = {}) {
  const b64 = (text) => Buffer.from(text).toString('base64');
  // Started with sysinit.target, so that it answers while a later part of the boot hangs
  const unit = ['[Unit]', 'Description=Amethystora VM lab helper', 'DefaultDependencies=no',
    'After=systemd-udevd.service', 'Conflicts=shutdown.target', 'Before=shutdown.target', '[Service]', 'ExecStart=/usr/bin/python3 /run/credentials/@system/vmlab-agent.py',
    'Restart=always', 'RestartSec=2', ''].join('\n');
  const dropin = ['[Unit]', 'Wants=amethystora-vmlab.service', ''].join('\n');
  return [
    ['systemd.extra-unit.amethystora-vmlab.service', b64(unit)],
    ['systemd.unit-dropin.sysinit.target', b64(dropin)],
    ['vmlab-agent.py', fs.readFileSync(path.join(HERE, 'agent.py')).toString('base64')],
    ...Object.entries(extra).map(([key, value]) => [checkName(key, 'credential'), b64(value)]),
  ].flatMap(([key, value]) => ['-smbios', `type=11,value=io.systemd.credential.binary:${key}=${value}`]);
}

// --- Tools --------------------------------------------------------------------------------------------------

const KEYS = {
  ' ': 'spc', '\n': 'ret', '\t': 'tab', '-': 'minus', '=': 'equal', '[': 'bracket_left', ']': 'bracket_right',
  ';': 'semicolon', "'": 'apostrophe', '`': 'grave_accent', '\\': 'backslash', ',': 'comma', '.': 'dot', '/': 'slash',
};
const SHIFTED = {
  '!': '1', '@': '2', '#': '3', $: '4', '%': '5', '^': '6', '&': '7', '*': '8', '(': '9', ')': '0', _: 'minus',
  '+': 'equal', '{': 'bracket_left', '}': 'bracket_right', ':': 'semicolon', '"': 'apostrophe', '~': 'grave_accent',
  '|': 'backslash', '<': 'comma', '>': 'dot', '?': 'slash',
};
function chord(text) {
  return text.split('-').map((key) => ({ type: 'qcode', data: key }));
}
function typed(character) {
  if (/^[a-z0-9]$/.test(character)) return [character];
  if (/^[A-Z]$/.test(character)) return ['shift', character.toLowerCase()];
  if (KEYS[character]) return [KEYS[character]];
  if (SHIFTED[character]) return ['shift', SHIFTED[character]];
  throw new Error(`cannot type ${JSON.stringify(character)} (US layout, ASCII only)`);
}

function pngSize(png) {
  return { width: png.readUInt32BE(16), height: png.readUInt32BE(20) };
}
// Each screen after the first of a VM started with screens is its own display device
export async function screendump(name, screen = 1) {
  const target = file(name, 'png');
  await qmp(name, 'screendump', { filename: target, format: 'png', ...(screen > 1 ? { device: `vga${screen}` } : {}) });
  return fs.readFileSync(target);
}

function text(value) {
  return { content: [{ type: 'text', text: clip(String(value)) }] };
}
export function decode(value) {
  return Buffer.from(value || '', 'base64').toString('utf8');
}

export const tools = [
  {
    name: 'disk_build',
    description: 'Install an image onto a lab disk with `bootc install to-disk` (pulls it first; about 10 minutes). Runs in the background: follow it with disk_list.',
    schema: { image: { type: 'string', description: 'default ghcr.io/arsslensoft/amethystora:latest' }, disk: { type: 'string', description: 'the disk\'s name, default latest' } },
    async call({ image = 'ghcr.io/arsslensoft/amethystora:latest', disk = 'latest' }) {
      checkName(disk, 'disk');
      if (alive(`${DISKS}/${disk}.build.pid`)) throw new Error(`disk ${disk} is being built already`);
      const log = fs.openSync(`${DISKS}/${disk}.log`, 'w');
      const child = spawn('/usr/bin/bash', [path.join(HERE, 'make-disk.sh'), image, disk], { detached: true, stdio: ['ignore', log, log] });
      fs.writeFileSync(`${DISKS}/${disk}.build.pid`, String(child.pid));
      child.unref();
      return text(`Building disk ${disk} from ${image}; log in ${DISKS}/${disk}.log`);
    },
  },
  {
    name: 'disk_list',
    description: 'The lab disks, the image digest each was made from, and the end of any build\'s log.',
    schema: {},
    async call() {
      const lines = [];
      for (const entry of fs.readdirSync(DISKS).filter((f) => f.endsWith('.log')).sort()) {
        const disk = entry.slice(0, -4);
        const qcow2 = `${DISKS}/${disk}.qcow2`;
        const building = alive(`${DISKS}/${disk}.build.pid`);
        const digest = fs.existsSync(`${DISKS}/${disk}.digest`) ? fs.readFileSync(`${DISKS}/${disk}.digest`, 'utf8').trim() : '';
        lines.push(`${disk}: ${building ? 'building' : fs.existsSync(qcow2) ? `ready ${digest}` : 'failed'}`);
        if (building || !fs.existsSync(qcow2)) {
          lines.push(...fs.readFileSync(`${DISKS}/${entry}`, 'utf8').trim().split('\n').slice(-8).map((l) => `  ${l}`));
        }
      }
      return text(lines.join('\n') || 'No disks yet: disk_build makes one.');
    },
  },
  {
    name: 'vm_start',
    description: 'Start a VM on its own overlay of a lab disk (made on first start, or again with fresh). UEFI with Secure Boot and Microsoft\'s keys, a TPM 2.0, user networking. Then vm_wait.',
    schema: {
      name: { type: 'string' }, disk: { type: 'string', description: 'default latest' },
      memory_mb: { type: 'integer', description: 'default 4096' }, cpus: { type: 'integer', description: 'default 4' },
      secure_boot: { type: 'boolean', description: 'default true' }, tpm: { type: 'boolean', description: 'default true' },
      fresh: { type: 'boolean', description: 'throw the overlay, its checkpoints, the firmware variables, the TPM and the spare disks away first' },
      spares_gb: { type: 'array', items: { type: 'integer' }, description: 'blank SATA disks of these sizes in GiB, at most 6, serials SPARE-1...; kept until fresh' },
      absent: { type: 'array', items: { type: 'integer' }, description: 'spare disks (1, 2...) left unplugged this time, kept as they are' },
      system_absent: { type: 'boolean', description: 'the disk the VM was made from left unplugged this time, for a system mirrored onto a spare one' },
      faulty: { type: 'object', description: 'spare disks with bad sectors this time: {"3": [sector, ...]}, each 512-byte sector failing every read with an I/O error' },
      credentials: { type: 'object', description: 'systemd credentials for the guest, {"cryptsetup.passphrase": "..."}' },
      screens: { type: 'integer', description: 'screens of 1280x800 side by side, 1 to 4, default 1' },
      from: { type: 'string', description: 'another, stopped VM to start as: the new overlay on top of its overlay instead of a disk, with its firmware variables and TPM. While this one exists, that one must not start again (QEMU refuses it while this one runs)' },
    },
    required: ['name'],
    async call({ name, disk = 'latest', memory_mb = 4096, cpus = 4, secure_boot = true, tpm = true, fresh = false, spares_gb = [],
      absent = [], system_absent = false, faulty = {}, credentials: extra = {}, screens = 1, from }) {
      checkName(name);
      if (running(name)) throw new Error(`VM ${name} is running already`);
      const refused = refusal(memory_mb);
      if (refused) throw new Error(`${name} not started: ${refused}`);
      // A clone of another VM: its overlay is this one's backing file, nothing of it copied but its few KB of state
      if (from !== undefined) {
        checkName(from, 'from');
        if (from === name) throw new Error('a VM cannot start from itself');
        if (running(from)) throw new Error(`VM ${from} is running: a VM starts from a stopped one`);
        if (!fs.existsSync(file(from, 'qcow2'))) throw new Error(`no VM ${from}`);
      } else {
        checkName(disk, 'disk');
      }
      const base = from !== undefined ? file(from, 'qcow2') : `${DISKS}/${disk}.qcow2`;
      if (!fs.existsSync(base)) throw new Error(`no disk ${disk}: disk_build makes it`);
      if (![1, 2, 3, 4].includes(screens)) throw new Error('screens is 1 to 4');
      if (spares_gb.length > 6) throw new Error('at most 6 spare disks: the machine has six SATA ports');
      if (fresh) {
        for (const ext of ['qcow2', 'vars.json', 'serial', 'png']) fs.rmSync(file(name, ext), { force: true });
        for (const ext of ['tpm', 'checkpoints']) fs.rmSync(file(name, ext), { recursive: true, force: true });
        for (const entry of fs.readdirSync(VMS).filter((f) => f.startsWith(`${name}.spare`))) fs.rmSync(`${VMS}/${entry}`);
      }
      if (!fs.existsSync(file(name, 'qcow2'))) {
        await run('qemu-img', ['create', '-q', '-f', 'qcow2', '-F', 'qcow2', '-b', base, file(name, 'qcow2')]);
      }
      // The firmware keeps its variables in QEMU (uefi-vars), so that Secure Boot needs no SMM: with SMM, a nested
      // VM hangs as the kernel starts
      if (!fs.existsSync(file(name, 'vars.json'))) {
        // The other VM's, or the disk's own, with the image's certificates enrolled (make-disk.sh), else the firmware's
        const own = from !== undefined ? file(from, 'vars.json') : `${DISKS}/${disk}.vars.json`;
        fs.copyFileSync(secure_boot && fs.existsSync(own) ? own : `${OVMF}/${secure_boot ? 'vars.secboot.json' : 'vars.blank.json'}`, file(name, 'vars.json'));
      }
      // ...and its TPM, which a new overlay starts with (an empty one made below otherwise)
      if (from !== undefined && tpm && !fs.existsSync(file(name, 'tpm')) && fs.existsSync(file(from, 'tpm'))) {
        fs.cpSync(file(from, 'tpm'), file(name, 'tpm'), { recursive: true });
      }
      const args = ['-name', name, '-enable-kvm', '-machine', 'q35,smm=off', '-cpu', 'host', '-smp', String(cpus),
        '-m', String(memory_mb), '-global', 'ICH9-LPC.disable_s3=1',
        '-bios', `${OVMF}/OVMF.qemuvars.fd`, '-device', `uefi-vars-x64,jsonfile=${file(name, 'vars.json')}`,
        ...(system_absent ? [] : ['-drive', `file=${file(name, 'qcow2')},if=virtio,format=qcow2,discard=unmap`]),
        '-netdev', 'user,id=n0', '-device', 'virtio-net-pci,netdev=n0',
        '-device', 'virtio-serial-pci', '-chardev', `socket,id=lab,path=${file(name, 'agent')},server=on,wait=off`,
        '-device', 'virtserialport,chardev=lab,name=org.amethystora.vmtest',
        '-device', 'VGA,id=vga1,xres=1280,yres=800', ...Array.from({ length: screens - 1 }, (_, i) => ['-device', `secondary-vga,id=vga${i + 2},xres=1280,yres=800`]).flat(),
        '-device', 'qemu-xhci', '-device', 'usb-tablet', '-display', 'none',
        // The serial console, kept in NAME.serial, and typed into through NAME.console (serialWrite)
        '-chardev', `socket,id=con,path=${file(name, 'console')},server=on,wait=off,logfile=${file(name, 'serial')},logappend=off`,
        '-serial', 'chardev:con', '-qmp', `unix:${file(name, 'qmp')},server=on,wait=off`,
        '-pidfile', file(name, 'pid'), ...credentials(extra)];
      // Blank disks on the machine's own SATA controller, which smartctl can read, for the pools of disks. One
      // left out is a disk that is not there at all; a faulty one reads through blkdebug, raw over it so that
      // its sectors are the guest's, which fails each read of a bad sector as a damaged disk does.
      for (const [i, size] of spares_gb.entries()) {
        const spare = file(name, `spare${i + 1}.qcow2`);
        if (!fs.existsSync(spare)) await run('qemu-img', ['create', '-q', '-f', 'qcow2', spare, `${size}G`]);
        if (absent.includes(i + 1)) continue;
        const bad = faulty[String(i + 1)] || [];
        if (bad.length) {
          const config = file(name, `spare${i + 1}.blkdebug`);
          // Reads only: a rule a read set off stays active until the next read, and without iotype it would fail
          // the writes btrfs makes to repair the sector too, which a damaged disk takes
          fs.writeFileSync(config, bad.map((sector) => ['[inject-error]', 'event = "read_aio"', 'iotype = "read"', 'errno = "5"',
            `sector = "${Number(sector)}"`, 'once = "off"', ''].join('\n')).join('\n'));
          args.push('-drive', `if=none,id=spare${i + 1},driver=raw,discard=unmap,file.driver=blkdebug,file.config=${config},` +
            `file.image.driver=qcow2,file.image.file.driver=file,file.image.file.filename=${spare}`);
        } else {
          args.push('-drive', `file=${spare},if=none,id=spare${i + 1},format=qcow2,discard=unmap`);
        }
        args.push('-device', `ide-hd,drive=spare${i + 1},bus=ide.${i},serial=SPARE-${i + 1}`);
      }
      if (tpm) {
        try {
          process.kill(Number(fs.readFileSync(file(name, 'swtpm.pid'), 'utf8')));
        } catch {
          // None left over
        }
        fs.mkdirSync(file(name, 'tpm'), { recursive: true });
        await run('swtpm', ['socket', '--tpm2', '--tpmstate', `dir=${file(name, 'tpm')}`, '--ctrl',
          `type=unixio,path=${file(name, 'swtpm')}`, '--pid', `file=${file(name, 'swtpm.pid')}`, '--daemon']);
        args.push('-chardev', `socket,id=tpm,path=${file(name, 'swtpm')}`, '-tpmdev', 'emulator,id=tpm0,chardev=tpm',
          '-device', 'tpm-crb,tpmdev=tpm0');
      }
      fs.rmSync(file(name, 'serial'), { force: true });
      fs.rmSync(file(name, 'pid'), { force: true });
      fs.writeFileSync(file(name, 'screens'), String(screens));
      // Detached, with what QEMU says kept in NAME.log: it outlives this server, which each session starts anew
      const log = fs.openSync(file(name, 'log'), 'w');
      const qemu = spawn('qemu-system-x86_64', args, { detached: true, stdio: ['ignore', log, log] });
      qemu.unref();
      for (let i = 0; i < 50 && !running(name); i++) {
        if (qemu.exitCode !== null) break;
        await sleep(200);
      }
      if (!running(name)) throw new Error(`QEMU did not start: ${fs.readFileSync(file(name, 'log'), 'utf8').trim()}`);
      return text(`Started ${name} from ${from !== undefined ? `VM ${from}` : disk} (${memory_mb} MB, ${cpus} CPUs, Secure Boot ${secure_boot ? 'on' : 'off'}, TPM ${tpm ? 'on' : 'off'}).`);
    },
  },
  {
    name: 'vm_wait',
    description: 'Wait until the VM has booted: its helper answers (early in boot), then systemd says the boot is over. With early, only the first.',
    schema: { name: { type: 'string' }, timeout_s: { type: 'integer', description: 'default 600' }, early: { type: 'boolean' } },
    required: ['name'],
    async call({ name, timeout_s = 600, early = false }) {
      needRunning(name);
      const began = Date.now();
      const left = () => Math.max(5, Math.round(timeout_s - (Date.now() - began) / 1000));
      for (;;) {
        try {
          await agent(name, { op: 'ping' }, 5);
          break;
        } catch {
          if (!running(name)) throw new Error(`VM ${name} stopped while booting; see vm_serial`);
          if (Date.now() - began > timeout_s * 1000) throw new Error(`${name} did not answer within ${timeout_s} s; see vm_screenshot and vm_serial`);
          await sleep(3000);
        }
      }
      if (early) return text(`${name} answers, after ${Math.round((Date.now() - began) / 1000)} s.`);
      const state = await agent(name, { op: 'exec', command: 'systemctl is-system-running --wait', timeout: left() }, left());
      return text(`${name} is ${decode(state.stdout).trim() || state.error}, after ${Math.round((Date.now() - began) / 1000)} s.`);
    },
  },
  {
    name: 'vm_exec',
    description: 'Run a bash command in the VM as root, as an account (login shell), or in that account\'s graphical session (session: true). Returns the exit status, stdout and stderr.',
    schema: {
      name: { type: 'string' }, command: { type: 'string' }, user: { type: 'string', description: 'default root' },
      session: { type: 'boolean' }, timeout_s: { type: 'integer', description: 'default 120' }, stdin: { type: 'string' },
    },
    required: ['name', 'command'],
    async call({ name, command, user = 'root', session = false, timeout_s = 120, stdin = '' }) {
      needRunning(name);
      const answer = await agent(name, { op: 'exec', command, user, session, timeout: timeout_s, stdin: Buffer.from(stdin).toString('base64') }, timeout_s);
      const parts = [`exit ${answer.rc}${answer.error ? ` (${answer.error})` : ''}`];
      if (answer.stdout) parts.push(`--- stdout\n${decode(answer.stdout)}`);
      if (answer.stderr) parts.push(`--- stderr\n${decode(answer.stderr)}`);
      return { ...text(parts.join('\n')), isError: answer.rc !== 0 };
    },
  },
  {
    name: 'vm_write',
    description: 'Write a file in the VM, from text or base64.',
    schema: {
      name: { type: 'string' }, path: { type: 'string' }, content: { type: 'string' }, content_base64: { type: 'string' },
      mode: { type: 'string', description: 'octal, default 0644' }, owner: { type: 'string', description: 'user or user:group' },
    },
    required: ['name', 'path'],
    async call({ name, path: target, content = '', content_base64, mode = '0644', owner }) {
      needRunning(name);
      const data = content_base64 ?? Buffer.from(content).toString('base64');
      const answer = await agent(name, { op: 'write', path: target, data, mode, owner });
      if (answer.rc !== 0) throw new Error(answer.error);
      return text(`Wrote ${Buffer.from(data, 'base64').length} bytes to ${target}`);
    },
  },
  {
    name: 'vm_read',
    description: 'Read a file in the VM as text.',
    schema: { name: { type: 'string' }, path: { type: 'string' }, max_bytes: { type: 'integer', description: 'default 200000' } },
    required: ['name', 'path'],
    async call({ name, path: target, max_bytes = 200000 }) {
      needRunning(name);
      const answer = await agent(name, { op: 'read', path: target, max: max_bytes });
      if (answer.rc !== 0) throw new Error(answer.error);
      return text(decode(answer.data) + (answer.truncated ? `\n[... more than ${max_bytes} bytes ...]` : ''));
    },
  },
  {
    name: 'vm_screenshot',
    description: 'The VM\'s screen, as a PNG. Under nested virtualization GNOME draws in software, and the screen can show a click or keys a few seconds late.',
    schema: { name: { type: 'string' }, screen: { type: 'integer', description: 'default 1' } },
    required: ['name'],
    async call({ name, screen = 1 }) {
      needRunning(name);
      const png = await screendump(name, screen);
      const { width, height } = pngSize(png);
      return { content: [{ type: 'image', data: png.toString('base64'), mimeType: 'image/png' }, { type: 'text', text: `${width}x${height}` }] };
    },
  },
  {
    name: 'vm_keys',
    description: 'Press keys in the VM: chords as QEMU qcodes joined by - ("ctrl-alt-t", "meta_l", "ret", "esc"), and/or text typed on a US layout.',
    schema: { name: { type: 'string' }, keys: { type: 'array', items: { type: 'string' } }, text: { type: 'string' } },
    required: ['name'],
    async call({ name, keys = [], text: typing = '' }) {
      needRunning(name);
      for (const key of keys) {
        await qmp(name, 'send-key', { keys: chord(key) });
        await sleep(150);
      }
      for (const character of typing) {
        await qmp(name, 'send-key', { keys: typed(character).map((data) => ({ type: 'qcode', data })) });
        await sleep(40);
      }
      return text(`Sent ${keys.length} chord(s) and ${typing.length} character(s).`);
    },
  },
  {
    name: 'vm_mouse',
    description: 'Move the pointer to x, y in screen pixels (as vm_screenshot shows them) and click, double-click or just move; with press or release, hold or let go of the button there (a drag).',
    schema: {
      name: { type: 'string' }, x: { type: 'integer' }, y: { type: 'integer' }, screen: { type: 'integer', description: 'default 1' },
      button: { type: 'string', enum: ['left', 'right', 'middle'] }, action: { type: 'string', enum: ['click', 'double', 'move', 'press', 'release'] },
    },
    required: ['name', 'x', 'y'],
    async call({ name, x, y, screen = 1, button = 'left', action = 'click' }) {
      needRunning(name);
      const { width, height } = pngSize(await screendump(name));
      // The tablet spans every screen, which GNOME lays out side by side
      const screens = fs.existsSync(file(name, 'screens')) ? Number(fs.readFileSync(file(name, 'screens'), 'utf8')) : 1;
      const scale = (value, size) => Math.max(0, Math.min(32767, Math.round((value * 32767) / Math.max(1, size - 1))));
      await qmp(name, 'input-send-event', { events: [
        { type: 'abs', data: { axis: 'x', value: scale(x + (screen - 1) * width, width * screens) } },
        { type: 'abs', data: { axis: 'y', value: scale(y, height) } }] });
      if (action === 'press' || action === 'release') {
        await qmp(name, 'input-send-event', { events: [{ type: 'btn', data: { down: action === 'press', button } }] });
      }
      for (let i = 0; i < ({ click: 1, double: 2 }[action] || 0); i++) {
        await qmp(name, 'input-send-event', { events: [{ type: 'btn', data: { down: true, button } }] });
        await sleep(60);
        await qmp(name, 'input-send-event', { events: [{ type: 'btn', data: { down: false, button } }] });
        await sleep(120);
      }
      return text(`${action} at ${x},${y} of ${width}x${height}`);
    },
  },
  {
    name: 'vm_serial',
    description: 'The end of the VM\'s serial console: the firmware, the boot loader and, with console=ttyS0, the kernel.',
    schema: { name: { type: 'string' }, lines: { type: 'integer', description: 'default 80' } },
    required: ['name'],
    async call({ name, lines = 80 }) {
      checkName(name);
      const log = fs.existsSync(file(name, 'serial')) ? fs.readFileSync(file(name, 'serial'), 'latin1') : '';
      // eslint-disable-next-line no-control-regex
      return text(log.replace(/\x1b\[[0-9;?]*[A-Za-z]/g, '').split(/\r?\n/).slice(-lines).join('\n') || '(nothing yet)');
    },
  },
  {
    name: 'vm_snapshot',
    description: 'Save, load, list or delete a checkpoint of a stopped VM: its disk, firmware variables and TPM, not its memory, so a loaded one boots again. (QEMU 10.2 crashes loading a running VM\'s snapshot with its UEFI variable device.)',
    schema: { name: { type: 'string' }, action: { type: 'string', enum: ['save', 'load', 'list', 'delete'] }, tag: { type: 'string' } },
    required: ['name', 'action'],
    async call({ name, action, tag }) {
      checkName(name);
      if (!fs.existsSync(file(name, 'qcow2'))) throw new Error(`no VM ${name}`);
      if (action === 'list') return text((await run('qemu-img', ['snapshot', '-U', '-l', file(name, 'qcow2')])).stdout.trim() || 'No checkpoints.');
      checkName(tag, 'tag');
      if (running(name)) throw new Error(`VM ${name} is running: vm_stop it first`);
      const saved = `${file(name, 'checkpoints')}/${tag}`;
      const flag = { save: '-c', load: '-a', delete: '-d' }[action];
      if (action === 'load' && !fs.existsSync(saved)) throw new Error(`no checkpoint ${tag}`);
      // qemu-img adds a second snapshot under a tag it has already, and loads the first: a save replaces it
      if (action === 'save') await run('qemu-img', ['snapshot', '-d', tag, file(name, 'qcow2')]).catch(() => {});
      await run('qemu-img', ['snapshot', flag, tag, file(name, 'qcow2')]);
      if (action === 'save') {
        fs.rmSync(saved, { recursive: true, force: true });
        fs.mkdirSync(saved, { recursive: true });
        fs.copyFileSync(file(name, 'vars.json'), `${saved}/vars.json`);
        if (fs.existsSync(file(name, 'tpm'))) fs.cpSync(file(name, 'tpm'), `${saved}/tpm`, { recursive: true });
      } else if (action === 'load') {
        fs.copyFileSync(`${saved}/vars.json`, file(name, 'vars.json'));
        fs.rmSync(file(name, 'tpm'), { recursive: true, force: true });
        if (fs.existsSync(`${saved}/tpm`)) fs.cpSync(`${saved}/tpm`, file(name, 'tpm'), { recursive: true });
      } else {
        fs.rmSync(saved, { recursive: true, force: true });
      }
      return text(`${action} ${tag}: done`);
    },
  },
  {
    name: 'vm_stop',
    description: 'Shut the VM down (ACPI power button, then QEMU quits after 90 s), or pull the plug with force.',
    schema: { name: { type: 'string' }, force: { type: 'boolean' } },
    required: ['name'],
    async call({ name, force = false }) {
      needRunning(name);
      if (!force) {
        await qmp(name, 'system_powerdown', {});
        for (let i = 0; i < 90 && running(name); i++) await sleep(1000);
      }
      if (running(name)) await qmp(name, 'quit', {}).catch(() => {});
      for (let i = 0; i < 10 && running(name); i++) await sleep(500);
      try {
        process.kill(Number(fs.readFileSync(file(name, 'swtpm.pid'), 'utf8')));
      } catch {
        // No TPM, or it went with QEMU
      }
      return text(`${name} stopped${force ? ' (forced)' : ''}.`);
    },
  },
  {
    name: 'vm_list',
    description: 'The lab\'s VMs: running or not, the disk each started from, and how much each has written.',
    schema: {},
    async call() {
      const names = [...new Set(fs.readdirSync(VMS).filter((f) => f.endsWith('.qcow2') && !/\.spare\d+\.qcow2$/.test(f)).map((f) => f.slice(0, -6)))].sort();
      const lines = [];
      for (const name of names) {
        const { stdout } = await run('qemu-img', ['info', '-U', '--output=json', file(name, 'qcow2')]);
        const info = JSON.parse(stdout);
        lines.push(`${name}: ${running(name) ? 'running' : 'stopped'}, on ${path.basename(info['backing-filename'] || '?', '.qcow2')}, ${(info['actual-size'] / 2 ** 30).toFixed(1)} GiB written`);
      }
      return text(lines.join('\n') || 'No VMs yet: vm_start makes one.');
    },
  },
  {
    name: 'vm_delete',
    description: 'Delete a stopped VM: its overlay, checkpoints, firmware variables and TPM.',
    schema: { name: { type: 'string' } },
    required: ['name'],
    async call({ name }) {
      checkName(name);
      if (running(name)) throw new Error(`VM ${name} is running: vm_stop it first`);
      for (const entry of fs.readdirSync(VMS).filter((f) => f.startsWith(`${name}.`))) {
        fs.rmSync(`${VMS}/${entry}`, { recursive: true, force: true });
      }
      return text(`Deleted ${name}.`);
    },
  },
];

// A tool by name, as the MCP server and a test run call it: its result, or an error with what it said
export async function call(name, args = {}) {
  const tool = tools.find((t) => t.name === name);
  if (!tool) throw new Error(`no tool ${name}`);
  return tool.call(args);
}
