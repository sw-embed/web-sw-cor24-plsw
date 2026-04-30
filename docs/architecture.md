# Architecture: web-sw-cor24-plsw

## Overview

Browser-based PL/SW development environment running the full PL/SW compiler
pipeline on the COR24 emulator via WASM. The PL/SW compiler itself is written
in C and runs on COR24 natively -- the web UI hosts it by running the compiled
compiler binary on the COR24 emulator in WASM, the same approach used by the
cor24-rs C pipeline (web-tc24r fork).

## Component Diagram

```
+-----------------------------------------------------------+
|  Browser (WASM)                                           |
|                                                           |
|  +-----------------------------------------------------+ |
|  | Yew 0.21 CSR App                                    | |
|  |                                                     | |
|  |  [Tab: Editor]           [Tab: Pipeline]            | |
|  |                                                     | |
|  |  +-- Wizard Sidebar --+  +-- Pipeline Viewer -----+ | |
|  |  | 1. Source (.plsw)  |  | Stage 1: Preprocess    | | |
|  |  | 2. Macros (.msw)   |  | Stage 2: Lex           | | |
|  |  | 3. Preprocess      |  | Stage 3: Parse         | | |
|  |  | 4. Compile         |  | Stage 4: Semantic      | | |
|  |  | 5. Assemble        |  | Stage 5: IR Lower      | | |
|  |  | 6. Run/Debug       |  | Stage 6: Reg Alloc     | | |
|  |  +--------------------+  | Stage 7: Emit          | | |
|  |                          +------------------------+ | |
|  |  +-- Notebook Cells (scrolling) -----------------+  | |
|  |  | .plsw source editor + optional PL/EDIT mode   |  | |
|  |  | .msw macro file editors + optional PL/EDIT    |  | |
|  |  | Preprocessed PL/SW output                     |  | |
|  |  | COR24 .s assembly output                      |  | |
|  |  | Assembly listing + Debugger (run/step)         |  | |
|  |  +------------------------------------------------+ | |
|  +-----------------------------------------------------+ |
|                                                           |
|  +-----------------------------------------------------+ |
|  | cor24-emulator (EmulatorCore)                       | |
|  |   Runs PL/SW compiler binary (compiled from C)      | |
|  |   Then runs the compiled PL/SW program              | |
|  +-----------------------------------------------------+ |
+-----------------------------------------------------------+
```

## Data Flow

The pipeline has two execution phases on the emulator:

### Phase A: Compilation (PL/SW compiler running on COR24)

```
.plsw source + .msw macros
    |
    v
[COR24 Emulator running plsw compiler binary]
    |
    +-- Stage 1: Macro preprocessing (expand ?MACRO invocations)
    +-- Stage 2: Lexical analysis (tokenize PL/SW)
    +-- Stage 3: Parsing (recursive descent -> AST)
    +-- Stage 4: Semantic analysis (types, symbols)
    +-- Stage 5: IR lowering (desugar control flow)
    +-- Stage 6: Register allocation (linear-scan, r0-r2)
    +-- Stage 7: Assembly emission
    |
    v
COR24 .s assembly output (captured from emulator UART)
```

### Phase B: Assembly + Execution

```
COR24 .s assembly
    |
    v
[cor24-emulator Assembler (Rust, direct WASM call)]
    |
    v
Machine code binary + listing + labels
    |
    v
[COR24 Emulator running compiled program]
    |
    v
Output (UART TX), register state, memory view
```

## WASM Build Pipeline

```
Trunk
  |
  +-- Compiles Rust to WASM via wasm-bindgen
  +-- Bundles index.html + CSS
  +-- Copies CHANGES as a static asset
  +-- Outputs to dist/
  |
  +-- build-pages.sh: trunk build --release -> rsync to pages/
  |
  v
pages/ -> GitHub Pages (https://softwarewrighter.github.io/web-sw-cor24-plsw/)
```

## Path Dependency Structure

