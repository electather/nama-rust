/** Each shape's artwork to pick from, smaller than the preview, and the width it's shown at. */
export const choiceShapes = {
  poster: { className: "w-12 aspect-2/3", sizes: "48px" },
  backdrop: { className: "w-24 aspect-video", sizes: "96px" },
};

/** Artwork to pick as an image: its URLs, best first, and the shape to show them in. */
export interface Choices {
  urls: string[];
  shape: keyof typeof choiceShapes;
}
