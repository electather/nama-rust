// Written from scratch to look like `@unlumen-ui/file-tree`'s rendered demo, never from its source:
// Unlumen UI's licence forbids redistributing its components, modified ones included, and this repo
// is public AGPL-3.0. It also has no lazy loading, ARIA or selection, which this tree needs.
// ponytail: every row is in the page, as the Server pages no listing; virtualise it with that.
import {
  type Branches,
  type Shown,
  TreeBranch,
  type TreeNode,
} from "nama-ui/components/tree-branch";
import { type KeyboardEvent, useEffect, useRef, useState } from "react";

/** A row the keyboard moves through, with its parent's id, which ← moves to. */
interface Row {
  id: string;
  parent: string | undefined;
}

function visibleRows(nodes: TreeNode[], shown: Shown, parent?: string): Row[] {
  return nodes.flatMap(({ id }) => {
    const loaded = shown.children.get(id);
    const below = shown.open.has(id) && Array.isArray(loaded) ? visibleRows(loaded, shown, id) : [];
    return [{ id, parent }, ...below];
  });
}

/**
 * A tree whose nodes load their children as they're opened, as the APG tree pattern has it without
 * typeahead: ↑ and ↓ move, → opens or moves in, ← closes or moves out, Home and End jump, and Enter
 * or a click picks. Every node may open: one with no children says `empty`, and one whose
 * `loadChildren` rejects says the error's message. `openAt` names nodes from a root down, which
 * open at once, the deepest of them shown selected: pass `[]` to open none.
 */
function Tree({
  label,
  nodes,
  openAt,
  empty,
  loadChildren,
  onSelect,
}: {
  label: string;
  nodes: TreeNode[];
  openAt: string[];
  empty: string;
  loadChildren: (id: string) => Promise<TreeNode[]>;
  onSelect: (id: string) => void;
}) {
  const [shown, setShown] = useState<Shown>(() => ({ open: new Set(openAt), children: new Map() }));
  const [active, setActive] = useState<string>();
  const asked = useRef(new Set<string>());
  const rows = useRef(new Map<string, HTMLElement>());
  const tree = useRef<HTMLDivElement>(null);
  const visible = visibleRows(nodes, shown);
  const ids = new Set(visible.map((row) => row.id));
  let deepest: string | undefined;
  for (const id of openAt) {
    deepest = ids.has(id) ? id : deepest;
  }
  const tabStop = active !== undefined && ids.has(active) ? active : (deepest ?? visible[0]?.id);

  // Each open node loads its children once, those `openAt` names all at once.
  useEffect(() => {
    for (const id of shown.open) {
      if (!asked.current.has(id)) {
        asked.current.add(id);
        void loadInto(id);
      }
    }
    async function loadInto(id: string) {
      const loaded = await loadChildren(id).catch((error: unknown) =>
        error instanceof Error ? error : new Error(String(error)),
      );
      setShown((now) => ({ ...now, children: new Map(now.children).set(id, loaded) }));
    }
  }, [shown.open, loadChildren]);

  // The selected row comes into view as the levels above it load, and takes the focus from a row
  // above it that had it first.
  useEffect(() => {
    const row = tabStop === undefined ? undefined : rows.current.get(tabStop);
    if (tree.current?.contains(document.activeElement) === true) {
      row?.focus();
    } else {
      row?.scrollIntoView({ block: "nearest" });
    }
  }, [tabStop]);

  function toggle(id: string) {
    const open = new Set(shown.open);
    if (!open.delete(id)) {
      open.add(id);
    }
    setShown({ ...shown, open });
  }

  // A click moves the tab stop too, or the row the click focused and the selected one would differ.
  function click(id: string, onChevron: boolean) {
    setActive(id);
    (onChevron ? toggle : onSelect)(id);
  }

  function move(row: Row | undefined) {
    if (row !== undefined) {
      setActive(row.id);
      rows.current.get(row.id)?.focus();
    }
  }

  function onKeyDown(event: KeyboardEvent, id: string) {
    const at = visible.findIndex((row) => row.id === id);
    const next = visible[at + 1];
    const open = shown.open.has(id);
    const moves: Record<string, Row | undefined> = {
      ArrowDown: next,
      ArrowUp: visible[at - 1],
      Home: visible[0],
      End: visible.at(-1),
      ArrowRight: open && next?.parent === id ? next : undefined,
      ArrowLeft: open ? undefined : visible.find((row) => row.id === visible[at]?.parent),
    };
    if (event.key === "Enter") {
      onSelect(id);
    } else if (event.key === (open ? "ArrowLeft" : "ArrowRight")) {
      toggle(id);
    } else if (event.key in moves) {
      move(moves[event.key]);
    } else {
      return;
    }
    event.preventDefault();
  }

  const branches: Branches = {
    shown,
    current: tabStop,
    empty,
    rows,
    onClick: click,
    onKeyDown,
  };
  return (
    <div
      ref={tree}
      role="tree"
      aria-label={label}
      className="max-h-80 overflow-y-auto overscroll-contain p-1"
    >
      {nodes.map((node) => (
        <TreeBranch key={node.id} node={node} level={1} tree={branches} />
      ))}
    </div>
  );
}

export { Tree };
