# Telegram Bot API responses for tests

Responses from the Telegram Bot API, served by the fake Telegram in the Server's tests
(`crates/nama-server/tests/support/fake_telegram.rs`), so no test needs Telegram. They follow
the Bot API's documented response shapes, trimmed to a realistic subset of fields, and were
written by hand rather than captured, because the factory that built the Plugin had no bot. When
re-recording one, trim it the same way.

| File | Answers |
|---|---|
| `sendMessage.json` | `POST /bot<token>/sendMessage` with the right token: `200 OK` |
| `unauthorized.json` | any request with a token Telegram doesn't know: `401 Unauthorized` |
| `not-found.json` | any request with a token not shaped like one, `<bot id>:<secret>`: `404 Not Found` |