```
~/github/sw-embed/
  |
  +-- web-sw-cor24-plsw/     (this project)
  |     Cargo.toml depends on:
  |       cor24-emulator = { path = "../sw-cor24-emulator" }
  |       cor24-isa = { path = "../sw-cor24-emulator/isa" }
  |
  +-- sw-cor24-emulator/      (assembler + emulator)
  +-- sw-cor24-plsw/          (PL/SW compiler source in C)
  +-- sw-cor24-assembler/     (assembler library)
```

## Pre-Built Compiler Binary

The PL/SW compiler is written in C and compiled to COR24 assembly via tc24r,
then assembled to a binary. This binary is embedded in the WASM app at build
time via `build.rs`:

```
sw-cor24-plsw/src/*.c
    |  (tc24r: C -> COR24 .s)
    v
plsw-compiler.s
    |  (cor24 assembler at build time)
    v
plsw-compiler.bin  (embedded via include_bytes!)
```

At runtime, the web UI loads this binary into the COR24 emulator, feeds PL/SW
source via UART RX, and captures compiled .s assembly from UART TX.

## Macro Expansion Visualization

The preprocessor stage is surfaced in the UI with collapsible expansion:

- Original .plsw source with `?MACRO(...)` invocations highlighted
- Expanded output showing what each macro generated
- Toggle to show/hide expansion per macro invocation
- GEN block output shown inline with assembler-colored syntax

## PL/EDIT Template Expansion

PL/EDIT is implemented as a Yew editor mode layered onto the existing
textarea-based source and macro editors. The template registry lives in
`src/components/pl_edit.rs` and is shared by `.plsw` and `.msw` editor
surfaces.

At runtime, PL/EDIT:

- Matches the token immediately before the textarea cursor
- Replaces it with a language-specific skeleton on `F4` or `Ctrl+Space`
- Records fill-field cursor offsets from `$0`, `$1`, ... template markers
- Intercepts `Tab` and `Enter` while a fill session is active to move between
  fields
- Leaves `Ctrl+Enter` to the browser textarea so users can insert new lines
- Supports the same behavior in normal and fullscreen editor layouts

## Pipeline Stage Mapping to UI

The 7-layer pipeline runs inside the COR24 emulator as a single compiler
execution. The Pipeline tab provides visibility into each stage:

| Stage | UI Display | Source |
|-------|-----------|--------|
| 1. Preprocess | Expanded .plsw with macro annotations | Compiler diagnostic output |
| 2. Lex | Token stream (keyword, ident, literal, etc.) | Compiler diagnostic output |
| 3. Parse | AST tree view (collapsible) | Compiler diagnostic output |
| 4. Semantic | Symbol table, type annotations | Compiler diagnostic output |
| 5. IR Lower | Intermediate representation | Compiler diagnostic output |
| 6. Reg Alloc | Register assignment map | Compiler diagnostic output |
| 7. Emit | Final .s assembly | Compiler UART output |

Note: Stages 2-6 require the PL/SW compiler to emit diagnostic output, which
may be added incrementally. Stage 1 (preprocess) and Stage 7 (emit) are
visible from the start.

## Yew Component Architecture

```
App (function_component)
  |
  +-- Header (title, tab bar)
  +-- TabContent
  |     +-- EditorTab (wizard + notebook cells)
  |     |     +-- WizardSidebar (step indicators, action button)
  |     |     +-- NotebookCells (scrolling container)
  |     |           +-- SourceCell (.plsw editor)
  |     |           +-- MacroCell (.msw editor, one per file)
  |     |           +-- PreprocessedCell (expanded output)
  |     |           +-- AssemblyCell (.s output)
  |     |           +-- DebugCell (listing + debugger)
  |     |                 +-- ControlBar (run/step/reset/speed)
  |     |                 +-- ListingPanel (assembled lines, PC highlight)
  |     |                 +-- RegisterPanel (r0-r7, PC, flags, heatmap)
  |     |                 +-- MemoryPanel (sparse hex dump)
  |     |                 +-- IoPanel (UART RX/TX, LED, switch)
  |     +-- PipelineTab (7-stage viewer)
  |           +-- StagePanel (one per pipeline stage)
  +-- Footer (license, links, build metadata)
```
