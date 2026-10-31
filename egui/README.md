# elle-egui

<!-- audited: 2026-10-30 -->

An immediate-mode GUI plugin for Elle: a window, and frames drawn from a tree of widget nodes, via `egui`, `winit` and `glow`.

## Building

The plugin is outside the portable set, because it needs a display before it
can do anything. Build it alone or with the whole workspace:

```sh
cargo build --release -p elle-egui
```

Produces `target/release/libelle_egui.so`. `cargo test -p elle-egui` lays
frames out in a headless egui context, so the tests need no display.

## Usage

The `std/egui` library in Elle wraps this plugin with a frame loop and a
constructor for each node. The plugin alone draws one frame per call:

```lisp
(def egui (import "plugin/egui"))
(def win (egui:open {:title "Notes"}))
(def ix (egui:frame win [[:side-panel {:id :list} [:button {:id :first} "First note"]]
                         [:heading "Pick a note"]]))
(ix :clicks)   # the ids of the buttons clicked in this frame, as a set
```

## Primitives

| Name | Arguments | Output | Description |
|------|-----------|--------|-------------|
| `egui/open` | options | window | open a window |
| `egui/frame` | window, tree | interactions | read the window's events, draw the tree, and answer what the person did |
| `egui/display-fd` | window | integer or nil | the display connection's file descriptor, for `ev/poll-fd` |
| `egui/close` | window | nil | close the window and release its resources |
| `egui/open?` | window | boolean | false after `egui/close`, or once the person asks to close the window |
| `egui/set-text` | window, id, string | nil | set the text of a text input or a text editor |
| `egui/set-check` | window, id, boolean | nil | set a checkbox |
| `egui/set-slider` | window, id, number | nil | set a slider |
| `egui/set-combo` | window, id, string | nil | set the selection of a combo box |
| `egui/set-title` | window, string | nil | change the window's title |

The options of `egui/open` are optional: a struct with `:title`, default
`"Elle"`, and `:width` and `:height` in logical pixels, default 800 by 600.
`egui/display-fd` answers nil on a platform with no display descriptor, such
as macOS.

## The tree

A tree is one node or an array of nodes. A node is an array: a keyword tag,
then a props struct when the node takes props, then its content.
`egui/frame` draws the tree from the top down, inside a panel that fills the
window. A node the plugin cannot read is a `:ui-error`, and so is a node
that lacks a required `:id`.

| Node | Props | Content |
|------|-------|---------|
| `:label` | | a string |
| `:heading` | | a string |
| `:progress-bar` | `:fraction` from 0.0 to 1.0, `:text` | |
| `:separator` | | |
| `:spacer` | `:size`, default 8 | |
| `:button` | `:id` | a string |
| `:text-input` | `:id`, `:hint` | |
| `:text-edit` | `:id`, `:rows`, default 4 | |
| `:checkbox` | `:id` | a string |
| `:slider` | `:id`, `:min`, default 0, `:max`, default 100 | |
| `:combo-box` | `:id` | an array of option strings |
| `:v-layout` | | nodes, from top to bottom |
| `:h-layout` | | nodes, from left to right |
| `:centered` | | nodes, centered across the width |
| `:centered-justified` | | nodes, centered and stretched across the width |
| `:scroll-area` | `:id` | nodes, in an area that scrolls vertically |
| `:collapsing` | `:id` | a title string, then nodes |
| `:group` | | nodes, inside a frame |
| `:grid` | `:id`, `:columns`, default 2 | nodes, filled into rows |
| `:side-panel` | `:id`, `:side`, `:width` | nodes, in a panel along one edge |

An `:id` is a keyword or a string, and the interactions name it as a
keyword. A widget keeps its state in the window between frames, so a text
input keeps its text when the next frame draws it again.

### Side panels

A `:side-panel` runs the full height of the space it is drawn in, along the
left or the right edge. At the top level of a tree, that is the full height
of the window. The nodes after the panel fill the space beside it, so put
the panel before the content it sits beside.

`:side` is `:left` or `:right`, and `:left` when absent. Any other value is
a `:ui-error`. `:width` is the starting width in logical pixels, default
200. The person can drag the panel's inner edge to change the width, and
the window keeps the new width for that id.

## Interactions

`egui/frame` answers a struct:

| Key | Holds |
|-----|-------|
| `:clicks` | a set of the ids of the buttons clicked in this frame |
| `:text` | a struct from the id of each text input and text editor to its text |
| `:checks` | a struct from each checkbox's id to its state |
| `:sliders` | a struct from each slider's id to its value |
| `:combos` | a struct from each combo box's id to its selection |
| `:collapsed` | a struct from each collapsing section's id to whether it is closed |
| `:closed` | whether the person asked to close the window |
| `:size` | the window's inner size, `[width height]` in physical pixels |
