# Telegram Bot API responses for tests

Responses from the Telegram Bot API, served by the fake Telegram in the Server's tests
(`crates/nama-server/tests/support/fake_telegram.rs`), so no test needs Telegram. They follow
the Bot API's documented response shapes, trimmed to a realistic subset of fields, and were
written by hand rather than captured, because the factory that built the Plugin had no bot. When
re-recording one, trim it the same way.

| File | Answers |
|---|---|
| `sendMessage.json` | `POST /bot<token>/sendMessage` with the right token: `200 OK` |
| `getMe.json` | `POST /bot<token>/getMe` with the right token: `200 OK`, the bot `@nama_test_bot` |
| `getUpdates.json` | `POST /bot<token>/getUpdates` with the right token: `200 OK`. The fake answers with its first update, someone saying hello, and one copy of its second per press of Start, its `/start CODE` taking the code pressed and its chat id the chat pressing |
| `forbidden.json` | `POST /bot<token>/sendMessage` to the chat of a User who blocked the bot: `403 Forbidden` |
| `unauthorized.json` | any request with a token Telegram doesn't know: `401 Unauthorized` |
| `not-found.json` | any request with a token not shaped like one, `<bot id>:<secret>`: `404 Not Found` |
