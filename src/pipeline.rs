//! PL/SW compiler pipeline running on the COR24 emulator.
//!
//! The PL/SW compiler is a C program cross-compiled to COR24 assembly.
//! It runs on the emulator: source code enters via UART RX, output
//! exits via UART TX.  Currently the compiler implements lexer + parser
//! self-tests followed by a tokenizer REPL.

use std::collections::VecDeque;

use cor24_emulator::{Assembler, EmulatorCore, StopReason};

/// Embedded PL/SW compiler assembly source (from sibling project build).
const COMPILER_ASM: &str = include_str!(env!("PLSW_COMPILER_PATH"));

/// Maximum instructions to run during self-test boot phase.
const BOOT_LIMIT: u64 = 50_000_000;

/// Instructions per batch when feeding input.
const FEED_BATCH: u64 = 100_000;

/// Maximum total instructions for a compilation run.
const COMPILE_LIMIT: u64 = 200_000_000;

/// Result of running the compiler pipeline.
#[derive(Clone, Debug, PartialEq)]
pub struct CompileResult {
    /// Self-test output from the compiler boot phase.
    pub boot_output: String,
    /// Compiler output for the user's source (token dump from REPL).
    pub compiler_output: String,
    /// Total instructions executed.
    pub instructions: u64,
    /// Whether the compiler halted (vs hit instruction limit).
    pub halted: bool,
    /// Error message if something went wrong.
    pub error: Option<String>,
}

/// Assemble the PL/SW compiler, boot it on the emulator, feed source code,
/// and capture output.
pub fn run_compiler(source: &str, macro_sources: &[(String, String)]) -> CompileResult {
    // Step 1: Assemble the compiler .s to binary
    let mut asm = Assembler::new();
    let asm_result = asm.assemble(COMPILER_ASM);
    if !asm_result.errors.is_empty() {
        return CompileResult {
            boot_output: String::new(),
            compiler_output: String::new(),
            instructions: 0,
            halted: false,
            error: Some(format!(
                "Compiler assembly failed:\n{}",
                asm_result.errors.join("\n")
            )),
        };
    }

    // Step 2: Load compiler into emulator
    let mut emu = EmulatorCore::new();
    emu.set_uart_tx_busy_cycles(0); // No TX delay in WASM
    emu.load_program(0, &asm_result.bytes);
    emu.load_program_extent(asm_result.bytes.len() as u32);
    emu.set_pc(0);
    emu.resume();

    // Step 3: Run self-tests (boot phase) until we see the REPL prompt "> "
    let mut total_instructions: u64 = 0;
    let mut boot_output = String::new();

    loop {
        let result = emu.run_batch(FEED_BATCH);
        total_instructions += result.instructions_run;
        collect_uart(&mut emu, &mut boot_output);

        if matches!(result.reason, StopReason::Halted) {
            return CompileResult {
                boot_output,
                compiler_output: String::new(),
                instructions: total_instructions,
                halted: true,
                error: Some("Compiler halted during self-tests".into()),
            };
        }

        // Check for REPL prompt
        if boot_output.ends_with("> ") {
            break;
        }

        if total_instructions >= BOOT_LIMIT {
            return CompileResult {
                boot_output,
                compiler_output: String::new(),
                instructions: total_instructions,
                halted: false,
                error: Some(format!(
                    "Compiler boot exceeded {BOOT_LIMIT} instructions without reaching REPL"
                )),
            };
        }
    }

    // Step 4: Build input -- prepend macro files, then source
    let mut input = String::new();
    for (name, macro_src) in macro_sources {
        input.push_str(&format!("/* --- {name} --- */\n"));
        input.push_str(macro_src);
        if !macro_src.ends_with('\n') {
            input.push('\n');
        }
    }
    input.push_str(source);
    if !source.ends_with('\n') {
        input.push('\n');
    }

    // Step 5: Feed input via UART RX and capture output
    let mut rx_queue: VecDeque<u8> = input.bytes().collect();
    let mut compiler_output = String::new();

    loop {
        // Feed bytes while UART RX is ready
        feed_uart_bytes(&mut emu, &mut rx_queue);

        let result = emu.run_batch(FEED_BATCH);
        total_instructions += result.instructions_run;
        collect_uart(&mut emu, &mut compiler_output);

        if matches!(result.reason, StopReason::Halted) {
            return CompileResult {
                boot_output,
                compiler_output: strip_echo(&compiler_output),
                instructions: total_instructions,
                halted: true,
                error: None,
            };
        }

        // If all input consumed and output contains a fresh prompt, we're done
        if rx_queue.is_empty() && compiler_output.contains("> ") {
            // Check if the last prompt has no pending input
            // (output may contain multiple "> " from multi-line input)
            if compiler_output.ends_with("> ") {
                break;
            }
        }

        if total_instructions >= COMPILE_LIMIT {
            return CompileResult {
                boot_output,
                compiler_output: strip_echo(&compiler_output),
                instructions: total_instructions,
                halted: false,
                error: Some("Compilation exceeded instruction limit".into()),
            };
        }
    }

    CompileResult {
        boot_output,
        compiler_output: strip_echo(&compiler_output),
        instructions: total_instructions,
        halted: false,
        error: None,
    }
}

/// Feed as many bytes as possible from the queue while UART RX is ready.
fn feed_uart_bytes(emu: &mut EmulatorCore, queue: &mut VecDeque<u8>) {
    while !queue.is_empty() {
        let status = emu.read_byte(0xFF0101);
        if status & 0x01 != 0 {
            break; // RX buffer full, try again next batch
        }
        if let Some(byte) = queue.pop_front() {
            emu.send_uart_byte(byte);
        }
    }
}

/// Collect UART TX output into string buffer.
fn collect_uart(emu: &mut EmulatorCore, buf: &mut String) {
    let output = emu.get_uart_output();
    if !output.is_empty() {
        buf.push_str(output);
        emu.clear_uart_output();
    }
}

/// Strip echoed input characters from compiler output.
///
/// The REPL echoes each typed character back, interleaved with token output.
/// We extract just the token lines (indented with spaces) and prompts.
fn strip_echo(raw: &str) -> String {
    let mut result = String::new();
    let mut in_prompt = false;

    for line in raw.split('\n') {
        // Skip the echo of the prompt itself
        if line == "> " || line.starts_with("> ") {
            in_prompt = true;
            // Start of a new REPL interaction -- add separator
            if !result.is_empty() && !result.ends_with('\n') {
                result.push('\n');
            }
            continue;
        }

        if in_prompt {
            // Lines starting with spaces are token output
            if line.starts_with("  ") {
                result.push_str(line.trim());
                result.push('\n');
            }
            // Other lines might be echoed chars or other output
        }
    }

    result
}
