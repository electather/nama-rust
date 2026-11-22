import { Tabs as TabsPrimitive } from "@base-ui/react/tabs";

// The design system's tabs: a Muted track whose chosen tab is raised on the Rim, the shape that
// marks it besides its color. The row scrolls sideways once the tabs don't all fit, snapping to a
// tab's start so none rests cut in half.

function Tabs(props: TabsPrimitive.Root.Props) {
  return <TabsPrimitive.Root data-slot="tabs" className="flex min-h-0 flex-col" {...props} />;
}

function TabsList(props: TabsPrimitive.List.Props) {
  return (
    <TabsPrimitive.List
      data-slot="tabs-list"
      className="inline-flex h-9 w-fit max-w-full snap-x snap-mandatory scroll-px-0.75 scrollbar-none items-center overflow-x-auto rounded-lg bg-muted p-0.75"
      {...props}
    />
  );
}

function TabsTab(props: TabsPrimitive.Tab.Props) {
  return (
    <TabsPrimitive.Tab
      data-slot="tabs-tab"
      className="inline-flex h-full flex-none cursor-pointer snap-start items-center rounded-md border border-transparent px-3 text-sm font-medium whitespace-nowrap transition-colors outline-none select-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 aria-selected:border-input aria-selected:bg-input/30 aria-selected:shadow-xs"
      {...props}
    />
  );
}

function TabsPanel(props: TabsPrimitive.Panel.Props) {
  return (
    <TabsPrimitive.Panel
      data-slot="tabs-panel"
      className="flex min-h-0 flex-col outline-none"
      {...props}
    />
  );
}

export { Tabs, TabsList, TabsPanel, TabsTab };
