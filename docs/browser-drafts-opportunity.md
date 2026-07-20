# Browser opportunity: Drafts

_Research date: 2026-07-20. Sources are first-party product documentation or product-owned repositories. “Not documented” means only that the cited first-party material does not present the capability; it is not proof that no version or experiment has it._

## Opportunity statement

Wind should introduce **Drafts**: persistent, toggleable sets of design and code decisions made directly on a running page and owned by a Wind Space.

A Draft is more than a CSS override. Each decision binds:

- the real page state (route, viewport, environment, screenshot, and Space identity);
- a resilient element locator and the proposed visual/content mutation;
- the reason for the change, its status, and before/after evidence; and
- an implementation handoff: CSS/DOM diff, source hint when available, and an exportable brief.

The product bet is that the browser can become a shared surface between a design file and source code: designers decide against the actual authenticated, responsive product; developers receive an inspectable, reversible proposal rather than reconstructing intent from a screenshot or comment. The page remains live—Drafts do not capture it into a detached canvas.

This fits Wind unusually well. A Space is already Wind's persistent identity seam, with its own tabs and isolated cookies/storage ([Wind domain model](../CONTEXT.md)); each Space also owns a persistent CEF request context ([CEF renderer](./cef.md)). Drafts can therefore inherit a real product context without introducing a second “project” container.

## Competitive boundary

