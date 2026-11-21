# Telegram Bot API responses for tests

Responses from the Telegram Bot API, which the Plugin's unit tests (`src/lib_tests.rs`) answer
its requests with, so no test needs Telegram. They follow the Bot API's documented response
shapes, trimmed to a realistic subset of fields, and were written by hand rather than captured,
because the factory that built the Plugin had no bot. When re-recording one, trim it the same way.

| File | Answers |
|---|---|
| `sendMessage.json` | `POST /bot<token>/sendMessage`: `200 OK` |
| `getMe.json` | `POST /bot<token>/getMe`: `200 OK`, the bot `@nama_test_bot` |
| `getUpdates.json` | `POST /bot<token>/getUpdates`: `200 OK`, someone saying hello, then Ada's chat `987654321` pressing Start with the code `CODE` |
| `forbidden.json` | `POST /bot<token>/sendMessage` to the chat of a User who blocked the bot: `403 Forbidden` |
| `unauthorized.json` | any request with a token Telegram doesn't know: `401 Unauthorized` |
| `not-found.json` | any request with a token not shaped like one, `<bot id>:<secret>`: `404 Not Found` |
