# The web app has a push-only service worker

The web app had no service worker, so that none could keep serving an old app, or cached API answers, after the Server upgrades. Web Push, which #1421 chose so a User hears about their Requests with Nama closed, can't work without one: the browser wakes a service worker to show each push. So the web app has one, `web/public/push-worker.js`, that does only that:
- It listens for `push`, to show the notification, and `notificationclick`, to focus or open its link, and for nothing else. With no `fetch` listener, every request goes to the network as if it weren't there.
- It uses no Cache API and doesn't precache or call `skipWaiting`, so it holds no copy of the app to go stale.
- It's registered only when a User turns on "Notify this browser", at the root so its scope covers every page.

A test reads the file and fails on a `fetch` listener or the Cache API.

## Considered Options

- **A full service worker, caching the app to work offline**: rejected. It's the stale app after an upgrade that kept the web app without one, and Nama is no use offline.
- **Notifications only while a page is open, from the Events stream**: rejected. A User hears nothing once the tab is closed, which is most of the time.

## Consequences

- A browser holds the worker until it's unregistered, so a Server upgrade that changes it reaches a browser on its next update check, within a day. It shows notifications and nothing else, so an old one can't serve an old app.
