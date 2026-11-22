/** One option of a dropdown: the `value` it sends, the `label` it shows, and whether it can't be picked. */
export interface SelectOption {
  value: string;
  label: string;
  disabled?: boolean;
}
