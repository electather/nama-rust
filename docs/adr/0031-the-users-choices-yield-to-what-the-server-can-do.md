# The User's choices yield to what the Server can do

ADR 0027's steps all narrow, so a choice that needs a Remux or a Transcode (an audio Track other than the file's own, a burned-in subtitle, or a Playback Limit, the remote default included) left no method when the Server couldn't run one, and the player stopped. Now the Kernel runs the steps for what the Version, Device and Server allow first, and the steps for the User's choices after them. A choice narrows the plan only if a method is still left. Otherwise the plan plays without it and its reasons say which choice it dropped and why. The plan reports which choices it honoured, so the player shows those, and it lists the choices this Version can ever have here, so the player offers only those.

## Considered Options

- **The player keeps the plan it had and says why**: rejected. Nothing helps on first opening, when the remote default chose the limit, or after a Transcode failed with no plan to go back to.
- **The player offers only choices the Server can honour**: not enough on its own. A busy Server or a Transcode that failed on this Device rules a method out only for now, so the Kernel must still drop the choice then.

## Consequences

- A busy Server refuses a plan only when the Device itself needs a Transcode, never because of a choice.
- A dropped choice is forgotten, so it isn't honoured later just because something else was chosen.
