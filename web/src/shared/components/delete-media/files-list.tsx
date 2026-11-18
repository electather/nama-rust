import type { MediaFile } from "@/gen/nama/v1/library_pb";
import { sizeOf } from "@/shared/lib/title-facts";

/** The files that go, and the space they free. */
export function FilesList({ files, bytesFreed }: { files: MediaFile[]; bytesFreed: bigint }) {
  return (
    <div>
      <p>
        {files.length === 1 ? "This file goes" : `These ${files.length} files go`}, freeing{" "}
        {sizeOf(bytesFreed)}:
      </p>
      <ul className="mt-2 flex max-h-48 flex-col gap-1 overflow-y-auto text-muted-foreground">
        {/* Versions in different folders can have files named alike, so a name isn't a key. */}
        {files.map((file, index) => (
          <li key={`${String(index)} ${file.fileName}`} className="break-all">
            {file.fileName}
          </li>
        ))}
      </ul>
    </div>
  );
}
