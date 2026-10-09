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
- Click to select, double-click a folder to zoom in, double-click a file to open it.
- **Zoom In / Out / Full**, **Run or Open**, **Delete** (moves to trash, after confirmation).
- Right-click for the same commands as a context menu.
- **Free Space** toggles the free-space block (off by default when scanning a folder).
- Hovering shows the full name (when truncated) and a size/date tip.
- Keys: Enter = zoom in, Backspace = zoom out, Delete = delete, F5 = rescan.
- Dark mode toggle at the right of the toolbar. Dark mode and free-space state persist.

## Scope vs. the Java version

Ported: parallel scanner (stays on one filesystem, de-duplicates hard links), the original
greedy split-treemap layout and box rendering, rainbow palette (light and dark), zoom
animation, name/info tips, context menu, drive picker, trash/open, title-bar info.

Not ported: settings dialog (density, bias, colour schemes, tip options use the Java
defaults), translations, About dialog, toolbar bitmaps.

## Layout

| File | Java origin |
|---|---|
| `src/scan.rs` | `CFolder`, `CFolderTree`, `fs/*` |
| `src/layout.rs` | `FolderView.buildFolderLayout` / `sizeFolders` |
| `src/app.rs` | `AppController`, `FolderView` (drawing, input, tips), dialogs |
| `src/colors.rs` | `ColorService` |
| `src/format.rs` | `FormatService` |

`cargo test` covers scanning/removal, layout proportions and formatting.
