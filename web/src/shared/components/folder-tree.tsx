import { Tree } from "nama-ui/components/tree";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { useListServerFolders, useServerRoots } from "@/shared/hooks/use-server-folders";
import { folderChain } from "@/shared/lib/folder-chain";
import { refusal } from "@/shared/lib/forms";

const words = {
  [RefusalReason.FOLDER_NOT_READABLE]: { form: "The Server can’t read this folder." },
  [RefusalReason.FOLDER_NOT_FOUND]: { form: "This folder isn’t on the Server anymore." },
};

/** The Server's folders as a tree, open down to `path`, whose picked folder `onPick` gets. */
export function FolderTree({ path, onPick }: { path: string; onPick: (path: string) => void }) {
  const roots = useServerRoots();
  const list = useListServerFolders();
  return (
    <Tree
      label="Folders on the Server"
      nodes={roots.map((folder) => asNode(folder))}
      openAt={folderChain(path)}
      empty="No folders"
      loadChildren={async (parent) => {
        try {
          const folders = await list(parent);
          return folders.map((folder) => asNode(folder));
        } catch (error) {
          // The tree shows the message as the folder's only row.
          const { form = "Couldn’t list this folder." } = refusal(
            error,
            "list this folder",
            {},
            words,
          );
          throw new Error(form, { cause: error });
        }
      }}
      onSelect={onPick}
    />
  );
}

function asNode({ name, path }: { name: string; path: string }) {
  return { id: path, label: name };
}
