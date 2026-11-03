/* The seccomp filter of a plugin's sandbox (plans/PLAN-plugins.md, "The sandbox"), written as the BPF program bubblewrap's
 * --seccomp takes. build-apps.sh compiles this with libseccomp, runs it, and 13-apps.sh installs the result as
 * /usr/share/amethystora/plugins/seccomp.bpf. Everything is allowed but what a plugin never needs and an escape would:
 * a system call listed here fails with EPERM, so a program that tries gets an error and goes on.
 *
 * Usage: gen OUTPUT
 */
#define _GNU_SOURCE
#include <errno.h>
#include <sched.h>
#include <seccomp.h>
#include <stdio.h>
#include <sys/ioctl.h>

static const char *const DENIED[] = {
    "ptrace", "process_vm_readv", "process_vm_writev", "kcmp",
    "mount", "umount", "umount2", "pivot_root", "chroot", "mount_setattr", "move_mount", "open_tree", "fsopen", "fsmount", "fsconfig", "fspick",
    "bpf", "io_uring_setup", "io_uring_enter", "io_uring_register", "perf_event_open", "userfaultfd",
    "keyctl", "add_key", "request_key",
    "unshare", "setns",
    "kexec_load", "kexec_file_load", "init_module", "finit_module", "delete_module", "reboot", "swapon", "swapoff",
    "open_by_handle_at", "name_to_handle_at", "acct", "quotactl", "iopl", "ioperm",
};

int main(int argc, char **argv)
{
    if (argc != 2)
        return 64;
    scmp_filter_ctx ctx = seccomp_init(SCMP_ACT_ALLOW);
    if (!ctx)
        return 1;
    for (unsigned i = 0; i < sizeof DENIED / sizeof *DENIED; i++) {
        int nr = seccomp_syscall_resolve_name(DENIED[i]);
        /* A name this kernel's architecture does not have is nothing to deny */
        if (nr == __NR_SCMP_ERROR)
            continue;
        if (seccomp_rule_add(ctx, SCMP_ACT_ERRNO(EPERM), nr, 0) < 0)
            return 2;
    }
    /* A new user namespace, by clone: bubblewrap's --disable-userns is the first wall, this is the second. clone3 takes its flags
     * in a structure a filter cannot read, so it fails as an old kernel's would, and libc falls back to clone */
    if (seccomp_rule_add(ctx, SCMP_ACT_ERRNO(EPERM), SCMP_SYS(clone), 1, SCMP_A0(SCMP_CMP_MASKED_EQ, CLONE_NEWUSER, CLONE_NEWUSER)) < 0)
        return 2;
    if (seccomp_rule_add(ctx, SCMP_ACT_ERRNO(ENOSYS), SCMP_SYS(clone3), 0) < 0)
        return 2;
    /* Typing into the terminal that runs it */
    if (seccomp_rule_add(ctx, SCMP_ACT_ERRNO(EPERM), SCMP_SYS(ioctl), 1, SCMP_A1(SCMP_CMP_MASKED_EQ, 0xffffffff, TIOCSTI)) < 0)
        return 2;
    FILE *out = fopen(argv[1], "wb");
    if (!out)
        return 3;
    if (seccomp_export_bpf(ctx, fileno(out)) < 0 || fclose(out) != 0)
        return 4;
    seccomp_release(ctx);
    return 0;
}
