# A Playback Plan's reasons are typed facts

ADR 0027 had every step record why in an English sentence, even a step that ruled nothing out. A client couldn't translate the sentences, couldn't tell the two that decided a plan from the nine that didn't, and showed whatever wording the Server had. Now a plan carries Playback Reasons. Each is one fact: a `PlaybackReasonKind` with the values it mentions in `params`, shaped as a Refusal is (ADR 0019). The client owns the wording, and the Server's English `text` rides beside each for a client that doesn't know the kind.

A step gives a reason only when it rules a method out, changes what plays, drops a choice (ADR 0031) or assumes what it couldn't know, so a plain Direct Play has none. The Server marks each reason as having decided the plan or not. One decided it if it ruled out a method the plan would rather have used than the one it plays (Direct Play before Remux before Transcode), whether or not another reason ruled that method out too, or if it says what the method it plays changes, or that a choice was dropped. The rest is context, such as a busy Server on a plan that Remuxes anyway.

## Considered Options

- **One kind per step outcome**: rejected. A step's sentence joins several facts, so its params would hold lists and each combination would need a kind.
- **Every step still gives a reason**: rejected. A dozen kinds saying nothing happened would be API contract for good, each needing wording and translation, and `GetPlaybackStatus` already says what the Server's ffmpeg can do.
- **The client works out what decided the plan**, from the methods each reason rules out: rejected, as in ADR 0027. Every client would repeat the rule.
- **A reason decided the plan only if the plan would differ without it**: rejected. An MKV with HEVC video on a Device that decodes neither would show only half of what stands in the way of Direct Play, or neither, and the Server would plan again once per reason to find out.
- **A typed list beside the sentences**: rejected. It breaks nothing, but leaves two lists a comment must keep in step (ADR 0032).

## Consequences

- A kind, its params and the methods it rules out are API contract. Renaming or reusing one breaks clients.
- No reason says what a conversion makes. The plan's Tracks show what the Device gets.
- The reason for a dropped choice doesn't repeat why. The reasons that ruled its method out are in the same list.
- When hardware acceleration fails partway, the playback's reasons are replaced with the software fallback's, worked out when the plan was made, so none contradicts another.
- Reasons stay in the order the steps ran. A client that leads with what decided the plan sorts them itself.
