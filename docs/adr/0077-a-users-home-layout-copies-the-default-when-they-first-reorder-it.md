# A User's Home layout copies the default when they first reorder it

An Admin sets the default Home layout, and each User may change their own (#999). A User who has never reordered a row follows the default as the Admin changes it. Their first reorder saves their whole order, copied from the default at that moment, and from then on the Admin's reordering no longer moves their rows. Whether each row and Featured is shown is saved per row and apart from the order, so a User who only hides a row keeps following the default's order. A row a User hasn't placed, such as one from a newly enabled Plugin, goes to the end of their order. One the Admin hasn't placed goes to the end of the default and is shown. "Reset to default" drops the User's order and shown flags.

## Considered Options

- **A User's order as a diff on top of the default** ("move X above Y"): rejected. When the default changes, the diff can contradict it, and which one wins is hard to predict and hard to show in the settings list. With a copy, what the User sees is what they get.
- **New rows follow where the default puts them**: rejected. It needs the same merge, and a new Plugin's row could push down the row a User chose to put first.

## Consequences

- A User who reordered once doesn't see a later default reorder. "Reset to default" is how they pick it up.
- A saved layout keeps entries for rows the editor didn't see, such as a disabled Plugin's rows, each after the row it followed before. They're dropped only when the Plugin is uninstalled or the User is deleted.
