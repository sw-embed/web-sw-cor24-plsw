Wire the PL/SW compiler binary to run on the COR24 emulator.

- Embed pre-built plsw compiler binary in build.rs (from sibling project build output at ../sw-cor24-plsw/build/plsw.s)
- Create compilation workflow: assemble compiler .s -> load binary into emulator -> feed .plsw source via UART RX -> capture .s assembly output from UART TX
- Handle macro file feeding (concatenate .msw files before source, or use include mechanism)
- Error capture and display
- Notebook cell for preprocessed output (wizard step: Preprocess)
- Notebook cell for .s assembly output (wizard step: Compile)
- Scroll-to-cell on step completion