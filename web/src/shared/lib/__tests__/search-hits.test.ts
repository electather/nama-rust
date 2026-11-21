import { create, type MessageInitShape } from "@bufbuild/protobuf";
import { expect, test } from "vite-plus/test";

import { type DoneSchema, SearchResponseSchema } from "@/gen/nama/v1/search_pb";
import { searchNote } from "@/shared/lib/search-hits";

/** What a search streamed when it ended with `done`. */
function endingWith(done: MessageInitShape<typeof DoneSchema>) {
  return [create(SearchResponseSchema, { part: { case: "done", value: done } })];
}

test("a search with nothing missing has no note", () => {
  expect(searchNote(endingWith({}))).toBe("");
});

test("a search names the Search sources that didn't answer", () => {
  expect(searchNote(endingWith({ failedSearchSources: ["dreams"] }))).toBe(
    "The Search source dreams didn’t answer, so some titles may be missing.",
  );
  expect(searchNote(endingWith({ failedSearchSources: ["dreams", "meaning"] }))).toBe(
    "The Search sources dreams and meaning didn’t answer, so some titles may be missing.",
  );
});

test("a search whose title source failed too says both", () => {
  const done = { titleSourceUnavailable: true, failedSearchSources: ["dreams"] };

  expect(searchNote(endingWith(done))).toBe(
    "Titles to request couldn’t be searched, so only this Server’s are listed. " +
      "The Search source dreams didn’t answer, so some titles may be missing.",
  );
});
