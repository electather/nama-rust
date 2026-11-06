# Browser sessions rely on SameSite=Strict and Connect content types, not CSRF tokens

The web app holds its session in an HttpOnly `SameSite=Strict` cookie, and there is no CSRF token. A page on another origin can only send an RPC's JSON or protobuf `POST` after a CORS preflight, which the Server never approves, and `Strict` keeps the cookie off any request another site starts, including Connect `GET`s. `Strict` costs the web app nothing because its shell and static files are public. A client chooses a cookie or a bearer token when it signs in, and a cookie sign-in never puts the token in the response body, so page scripts can't read it.

## Considered Options

- **CSRF tokens**: rejected. They would repeat what the content-type rule already enforces, and native clients, which use bearer tokens, don't need them.
- **`SameSite=Lax`**: rejected. It would only matter for a link from another site straight to a protected URL such as `/media/...`, and it lets the cookie ride on top-level cross-site `GET`s.

## Consequences

- The Server must never answer CORS preflights for credentialed requests. Allowing them would need a CSRF defence first.
- `Secure` is only set when a request arrived over HTTPS (through a reverse proxy's `X-Forwarded-Proto`), because a LAN Server is usually plain HTTP and a browser drops a `Secure` cookie sent over it.
