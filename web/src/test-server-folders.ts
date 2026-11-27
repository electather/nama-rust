import { Code } from "@connectrpc/connect";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import type { ListServerFoldersRequest } from "@/gen/nama/v1/server_pb";
import { refused } from "@/test-refusal";

/** A Server's folders: the folders in each, by path, `""` for the top, or that it can't be read. */
export type Disk = Record<string, string[] | "unreadable">;

/**
 * `ListServerFolders` answered from `disk`, recording each path it's asked for in `asked`. A path
 * `disk` doesn't name isn't on the Server.
 */
export function listFoldersOf(disk: Disk, asked: string[] = []) {
  return ({ path }: ListServerFoldersRequest) => {
    asked.push(path);
    const folders = disk[path];
    if (folders === undefined || folders === "unreadable") {
      const reason =
        folders === undefined ? RefusalReason.FOLDER_NOT_FOUND : RefusalReason.FOLDER_NOT_READABLE;
      throw refused(Code.InvalidArgument, `${path} can't be listed`, {
        reason,
        field: "path",
        params: { folder: path },
      });
    }
    return {
      folders: folders.map((folder) => ({
        name: folder.slice(folder.lastIndexOf("/") + 1),
        path: folder,
      })),
    };
  };
}
