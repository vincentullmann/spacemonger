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

build for windows

```sh
# install dependencies
rustup target add x86_64-pc-windows-gnu
sudo pacman -S mingw-w64-gcc

# build
cargo build --release --target x86_64-pc-windows-gnu
```
