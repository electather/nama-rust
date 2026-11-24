# A Device is signed in as several Users, and a Switch PIN guards switching to one

_Amends [ADR 0014](0014-kernel-owns-sessions-plugins-check-credentials.md)._

A household shares one TV or iPad, and each person is a User with their own Watch State, Maximum Age and Libraries (#1414). Signing out and back in to change who is watching is too slow, and a second identity level under a User, as Plex's and Silo's profiles are, would clash with the Hub's Accounts, which are linked to one User per Server (ADR 0011). So a Device may be signed in as several Users, and it acts as one of them at a time:
- Each User signs in to the Device once, with any of their Sign-in Methods or a Sign-in Code. A Device that is already signed in sends its token with the sign-in, and the User is added to it instead of a new Device being made.
- The Device then shows a User picker of only the Users signed in to it, and switching to one of them needs no sign-in. It stays as the User it last switched to, across restarts, until it switches again.
- Revoking works per User: signing a User out of the Device, or an Admin revoking it from that User's Devices, leaves the others signed in. Removing the last User signs the Device out. When the User it is acting as goes and others stay, it acts as none of them until one is picked. Removing a Sign-in Method signs its User out of the Devices it signed them in to, and deleting a User signs them out of every Device.

The Device keeps one session token, as ADR 0014 has it, and the Server holds the rest. The token's hash and its sliding expiry stay on the Device, so a Device left unused signs all its Users out together. Each User on the Device gets their own record of the Sign-in Method that signed them in (none for a Sign-in Code) and when they last proved who they are; the Device records which of them it is now. The token resolves to the Device and its current User, so every route still sees one User per request. Switching is a call to the Server, which checks that the User is signed in to that Device and asks for their Switch PIN.

A Switch PIN is 4 to 6 digits a User may set so that nobody else on a shared Device can switch to them. It is stored hashed like a password.
- It is asked only when switching to its User on a Device they already signed in to. It is not a Sign-in Method: sign-in, Sign-in Code approval and Recent Authentication never accept it.
- A User sets, changes or clears their own Switch PIN. An Admin can clear anyone's, but never sets one. Both need Recent Authentication.
- After 5 wrong tries in a row for one User on one Device, that User is signed out of that Device. Signing in to it again, with a Sign-in Method or a Sign-in Code, is the only way back. Only a right Switch PIN, or the sign-out the fifth wrong one causes, resets the count. Unlike the sliding windows that limit guesses at passwords and Sign-in Codes, wrong tries don't expire with time. The count is kept in memory like those limits, so a Server restart forgets it, which someone holding only the Device can't cause.
- Switching never brings Recent Authentication with it: switching to a User clears theirs on that Device, even if they proved who they are there minutes before, and they have none until they prove it again. Otherwise switching to a User without a Switch PIN would let anyone change their password.

## Considered Options

- **A Device per User, with the app holding one token for each**: rejected. Holding a User's token is enough to act as them, so the app alone would enforce the Switch PIN, and a browser keeps a session in one HttpOnly cookie (ADR 0015), not one per User.
- **Profiles under one User, each with its own Watch State, Maximum Age and Libraries**: rejected in #1414. A second identity level the Hub's Accounts don't know, where a child as a User of their own gives the same.
- **A Switch PIN that locks for a while after wrong tries**: rejected. With 10,000 four-digit Switch PINs, a limit of a few tries every 15 minutes still lets someone who has the Device all week find one. Signing the User out costs them one sign-in instead.
- **Accepting the Switch PIN as a Sign-in Method on a Device the User already signed in to**: rejected. A Switch PIN that grants Recent Authentication would guard a password change with as few as 4 digits.

## Consequences

- The Switch PIN guards switching, not the Device. A User who leaves a shared Device as themselves leaves it open to whoever picks it up.
- The Device's current User is the one whose Maximum Age and Libraries apply, so a switch changes what it may see at once. A Playback Plan belongs to the User it was made for, and the Device can't keep playing it after switching away.
- A page at the Hub trades its Account's token for its User on each Server (ADR 0105), so the Devices it makes stay one User each.
