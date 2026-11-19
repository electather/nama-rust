import { type RefObject, useSyncExternalStore } from "react";

/** Whether `videoRef`'s video plays in its own picture-in-picture window. */
export function usePictureInPicture(videoRef: RefObject<HTMLVideoElement | null>) {
  return useSyncExternalStore(
    (changed) => {
      const video = videoRef.current;
      video?.addEventListener("enterpictureinpicture", changed);
      video?.addEventListener("leavepictureinpicture", changed);
      return () => {
        video?.removeEventListener("enterpictureinpicture", changed);
        video?.removeEventListener("leavepictureinpicture", changed);
      };
    },
    () => videoRef.current !== null && document.pictureInPictureElement === videoRef.current,
  );
}
