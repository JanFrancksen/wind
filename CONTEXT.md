# Wind Browser

Wind is a desktop browser organized around tabs and their navigation state.

## Language

**Tab**:
A navigable page context with its own history and organizational state.

**Space**:
A named, colored tab workspace with its own Active Tab, recently closed Tabs, and isolated persistent browsing identity. Tabs in one Space share cookies and site storage; different Spaces do not. Space is the identity seam in Wind; there is no separate Profile concept.

**Active Space**:
The single Space whose Active Tab and sidebar organization are currently presented. Tabs in other Spaces may retain live renderer sessions while hidden.

**Active Tab**:
The single Tab currently presented for interaction.

**Open Tab**:
A Tab with a live navigation session. An Open Tab may be active or in the background.

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
