# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## CRITICAL: AgentRail Session Protocol (MUST follow exactly)

Every agent session follows this 6-step loop. Do NOT skip or reorder steps.

1. **`agentrail next`** -- Read the current step prompt. The step prompt IS the instruction. Do not ask for confirmation; just do it.
2. **`agentrail begin`** -- Mark the step as in-progress. Do this immediately after reading.
3. **WORK** -- Implement what the step describes. Follow all project conventions below.
4. **`git commit`** -- Commit your work with a descriptive message. All pre-commit quality gates must pass.
5. **`agentrail complete`** -- Mark the step done. Include `--summary "..."`, `--reward N` (1-10), and `--actions N` (number of tool calls). If the step failed, use `--failure-mode "..."` and `--reward -1`.
6. **STOP** -- Do not continue. New work belongs in the next step (next session).

## Project: web-sw-cor24-plsw -- PL/SW Development Environment on COR24

Browser-based PL/SW development environment (editor/compiler) running the PL/SW
compiler pipeline on the COR24 emulator via WASM. Built with Yew 0.21 CSR + Trunk.

PL/SW is a PL/I-inspired freestanding systems programming language for the COR24
24-bit RISC ISA. The web UI provides:
- Editor for one .plsw file and multiple .msw macro files
- Wizard-driven compilation pipeline (preprocess -> compile -> assemble -> run)
- Notebook-cell scrolling UI (same pattern as cor24-rs Rust/C tabs)
- Assembly listing + debugger with run/step, registers, memory, UART I/O
- Pipeline visualization tab showing all 7 compiler stages
- Collapsible macro expansion view
- Demo programs and .plsw/.msw file upload

The PL/SW compiler is written in C and runs on the COR24 emulator in WASM
(same approach as the cor24-rs C pipeline).

## Related Projects

All COR24 repos live under `~/github/sw-embed/` as siblings:

- `sw-cor24-plsw` -- PL/SW compiler (C, runs on COR24 natively)
- `sw-cor24-emulator` -- COR24 assembler and emulator (Rust)
- `sw-cor24-assembler` -- COR24 assembler library
- `web-sw-cor24-assembler` -- COR24 assembly IDE (browser)
- `web-sw-cor24-pcode` -- P-code VM debugger (browser, closest reference project)
- `sw-cor24-pcode` -- P-code VM, assembler, linker (Rust workspace)
- `sw-cor24-rust` -- Rust-to-COR24 pipeline

## Build

Edition 2024 for all Rust code. Never suppress warnings.

```bash
trunk build                    # Build WASM to dist/
./scripts/serve.sh             # Dev server (port 9507)
./scripts/build-pages.sh       # Release build to pages/ for GitHub Pages
cargo clippy --all-targets --all-features -- -D warnings  # Lint
cargo fmt --all                # Format
```

## Available Task Types

rust-project-init, yew-component, wasm-build, rust-clippy-fix, pre-commit, rust-test-write, css-styling, documentation

## Architecture

- **Trunk** builds the WASM binary and serves it (port 9507)
- **cor24-emulator** provides `EmulatorCore` + `Assembler` (path dep to `../sw-cor24-emulator`)
- **cor24-isa** for ISA definitions (path dep to `../sw-cor24-emulator/isa`)
- **Yew 0.21** CSR framework for the UI
- 3-column grid layout: sidebar, wizard steps, notebook cells (scrolling)
- Two tabs: Editor (wizard-driven workflow) and Pipeline (7-stage viewer)
- PL/SW compiler binary (C, pre-built) runs on COR24 emulator in WASM
- Compiler 'c' mode: send source via UART, receive .s assembly output
- `build.rs` embeds compiler binary + build metadata
- Catppuccin Mocha dark theme
- `pages/` directory committed to git, deployed via GitHub Pages
- Pipeline: .plsw + .msw -> preprocess -> compile via 'c' mode (on emulator) -> .s -> assemble (Rust) -> run (on emulator)

## Key Conventions

- Monospace font stack: JetBrains Mono, Fira Code, Cascadia Code
- CSS custom properties for Catppuccin Mocha colors
- Flexbox layout for responsive multi-panel design
- `include_bytes!()` / `include!()` for embedding build-time outputs
- Release profile: `opt-level = "z"`, `lto = true` for minimal WASM size
- GitHub Pages URL: https://softwarewrighter.github.io/web-sw-cor24-plsw/
