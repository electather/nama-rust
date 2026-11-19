/** A choice's name, then what tells it apart, quieter. */
export function ChoiceLabel({ name, details }: { name: string; details: string }) {
  return (
    <>
      {name}
      {details !== "" && <span className="font-normal text-muted-foreground"> {details}</span>}
    </>
  );
}
