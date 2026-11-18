import { type RefObject, createContext, use } from "react";

import type { ListHandle, Pick } from "@/features/action-bar/lib/menu";

/** Which row of the open action bar is selected, and what picking one does. */
export interface Selection {
  /** The rows' ids are this and their index. */
  idPrefix: string;
  selected: number;
  onSelect: (index: number) => void;
  onPick: (pick: Pick) => void;
  /** Where the list shown registers itself, for the field's keys. */
  list: RefObject<ListHandle | null>;
}

/** The open action bar's `Selection`. */
export const SelectionContext = createContext<Selection | undefined>(undefined);

/** The open action bar's `Selection`, for a row or the list. */
export function useSelection() {
  const selection = use(SelectionContext);
  if (selection === undefined) {
    throw new Error("useSelection is only for the action bar's rows");
  }
  return selection;
}
