import type { MessageInitShape } from "@bufbuild/protobuf";

import type { ListArtworkRequestSchema } from "@/gen/nama/v1/library_pb";

/** Each shape's preview: a Poster tall, a Backdrop wide. */
export const shapes = {
  poster: "w-24 aspect-2/3",
  backdrop: "w-40 aspect-video",
};

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
  /** The artwork lists asked for others to pick from, and which of them offers this image's. */
  artwork?: {
    asked: MessageInitShape<typeof ListArtworkRequestSchema>;
    list: "posterUrls" | "backdropUrls";
  };
}
