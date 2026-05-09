//! PL/SW compiler pipeline running on the COR24 emulator.
//!
//! The PL/SW compiler is a C program cross-compiled to COR24 assembly.
//! It runs on the emulator: we send 'c' to enter compile mode, feed
//! PL/SW source via UART, and extract the generated COR24 assembly.
//! The assembly is then assembled and run on a fresh emulator.

use std::collections::VecDeque;
use std::time::Duration;

use cor24_assembler::Assembler;
use cor24_emulator::{EmulatorCore, StopReason};

/// Embedded PL/SW compiler assembly source (from sibling project build).
const COMPILER_ASM: &str = include_str!(env!("PLSW_COMPILER_PATH"));

/// Maximum instructions during boot (reaching the menu prompt).
const BOOT_LIMIT: u64 = 50_000_000;

/// Instructions per batch when feeding input.
const FEED_BATCH: u64 = 100_000;

/// Maximum total instructions for a compilation run.
const COMPILE_LIMIT: u64 = 500_000_000;

/// Maximum instructions for running the compiled program.
const RUN_LIMIT: u64 = 10_000_000;

/// Cooperative browser yield cadence for Mass Compile.
const COOPERATIVE_YIELD_BATCHES: u64 = 8;

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
    /// Post-run emulator state dump.
    pub dump: Option<EmulatorDump>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AssembleReport {
    pub listing: String,
    pub byte_count: usize,
}

