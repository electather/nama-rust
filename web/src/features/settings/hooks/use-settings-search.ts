import { useNavigate, useSearch } from "@tanstack/react-router";
import { type KeyboardEvent, useState } from "react";

import { useServerDefined } from "@/features/settings/data/use-server-defined";
import { type SectionGroup, sectionSearch } from "@/features/settings/lib/sections";
import {
  type Findable,
  findableId,
  findables,
  matching,
} from "@/features/settings/lib/setting-search";
import type { User } from "@/gen/nama/v1/sign_in_pb";

/** Settings' search as it's typed in: what's typed, what it found, and the result picked. */
export type SearchState = ReturnType<typeof useSettingsSearch>;

/**
 * Settings' search over the Sections of `groups` and the settings in them `user` sees. Down and Up
 * pick a result, Enter opens it and Escape clears the field. Opening one clears it too.
 */
export function useSettingsSearch(groups: SectionGroup[], user: User | undefined) {
  const [typed, setTyped] = useState("");
  // By identity, so a result stays picked as the Server's settings join around it.
  const [pickedId, setPickedId] = useState<string>();
  const navigate = useNavigate();
  const opened = useSearch({
    strict: false,
    select: ({ settings, setting }) => ({ settings, setting }),
  });
  const results = matching(findables(groups, user, useServerDefined(typed !== "", groups)), typed);
  const at = Math.max(
    0,
    results.findIndex((result) => findableId(result) === pickedId),
  );

  function type(words: string) {
    setTyped(words);
    setPickedId(undefined);
  }

  async function landOn({ section, key }: Findable) {
    // Sections share one history entry, as their links do, so Back closes Settings.
    const go = async (setting?: string) => {
      await navigate({
        to: ".",
        search: (rest) => ({ ...rest, ...sectionSearch(section, setting) }),
        replace: true,
      });
    };
    // Asked for again, the setting open would leave the address as it is and land nowhere.
    if (key !== undefined && opened.settings === section && opened.setting === key) {
      await go();
    }
    await go(key);
  }

  function handleOpen(result: Findable) {
    type("");
    void landOn(result);
  }

  function pick(index: number) {
    const result = results[Math.min(Math.max(index, 0), results.length - 1)];
    setPickedId(result && findableId(result));
  }

  const keys = new Map([
    [
      "ArrowDown",
      () => {
        pick(at + 1);
      },
    ],
    [
      "ArrowUp",
      () => {
        pick(at - 1);
      },
    ],
    [
      "Enter",
      () => {
        const result = results[at];
        if (result) {
          handleOpen(result);
        }
      },
    ],
    // Escape clears the field before it closes Settings.
    [
      "Escape",
      () => {
        type("");
      },
    ],
  ]);

  function handleKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    const press = keys.get(event.key);
    // Enter that ends composing a word, as in Japanese, is the input method's.
    if (typed !== "" && press !== undefined && !event.nativeEvent.isComposing) {
      event.preventDefault();
      press();
    }
  }

  return { typed, results, picked: results[at], type, handleOpen, handleKeyDown };
}
