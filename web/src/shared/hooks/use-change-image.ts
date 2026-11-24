import { useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import { serverQueries } from "@/shared/hooks/server-key";
import type { ImageRules } from "@/shared/lib/image-rules";
import { onServer } from "@/shared/lib/server";

/** The most bytes the Server takes for an image: 20 MiB. */
const MAX_UPLOAD_BYTES = 20 * 1024 * 1024;

/**
 * Uploads to or removes the Custom Image at `path` on the Server with `change`, then has the page
 * read that Server's images again, and says why the Server refused.
 */
export function useChangeImage(path: string, { refusals }: ImageRules) {
  const queryClient = useQueryClient();
  const transport = useTransport();
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string>();

  async function change(init: RequestInit, url = path) {
    setPending(true);
    setError(undefined);
    try {
      const response = await fetch(onServer(transport, url), init);
      if (response.ok) {
        // Any of the Server's queries may send the image's URL, which is new now.
        await queryClient.invalidateQueries(serverQueries(transport));
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
