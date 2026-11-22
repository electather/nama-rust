# A Sign-in Link proves an Account's email, and only a proven Account can Link

Creating an Account takes an email and a password, and nothing proves the person owns that inbox, so anyone can create an Account under someone else's email first. The Hub counts an Account's email as proven only once a Sign-in Link sent to it has been used. Until then the Account can sign in, but it can't register a Server, Link a User or accept a User created from it (ADR 0092). The first Sign-in Link used on an unproven Account also takes it over: it ends every other session and removes the password and passkeys.

Both halves are needed. A takeover alone would leave any Link made before it, and a Server Linked by whoever created the Account would keep receiving the owner's Watch State and Watchlist. `accounts` can't remove those Links, because it sits below `links` (ADR 0099). Refusing to Link while unproven means there are none to remove.

A Sign-in Link opens a page naming the Account, and only that page's button uses the link up. Mail scanners open every link in an email, a click from a mail client doesn't carry the SameSite=Strict session cookie (ADR 0015), and a link that signed in on opening would let someone send a victim a link to the sender's own Account, which the victim might then Link their Server to.

## Considered Options

- **Proving the email when the Account is created**: rejected. Everyone would wait on an email before using the Hub at all, when only Linking needs the proof.
- **An emailed code typed into the page that asked for it**: rejected for now. It survives mail scanners and keeps a sign-in started from a Server in its own tab, but needs its own limit on guesses and would sit beside the Server's Sign-in Code. The Hub stores where to continue with the link instead.

## Consequences

- A Hub without mail set up can't prove emails, so none of its Accounts can Link. Development uses the mail fake or a Resend test key.
- Someone who created an Account with a password and first proves their email later loses that password, and the page tells them.
- While mail is set up, every Account has the Sign-in Link as a Sign-in Method it can't remove, so an Account with no password or passkey can still sign in.
