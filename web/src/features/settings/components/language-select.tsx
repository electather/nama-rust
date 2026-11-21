import { SearchSelect } from "@/features/settings/components/search-select";
import { languageName, languages } from "@/features/settings/lib/languages";
import type { RowControl } from "@/shared/components/setting-row";

const listed = languages.map(({ code, name }) => ({ value: code, label: name }));

/**
 * A dropdown of every language by name, with a search, under `none`, which stands for no language. A saved code
 * that isn't on the list stays an option, so opening the pane never changes it.
 */
export function LanguageSelect({
  control,
  none,
  value,
  onChange,
}: {
  control: RowControl;
  none: string;
  value: string;
  onChange: (code: string) => void;
}) {
  const known = value === "" || listed.some((language) => language.value === value);
  const saved = known ? [] : [{ value, label: languageName(value) }];
  return (
    <SearchSelect
      control={control}
      options={[{ value: "", label: none }, ...saved, ...listed]}
      value={value}
      search="Search languages"
      onChange={onChange}
    />
  );
}
