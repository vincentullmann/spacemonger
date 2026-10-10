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
- Default keys (all rebindable in Settings → Keys): Enter = zoom in, Backspace = zoom out, H = hide, Shift+H = unhide all,
  Delete = delete, F5 = rescan, Esc = clear selection, Ctrl+O = open, Ctrl+, = settings,
  F = frame selection (a single folder fills the view; several items are framed by their bounding
  box; nothing selected = Zoom Full; the selection is kept).
- Arrow keys move the selection (every selected item at once) to the neighbouring box in that
  direction, within the same folder; from a big box to several smaller ones it picks the
  top-most (left / right) or left-most (up / down). Nothing there = stays put.
  Alt+Up = parent folder, Alt+Down = first (top-left) child. The view pans to keep the
  selection visible.
- Ctrl+arrow or Shift+arrow extends the selection from the last selected item to its
  neighbour; stepping back the way you came shrinks it again.
- **⚙ Settings** (or `Ctrl+,`) opens the settings window, a dialog that stays in front of the
  main window (on X11 it has no taskbar entry). Every change shows in the main window straight
  away and is saved between runs. Each setting has a tooltip; changed ones are marked with a
  dot and reset with ⟲ or right-click. Up / Down change the hovered or focused field.
  - General: theme (light / dark / follow system), animation length, scroll zoom speed,
    frame-selection fill, delete confirmation; Layout: split bias, free space.
  - Scan: ignore hidden (dot) files — applies at once, without a rescan; stay on one filesystem;
    count hard links once; exclude patterns (mock-up, not used yet).
  - Display:
    - Font: any system font (via fontdb) for labels and the path bar.
    - Tiles: colour scheme (Classic or a colorgrad preset; picking one resets the colour count)
      with any number of colours — editing one makes the scheme Custom; borders (collapsed into
      one line between neighbours at gap 0), gap, hover highlight.
    - Tiles / Labels: font size (folder title bars grow with it), drop shadow, density, size and
      date lines, size format (bytes, KiB or kB — used by labels and tips), date format.
    - Path bar: font size (the bar's height follows it).
    - Tooltips: full path, size, modified and created date (labelled when both are on), font
      size, delay.
  - Keys, grouped (Scanning, Navigation, Selection & actions, Display): click a shortcut to
    rebind it, right-click to remove, `+` to add; clashes show in red.

## Scope vs. the Java version

Ported: parallel scanner (stays on one filesystem, de-duplicates hard links), the original
greedy split-treemap layout, rainbow palette (light and dark) drawn flat, zoom animation, name/info tips, context menu, drive picker, trash/open, title-bar info.

Not ported: the Java settings dialog as such (there is a new one, see above), translations,
About dialog, toolbar bitmaps.

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
| `src/ui/palette.rs` | Colour schemes and drawing options | `ColorService` |
| `src/ui/settings/` | Persisted settings, settings window and its form rows | |
| `src/ui/x11_sync.rs`, `x11_dialog.rs` | X11: smooth resizing; settings window as a dialog | |
| `src/ui/fonts.rs` | System font lookup (fontdb) | |
| `src/ui/keymap.rs`, `title.rs`, `error.rs` | Rebindable shortcuts, window title, user-facing errors | |
| `src/utils/` | Size / date formatting, interpolation, text eliding | `FormatService` |
| `src/helpers/` | egui conveniences | |

`cargo test` covers scanning / removal, layout, camera moves, selection, formatting, settings
migration, colour schemes and key lookup.
