#!/bin/sh
case "${1-}" in
    inner) ready=inner-ready; result=inner-result ;;
    *) ready=capture-ready; result=worker-result ;;
esac
printf '%s\n' waiting > "$ready.tmp"
/bin/mv "$ready.tmp" "$ready"
attempts=0
while test ! -s "$result"; do
    attempts=$((attempts + 1))
    if test "$attempts" -gt 250; then
        printf '%s\n' capture-deadline
        exit 19
    fi
    /bin/sleep .010
done
/bin/cat "$result"
