import { useNavigate } from "@tanstack/react-router";
import {
  type KeyboardEvent,
  type MouseEvent,
  useDeferredValue,
  useEffect,
  useId,
  useRef,
  useState,
} from "react";

import { MenuBody } from "@/features/action-bar/components/menu-body";
import { MenuFooter } from "@/features/action-bar/components/menu-footer";
import { MenuHeader } from "@/features/action-bar/components/menu-header";
import { SelectionContext } from "@/features/action-bar/hooks/use-selection";
import type { FoundTitle } from "@/features/action-bar/lib/found-titles";
import type { Frame, ListHandle, Opening, Page, Pick } from "@/features/action-bar/lib/menu";

/**
 * The action bar, open on `opening`: a modal `<dialog>`, so the browser traps and returns focus.
 * Esc steps back out of a narrowed search or the shortcuts before it closes. `onOpenTitle` hears
 * of each title opened from it, and `onClose` of its closing.
 */
export function ActionBarDialog({
  opening,
  pages,
  recents,
  onOpenTitle,
  onClose,
}: {
  opening: Opening;
  pages: Page[];
  recents: FoundTitle[];
  onOpenTitle: (title: FoundTitle) => void;
  onClose: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const list = useRef<ListHandle>(null);
  const idPrefix = useId();
  const [frame, setFrame] = useState(opening.frame);
  const [query, setQuery] = useState(opening.query);
  const [selected, setSelected] = useState(0);
  const typed = query.trim();
  const searched = useDeferredValue(frame === "shortcuts" ? "" : typed, "");
  const navigate = useNavigate();
  const activeId = `${idPrefix}-${selected}`;
  useEffect(() => {
    dialog.current?.showModal();
  }, []);
  useEffect(() => {
    document.querySelector(`#${CSS.escape(activeId)}`)?.scrollIntoView({ block: "nearest" });
  }, [activeId]);

  const show = (next: Frame) => {
    setFrame(next);
    setQuery("");
    setSelected(0);
    dialog.current?.querySelector("input")?.focus();
  };
  const close = () => {
    dialog.current?.close();
  };
  const onPick = (pick: Pick) => {
    if (pick.kind === "frame") {
      show(pick.frame);
    } else if (pick.kind === "title") {
      onOpenTitle(pick.title);
      void navigate(pick.title.link);
      close();
    } else if (pick.kind === "page") {
      void navigate({ to: pick.page.to });
      close();
    }
  };
  const back = () => {
    if (frame === "root") {
      close();
    } else {
      show("root");
    }
  };
  const move = (by: number) => {
    const count = list.current?.count ?? 0;
    if (count > 0) {
      setSelected((selected + by + count) % count);
    }
    return true;
  };
  const fieldKeys: Partial<Record<string, () => boolean>> = {
    ArrowDown: () => move(1),
    ArrowUp: () => move(-1),
    Enter: () => {
      list.current?.pick(selected);
      return true;
    },
    Escape: () => {
      back();
      return true;
    },
    Backspace: () => {
      if (query !== "" || frame === "root") {
        return false;
      }
      show("root");
      return true;
    },
  };
  const modKeys: Partial<Record<string, () => void>> = {
    k: close,
    "/": () => {
      show("shortcuts");
    },
  };
  const onKey = (event: KeyboardEvent<HTMLInputElement>) => {
    if (fieldKeys[event.key]?.() === true) {
      event.preventDefault();
    }
  };
  const onShortcut = (event: KeyboardEvent<HTMLDialogElement>) => {
    const action = event.metaKey || event.ctrlKey ? modKeys[event.key.toLowerCase()] : undefined;
    if (action !== undefined) {
      event.preventDefault();
      action();
    }
  };

  return (
    <SelectionContext value={{ idPrefix, selected, onSelect: setSelected, onPick, list }}>
      {/* oxlint-disable-next-line jsx-a11y/no-noninteractive-element-interactions -- a click on a modal's backdrop lands on the dialog; Esc closes it from the keyboard */}
      <dialog
        ref={dialog}
        aria-label="Command menu"
        onClose={onClose}
        onKeyDown={onShortcut}
        onClick={(event) => {
          if (clickedBackdrop(event)) {
            close();
          }
        }}
        className="mx-4 mt-24 mb-auto w-auto max-w-160 flex-col overflow-hidden rounded-xl border bg-popover p-1 text-popover-foreground shadow-float backdrop:bg-scrim open:flex open:animate-in open:duration-150 open:ease-out-expo open:fade-in-0 open:zoom-in-95 motion-reduce:open:animate-none sm:mx-auto"
      >
        <MenuHeader
          frame={frame}
          query={query}
          combobox={{ listId: `${idPrefix}-list`, activeId }}
          onType={(value) => {
            setQuery(value);
            setSelected(0);
          }}
          onKey={onKey}
          onBack={() => {
            show("root");
          }}
        />
        <MenuBody state={{ frame, typed, searched, pages, recents }} listId={`${idPrefix}-list`} />
        <MenuFooter />
      </dialog>
    </SelectionContext>
  );
}

/** Whether a click on the dialog landed on its backdrop, outside its box. */
function clickedBackdrop(event: MouseEvent<HTMLDialogElement>) {
  const { left, right, top, bottom } = event.currentTarget.getBoundingClientRect();
  const { clientX: x, clientY: y } = event;
  return (
    event.target === event.currentTarget && Math.min(x - left, right - x, y - top, bottom - y) < 0
  );
}
