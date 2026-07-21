# Wind Browser

Wind is a desktop browser organized around tabs and their navigation state.

## Language

**Tab**:
A navigable page context with its own history and organizational state.

**Space**:
A named, colored tab workspace with its own Active View, recently closed Tabs, and isolated persistent browsing identity. Tabs in one Space share cookies and site storage; different Spaces do not. Space is the identity seam in Wind; there is no separate Profile concept.

**Active Space**:
The single Space whose Active View and sidebar organization are currently presented. Tabs in other Spaces may retain live renderer sessions while hidden.

**Active View**:
The single presentation selected in the Active Space. It is either one Tab or a Split View.

**Split View**:
A persistent pairing of two Open Tabs from the same Space, presented side by side. Each Tab belongs to at most one Split View.

**Split Pane**:
The left or right position in a Split View. A Split Pane presents a Tab but does not own its navigation state.

**Active Tab**:
The Tab currently focused for interaction. In a Split View, either Split Pane may contain the Active Tab.

**Visible Tab**:
An Open Tab presented in the Active View. A single-tab Active View has one Visible Tab; a Split View has two.

**Open Tab**:
A Tab with a live navigation session. An Open Tab may be active, visible without focus, or in the background.

**Organized Tab**:
A Pinned or Highlight Tab. Organized Tabs retain their identity when their live navigation session is closed.

**Closed Organized Tab**:
An Organized Tab retained only as a saved pinned destination, without a live navigation session. Unpinning it deletes the Tab.

**Today Tab**:
An unpinned Tab in the ordinary browsing list. Duplicating any Tab creates a Today Tab.

**Pinned Tab**:
An organized Tab with a pinned destination, shown in the pinned list.

**Highlight Tab**:
An organized Tab with a pinned destination, shown in the prominent grid instead of the pinned list.

**Pinned Destination**:
The location an organized Tab returns to after browsing away from it.

**Tab Action**:
A requested change targeting an existing Tab, including explicit selection, navigation, organization, and lifecycle changes. A Tab Action does not implicitly change the Active Tab; opening a new Tab and reopening a closed Tab are collection behavior.
_Avoid_: Tab command
