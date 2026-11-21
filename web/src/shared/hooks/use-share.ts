/** How a share went: the share sheet took it, the link was copied, or neither could be done. */
export type Shared = "shared" | "copied" | "failed";

/**
 * Shares a link named `title`: the system share sheet where there is one, else copies `url`.
 * Dismissing the share sheet counts as shared, since the User chose to.
 */
export function useShare(): (link: { url: string; title: string }) => Promise<Shared> {
  return async ({ url, title }) => {
    // Desktop Firefox and Chrome on Linux have no share sheet.
    if (typeof navigator.share === "function") {
      try {
        await navigator.share({ url, title });
      } catch {
        // Dismissing the sheet rejects, which is no failure.
      }
      return "shared";
    }
    try {
      await navigator.clipboard.writeText(url);
      return "copied";
    } catch {
      return "failed";
    }
  };
}
