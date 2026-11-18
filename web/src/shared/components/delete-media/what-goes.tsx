import { useState } from "react";

import { FilesThatGo } from "@/shared/components/delete-media/files-that-go";
import { FormSelect } from "@/shared/components/form-field/form-select";
import { type Deletable, EVERY_VERSION } from "@/shared/lib/deletable";
import { metadataLine, sizeOf } from "@/shared/lib/title-facts";
import { versionLine } from "@/shared/lib/version-facts";

/** The choice of Version, when there are several, and the files the choice deletes. */
export function WhatGoes(deletable: Deletable) {
  const [chosen, setChosen] = useState(EVERY_VERSION);
  return (
    <>
      {deletable.versions.length > 1 && (
        <FormSelect
          label="What to delete"
          name="version"
          value={chosen}
          onChange={(event) => {
            setChosen(event.target.value);
          }}
        >
          <option value={EVERY_VERSION}>Every Version</option>
          {deletable.versions.map((version) => (
            <option key={String(version.id)} value={String(version.id)}>
              {version.fileName} ({metadataLine(versionLine(version), sizeOf(version.size))})
            </option>
          ))}
        </FormSelect>
      )}
      <FilesThatGo deletable={deletable} chosen={chosen} />
    </>
  );
}
