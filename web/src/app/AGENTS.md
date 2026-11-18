## web/src/app

The shell: the layouts the layout routes render, the app chrome, and what a route shows when it fails. Its queries and mutations are data hooks in `data/`, one per file, like a feature's.

- **The `_member` layout hosts "Confirm it’s you".** `MemberLayout` wraps every signed-in page in `ConfirmItsYou`, the one dialog `useSensitiveMutation` opens. A page never renders its own.
- **`fullScreen` leaves out the app chrome.** A route that sets `staticData: { fullScreen: true }` (declared in `src/router.tsx`), such as the player's, gets no app chrome once it has loaded. One that failed keeps it, so its error or not-found page has a way out.
- **Diagnostics load before the page.** The `_member` route's `beforeLoad` awaits `recordDiagnosticsIfExported` (`src/lib/diagnostics.ts`), so the page's own calls are traced too. It loads the OpenTelemetry SDK only while the Server exports Diagnostics, and only `src/lib/diagnostics-recorder.ts` imports the SDK, so it stays a chunk of its own (ADR 0054).
