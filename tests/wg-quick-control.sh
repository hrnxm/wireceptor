#!/bin/sh
set -eu
dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT
dpkg-deb -x "$1" "$dir/pkg"
printf '%s\n' '#!/bin/sh' 'printf "%s\n" "$@"' > "$dir/systemctl"
chmod 755 "$dir/systemctl"

# Exercise the shipped root helper without touching any real system service.
unshare --mount --propagation private sh -eu -s -- "$dir" <<'SH'
mount --bind "$1/systemctl" /usr/bin/systemctl
helper="$1/pkg/usr/libexec/wireceptor/wg-quick-control"
for verb in start stop; do
    expected=$(printf "%s\n" --system --no-ask-password "$verb" wg-quick@office-1.service)
    test "$("$helper" "$verb" office-1)" = "$expected"
done
reject() {
    rc=0
    "$helper" "$@" || rc=$?
    test "$rc" = 2
}
reject restart wg0
reject start
reject stop wg0 extra
reject start "../wg0"
reject start "wg*"
reject start ""
reject start 1234567890123456
SH
