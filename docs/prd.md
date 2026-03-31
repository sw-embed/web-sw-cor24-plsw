# Product Requirements: web-sw-cor24-plsw

## Target User

Systems programming enthusiasts and retrocomputing hobbyists exploring the
COR24 24-bit RISC architecture. Users want to write PL/SW code in the browser,
see it compiled and run on the COR24 emulator, and understand the compilation
pipeline of this experimental PL/I-inspired language.

## Use Case

A browser-based PL/SW development environment that requires no local toolchain
installation. Users write PL/SW source and macro files, compile them through
the full 7-stage pipeline, view the generated COR24 assembly, and execute it
on the emulator -- all in one page.

## Core Features

### F1: PL/SW Source Editor
- Edit a single `.plsw` source file with syntax highlighting
- PL/SW keywords: DCL, PROC, DO, END, IF, THEN, ELSE, RETURNS, CALL, etc.
- Highlight macro invocations (`?MACRO(...)`) distinctly
- Highlight inline assembly blocks (`ASM DO; ... END;`)
- Highlight GEN blocks within macros

### F2: Macro File Editor
- Edit one or more `.msw` macro definition files
- Add/remove macro files from the project
- Macro syntax highlighting (MACRODEF, GEN, expansion directives)

### F3: Preprocessing Stage
- Run PL/SW preprocessor to expand macro invocations
- Display preprocessed PL/SW output as a notebook cell
- Collapsible macro expansion: show what each `?MACRO(...)` expanded to
- Scroll down to preprocessed output on completion

### F4: Compilation to Assembly
- Run the PL/SW compiler (on COR24 emulator) to produce `.s` assembly
- Display generated COR24 assembly as a notebook cell
- Scroll down to assembly output on completion
- Show compilation errors inline with source location

### F5: Assembly + Execution
- Assemble the generated `.s` using cor24-emulator's Rust assembler
- Display assembly listing with addresses and labels
- Run/Step/Reset controls for the emulator
- Register display (r0-r7, PC, condition flag) with change heatmap
- Memory viewer (sparse hex dump of SRAM and stack)
- UART I/O (TX output display, RX input field)
- Auto-scroll listing to current PC

### F6: Demo Programs
- Embedded demo PL/SW programs selectable from a dropdown
- Each demo includes .plsw source and any required .msw files
- Demos showcase language features: hello world, arithmetic, records,
  macros, inline assembly, etc.

### F7: File Upload
- Upload `.plsw` files from local filesystem
- Upload `.msw` macro files
- Replace current editor content with uploaded file

### F8: Pipeline Visualization (Tab 2)
- Second tab showing the 7-stage compiler pipeline
- Each stage displayed as a collapsible panel
- Stage 1 (Preprocess): macro expansion with annotations
- Stage 7 (Emit): final .s assembly
- Stages 2-6: diagnostic output as compiler support is added

### F9: Wizard-Style Progressive Workflow
- Left sidebar showing pipeline steps (Source, Macros, Preprocess,
  Compile, Assemble, Run)
- Action button advances to next step
- Completed steps clickable to scroll back
- Notebook cells revealed progressively as each step completes

## Non-Functional Requirements

### Performance
- Compilation via emulator should complete within a few seconds for
  typical programs (< 500 lines)
- Batch execution (50K+ instructions/tick) to prevent browser blocking
- WASM binary optimized for size (opt-level z, LTO)

### Browser Compatibility
- Modern browsers with WASM support (Chrome, Firefox, Safari, Edge)
- No server backend required -- fully client-side

### Offline Capability
- Once loaded, the app works offline (all compilation happens in WASM)
- GitHub Pages deployment with static assets only

### Accessibility
- Monospace font for all code display
- High contrast Catppuccin Mocha dark theme
- Keyboard shortcuts for compile/run workflow

## Scope: v1

### In Scope
- Single .plsw file + multiple .msw files editing
- Full compile -> assemble -> run pipeline
- Demo programs with embedded source
- File upload for .plsw and .msw
- Assembly listing with debugger (run/step/reset)
- Register and memory display
- UART I/O
- Macro expansion view (collapsible)
- Pipeline tab (stages 1 and 7 initially; 2-6 as compiler adds diagnostics)

### Out of Scope (v1)
- Multi-file .plsw projects
- Persistent storage / save to disk
- Collaborative editing
- LSP-style code intelligence (autocomplete, go-to-definition)
- Source-level debugging (stepping through PL/SW, not COR24)
- Optimization passes or optimization visualization
- Mobile-optimized layout

## Success Criteria

1. User can write a PL/SW hello world program and see it compile and run
2. Demo programs all compile and execute correctly
3. Macro expansion is visible and collapsible
4. Assembly listing scrolls to current PC during stepping
5. Register heatmap shows recent changes
6. File upload works for .plsw and .msw files
7. Pipeline tab shows at minimum preprocessing and emission stages
8. Page loads and works offline after initial load
9. Deployed to GitHub Pages at the project URL
