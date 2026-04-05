# web-sw-cor24-plsw

Browser-based PL/SW development environment for the [sw-cor24-plsw](https://github.com/sw-embed/sw-cor24-plsw) compiler running on the [COR24](https://github.com/sw-embed/sw-cor24-emulator) emulator via WASM.

[Live Demo](https://sw-embed.github.io/web-sw-cor24-plsw/)

## Overview

web-sw-cor24-plsw provides an integrated editor and compiler pipeline for PL/SW, a PL/I-inspired freestanding systems programming language targeting the COR24 24-bit RISC ISA. The PL/SW compiler (written in C) runs directly on the COR24 emulator in the browser -- the same self-hosting approach used by the COR24 toolchain.

Built with Rust, Yew 0.21, and Trunk. Runs entirely in the browser as a WASM application.

## Features

- PL/SW source editor with .plsw syntax highlighting
- Macro file editor with .msw syntax highlighting and collapsible cells
- End-to-end compilation pipeline: edit -> preprocess -> compile -> assemble -> run
- Compiler runs on COR24 emulator in WASM via 'c' (compile) mode
- Generated assembly listing and program execution with UART output
- Collapsible macro expansion view with preprocessor output
- Demo programs and .plsw/.msw file upload
- Notebook-cell scrolling UI

## Build

```bash
trunk build                    # Build WASM to dist/
./scripts/serve.sh             # Dev server (port 9507)
./scripts/build.sh             # Dev build (no serve)
./scripts/build-pages.sh       # Release build to pages/ for GitHub Pages
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all
```

Prerequisites: Rust 1.85+, Trunk (`cargo install trunk`), `rustup target add wasm32-unknown-unknown`

## Documentation

- [docs/design.md](docs/design.md) -- UI design, layout, component structure
- [docs/architecture.md](docs/architecture.md) -- System architecture and module organization
- [docs/prd.md](docs/prd.md) -- Product requirements
- [docs/plan.md](docs/plan.md) -- Implementation plan
- [docs/tools.md](docs/tools.md) -- Toolchain and build system details

## Related Projects

- [sw-cor24-plsw](https://github.com/sw-embed/sw-cor24-plsw) -- PL/SW compiler (C, runs on COR24 natively)
- [sw-cor24-emulator](https://github.com/sw-embed/sw-cor24-emulator) -- COR24 assembler and emulator
- [sw-cor24-x-assembler](https://github.com/sw-embed/sw-cor24-x-assembler) -- COR24 assembler library
- [web-sw-cor24-assembler](https://github.com/sw-embed/web-sw-cor24-assembler) -- COR24 assembly IDE (browser)
- [web-sw-cor24-pcode](https://github.com/sw-embed/web-sw-cor24-pcode) -- P-code VM debugger (browser)
- [sw-cor24-pcode](https://github.com/sw-embed/sw-cor24-pcode) -- P-code VM, assembler, and linker (Rust workspace)
- [COR24-TB](https://makerlisp.com) -- COR24 FPGA board

## License

MIT License -- see [LICENSE](LICENSE) for details.

Copyright (c) 2026 Michael A. Wright
