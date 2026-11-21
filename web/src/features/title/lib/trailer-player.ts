/**
 * The address of the host's own player for the Trailer at `url`, a YouTube or Vimeo page, which
 * starts playing as it loads. `undefined` for any other address, which only opens in a new tab.
 */
export function trailerPlayer(url: string): string | undefined {
  const page = URL.parse(url);
  const host = page?.hostname.replace(/^(?:www\.|m\.)/u, "");
  const youtube = page ? youtubeId(page, host) : undefined;
  if (youtube !== undefined) {
    // youtube-nocookie sets no cookie until the video plays.
    return `https://www.youtube-nocookie.com/embed/${youtube}?autoplay=1`;
  }
  const vimeo = host === "vimeo.com" ? /^\/(?<id>\d+)$/u.exec(page?.pathname ?? "") : null;
  return vimeo?.groups && `https://player.vimeo.com/video/${vimeo.groups["id"]}?autoplay=1&dnt=1`;
}

/** The video's id in a `youtube.com/watch?v=` or `youtu.be/` address on `host`. */
function youtubeId(page: URL, host: string | undefined): string | undefined {
  const id = host === "youtu.be" ? page.pathname.slice(1) : page.searchParams.get("v");
  const onYoutube = host === "youtu.be" || host === "youtube.com";
  return onYoutube && id !== null && /^[\w-]+$/u.test(id) ? id : undefined;
}

/** The name of the site hosting the Trailer at `url`, `YouTube` or `Vimeo`, if it's either. */
export function trailerHost(url: string): string | undefined {
  const host = URL.parse(url)?.hostname.replace(/^(?:www\.|m\.)/u, "");
  return { "youtube.com": "YouTube", "youtu.be": "YouTube", "vimeo.com": "Vimeo" }[host ?? ""];
}
