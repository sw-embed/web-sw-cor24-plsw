# Implementation Plan: web-sw-cor24-plsw

## Phase 1: Project Scaffold (Steps 1-2)

### Step 1: rust-project-init
Scaffold the Yew/WASM project with Trunk build system.

- Update Cargo.toml: Yew 0.21 (csr), wasm-bindgen, web-sys, gloo,
  console_error_panic_hook, cor24-emulator path dep, cor24-isa path dep
- Create lib crate type (cdylib + rlib)
- Create index.html with Catppuccin Mocha theme, Trunk directives
- Create src/main.rs (panic hook + Yew renderer)
- Create src/lib.rs with App function_component (header + footer + placeholder)
- Create scripts/serve.sh (port 9507) and scripts/build-pages.sh
- Create Trunk.toml with dist dir and public URL
- Create build.rs with git SHA, hostname, timestamp metadata
- Add .nojekyll, pages/ directory
- Verify `trunk build` succeeds, `./scripts/serve.sh` serves the page

### Step 2: wasm-build
Wire the cor24-emulator dependency and verify WASM integration.

- Import EmulatorCore and Assembler from cor24-emulator
- Create a minimal test: assemble a trivial COR24 program, load, step,
  read registers
- Verify the emulator runs in WASM (no panics, correct output)
- Add release profile (opt-level z, lto)

## Phase 2: Editor Panel (Steps 3-4)

### Step 3: yew-component -- Source Editor
Build the wizard sidebar + notebook cell layout with PL/SW source editor.

- Create 3-column grid layout (sidebar, wizard steps, notebook cells)
- Wizard sidebar with step indicators (Source, Macros, Preprocess,
  Compile, Assemble, Run)
- First notebook cell: textarea for .plsw source editing
- PL/SW syntax highlighting (keywords, macros, strings, comments, inline asm)
- Demo dropdown in sidebar or header to load example programs
- File upload button for .plsw files
- CSS styling: Catppuccin Mocha colors, monospace fonts

### Step 4: yew-component -- Macro File Editor
Add support for editing .msw macro files.

- Notebook cell(s) for .msw file editing below the .plsw cell
- Add/remove macro file buttons
- File upload for .msw files
- Macro syntax highlighting (MACRODEF, GEN, expansion directives)
- Each .msw file gets its own collapsible cell with filename header

## Phase 3: Compilation Pipeline (Steps 5-6)

### Step 5: wasm-build -- Compiler Integration
Wire the PL/SW compiler binary to run on the COR24 emulator.

- Embed pre-built plsw compiler binary in build.rs (or load from
  sibling project's build output)
- Create compilation workflow: load compiler binary -> feed .plsw source
  via UART RX -> capture .s assembly from UART TX
- Handle macro file feeding (concatenate .msw files before source, or
  use include mechanism)
- Error capture and display
- Notebook cell for preprocessed output (wizard step: Preprocess)
- Notebook cell for .s assembly output (wizard step: Compile)
- Scroll-to-cell on step completion

### Step 6: yew-component -- Macro Expansion View
Add collapsible macro expansion visualization.

- In the preprocessed output cell, annotate expanded macros
- Each `?MACRO(...)` invocation has a toggle to show/hide expansion
- GEN block output shown with assembly syntax coloring
- Collapsed by default, expandable per-invocation

## Phase 4: Assembly + Execution (Steps 7-8)

### Step 7: yew-component -- Assembly Listing + Debugger
Wire assembler and build the debugger panel.

- Assemble generated .s using cor24-emulator Assembler (Rust, not via
  emulator -- direct WASM call)
- Notebook cell for assembled listing with addresses
- Control bar: Run, Step (x1/x10/x100/x1K), Stop, Reset, speed slider
- Auto-scroll listing to current PC
- Current instruction highlighting
- Batch execution (50K instructions/tick) with gloo timer

### Step 8: yew-component -- Register + Memory + I/O
Add register display, memory viewer, and UART I/O.

- Register grid: r0-r7, PC, condition flag
- Change heatmap (hot = changed last step, warm = changed 2 steps ago)
- Sparse memory viewer (SRAM + stack regions)
- UART TX output display
- UART RX input field (type to send characters)
- Instruction count and emulator status (READY/RUNNING/HALTED)

## Phase 5: Pipeline Tab + Polish (Steps 9-10)

### Step 9: yew-component -- Pipeline Visualization Tab
Build the second tab showing 7-stage compiler pipeline.

- Tab bar in header (Editor | Pipeline)
- Pipeline tab with 7 collapsible stage panels
- Stage 1 (Preprocess): show macro expansion with annotations
- Stage 7 (Emit): show final .s assembly
- Stages 2-6: placeholder panels with descriptions of what each stage
  does, populated when compiler diagnostic output is available
- Each stage shows input -> output transformation

### Step 10: pre-commit -- Demo Programs + Polish
Finalize demos, build GitHub Pages, polish.

- Create embedded demo programs (hello, arithmetic, records, macros,
  inline asm)
- Each demo: name, description, .plsw source, optional .msw files
- Build to pages/ via build-pages.sh
- Verify GitHub Pages deployment
- Keyboard shortcuts (Ctrl+Enter to compile, F5 to run, F10 to step)
- Error display polish (inline error markers in editor)
- README with screenshot and link to live demo

## Phase 6+: Future Enhancements

- Source-level PL/SW debugging (step through PL/SW lines, not COR24)
- Pipeline stages 2-6 populated with real compiler diagnostics
- Mixed listing output (PL/SW source interleaved with generated assembly)
- Symbol table browser
- Code completion for PL/SW keywords and macro names
- Persistent editor state (localStorage)
- Multiple .plsw file support
