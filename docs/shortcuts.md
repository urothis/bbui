# Keyboard and Pointer Shortcuts

These shortcuts are implemented by the Bevy input bridge and routed widget actions. `Ctrl` also accepts Command on macOS for text and undo commands. The focused control receives keyboard input; disabled and hidden controls are skipped.

## Common controls

| Input | Behavior |
| --- | --- |
| Tab / Shift+Tab | Move focus forward / backward through visible controls. Leaving an open popup closes it. |
| Ctrl+Z / Ctrl+Shift+Z (or Ctrl+Y) | Undo / redo the focused text edit first, then application value edits. |
| Enter / Space | Activate a focused button, checkbox, radio group, or panel header. |
| Arrow keys | Adjust a focused slider or radio group; navigate menus and virtualized views. |
| Dropdown Space / Enter | Open the options; Enter chooses the highlighted option and Escape closes without changing the value. |
| Dropdown Arrow / Page / Home / End | Move the highlighted option by one, by a visible page, or to the first or last option. |

## Numeric fields and sliders

| Input | Behavior |
| --- | --- |
| Horizontal drag | Scrub a number with a hidden, locked cursor. Release commits one undo step; Escape restores the starting value. |
| Shift during drag / Ctrl during drag | Use precision movement / snap to the field's step. Modifiers can change mid-drag. |
| Vertical drag across fields, then horizontal drag | Scrub compatible fields as one transaction. |
| Ctrl+Wheel / Arrow Up or Down | Step the focused value. |
| Ctrl+C / Ctrl+V / Backspace | Copy the full-precision value / paste a valid number or expression / reset to default. |
| Double-click, type expression, Enter | Evaluate a Rust numeric expression such as `2*pi` or `x if c else y`. Escape cancels. |
| Slider Home / End | Move to the configured minimum / maximum. |

## Text, menus, and panels

Text fields and areas use Ctrl+Arrow to move by word, Ctrl+Backspace/Delete to delete a word, Shift with navigation to extend selection, and Ctrl+A/C/X/V for selection and clipboard. IME preedit is separate from committed text; leaving the field cancels unfinished preedit. A burst of typing is one local undo step until the 750 ms pause or a caret move.

Tabbing away from a text field or area commits its current text session to shared history; Enter also commits a single-line field. Once focus is elsewhere, Ctrl+Z undoes that session and restores its original selection. Ctrl+Shift+Z restores the committed text and selection.

Open menus accept Arrow keys, Enter, Escape, and typed fuzzy search; a press-drag-release gesture selects an item without a second click. Context menus open with the secondary pointer button. Ctrl+click on an exclusive panel header expands it and collapses its group in one undoable step.
In a group created with `PanelGroup::with_reordering(key)`, Ctrl+ArrowUp/Down moves the focused panel one position. Drag a panel header to reorder it; Escape cancels the drag. Each committed move is one undo step and is saved under the group's persistence key.

## Editors and browser

Canvas: middle-drag pans, wheel zooms, Arrow keys nudge selected objects, Delete removes them, and Escape cancels a drag. Node editor: drag sockets to link, Alt-drag cuts links, middle-drag pans, Ctrl+Wheel zooms, Ctrl+A selects nodes, C collapses, N adds a node, and Delete removes selected nodes. Both editors support Ctrl+Z/Ctrl+Shift+Z.

FileBrowser: Arrow keys move the cursor, Page Up/Down move a viewport, Home/End jump to the ends, Ctrl+A selects visible entries, Enter opens a selection, Backspace goes up when the filter is empty, and Escape clears the filter. Browser view and selection edits are undoable; filesystem create, rename, delete, copy, and move operations are external side effects reported separately.
