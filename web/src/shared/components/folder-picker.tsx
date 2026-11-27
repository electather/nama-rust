import { FolderOpen } from "lucide-react";
import { Button } from "nama-ui/components/button";
import { Popover, PopoverContent, PopoverTrigger } from "nama-ui/components/popover";
import { useState } from "react";

import { FolderTreeLoader } from "@/shared/components/folder-tree-loader";
import { useModalContainer } from "@/shared/hooks/use-modal-container";
import { useListServerFolders } from "@/shared/hooks/use-server-folders";
import { folderChain } from "@/shared/lib/folder-chain";

/**
 * The button beside a field that takes a folder on the Server, opening a tree of its folders open
 * down to `path`, the field's own. Picking one hands `onPick` its path and closes the tree, as
 * Escape does.
 */
export function FolderPicker({ path, onPick }: { path: string; onPick: (path: string) => void }) {
  const [open, setOpen] = useState(false);
  const [trigger, container] = useModalContainer();
  const list = useListServerFolders();
  return (
    <Popover
      open={open}
      onOpenChange={(opening) => {
        setOpen(opening);
        if (opening) {
          // Every level at once, beside the top's, for the tree to open as they arrive.
          void Promise.allSettled(
            folderChain(path).map(async (level) => {
              await list(level);
            }),
          );
        }
      }}
    >
      <PopoverTrigger
        ref={trigger}
        render={<Button variant="outline" size="icon-sm" />}
        aria-label="Pick a folder on the Server"
      >
        <FolderOpen aria-hidden />
      </PopoverTrigger>
      <PopoverContent container={container} side="bottom" width="list" flip>
        <FolderTreeLoader
          path={path}
          onPick={(picked) => {
            setOpen(false);
            onPick(picked);
          }}
        />
      </PopoverContent>
    </Popover>
  );
}
