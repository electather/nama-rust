import { useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import type { ImageRules } from "@/shared/lib/image-rules";

/** The most bytes the Server takes for an image: 20 MiB. */
const MAX_UPLOAD_BYTES = 20 * 1024 * 1024;

/**
 * Uploads to or removes the Custom Image at `path` with `change`, then has the page read its images
 * again, and says why the Server refused.
 */
export function useChangeImage(path: string, { refusals, queryKey }: ImageRules) {
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
    if (file.size > MAX_UPLOAD_BYTES) {
      setError(refusals[413]);
    } else {
      void change({ method: "PUT", body: file });
    }
  }

  function pick(artwork: string) {
    // The Server downloads it through its title source, so the request carries no body.
    void change({ method: "PUT" }, `${path}?${new URLSearchParams({ artwork }).toString()}`);
  }

  return { upload, pick, change, pending, error };
}
