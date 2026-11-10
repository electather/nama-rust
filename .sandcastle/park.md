Your diff is the ticket and nothing else. A bug or optimisation you notice elsewhere gets _parked_ as an issue.

1. Search first: `gh issue list --state all --search "<keywords>"`. A match means it's already parked.
2. Park it: `gh issue create --label needs-triage --title "..." --body "..."`. The body gives file and line, what's wrong, how you saw it, and this ticket's number.
3. `needs-triage` is the only label: a human triages it. Park at most three per ticket, each one you can point at.
