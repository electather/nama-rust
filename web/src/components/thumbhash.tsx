import { useMemo } from "react";
import { thumbHashToDataURL } from "thumbhash";

/**
 * The blurred preview a `thumbhash` holds, filling its box behind the image it stands in for, so
 * it shows until that image loads and covers it. The box must be `relative` and `isolate`, and
 * the image come after this. Nothing without a thumbhash.
 */
export function Thumbhash({ thumbhash }: { thumbhash: Uint8Array | undefined }) {
  const preview = useMemo(
    () => (thumbhash === undefined ? undefined : thumbHashToDataURL(thumbhash)),
    [thumbhash],
  );
  if (preview === undefined) return null;
  return <img src={preview} alt="" className="absolute inset-0 -z-10 size-full object-cover" />;
}