| Product | Live-page manipulation | Persistence and project context | Responsive/state coverage | Code and handoff boundary |
| --- | --- | --- | --- | --- |
| **Chrome DevTools** | Deep DOM/CSS inspection and editing; the Styles pane operates on a selected element. | Local Overrides survive reloads; Workspaces map served assets to local files. AI assistance now retains chat history across sessions. | Device Mode simulates one responsive/mobile viewport at a time and is explicitly an approximation. | Workspaces write CSS/HTML/JS to local source files; AI styling changes can be reviewed and saved to a connected workspace. Chrome is the strongest implementation competitor. ([CSS tools](https://developer.chrome.com/docs/devtools/css/reference), [Overrides](https://developer.chrome.com/docs/devtools/overrides/), [Workspaces](https://developer.chrome.com/docs/devtools/workspaces), [Device Mode](https://developer.chrome.com/docs/devtools/device-mode), [AI assistance](https://developer.chrome.com/docs/devtools/ai-assistance/chat)) |
| **Polypane** | Its Elements panel edits styles, attributes, and HTML across panes; CSS/JS snippets can be previewed and applied. | Projects group tabs, environments, bookmarks, and isolated sessions. Snippets are saved and importable/exportable; pane layouts are saved as Workspaces. | Multiple panes show breakpoints together and synchronize navigation, scroll, hover, clicks, and form input. | Excellent testing and reusable-snippet workflow. The cited docs do not describe an element-anchored decision record with rationale/status/source handoff. ([Elements](https://polypane.app/docs/elements-panel/), [Projects](https://polypane.app/docs/projects/), [Snippets](https://polypane.app/docs/snippets/), [Panes](https://polypane.app/docs/intro-to-panes/)) |
| **Sizzy** | Universal DevTools and visual CSS debugging across devices. | Project Workspaces advertise bookmarks, **notes**, presets, and snippets; Session Manager covers multiple accounts. | Its core is many synchronized device viewports, with independent navigation and role/session testing. | Strong overlap on project context and notes. The cited product surface does not present notes as live-element decisions or connect them to implementation diffs/source. ([Sizzy product](https://sizzy.co/)) |
| **VisBug** | Designer-friendly point/click manipulation, text/image replacement, layout nudging, and inspection against any current page state. | The product-owned README focuses on in-page tinkering; durable project state is not part of its documented pitch. | Works at any device size but is not a multi-viewport lab. | Explicitly complements design authoring tools and helps make decisions on the front end. The documented surface stops before decision lifecycle or source mapping. ([VisBug repository](https://github.com/GoogleChromeLabs/ProjectVisBug)) |
| **Figma Dev Mode / Make / Code layers** | Dev Mode works on design layers, while Figma's MCP workflow can now capture production, staging, or localhost UI into editable layers. Make's May 2026 beta visually edits a connected production codebase. | Annotations, statuses, notifications, and version comparison persist handoff. Make adds Git branches/local commits; code layers keep interactive variants and comments on the canvas. | Captured or code-backed screens can be compared and resized, but the collaboration surface remains Figma rather than the original authenticated browser session. | The strongest strategic threat: Code Connect maps components to real code; Make beta supports annotations and PR creation; closed-beta code layers can import a repo/folder, convert code states to editable designs, and push changes back. ([Dev Mode](https://help.figma.com/hc/en-us/articles/15023124644247-Guide-to-Dev-Mode), [Code Connect](https://help.figma.com/hc/en-us/articles/23920389749655-Code-Connect), [MCP workflows](https://help.figma.com/hc/en-us/articles/40219873508247-Release-notes-roundup-May-2026), [Make local code beta](https://www.figma.com/blog/figma-make-now-on-your-local-code/), [Code layers beta](https://www.figma.com/blog/code-on-the-figma-canvas/)) |
| **Arc Boosts** | No-code color/font/size controls, element “Zap,” and CSS/JavaScript editors modify real sites. | Boosts persist per domain, but cannot be scoped to a Profile; sharing has been deprecated. | Applies to the browsed site rather than providing systematic responsive/state review. | Proves browser-native site customization is approachable, but does not map changes to product source or a team decision workflow. ([Arc Boosts](https://resources.arc.net/hc/en-us/articles/19212718608151-Boosts-Customize-Any-Website)) |
| **BugHerd / Jam** | BugHerd pins comments to exact live-page elements; Jam captures page state as screenshot/video. | BugHerd turns comments into tracked tasks; Jam produces shareable reports. | Both capture URL/device context; Jam also captures console, network, and user events. | Live-page annotation and context-rich issue capture are commodity. Their documented artifacts describe feedback/bugs, not a reversible visual mutation that remains executable on the page. ([BugHerd](https://bugherd.com/chrome-annotation-extension), [Jam](https://jam.dev/docs/creating-a-jam)) |

## What is commodity now

- DOM/CSS inspection, live property editing, color/box-model controls, and responsive emulation are baseline DevTools capabilities.
- Multi-viewport synchronized testing is the defining feature of both Polypane and Sizzy.
- Persistent CSS/JS injection is covered by Chrome Overrides, Polypane Snippets, and Arc Boosts.
- Project containers, tabs/bookmarks, environment switching, isolated sessions, notes, and saved viewport setups are already marketed by Polypane and Sizzy.
- Source-aware CSS application is no longer an open field: Chrome Workspaces and AI assistance can apply tested changes to mapped local sources.
- Element-pinned comments, screenshots, technical reproduction context, and issue-tracker handoff are established website-feedback features.
- Durable annotations, status, version comparison, live-UI-to-canvas capture, component-to-code mapping, and even visual code editing/Git handoff are established or in beta at Figma.

Wind should therefore **not** lead with “inspect for designers,” “all breakpoints at once,” “persistent CSS,” “projects,” “comment on a website,” or “AI that edits CSS.” Each is useful, but none is a defensible wedge by itself.

## The remaining workflow gap

The tools split into two worlds:

1. browser tools manipulate the real runtime well, but primarily store files, snippets, chats, or configurations; and
2. feedback tools store element-pinned comments and runtime evidence, but not an executable proposed design; and
3. Figma stores design intent and code handoff well—and is rapidly adding live capture and source editing—but moves the work into a Figma canvas or Make workspace.

The narrower gap is a **persistent live-page decision object** that joins runtime evidence, an executable/reversible visual mutation, rationale, and implementation state without leaving the browsed page. This is an inference from the documented product boundaries above, not a claim that no vendor has experimented with it.

Drafts should feel like a lightweight design branch over the real web app:

- scoped to a Space and optionally to named environments such as local, staging, and production;
- composed of element-anchored decisions, not a loose note pad or global snippet;
- automatically re-applied and re-attached after navigation/reload, with a visible “detached” state when the DOM has drifted;
- reviewable as before/after plus a human explanation, not just a raw CSS diff; and
- portable into implementation without pretending every runtime mutation can be safely rewritten into React, Tailwind, CSS-in-JS, or server-rendered source.

This targets the collaboration seam: **designers author intent in the product; developers resolve intent in code**.

## Recommended wedge

Ship Drafts first as a decision-and-handoff layer, not a replacement for DevTools or Figma.

The smallest distinctive loop is:

1. Enter Draft mode in a Space and click an element on the live page.
2. Adjust a deliberately narrow set of properties (spacing, size, typography, color, visibility, and text), or paste CSS declarations.
3. Add a short rationale; Wind captures route, viewport, element locator, applicable stylesheet/source URL, and before/after images.
4. Save the decision. It survives reload/restart, can be toggled, and shows whether it still attaches.
5. A developer opens the Draft drawer and copies an implementation brief containing the locator, CSS diff, screenshots, environment, and rationale; the decision can be marked accepted, implemented, or rejected.

The important product object is the **Draft**, not the editor. A modest editor attached to a durable decision history is more differentiated than a sophisticated editor whose work disappears into an override file.

## MVP hypothesis

**Hypothesis:** a product designer and front-end developer can resolve a real visual change faster, with less restatement, when the proposal is made and preserved on the running product.

First slice:

- one Draft per Space, with multiple decisions;
- CSS declaration and text overrides only—no arbitrary JavaScript;
- exact URL plus optional route pattern, viewport metadata, and a resilient locator bundle;
- automatic replay, attachment health, before/after capture, rationale, and four statuses;
- an optional project-folder binding, with a local, reviewable `.wind/drafts/*.json` sidecar (or Space-local storage when unbound) plus “Copy implementation brief” (Markdown/CSS); and
- an always-visible modified-page badge and one-click disable.

Do **not** write to application source in the first slice. Export truthful runtime evidence and source hints; validate demand before taking on framework-aware code transformation. Do not build multi-viewport mode first either—capture/replay at named viewport presets is enough to test the decision model.

Suggested success test: in five paired designer/developer trials on active products, at least three pairs use a Draft to reach an implemented decision without recreating the proposal in Figma or explaining it again in chat; at least 90% of saved decisions re-attach correctly after reload on the tested route.

## Risks

- **DOM drift:** generated class names, lists, and framework rerenders can invalidate locators. Store multiple signals (stable attributes, DOM path, text fingerprint, geometry) and surface uncertainty rather than silently attaching to the wrong node.
- **False source confidence:** runtime CSS does not map cleanly to component props, tokens, Tailwind utilities, or CSS-in-JS. Keep source locations as hints until framework-specific adapters are proven.
- **Commodity absorption:** Chrome, Polypane, or Sizzy could add annotations to their existing project/AI surfaces. Wind must make the decision lifecycle and Space-native continuity excellent, not compete on inspector depth.
- **Figma velocity:** its 2026 live capture, local-code, Git/PR, and code-layer bet already occupies the broad “design ↔ code” story. Wind's claim must stay narrow: the live authenticated page is the durable review surface, and every proposal remains a reversible overlay with traceability.
- **Security and surprise:** injected JavaScript, hidden page modifications, and cross-environment replay can be dangerous. Begin with CSS/text, keep modifications conspicuous, scope them tightly, and never export credentials or storage.
- **Designer adoption:** raw CSS recreates DevTools' learning curve. The first editor needs direct, visual controls and familiar nudging, while keeping the underlying diff inspectable.
- **Collaboration scope:** real-time cloud sharing would multiply identity, privacy, and permissions work. Start with a repository-shareable sidecar and copied brief; add hosted review only after repeated use.

## Recommendation

Prototype **Drafts** before any broader “developer browser” suite. It leverages Wind's existing Spaces, addresses a gap between live-page tools and design handoff, and can be tested without first matching Chrome DevTools or Polypane feature-for-feature.
