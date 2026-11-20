/**
 * How long a hook reuses what its route's loader fetched before a mount or window focus refetches
 * it. Suspense would otherwise treat it as stale after 1 s, so a page that mounts later, while its
 * chunk downloads, asks the Server again. Events and mutations refetch it whenever they need to.
 */
export const loaderStaleTime = 30_000;
