#!/usr/bin/bash
#
# A screen share does not outlive the session that started it (R7-26, R7-50): the remote desktop service stops with the session, and
# what `ame desktop help-me` and `second-screen` changed in the account's settings (and the code in its keyring) is put back here at the
# next login, if a restart or a crash kept them from doing it themselves. Every login; it does nothing when no share was on.

[[ -x /usr/libexec/amethystora-share-screen ]] || exit 0
/usr/libexec/amethystora-share-screen end 2>/dev/null || true
exit 0
