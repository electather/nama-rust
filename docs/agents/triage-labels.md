# Triage Labels

The skills speak in terms of five canonical triage roles. This file maps those roles to the actual label strings used in this repo's issue tracker.

| Label in mattpocock/skills | Label in our tracker | Meaning                                  |
| -------------------------- | -------------------- | ---------------------------------------- |
| `needs-triage`             | `needs-triage`       | Maintainer needs to evaluate this issue  |
| `needs-info`               | `needs-info`         | Waiting on reporter for more information |
| `ready-for-agent`          | `ready-for-agent`    | Fully specified, ready for an AFK agent  |
| `ready-for-human`          | `ready-for-human`    | Requires human implementation            |
| `wontfix`                  | `wontfix`            | Will not be actioned                     |
| —                          | `agent-working`      | The sandcastle factory is triaging or building it (set and cleared by `.sandcastle/main.ts`) |
| —                          | `needs-decision`     | Triaged, but only the maintainer can settle it: the triage comment holds the options, a pick and the question |

The factory triages `needs-triage` issues itself (`.sandcastle/triage-prompt.md`): it closes, specifies, splits or asks. It applies `needs-decision` where the skills would ask the maintainer, and never `wontfix`.

`ready-for-agent` goes on the issues the factory builds, never on a parent. A parent with sub-issues isn't built itself: its children carry the label, and the factory closes the parent once they're all closed.

When a skill mentions a role (e.g. "apply the AFK-ready triage label"), use the corresponding label string from this table.

Edit the "Label in our tracker" column to match whatever vocabulary you actually use.
