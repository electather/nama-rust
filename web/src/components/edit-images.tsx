import type { MessageInitShape } from "@bufbuild/protobuf";
import { useQuery } from "@connectrpc/connect-query";
import { useRouteContext } from "@tanstack/react-router";
import { cn } from "cn";
import { useId, useRef, useState } from "react";

import type { Choices } from "@/components/artwork-choices";
import { ChangeImage, type ImageRules } from "@/components/change-image";
import { Button } from "@/components/ui/button";
import type { Movie, Season, Show } from "@/gen/nama/v1/browse_pb";
import {
  LibraryService,
  type ListArtworkRequestSchema,
  type ListArtworkResponse,
} from "@/gen/nama/v1/library_pb";
// oxlint-disable-next-line import/max-dependencies -- the dialog puts together a title's images, changing each and its artwork lists
import { type ExternalId, TitleKind } from "@/gen/nama/v1/match_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { seasonName, titleParam } from "@/lib/browse";
import { may } from "@/lib/permissions";

/** One of a title's images as the Server sends it, and where its Custom Image goes. */
export interface EditableImage {
  /** What it is, such as "Poster" or "Season 2 Poster". */
  name: string;
  url: string | undefined;
  /** Whether `url` is a Custom Image, which removing brings the downloaded one back from. */
  custom: boolean;
  /** Where its Custom Image is uploaded to and removed from, as the Server names it. */
  path: string;
  shape: keyof typeof shapes;
  /** Which of the title's artwork lists offers others to pick from; a Season's has none. */
  artwork?: "posterUrls" | "backdropUrls";
}

/** Each shape's preview: a Poster tall, a Backdrop wide. */
const shapes = {
  poster: "w-24 aspect-2/3",
  backdrop: "w-40 aspect-video",
};

/**
 * A title's Poster or Backdrop, which Browse sends, and the Libraries too, since a title's Backdrop
 * can be its Library's.
 */
const titleImages: ImageRules = {
  accept: "image/jpeg,image/png,image/webp",
  refusals: {
    403: "This needs the Manage Libraries Permission.",
    413: "That file is over 20 MB. Pick a smaller one.",
    415: "That isn’t a JPEG, PNG or WebP image.",
    422: "That image is over 12 megapixels. Pick a smaller one.",
    404: "That image isn’t offered any more. Close Edit images and open it again.",
    503: "The title source couldn’t be reached. Try again later.",
  },
  queryKey: [],
};

/** A title's images and the artwork lists to pick others from, for {@link EditImages}. */
export interface TitleImages {
  images: EditableImage[];
  artwork: MessageInitShape<typeof ListArtworkRequestSchema>;
}

/** A Movie's Poster and Backdrop, for {@link EditImages}. */
export function movieImages(movieId: ExternalId, movie: Movie): TitleImages {
  const path = `/custom-images/movie/${titleParam(movieId)}`;
  const images: EditableImage[] = [
    {
      name: "Poster",
      url: movie.posterUrl,
      custom: movie.customPoster,
      path: `${path}/poster`,
      shape: "poster",
      artwork: "posterUrls",
    },
    {
      name: "Backdrop",
      url: movie.backdropUrl,
      custom: movie.customBackdrop,
      path: `${path}/backdrop`,
      shape: "backdrop",
      artwork: "backdropUrls",
    },
  ];
  return { images, artwork: { kind: TitleKind.MOVIE, id: movieId } };
}

/** A Show's Poster and Backdrop, and the Poster of its Season `season`, for {@link EditImages}. */
export function showImages(showId: ExternalId, show: Show, season?: Season): TitleImages {
  const path = `/custom-images/show/${titleParam(showId)}`;
  const images: EditableImage[] = [
    {
      name: "Poster",
      url: show.posterUrl,
      custom: show.customPoster,
      path: `${path}/poster`,
      shape: "poster",
      artwork: "posterUrls",
    },
  ];
  if (season) {
    images.push({
      name: `${seasonName(season.number)} Poster`,
      url: season.posterUrl,
      custom: season.customPoster,
      path: `${path}/season/${String(season.number)}/poster`,
      shape: "poster",
    });
  }
  images.push({
    name: "Backdrop",
    url: show.backdropUrl,
    custom: show.customBackdrop,
    path: `${path}/backdrop`,
    shape: "backdrop",
    artwork: "backdropUrls",
  });
  return { images, artwork: { kind: TitleKind.SHOW, id: showId } };
}

