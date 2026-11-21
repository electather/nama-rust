import { useEffect, useRef } from "react";

import type { FrameCrop } from "@/features/player/lib/preview-frame";

// Each sheet, fetched and decoded once, so a frame is only copied out of it, however fast the
// pointer moves. Showing a 3200px sheet as a scaled CSS background instead had the browser
// rescale it on every move, and the card's paint fell behind the pointer.
// ponytail: kept for the page's life, a few sheets per Version played; drop them on leaving the
// player if memory matters.
const sheets = new Map<string, HTMLImageElement>();

function sheetOf(src: string) {
  const known = sheets.get(src);
  if (known) {
    return known;
  }
  const image = new Image();
  image.src = src;
  sheets.set(src, image);
  return image;
}

function draw(canvas: HTMLCanvasElement | null, image: HTMLImageElement, crop: FrameCrop) {
  const { x, y, width, height } = crop;
  canvas?.getContext("2d")?.drawImage(image, x, y, width, height, 0, 0, width, height);
}

/** The Preview frame `crop` picks, drawn as soon as its sheet is decoded. */
export function PreviewFrame({ crop }: { crop: FrameCrop }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const image = sheetOf(crop.src);
    let current = true;
    async function drawDecoded() {
      try {
        await image.decode();
      } catch {
        // A sheet that fails to load leaves the box blank.
        return;
      }
      if (current) {
        draw(ref.current, image, crop);
      }
    }
    void drawDecoded();
    return () => {
      current = false;
    };
  }, [crop]);
  return (
    <canvas
      ref={ref}
      width={crop.width}
      height={crop.height}
      data-sheet={crop.src}
      data-at={`${crop.x},${crop.y}`}
      className="h-auto w-full rounded-lg bg-muted"
    />
  );
}
