import type { GetServerInfoResponse } from "@/gen/nama/v1/server_pb";

/** What a Server image is, which decides the widths it's offered at (`widthsOf`). */
export type ImageKind =
  | "poster"
  | "backdrop"
  | "titleLogo"
  | "still"
  | "trailerThumbnail"
  | "headshot"
  | "pluginLogo"
  | "artwork";

/**
 * An image the Server sent: its URL, a path on that Server; the thumbhash shown until it loads; and
 * its kind. Built by the constructor for where its URL was read, so a stand-in keeps its own kind.
 */
export interface ServerImage {
  url: string;
  thumbhash: Uint8Array | undefined;
  kind: ImageKind;
}

/** The Server's width list for each kind, or `undefined` for one shown only as stored. */
const lists = {
  poster: "posterWidths",
  backdrop: "backdropWidths",
  // The Server makes a Trailer's thumbnail ahead as a Backdrop (#2561).
  trailerThumbnail: "backdropWidths",
  titleLogo: "titleLogoWidths",
  // An SVG, which resizing would turn into a picture.
  pluginLogo: undefined,
  still: "imageWidths",
  headshot: "imageWidths",
  artwork: "imageWidths",
} satisfies Record<
  ImageKind,
  "imageWidths" | "posterWidths" | "backdropWidths" | "titleLogoWidths" | undefined
>;

/** The widths `info`'s Server offers an image of `kind` at: none for one it doesn't resize. */
export function widthsOf(kind: ImageKind, info: GetServerInfoResponse): number[] {
  const list = lists[kind];
  if (list === undefined) {
    return [];
  }
  // A Server older than a kind's list sends none, and still resizes to every width.
  return info[list].length > 0 ? info[list] : info.imageWidths;
}

/** The `srcset` of the image at `src`, a copy at each of `widths` (ADR 0039), or none without. */
export function srcSetOf(src: string, widths: number[]) {
  return widths.length > 0
    ? widths.map((width) => `${src}?width=${width} ${width}w`).join(", ")
    : undefined;
}

function image(
  kind: ImageKind,
  url: string | undefined,
  thumbhash?: Uint8Array,
): ServerImage | undefined {
  return url === undefined || url === "" ? undefined : { url, thumbhash, kind };
}

/** A Movie's, Show's, Box Set's or found title's Poster. */
export function posterOf(title: {
  posterUrl?: string | undefined;
  posterThumbhash?: Uint8Array | undefined;
}) {
  return image("poster", title.posterUrl, title.posterThumbhash);
}

/** A title's or Library's Backdrop. */
export function backdropOf(title: {
  backdropUrl?: string | undefined;
  backdropThumbhash?: Uint8Array | undefined;
}) {
  return image("backdrop", title.backdropUrl, title.backdropThumbhash);
}

/** A title's Backdrop with its title in it, made ahead as any Backdrop is. */
export function titledBackdropOf(title: {
  titledBackdropUrl?: string | undefined;
  titledBackdropThumbhash?: Uint8Array | undefined;
}) {
  return image("backdrop", title.titledBackdropUrl, title.titledBackdropThumbhash);
}

/** A title's Title Logo. */
export function titleLogoOf(title: { titleLogoUrl?: string | undefined }) {
  return image("titleLogo", title.titleLogoUrl);
}

/** An Episode's Still. */
export function stillOf(episode: {
  stillUrl?: string | undefined;
  stillThumbhash?: Uint8Array | undefined;
}) {
  return image("still", episode.stillUrl, episode.stillThumbhash);
}

/** A Trailer's thumbnail. */
export function thumbnailOf(trailer: {
  thumbnailUrl?: string | undefined;
  thumbhash?: Uint8Array | undefined;
}) {
  return image("trailerThumbnail", trailer.thumbnailUrl, trailer.thumbhash);
}

/** A Person's headshot. */
export function headshotOf(person: { headshotUrl?: string | undefined }) {
  return image("headshot", person.headshotUrl);
}

/** A Plugin's logo, which it has none of as `""`. */
export function pluginLogoOf(plugin: { logoUrl: string }) {
  return image("pluginLogo", plugin.logoUrl);
}

/** An image an Artwork list offers, or one "Edit images" shows as it is now. */
export function artworkOf(url: string | undefined) {
  return image("artwork", url);
}
