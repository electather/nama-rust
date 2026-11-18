## web/src/features/auth

- **One person's cache.** Every flow that changes who is signed in (signing in, signing out, setting up) clears the query cache, so the next person sees only their own data.