/**
 * With Manage Libraries, "Edit images": a dialog showing a title's `images`, each with a way to
 * upload a Custom Image in its place, or pick one from the title's `artwork` lists, and to remove
 * one. The lists are asked for each time it opens. The page shows a change as soon as the Server
 * has it; other people's pages, the next time they load.
 */
export function EditImages({ title, images, artwork }: TitleImages & { title: string }) {
  const { signedIn } = useRouteContext({ from: "/_member" });
  const dialog = useRef<HTMLDialogElement>(null);
  const id = useId();
  // How many times it has opened, which keys the list, so each opening asks for the artwork anew.
  const [opened, setOpened] = useState(0);
  if (!may(signedIn.user, Permission.MANAGE_LIBRARIES)) return null;
  function open() {
    dialog.current?.showModal();
    setOpened(opened + 1);
  }
  return (
    <>
      <Button variant="ghost" aria-haspopup="dialog" onClick={open}>
        Edit images
      </Button>
      {/* ponytail: the native modal <dialog> traps focus, closes on Escape and returns focus itself. */}
      <dialog
        ref={dialog}
        aria-labelledby={`${id}-title`}
        aria-describedby={`${id}-description`}
        // Floats above the page, so it alone gets a shadow (DESIGN.md, "Elevation").
        className="m-auto w-full max-w-sm rounded-xl border bg-popover p-6 text-popover-foreground shadow-xl backdrop:bg-scrim sm:max-w-lg"
      >
        <h2 id={`${id}-title`} className="text-lg font-semibold tracking-tight text-balance">
          Images of {title}
        </h2>
        <p id={`${id}-description`} className="mt-2 text-sm text-pretty text-muted-foreground">
          Everyone sees the images you upload or pick in place of the downloaded ones, until you
          remove them. Uploads are JPEG, PNG or WebP, up to 20 MB.
        </p>
        {opened > 0 && <ImageChoices key={opened} images={images} artwork={artwork} />}
        <div className="mt-6 flex justify-end">
          <Button variant="ghost" onClick={() => dialog.current?.close()}>
            Done
          </Button>
        </div>
      </dialog>
    </>
  );
}

/** Each of `images`, with the artwork the title's `artwork` lists offer in its place. */
function ImageChoices({ images, artwork }: TitleImages) {
  const listed = useQuery(LibraryService.method.listArtwork, artwork).data;
  return (
    <div className="mt-6 flex flex-col gap-5">
      {images.map((image) => (
        <ImageChoice key={image.path} image={image} choices={choicesOf(image, listed)} />
      ))}
    </div>
  );
}

/** The artwork `listed` offers in place of `image`, once it has arrived, if `image` has a list. */
function choicesOf(
  image: EditableImage,
  listed: ListArtworkResponse | undefined,
): Choices | undefined {
  if (image.artwork === undefined || listed === undefined) return undefined;
  return { urls: listed[image.artwork], shape: image.shape };
}

/** One image, with a way to change it, and `choices` to pick from if a title source lists any. */
export function ImageChoice({
  image,
  choices,
}: {
  image: EditableImage;
  choices?: Choices | undefined;
}) {
  return (
    <figure className="flex items-start gap-4">
      <Preview image={image} />
      <div className="min-w-0 flex-1">
        <figcaption className="text-sm font-medium">{image.name}</figcaption>
        {image.custom && <p className="text-sm text-muted-foreground">Custom image</p>}
        <ChangeImage image={image} rules={titleImages} choices={choices} />
      </div>
    </figure>
  );
}

/** The image as it is now, in its shape's frame, which is there before it loads. */
function Preview({ image }: { image: EditableImage }) {
  return (
    <div className={cn("shrink-0 overflow-hidden rounded-lg border bg-muted", shapes[image.shape])}>
      {image.url !== undefined && (
        <img src={image.url} alt={image.name} className="size-full object-cover" />
      )}
    </div>
  );
}
