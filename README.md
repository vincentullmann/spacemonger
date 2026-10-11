# SpaceMonger One (Rust)

Rust port of [spacemonger1](https://github.com/scf37/spacemonger1) (Java), itself a port of
Sean Werkema's original SpaceMonger 1.x, with some additional tweaks.

Shows a disk or folder as a treemap:

- bigger box more space;
- boxes inside boxes are folders;
- colors indicate nesting depth.

<img src=docs/img/screenshot_main.png width=1280px />

## Features

The layout adjusts when you resize the window. Use the mouse wheel, or double-click a folder, to zoom in and out.

<!-- markdownlint-disable-next-line no-inline-html -->
<video src="https://github.com/user-attachments/assets/fd559264-df0a-45bd-95c1-85043a713ecc" width="720" controls></video>

Extensive settings.

<!-- markdownlint-disable-next-line no-inline-html -->
<video src="https://github.com/user-attachments/assets/7d1e5143-cfe3-4e99-9d11-36e89c85174b" width="720" controls></video>

## Build & run

Needs a Rust toolchain (`curl https://sh.rustup.rs -sSf | sh`). No system dev libraries are
required; X11/Wayland/GL are loaded at runtime.

```sh
cargo build --release
./target/release/spacemonger            # pick a drive or folder
./target/release/spacemonger /some/dir  # scan a folder straight away
```

build for windows

```sh
# install dependencies
rustup target add x86_64-pc-windows-gnu
sudo pacman -S mingw-w64-gcc

# build
cargo build --release --target x86_64-pc-windows-gnu
```