/// Validate generated COR24 assembly and return an object listing.
pub fn assemble_program(assembly_source: &str) -> Result<AssembleReport, String> {
    let mut asm = Assembler::new();
    let asm_result = asm.assemble(assembly_source);
    if !asm_result.errors.is_empty() {
        return Err(format!(
            "Assembly failed:\n{}",
            asm_result.errors.join("\n")
        ));
    }

    let listing = asm_result
        .lines
        .iter()
        .map(|line| {
            let bytes = line
                .bytes
                .iter()
                .map(|byte| format!("{byte:02X}"))
                .collect::<Vec<_>>()
                .join(" ");
            format!("{:06X}: {:<14} {}", line.address, bytes, line.source)
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok(AssembleReport {
        listing,
        byte_count: asm_result.bytes.len(),
    })
}

/// Snapshot of emulator state after execution.
#[derive(Clone, Debug, PartialEq)]
pub struct EmulatorDump {
    pub pc: u32,
    /// COR24 registers: r0, r1, r2, fp, sp, z/c, iv, ir
    pub registers: [u32; 8],
    pub condition_flag: bool,
    pub cycles: u64,
    pub led: u8,
    /// Non-zero memory regions (addr, bytes).
    pub memory_regions: Vec<(u32, Vec<u8>)>,
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

    let input = compiler_input(source, macro_sources);

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

    finish_compile(all_output, total_instructions)
}

/// Compile PL/SW source while yielding between emulator batches.
///
/// This is used by browser-driven batch workflows so the UI can remain
/// responsive while the compiler is running on the emulator.
pub async fn run_compiler_cooperative(
    source: &str,
    macro_sources: &[(String, String)],
) -> CompileResult {
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

    let mut emu = EmulatorCore::new();
    emu.set_uart_tx_busy_cycles(0);
    emu.load_program(0, &asm_result.bytes);
    emu.load_program_extent(asm_result.bytes.len() as u32);
    emu.set_pc(0);
    emu.resume();

    let mut total_instructions: u64 = 0;
    let mut all_output = String::new();
    let mut batches_since_yield = 0;

    loop {
        let result = emu.run_batch(FEED_BATCH);
        total_instructions += result.instructions_run;
        collect_uart(&mut emu, &mut all_output);
        batches_since_yield += 1;
        if batches_since_yield >= COOPERATIVE_YIELD_BATCHES {
            yield_to_browser().await;
            batches_since_yield = 0;
        }

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

    let input = compiler_input(source, macro_sources);
    let mut rx_queue: VecDeque<u8> = input.bytes().collect();

    loop {
        feed_uart_bytes(&mut emu, &mut rx_queue);

        let result = emu.run_batch(FEED_BATCH);
        total_instructions += result.instructions_run;
        collect_uart(&mut emu, &mut all_output);
        batches_since_yield += 1;
        if batches_since_yield >= COOPERATIVE_YIELD_BATCHES {
            yield_to_browser().await;
            batches_since_yield = 0;
        }

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

    finish_compile(all_output, total_instructions)
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
            dump: None,
        };
    }

    let mut emu = EmulatorCore::new();
    emu.set_uart_tx_busy_cycles(0);
    emu.load_program(0, &asm_result.bytes);
    let program_end = asm_result.bytes.len() as u32;
    emu.load_program_extent(program_end);
    emu.set_pc(0);
    emu.resume();

    let mut output = String::new();
    let mut total_instructions: u64 = 0;

    loop {
        let result = emu.run_batch(FEED_BATCH);
        total_instructions += result.instructions_run;
        collect_uart(&mut emu, &mut output);

        if matches!(result.reason, StopReason::Halted) {
            let dump = capture_dump(&emu, program_end);
            return RunResult {
                output,
                instructions: total_instructions,
                halted: true,
                error: None,
                dump: Some(dump),
            };
        }

        if total_instructions >= RUN_LIMIT {
            let dump = capture_dump(&emu, program_end);
            return RunResult {
                output,
                instructions: total_instructions,
                halted: false,
                error: Some("Program exceeded instruction limit".into()),
                dump: Some(dump),
            };
        }
    }
}

/// Capture emulator state for the post-run dump.
fn capture_dump(emu: &EmulatorCore, program_end: u32) -> EmulatorDump {
    let mut registers = [0u32; 8];
    for (i, reg) in registers.iter_mut().enumerate() {
        *reg = emu.get_reg(i as u8);
    }

    let mut memory_regions = Vec::new();

    // Scan regions for non-zero bytes, collecting contiguous runs.
    let scan_ranges: &[(u32, u32, &str)] = &[
        // Program .text + .data (0 to program_end)
        (0, program_end, "SRAM (program)"),
        // Data region after program
        (
            program_end,
            (program_end + 4096).min(0x010000),
            "SRAM (data)",
        ),
        // EBR stack area (top 3KB: 0xFEE400..0xFEEC00)
        (0xFEE400, 0xFEEC00, "EBR (stack)"),
        // I/O: LED + UART
        (0xFF0000, 0xFF0002, "I/O LED"),
        (0xFF0100, 0xFF0104, "I/O UART"),
    ];

    for &(start, end, _label) in scan_ranges {
        let mut region_start = None;
        let mut region_bytes = Vec::new();

        for addr in start..end {
            let byte = emu.read_byte(addr);
            if byte != 0 {
                if region_start.is_none() {
                    region_start = Some(addr);
                }
                region_bytes.push(byte);
            } else if region_start.is_some() {
                memory_regions.push((region_start.unwrap(), region_bytes.clone()));
                region_start = None;
                region_bytes.clear();
            }
        }
        if let Some(s) = region_start {
            memory_regions.push((s, region_bytes));
        }
    }

    EmulatorDump {
        pc: emu.pc(),
        registers,
        condition_flag: emu.condition_flag(),
        cycles: emu.cycles(),
        led: emu.get_led(),
        memory_regions,
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

fn compiler_input(source: &str, macro_sources: &[(String, String)]) -> String {
    let mut input = String::from("c\n");
    for (name, macro_src) in macro_sources {
        let include_name = name.strip_suffix(".msw").unwrap_or(name);
        input.push_str(&format!("FILE:{include_name}\n"));
        input.push_str(macro_src);
        if !macro_src.ends_with('\n') {
            input.push('\n');
        }
        input.push('\x1E');
    }
    if macro_sources.is_empty() {
        input.push_str(source);
        if !source.ends_with('\n') {
            input.push('\n');
        }
        input.push('\x04');
    } else {
        input.push_str("SOURCE:\n");
        input.push_str(source);
        if !source.ends_with('\n') {
            input.push('\n');
        }
        input.push('\x04');
    }
    input
}

fn finish_compile(all_output: String, total_instructions: u64) -> CompileResult {
    let assembly = extract_assembly(&all_output);
    let error = if assembly.is_none() && all_output.contains("compilation failed") {
        Some("Compilation failed".into())
    } else if assembly.is_none() && all_output.contains("ERROR:") {
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

async fn yield_to_browser() {
    yew::platform::time::sleep(Duration::ZERO).await;
}
