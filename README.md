# SpaceMonger One (Rust)

Rust port of [spacemonger1](https://github.com/scf37/spacemonger1) (Java), itself a port of
Sean Werkema's original SpaceMonger 1.x. Shows a disk or folder as a treemap: bigger box,
more space; boxes inside boxes are folders; colours show nesting depth.

## Build & run

Needs a Rust toolchain (`curl https://sh.rustup.rs -sSf | sh`). No system dev libraries are
required; X11/Wayland/GL are loaded at runtime.

```sh
cargo build --release
./target/release/spacemonger            # pick a drive or folder
./target/release/spacemonger /some/dir  # scan a folder straight away
```

## Usage

- **Open** — pick a mounted volume, or type / browse to a folder.
- Click to select, double-click a folder to zoom in so it exactly fills the view, double-click a file to open it.
- Clicking the only selected item deselects it.
- Multi-select: `Ctrl+click` adds or removes one item, `Shift+click` adds one; `Shift+drag` draws a rectangle and selects
  everything fully inside it (`Ctrl+Shift+drag` adds to the selection). Items inside a selected
  folder go with it. `Esc` clears the selection. Delete, Hide and Run or Open act on the whole
  selection; right-clicking a selected item keeps the selection for the context menu.
- The bar above the map shows the path from the scan root to the current folder, each folder in
  its treemap colour. Click any parent to zoom back to it.
- Mouse wheel (or trackpad pinch): smooth zoom in/out around the pointer, like an infinite canvas.
  Deeper folders' contents appear as they get big enough. Drag (left or middle button) to pan.
- Zooming to a folder (double-click, Zoom In/Out/Full, path bar) is one camera move: the map
  zooms evenly while the folder's box reshapes to fill the window. Scrolling back out eases it
  back to its natural shape.
- Resizing the window keeps the folder at the centre at about the same size and position.
- **Zoom In / Out / Full** — Zoom Out fits the current folder, or its parent if it's already fitted.
- **Run or Open**, **Delete** (moves to trash, after confirmation).
- **Hide** (or `H`) removes the selected item from the view only — nothing on disk changes.
  **Unhide All** (or `Shift+H`) brings every hidden item back; the button shows how many are hidden.
- Right-click for the same commands as a context menu.
- **Free Space** toggles the free-space block (off by default when scanning a folder).
- Hovering shows the full name (when truncated) and a size/date tip.
- Keys: Enter = zoom in, Backspace = zoom out, H = hide, Shift+H = unhide all, Delete = delete, F5 = rescan, Esc = clear selection,
  F = frame selection (a single folder fills the view; several items are framed by their bounding
  box; nothing selected = Zoom Full; the selection is kept).
- Arrow keys move the selection (every selected item at once) to the neighbouring box in that
  direction, within the same folder; from a big box to several smaller ones it picks the
  top-most (left / right) or left-most (up / down). Nothing there = stays put.
  Alt+Up = parent folder, Alt+Down = first (top-left) child. The view pans to keep the
  selection visible.
- Ctrl+arrow or Shift+arrow extends the selection from the last selected item to its
  neighbour; stepping back the way you came shrinks it again.
- Dark mode toggle at the right of the toolbar. Dark mode and free-space state persist.

## Scope vs. the Java version

Ported: parallel scanner (stays on one filesystem, de-duplicates hard links), the original
greedy split-treemap layout, rainbow palette (light and dark) drawn flat, zoom animation, name/info tips, context menu, drive picker, trash/open, title-bar info.

Not ported: settings dialog (density, bias, colour schemes, tip options use the Java
defaults), translations, About dialog, toolbar bitmaps.

## Layout

`core/` is everything that isn't drawing and has no egui dependency; `ui/` is the egui front
end. `utils/` holds generic helpers, `helpers/` app-specific glue.

| Path | What | Java origin |
|---|---|---|
| `src/main.rs` | Arguments, window setup | |
| `src/constants.rs` | App-wide constants | |
| `src/core/model/` | `Entry`, `Folder`, `Tree`, `EntryRef` | `CFolder`, `CFolderTree` |
| `src/core/fs/` | Volumes (lfs-core), parallel scanner, background scan job, errors | `fs/*` |
| `src/core/geometry/` | kurbo re-exports + `RectExt` | |
| `src/core/layout/` | Greedy split treemap, reshaping, geometry queries, arrow-key neighbours | `FolderView.buildFolderLayout` / `sizeFolders` |
| `src/core/camera/` | Camera: pan, wheel zoom, zoom-to-folder / frame animations, fit, resize anchor | `FolderView` (zoom) |
| `src/core/selection/` | `Selection`, arrow-key navigation, rectangle select | |
| `src/core/actions.rs` | `Action` enum | |
| `src/ui/app/` | `SpaceMonger` app state, commands, treemap panel | `AppController`, `FolderView` (input) |
| `src/ui/widgets/` | Toolbar, path bar, context menu, info tip | |
| `src/ui/dialogs/` | Drive picker, scan progress, delete confirm, error | |
| `src/ui/painter.rs` | Box and label drawing | `FolderView` (drawing) |
| `src/ui/palette.rs` | Colour palettes | `ColorService` |
| `src/ui/keymap.rs`, `title.rs`, `settings.rs`, `error.rs` | Shortcuts, window title, persisted settings, user-facing errors | |
| `src/utils/` | Size / date formatting, interpolation, text eliding | `FormatService` |
| `src/helpers/` | egui conveniences | |

`cargo test` covers scanning / removal, layout, camera moves, selection and formatting.
