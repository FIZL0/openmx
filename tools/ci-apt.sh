#!/usr/bin/env bash
# B36: apt-get install for the CI runners, retried. GitHub's ubuntu runners lose DNS for the apt mirrors now and then
# (2026-11-11, 13:49-14:06 UTC: every mirror "Temporary failure resolving", exit 100, two pages.yml runs red with no
# code at fault), so try again with a growing pause, about 15 minutes in all, before giving up.
set -u
for wait in 0 30 60 120 240 420; do
    sleep "$wait"
    if sudo apt-get -o Acquire::Retries=3 update -q &&
        sudo apt-get -o Acquire::Retries=3 install -yq --no-install-recommends "$@"; then
        exit 0
    fi
    echo "ci-apt: install of $* failed; retrying" >&2
done
exit 100
