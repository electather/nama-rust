import { type QueryKey, useQueryClient } from "@tanstack/react-query";
import { useRouteContext } from "@tanstack/react-router";
import { cn } from "cn";
import { type ChangeEvent, useId, useRef, useState } from "react";

import { Button, buttonVariants } from "@/components/ui/button";
import type { Movie, Season, Show } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { browseKey, seasonName, titleParam } from "@/lib/browse";
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
}

/** Each shape's preview: a Poster tall, a Backdrop wide. */
const shapes = {
  poster: "w-24 aspect-2/3",
  backdrop: "w-40 aspect-video",
};

/** The most bytes the Server takes for an image: 20 MiB. */
const MAX_UPLOAD_BYTES = 20 * 1024 * 1024;

/** What a kind of image may be, and what to read again once it changes. */
export interface ImageRules {
  /** The file types the picker offers. */
  accept: string;
  /** Why the Server refused an upload or removal, by its HTTP status. */
  refusals: Partial<Record<number, string>>;
  /** The queries that send its URL. */
  queryKey: QueryKey;
}

/** A title's Poster or Backdrop, which Browse sends. */
const titleImages: ImageRules = {
  accept: "image/jpeg,image/png,image/webp",
  refusals: {
    403: "This needs the Manage Libraries Permission.",
    413: "That file is over 20 MB. Pick a smaller one.",
    415: "That isn’t a JPEG, PNG or WebP image.",
    422: "That image is over 12 megapixels. Pick a smaller one.",
  },
  queryKey: browseKey,
};

/** A Movie's Poster and Backdrop, for {@link EditImages}. */
export function movieImages(movieId: ExternalId, movie: Movie): EditableImage[] {
  const path = `/custom-images/movie/${titleParam(movieId)}`;
  return [
    {
      name: "Poster",
      url: movie.posterUrl,
      custom: movie.customPoster,
      path: `${path}/poster`,
      shape: "poster",
    },
    {
      name: "Backdrop",
      url: movie.backdropUrl,
      custom: movie.customBackdrop,
      path: `${path}/backdrop`,
      shape: "backdrop",
    },
  ];
}

/** A Show's Poster and Backdrop, and the Poster of its Season `season`, for {@link EditImages}. */
export function showImages(showId: ExternalId, show: Show, season?: Season): EditableImage[] {
  const path = `/custom-images/show/${titleParam(showId)}`;
  const images: EditableImage[] = [
    {
      name: "Poster",
      url: show.posterUrl,
      custom: show.customPoster,
      path: `${path}/poster`,
      shape: "poster",
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
  });
  return images;
}

/**
 * With Manage Libraries, "Edit images": a dialog showing a title's `images`, each with a way to
 * upload a Custom Image in its place and to remove one. The page shows a change as soon as the
 * Server has it; other people's pages, the next time they load.
 */
export function EditImages({ title, images }: { title: string; images: EditableImage[] }) {
  const { signedIn } = useRouteContext({ from: "/_member" });
  const dialog = useRef<HTMLDialogElement>(null);
  const id = useId();
  if (!may(signedIn.user, Permission.MANAGE_LIBRARIES)) return null;
  return (
    <>
      <Button variant="ghost" aria-haspopup="dialog" onClick={() => dialog.current?.showModal()}>
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
          Everyone sees the images you upload in place of the downloaded ones, until you remove
          them. JPEG, PNG or WebP, up to 20 MB.
        </p>
        <div className="mt-6 flex flex-col gap-5">
          {images.map((image) => (
            <ImageChoice key={image.path} image={image} />
          ))}
        </div>
        <div className="mt-6 flex justify-end">
          <Button variant="ghost" onClick={() => dialog.current?.close()}>
            Done
          </Button>
        </div>
      </dialog>
    </>
  );
}

/** One image, with a way to change it. */
function ImageChoice({ image }: { image: EditableImage }) {
  return (
    <figure className="flex items-start gap-4">
      <Preview image={image} />
      <div className="min-w-0 flex-1">
        <figcaption className="text-sm font-medium">{image.name}</figcaption>
        {image.custom && <p className="text-sm text-muted-foreground">Custom image</p>}
        <ChangeImage image={image} rules={titleImages} />
      </div>
    </figure>
  );
}

/**
 * Upload and, for a Custom Image, Remove, for the image `image` names, and why the Server refused
 * either.
 */
export function ChangeImage({
  image,
  rules,
}: {
  image: Pick<EditableImage, "name" | "custom" | "path">;
  rules: ImageRules;
}) {
  const { upload, change, pending, error } = useChangeImage(image.path, rules);
  return (
    <>
      <div className="mt-2 -ms-3 flex flex-wrap">
        <UploadButton name={image.name} accept={rules.accept} pending={pending} onPicked={upload} />
        {image.custom && (
          <Button
            variant="ghost"
            aria-label={`Remove custom image: ${image.name}`}
            disabled={pending}
            onClick={() => void change({ method: "DELETE" })}
          >
            Remove custom image
          </Button>
        )}
      </div>
      {error !== undefined && (
        <p role="alert" className="text-sm text-pretty text-destructive">
          {error}
        </p>
      )}
    </>
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

/** A button that picks a file of a type in `accept` to upload as the image `name`. */
function UploadButton({
  name,
  accept,
  pending,
  onPicked,
}: {
  name: string;
  accept: string;
  pending: boolean;
  onPicked: (file: File) => void;
}) {
  function picked(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    // Cleared, so picking the same file again after a refusal still uploads it.
    event.target.value = "";
    if (file) onPicked(file);
  }
  return (
    <label
      className={cn(
        buttonVariants({ variant: "ghost" }),
        "cursor-pointer has-focus-visible:ring-3 has-focus-visible:ring-ring/50",
        pending && "pointer-events-none bg-muted",
      )}
    >
      Upload
      <input
        type="file"
        accept={accept}
        aria-label={`Upload a ${name}`}
        disabled={pending}
        onChange={picked}
        className="sr-only"
      />
    </label>
  );
}

/**
 * Uploads to or removes the Custom Image at `path` with `change`, then has the page read its images
 * again, and says why the Server refused.
 */
function useChangeImage(path: string, { refusals, queryKey }: ImageRules) {
  const queryClient = useQueryClient();
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string>();

  async function change(init: RequestInit) {
    setPending(true);
    setError(undefined);
    try {
      const response = await fetch(path, init);
      if (response.ok) {
        // The page reads its images from those queries, which now send the new URL.
        await queryClient.invalidateQueries({ queryKey });
      } else {
        setError(refusals[response.status] ?? "The Server couldn’t change the image. Try again.");
      }
    } catch {
      setError("Couldn’t reach the Server. Try again.");
    } finally {
      setPending(false);
    }
  }

  function upload(file: File) {
    // The Server refuses it too, but a browser can lose that answer to a body cut off partway.
    if (file.size > MAX_UPLOAD_BYTES) setError(refusals[413]);
    else void change({ method: "PUT", body: file });
  }

  return { upload, change, pending, error };
}
