# Refusals carry a typed `Refusal` error detail

A Connect code is too coarse for a client to act on: `invalid_argument` from `CreateUser` can mean the name or the password, and the web app told them apart by searching the English message. So every error a client can cause carries one `nama.v1.Refusal` Connect error detail. It holds a `RefusalReason` naming the rule that was broken, the request `field` it is about (if any), and `params` with the values its message mentions, so a client can pick a form field or translate the refusal without reading text. Each reason maps to exactly one Connect code. The English text stays in the error's `message` for people calling the API directly, and the detail's `debug` carries the Refusal as JSON for the same readers; clients decode `value`, never `debug`. Server-side failures carry no Refusal and stay a generic `internal` error.

The shape follows Google's AIP-193: reasons are UPPER_SNAKE names, `params` keys are lowerCamelCase with the unit in the key (`maxLength`, not `max`), and `field` is a request field path (`name`, `folders[2]`).

## Considered Options

- **`google.rpc.ErrorInfo` plus `google.rpc.BadRequest`**: rejected. Same shape and better known, but the reason is a string, so generated clients get no typed reasons; a field refusal needs two details; and the schema would need googleapis as a buf dependency. Moving later is mechanical: write each `RefusalReason` name into `ErrorInfo.reason`.
- **One detail type per refusal**, as first planned for Recent Authentication: rejected. Clients would look for a growing list of types.
- **Only the field, no reason or params**: rejected. Localisation is expected (the web app already lays out for right-to-left languages), and a translation needs the rule and its values, not English text.

## Consequences

- A reason, its Connect code and its params are API contract. Renaming or reusing one breaks clients.
- A Plugin's own refusal, such as an Authenticator's password rule, gets one generic reason with the Plugin's text as the message. Translating it would need a Plugin API change.
- A refused request names one broken rule: the Server stops at the first.
