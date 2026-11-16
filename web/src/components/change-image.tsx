import { type QueryKey, useQueryClient } from "@tanstack/react-query";
import { cn } from "cn";
import { type ChangeEvent, useState } from "react";

import { ArtworkChoices, type Choices } from "@/components/artwork-choices";
import { Button, buttonVariants } from "@/components/ui/button";

/** The most bytes the Server takes for an image: 20 MiB. */
const MAX_UPLOAD_BYTES = 20 * 1024 * 1024;

/** What a kind of image may be, and what to read again once it changes. */
export interface ImageRules {
  /** The file types the picker offers. */
  accept: string;
  /** Why the Server refused an upload or removal, by its HTTP status. */
  refusals: Partial<Record<number, string>>;
  /** The queries that send its URL; `[]` is every query. */
  queryKey: QueryKey;
}

/**
 * Upload, a pick of `choices` if there are any, and, for a Custom Image, Remove, for the image
 * `image` names, and why the Server refused any of them.
 */
export function ChangeImage({
  image,
  rules,
  choices,
}: {
  image: {
    /** What it is, such as "Poster". */
    name: string;
    /** Whether it is a Custom Image, which Remove takes away. */
    custom: boolean;
    /** Where its Custom Image is uploaded to and removed from, as the Server names it. */
    path: string;
  };
  rules: ImageRules;
  choices?: Choices | undefined;
}) {
  const { upload, pick, change, pending, error } = useChangeImage(image.path, rules);
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
      {choices && (
        <ArtworkChoices name={image.name} choices={choices} pending={pending} onPicked={pick} />
      )}
      {error !== undefined && (
        <p role="alert" className="text-sm text-pretty text-destructive">
          {error}
        </p>
      )}
    </>
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

  async function change(init: RequestInit, url = path) {
    setPending(true);
    setError(undefined);
    try {
      const response = await fetch(url, init);
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

  function pick(artwork: string) {
    // The Server downloads it through its title source, so the request carries no body.
    void change({ method: "PUT" }, `${path}?${new URLSearchParams({ artwork }).toString()}`);
  }

  return { upload, pick, change, pending, error };
}
