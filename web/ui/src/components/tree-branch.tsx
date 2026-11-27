import { ChevronRight } from "lucide-react";
import type { KeyboardEvent, MouseEvent, RefObject } from "react";

/** A node of a `Tree`: the id `loadChildren` and `onSelect` get, and the name its row shows. */
interface TreeNode {
  id: string;
  label: string;
}

/** Which nodes are open, and each loaded node's children, or why they couldn't be loaded. */
interface Shown {
  open: Set<string>;
  children: Map<string, TreeNode[] | Error>;
}

/** What an opened node says in place of children it has none of, or is still loading. */
function noteFor(loaded: TreeNode[] | Error | undefined, empty: string) {
  if (loaded === undefined) {
    return "Loading…";
  }
  return loaded instanceof Error ? loaded.message : empty;
}

/** What a `Tree` hands each of its rows. */
interface Branches {
  shown: Shown;
  current: string | undefined;
  empty: string;
  rows: RefObject<Map<string, HTMLElement>>;
  /** Opens or closes the row's node on its chevron, else picks it. */
  onClick: (id: string, onChevron: boolean) => void;
  onKeyDown: (event: KeyboardEvent, id: string) => void;
}

/** A node's row, and once it has been opened, its children under it, indented on a guide line. */
function TreeBranch({ node, level, tree }: { node: TreeNode; level: number; tree: Branches }) {
  const { id, label } = node;
  const open = tree.shown.open.has(id);
  const loaded = tree.shown.children.get(id);
  const onClick = (event: MouseEvent) => {
    const onChevron =
      event.target instanceof Element && event.target.closest("[data-slot=tree-chevron]") !== null;
    tree.onClick(id, onChevron);
  };
  return (
    <>
      <div
        ref={(row) => {
          if (row !== null) {
            tree.rows.current.set(id, row);
          }
          return () => {
            tree.rows.current.delete(id);
          };
        }}
        role="treeitem"
        aria-level={level}
        aria-expanded={open}
        aria-selected={id === tree.current}
        tabIndex={id === tree.current ? 0 : -1}
        className="group flex h-11 cursor-default items-center gap-1 rounded-md pe-2 text-sm outline-none select-none hover:bg-accent focus-visible:ring-3 focus-visible:ring-ring/50 aria-selected:bg-accent aria-selected:font-medium pointer-fine:h-8"
        onClick={onClick}
        onKeyDown={(event) => {
          tree.onKeyDown(event, id);
        }}
      >
        <span
          data-slot="tree-chevron"
          className="grid size-7 shrink-0 place-items-center text-muted-foreground hover:text-foreground"
        >
          <ChevronRight
            aria-hidden
            className="size-4 transition-transform duration-200 ease-out-expo group-aria-expanded:rotate-90 motion-reduce:transition-none"
          />
        </span>
        <span className="truncate">{label}</span>
      </div>
      {(open || loaded !== undefined) && (
        <div
          // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- no tag groups a tree's items
          role="group"
          data-slot="tree-group"
          data-open={open ? "" : undefined}
          inert={!open}
          className="grid transition-all duration-200 ease-out-expo motion-reduce:transition-none"
        >
          <div className="ms-3.5 min-h-0 overflow-hidden border-s ps-1">
            {Array.isArray(loaded) && loaded.length > 0 ? (
              loaded.map((child) => (
                <TreeBranch key={child.id} node={child} level={level + 1} tree={tree} />
              ))
            ) : (
              <p className="flex h-11 items-center ps-7 text-sm text-muted-foreground pointer-fine:h-8">
                {noteFor(loaded, tree.empty)}
              </p>
            )}
          </div>
        </div>
      )}
    </>
  );
}

export { type Branches, type Shown, TreeBranch, type TreeNode };
