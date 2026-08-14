#!/usr/bin/env bash
# Seeds a throwaway timewarrior database with fake, generic entries so
# timewarrior-tui can be screenshotted without exposing anyone's real
# tracked time. Never touches your real ~/.timewarrior.
set -euo pipefail

DEMO_DB="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/.timewarrior-demo"
rm -rf "$DEMO_DB"
mkdir -p "$DEMO_DB"
export TIMEWARRIORDB="$DEMO_DB"

add() {
  local start="$1" end="$2" tags="$3" note="$4"
  timew track "$start" - "$end" $tags >/dev/null
  timew annotate @1 "$note" >/dev/null
}

# entries: start | end | tags | annotation
entries=(
  "2026-07-27T09:00:00|2026-07-27T09:30:00|MEET|Daily standup"
  "2026-07-27T09:30:00|2026-07-27T12:00:00|DEV|Implement search filters"
  "2026-07-27T13:00:00|2026-07-27T15:00:00|DEV|Implement search filters"
  "2026-07-27T15:00:00|2026-07-27T16:00:00|REVIEW|Review PR #142"

  "2026-07-28T09:00:00|2026-07-28T09:30:00|MEET|Daily standup"
  "2026-07-28T09:30:00|2026-07-28T12:30:00|DEV|Fix pagination bug"
  "2026-07-28T13:30:00|2026-07-28T17:00:00|DEV|Fix pagination bug"

  "2026-07-29T09:00:00|2026-07-29T09:30:00|MEET|Daily standup"
  "2026-07-29T09:30:00|2026-07-29T11:00:00|PLANNING|Sprint planning"
  "2026-07-29T11:00:00|2026-07-29T15:00:00|DEV|Write integration tests"
  "2026-07-29T15:00:00|2026-07-29T16:30:00|REVIEW|Review PR #145"

  "2026-07-30T09:00:00|2026-07-30T09:30:00|MEET|Daily standup"
  "2026-07-30T09:30:00|2026-07-30T13:00:00|DEV|Refactor auth module"
  "2026-07-30T14:00:00|2026-07-30T17:30:00|DEV|Refactor auth module"

  "2026-07-31T09:00:00|2026-07-31T09:30:00|MEET|Daily standup"
  "2026-07-31T09:30:00|2026-07-31T12:00:00|DEV|Write documentation"
  "2026-07-31T13:00:00|2026-07-31T14:00:00|MEET|1:1 with manager"
  "2026-07-31T14:00:00|2026-07-31T16:00:00|REVIEW|Review PR #148"

  "2026-08-03T09:00:00|2026-08-03T09:30:00|MEET|Daily standup"
  "2026-08-03T09:30:00|2026-08-03T13:00:00|DEV|Add export feature"
  "2026-08-03T14:00:00|2026-08-03T17:00:00|DEV|Add export feature"

  "2026-08-04T09:00:00|2026-08-04T09:30:00|MEET|Daily standup"
  "2026-08-04T09:30:00|2026-08-04T12:00:00|DEV|Add export feature"
  "2026-08-04T13:00:00|2026-08-04T16:30:00|REVIEW|Code review: payments module"

  "2026-08-05T09:00:00|2026-08-05T09:30:00|MEET|Daily standup"
  "2026-08-05T09:30:00|2026-08-05T15:00:00|DEV|Fix flaky tests"
  "2026-08-05T15:00:00|2026-08-05T16:00:00|PLANNING|Backlog grooming"

  "2026-08-06T09:00:00|2026-08-06T09:30:00|MEET|Daily standup"
  "2026-08-06T09:30:00|2026-08-06T13:00:00|DEV|Performance tuning"
  "2026-08-06T14:00:00|2026-08-06T17:00:00|DEV|Performance tuning"

  "2026-08-07T09:00:00|2026-08-07T09:30:00|MEET|Daily standup"
  "2026-08-07T09:30:00|2026-08-07T12:00:00|DEV|Client demo prep"
  "2026-08-07T13:00:00|2026-08-07T14:00:00|MEET|Client demo"
  "2026-08-07T14:00:00|2026-08-07T16:00:00|DEV|Post-demo fixes"

  "2026-08-10T09:00:00|2026-08-10T09:30:00|MEET|Daily standup"
  "2026-08-10T09:30:00|2026-08-10T13:00:00|DEV|Migrate database schema"
  "2026-08-10T14:00:00|2026-08-10T17:00:00|DEV|Migrate database schema"

  "2026-08-11T09:00:00|2026-08-11T09:30:00|MEET|Daily standup"
  "2026-08-11T09:30:00|2026-08-11T12:30:00|REVIEW|Review PR #156"
  "2026-08-11T13:30:00|2026-08-11T17:00:00|DEV|Add caching layer"

  "2026-08-12T09:00:00|2026-08-12T09:30:00|MEET|Daily standup"
  "2026-08-12T09:30:00|2026-08-12T16:00:00|DEV|Add caching layer"

  "2026-08-13T09:00:00|2026-08-13T09:30:00|MEET|Daily standup"
  "2026-08-13T09:30:00|2026-08-13T12:00:00|PLANNING|Sprint retro"
  "2026-08-13T13:00:00|2026-08-13T17:00:00|DEV|Bug bash"

  "2026-08-14T09:00:00|2026-08-14T09:30:00|MEET|Daily standup"
  "2026-08-14T09:30:00|2026-08-14T12:00:00|DEV|Write release notes"
)

for entry in "${entries[@]}"; do
  IFS='|' read -r start end tags note <<<"$entry"
  add "$start" "$end" "$tags" "$note"
done

# Leave one interval open/active today, to show off the active-entry highlight.
timew start 2026-08-14T13:00:00 DEV >/dev/null
timew annotate @1 "Prepare release" >/dev/null

echo "Demo timewarrior database ready at: $DEMO_DB"
echo "Run the app against it with:"
echo "  TIMEWARRIORDB=\"$DEMO_DB\" ./target/release/timewarrior-tui"
