//! PL/SW compiler pipeline running on the COR24 emulator.
//!
//! The PL/SW compiler is a C program cross-compiled to COR24 assembly.
//! It runs on the emulator: we send 'c' to enter compile mode, feed
//! PL/SW source via UART, and extract the generated COR24 assembly.
//! The assembly is then assembled and run on a fresh emulator.

use std::collections::VecDeque;

use cor24_emulator::{Assembler, EmulatorCore, StopReason};

/// Embedded PL/SW compiler assembly source (from sibling project build).
const COMPILER_ASM: &str = include_str!(env!("PLSW_COMPILER_PATH"));

/// Maximum instructions during boot (reaching the menu prompt).
const BOOT_LIMIT: u64 = 50_000_000;

/// Instructions per batch when feeding input.
const FEED_BATCH: u64 = 100_000;

/// Maximum total instructions for a compilation run.
const COMPILE_LIMIT: u64 = 200_000_000;

/// Maximum instructions for running the compiled program.
const RUN_LIMIT: u64 = 10_000_000;

/// Result of running the compiler pipeline.
#[derive(Clone, Debug, PartialEq)]
pub struct CompileResult {
    /// Full compiler output (boot + compile messages).
    pub compiler_output: String,
    /// Generated COR24 assembly (.s) extracted from compiler output.
    pub assembly: Option<String>,
    /// Total instructions executed during compilation.
    pub instructions: u64,
    /// Whether the compiler halted cleanly.
    pub halted: bool,
    /// Error message if something went wrong.
    pub error: Option<String>,
}

/// Result of assembling and running the compiled program.
#[derive(Clone, Debug, PartialEq)]
pub struct RunResult {
    /// UART output from the running program.
    pub output: String,
    /// Total instructions executed.
    pub instructions: u64,
    /// Whether the program halted cleanly.
    pub halted: bool,
    /// Assembly errors if assembly failed.
    pub error: Option<String>,
}

/// Compile PL/SW source to COR24 assembly via the compiler running on the emulator.
///
/// Sends 'c' to enter compile mode, feeds the combined source (macros + main),
/// terminates with EOT (0x04), and extracts the generated assembly.
pub fn run_compiler(source: &str, macro_sources: &[(String, String)]) -> CompileResult {
    // Step 1: Assemble the compiler .s to binary
    let mut asm = Assembler::new();
    let asm_result = asm.assemble(COMPILER_ASM);
    if !asm_result.errors.is_empty() {
        return CompileResult {
            compiler_output: String::new(),
            assembly: None,
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
    emu.set_uart_tx_busy_cycles(0);
    emu.load_program(0, &asm_result.bytes);
    emu.load_program_extent(asm_result.bytes.len() as u32);
    emu.set_pc(0);
    emu.resume();

    // Step 3: Boot until we see the menu prompt
    let mut total_instructions: u64 = 0;
    let mut all_output = String::new();

    loop {
        let result = emu.run_batch(FEED_BATCH);
        total_instructions += result.instructions_run;
        collect_uart(&mut emu, &mut all_output);

        if matches!(result.reason, StopReason::Halted) {
            return CompileResult {
                compiler_output: all_output,
                assembly: None,
                instructions: total_instructions,
                halted: true,
                error: Some("Compiler halted during boot".into()),
            };
        }

        if all_output.contains("Enter suite #") {
            break;
        }

        if total_instructions >= BOOT_LIMIT {
            return CompileResult {
                compiler_output: all_output,
                assembly: None,
                instructions: total_instructions,
                halted: false,
                error: Some("Compiler boot exceeded instruction limit".into()),
            };
        }
    }

    // Step 4: Build input — "c\n" + macro sources + main source + EOT
    let mut input = String::from("c\n");
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
    input.push('\x04'); // EOT sentinel

    let mut rx_queue: VecDeque<u8> = input.bytes().collect();

    // Step 5: Feed input and collect output until compiler halts
    loop {
        feed_uart_bytes(&mut emu, &mut rx_queue);

        let result = emu.run_batch(FEED_BATCH);
        total_instructions += result.instructions_run;
        collect_uart(&mut emu, &mut all_output);

        if matches!(result.reason, StopReason::Halted) {
            break;
        }

        if total_instructions >= COMPILE_LIMIT {
            return CompileResult {
                compiler_output: all_output,
                assembly: None,
                instructions: total_instructions,
                halted: false,
                error: Some("Compilation exceeded instruction limit".into()),
            };
        }
    }

    // Step 6: Extract assembly from markers
    let assembly = extract_assembly(&all_output);
    let error = if assembly.is_none() && all_output.contains("compilation failed") {
        Some("Compilation failed".into())
    } else if assembly.is_none() && all_output.contains("ERROR:") {
        // Extract the error message
        let err_line = all_output
            .lines()
            .find(|l| l.contains("ERROR:"))
            .unwrap_or("Unknown error");
        Some(err_line.to_string())
    } else {
        None
    };

    CompileResult {
        compiler_output: all_output,
        assembly,
        instructions: total_instructions,
        halted: true,
        error,
    }
}

/// Assemble generated COR24 assembly and run it on a fresh emulator.
pub fn run_program(assembly_source: &str) -> RunResult {
    let mut asm = Assembler::new();
    let asm_result = asm.assemble(assembly_source);
    if !asm_result.errors.is_empty() {
        return RunResult {
            output: String::new(),
            instructions: 0,
            halted: false,
            error: Some(format!(
                "Assembly failed:\n{}",
                asm_result.errors.join("\n")
            )),
        };
    }

    let mut emu = EmulatorCore::new();
    emu.set_uart_tx_busy_cycles(0);
    emu.load_program(0, &asm_result.bytes);
    emu.load_program_extent(asm_result.bytes.len() as u32);
    emu.set_pc(0);
    emu.resume();

    let mut output = String::new();
    let mut total_instructions: u64 = 0;

    loop {
        let result = emu.run_batch(FEED_BATCH);
        total_instructions += result.instructions_run;
        collect_uart(&mut emu, &mut output);

        if matches!(result.reason, StopReason::Halted) {
            return RunResult {
                output,
                instructions: total_instructions,
                halted: true,
                error: None,
            };
        }

        if total_instructions >= RUN_LIMIT {
            return RunResult {
                output,
                instructions: total_instructions,
                halted: false,
                error: Some("Program exceeded instruction limit".into()),
            };
        }
    }
}

/// Extract assembly source between `--- generated assembly ---` and
/// `--- end assembly ---` markers in compiler output.
fn extract_assembly(output: &str) -> Option<String> {
    let start_marker = "--- generated assembly ---";
    let end_marker = "--- end assembly ---";

    let start = output.find(start_marker)?;
    let after_start = start + start_marker.len();
    let end = output[after_start..].find(end_marker)?;

    let asm = output[after_start..after_start + end].trim();
    if asm.is_empty() {
        None
    } else {
        Some(asm.to_string())
    }
}

/// Feed as many bytes as possible from the queue while UART RX is ready.
fn feed_uart_bytes(emu: &mut EmulatorCore, queue: &mut VecDeque<u8>) {
    while !queue.is_empty() {
        let status = emu.read_byte(0xFF0101);
        if status & 0x01 != 0 {
            break;
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
