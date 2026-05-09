//! Embedded PL/SW demo programs from the compiler's examples/ directory.

/// A macro file included with a demo.
#[derive(Clone, PartialEq)]
pub struct DemoMacro {
    pub name: &'static str,
    pub source: &'static str,
}

/// A demo program with a name, description, PL/SW source, and optional macro files.
#[derive(Clone, PartialEq)]
pub struct Demo {
    pub name: &'static str,
    pub description: &'static str,
    pub source: &'static str,
    pub macros: &'static [DemoMacro],
}

/// PL/SW free-list heap allocator runtime, shared by every Storage demo.
const PLSW_STORAGE_MSW: &str = r#"/* _plsw_storage.msw -- PL/SW heap allocator runtime
 *
 * Provides _PLSW_GETMAIN / _PLSW_FREEMAIN procedures backed by
 * a free-list allocator over a statically-declared heap region,
 * plus PL/I-flavored ?GETMAIN / ?FREEMAIN macros that wrap them.
 *
 *   %INCLUDE _plsw_storage;       -- entry module: impl + heap
 *
 *   %DEFINE PLSW_STORAGE_HEADERS_ONLY 1;
 *   %INCLUDE _plsw_storage;       -- non-entry module: header only
 *
 *   %DEFINE PLSW_HEAP_SIZE 32768; -- override default 64KB heap
 *   %INCLUDE _plsw_storage;
 *
 * Macro form (PL/I-flavored; preferred surface):
 *   ?GETMAIN  SET(P)  LENGTH(N)  RC(ret);
 *     -- expands to: P = _PLSW_GETMAIN(N);
 *                    IF (P = 0) THEN ret = 4; ELSE ret = 0;
 *     -- ret is 0 on success, 4 on out-of-memory.
 *
 *   ?FREEMAIN ADDR(P) LENGTH(N) RC(ret);
 *     -- expands to: ret = _PLSW_FREEMAIN(P, N);
 *     -- ret passes through the procedure's return:
 *          0 = success
 *          1 = double-free / invalid pointer
 *          2 = size mismatch (LENGTH doesn't match recorded size)
 *
 * Clause naming follows MVS convention:
 *   SET(lvalue)   -- GETMAIN: receive-pointer (written)
 *   ADDR(lvalue)  -- FREEMAIN: source-pointer (read)
 *   LENGTH(expr)  -- block size in bytes
 *   RC(lvalue)    -- return code (written)
 *
 * Procedure form (lower-level surface; same effect):
 *   P  = _PLSW_GETMAIN(N);            P=0 on OOM
 *   RC = _PLSW_FREEMAIN(P, N);        same RC values as above
 */

/* --- Default heap size (overridable via %DEFINE before include) --- */
%IF DEFINED(PLSW_HEAP_SIZE);
%ELSE;
%DEFINE PLSW_HEAP_SIZE 65536;
%ENDIF;

/* --- Allocated-block sentinel ---
 * Stored in BLOCK_NEXT while the block is allocated. 0xFFFFFF
 * cannot be a real heap address (heap lives in low SRAM under
 * 1 MB). Detects double-free and validates that _PLSW_FREEMAIN
 * received a previously-allocated block. */
%DEFINE _PLSW_ALLOC_MAGIC 16777215;

/* --- Block header (BASED template) --- */
DCL 1 _PLSW_BLOCK BASED,
  3 _PLSW_BLOCK_SIZE INT,
  3 _PLSW_BLOCK_NEXT INT;
DCL _PLSW_BP PTR;

%IF DEFINED(PLSW_STORAGE_HEADERS_ONLY);
/* Non-entry module: no heap, no impl. */
%ELSE;

/* --- Heap region + free-list head --- */
DCL _PLSW_HEAP_BUF(PLSW_HEAP_SIZE) BYTE;
DCL _PLSW_FREE_HEAD INT INIT(0);
DCL _PLSW_INIT_DONE INT INIT(0);

_PLSW_INIT: PROC;
    _PLSW_FREE_HEAD = ADDR(_PLSW_HEAP_BUF);
    _PLSW_BP = _PLSW_FREE_HEAD;
    _PLSW_BP->_PLSW_BLOCK_SIZE = PLSW_HEAP_SIZE;
    _PLSW_BP->_PLSW_BLOCK_NEXT = 0;
    _PLSW_INIT_DONE = 1;
END;

_PLSW_GETMAIN: PROC(SIZE INT) RETURNS(INT);
    DCL NEEDED INT;
    DCL PREV INT;
    DCL CURR INT;
    DCL CURR_SIZE INT;
    DCL CURR_NEXT INT;
    DCL NEW_FREE INT;

    IF (_PLSW_INIT_DONE = 0) THEN
        CALL _PLSW_INIT();

    NEEDED = SIZE + 6;

    PREV = 0;
    CURR = _PLSW_FREE_HEAD;
    DO WHILE (CURR != 0);
        _PLSW_BP = CURR;
        CURR_SIZE = _PLSW_BP->_PLSW_BLOCK_SIZE;
        CURR_NEXT = _PLSW_BP->_PLSW_BLOCK_NEXT;
        IF (CURR_SIZE >= NEEDED) THEN DO;
            IF (CURR_SIZE >= NEEDED + 7) THEN DO;
                NEW_FREE = CURR + NEEDED;
                _PLSW_BP = NEW_FREE;
                _PLSW_BP->_PLSW_BLOCK_SIZE = CURR_SIZE - NEEDED;
                _PLSW_BP->_PLSW_BLOCK_NEXT = CURR_NEXT;
                _PLSW_BP = CURR;
                _PLSW_BP->_PLSW_BLOCK_SIZE = NEEDED;
                _PLSW_BP->_PLSW_BLOCK_NEXT = _PLSW_ALLOC_MAGIC;
                IF (PREV = 0) THEN
                    _PLSW_FREE_HEAD = NEW_FREE;
                ELSE DO;
                    _PLSW_BP = PREV;
                    _PLSW_BP->_PLSW_BLOCK_NEXT = NEW_FREE;
                END;
            END;
            ELSE DO;
                _PLSW_BP = CURR;
                _PLSW_BP->_PLSW_BLOCK_NEXT = _PLSW_ALLOC_MAGIC;
                IF (PREV = 0) THEN
                    _PLSW_FREE_HEAD = CURR_NEXT;
                ELSE DO;
                    _PLSW_BP = PREV;
                    _PLSW_BP->_PLSW_BLOCK_NEXT = CURR_NEXT;
                END;
            END;
            RETURN(CURR + 6);
        END;
        PREV = CURR;
        CURR = CURR_NEXT;
    END;
    RETURN(0);
END;

_PLSW_FREEMAIN: PROC(USERADDR INT, LEN INT) RETURNS(INT);
    DCL BLOCK INT;
    DCL EXPECTED INT;
    DCL PREV INT;
    DCL CURR INT;
    DCL BLK_SIZE INT;
    DCL NEXT_AFTER INT;
    DCL NEXT_SIZE INT;

    BLOCK = USERADDR - 6;
    EXPECTED = LEN + 6;

    _PLSW_BP = BLOCK;
    IF (_PLSW_BP->_PLSW_BLOCK_NEXT != _PLSW_ALLOC_MAGIC) THEN
        RETURN(1);
    IF (_PLSW_BP->_PLSW_BLOCK_SIZE != EXPECTED) THEN
        RETURN(2);

    PREV = 0;
    CURR = _PLSW_FREE_HEAD;
    DO WHILE (CURR != 0 AND CURR < BLOCK);
        PREV = CURR;
        _PLSW_BP = CURR;
        CURR = _PLSW_BP->_PLSW_BLOCK_NEXT;
    END;

    _PLSW_BP = BLOCK;
    _PLSW_BP->_PLSW_BLOCK_NEXT = CURR;
    IF (PREV = 0) THEN
        _PLSW_FREE_HEAD = BLOCK;
    ELSE DO;
        _PLSW_BP = PREV;
        _PLSW_BP->_PLSW_BLOCK_NEXT = BLOCK;
    END;

    _PLSW_BP = BLOCK;
    BLK_SIZE = _PLSW_BP->_PLSW_BLOCK_SIZE;
    IF (CURR != 0) THEN DO;
        IF (BLOCK + BLK_SIZE = CURR) THEN DO;
            _PLSW_BP = CURR;
            NEXT_AFTER = _PLSW_BP->_PLSW_BLOCK_NEXT;
            NEXT_SIZE = _PLSW_BP->_PLSW_BLOCK_SIZE;
            _PLSW_BP = BLOCK;
            _PLSW_BP->_PLSW_BLOCK_SIZE = BLK_SIZE + NEXT_SIZE;
            _PLSW_BP->_PLSW_BLOCK_NEXT = NEXT_AFTER;
        END;
    END;

    RETURN(0);
END;

%ENDIF;

/* --- PL/I-flavored macros wrapping the procedures ---
 *
 * Defined unconditionally so non-entry modules can use the macros
 * even when their include doesn't emit the impl + heap. */

MACRODEF GETMAIN;
    REQUIRED SET(lvalue);
    REQUIRED LENGTH(expr);
    REQUIRED RC(lvalue);
    {SET} = _PLSW_GETMAIN({LENGTH});
    IF ({SET} = 0) THEN {RC} = 4;
    ELSE {RC} = 0;
END;

MACRODEF FREEMAIN;
    REQUIRED ADDR(lvalue);
    REQUIRED LENGTH(expr);
    REQUIRED RC(lvalue);
    {RC} = _PLSW_FREEMAIN({ADDR}, {LENGTH});
END;
"#;

const STORAGE_MACROS: &[DemoMacro] = &[DemoMacro {
    name: "_plsw_storage.msw",
    source: PLSW_STORAGE_MSW,
}];

/// All demos, alphabetized by name.
pub const DEMOS: &[Demo] = &[
    // ── An Empty Module ─────────────────────────────────────────────
    Demo {
        name: "An Empty Module",
        description: "No-op MAIN procedure, like IEFBR14",
        source: r#"/* empty_proc.plsw -- No-op PL/SW program
 * Equivalent in spirit to IEFBR14: enter MAIN, do nothing, return. */

MAIN: PROC;
    /* 1. Press the PL/EDIT button. */
    /* 2. Type IF on the empty line below, then press F4 or Ctrl-Space. */

    /* 3. Use the ? button for PL/EDIT template help. */
END;
"#,
        macros: &[],
    },
    // ── Chain ────────────────────────────────────────────────────────
    Demo {
        name: "Chain",
        description: "Control block chain (CVT/ASCB/ASXB/TCB) with SIZEOF",
        source: r#"/* chain.plsw -- Control Block Chain Demo
 * Allocates CVT/ASCB/ASXB/TCB from a bump arena,
 * wires pointer chain. Verify with --dump. */

%INCLUDE cvt;
DCL CVTPTR PTR;
%INCLUDE ascb;
DCL ASCBPTR PTR;
%INCLUDE asxb;
DCL ASXBPTR PTR;
%INCLUDE tcb;
DCL TCBPTR PTR;

/* --- Arena --- */
DCL ARENA(512) BYTE;
DCL ARENA_POS INT INIT(0);

ALLOC: PROC(SIZE INT) RETURNS(INT);
    DCL BASE INT;
    BASE = ADDR(ARENA) + ARENA_POS;
    ARENA_POS = ARENA_POS + SIZE;
    RETURN(BASE);
END;

/* --- Main: build the chain --- */
MAIN: PROC;
    /* Allocate control blocks */
    CVTPTR  = ALLOC(SIZEOF(CVT));
    ASCBPTR = ALLOC(SIZEOF(ASCB));
    ASXBPTR = ALLOC(SIZEOF(ASXB));
    TCBPTR  = ALLOC(SIZEOF(TCB));

    /* Set eyecatchers */
    CVTPTR->CVTEYE   = 'CVT ';
    ASCBPTR->ASCBEYE  = 'ASCB';
    ASXBPTR->ASXBEYE  = 'ASXB';
    TCBPTR->TCBEYE   = 'TCB ';

    /* Wire: CVT -> ASCB, CVT -> TCB */
    CVTPTR->CVTASCBH  = ASCBPTR;
    CVTPTR->CVTASCBL  = ASCBPTR;
    CVTPTR->CVTCTCB   = TCBPTR;

    /* Wire: ASCB (single entry) */
    ASCBPTR->ASCBNEXT  = 0;
    ASCBPTR->ASCBPREV  = 0;
    ASCBPTR->ASCBASID  = 1;
    ASCBPTR->ASCBASXB  = ASXBPTR;
    ASCBPTR->ASCBTCBH  = TCBPTR;
    ASCBPTR->ASCBTCBL  = TCBPTR;

    /* Wire: ASXB <-> ASCB, -> TCB */
    ASXBPTR->ASXBASCB  = ASCBPTR;
    ASXBPTR->ASXBFTCB  = TCBPTR;
    ASXBPTR->ASXBLTCB  = TCBPTR;

    /* Wire: TCB (single), back to ASCB */
    TCBPTR->TCBNEXT    = 0;
    TCBPTR->TCBPREV    = 0;
    TCBPTR->TCBASCB    = ASCBPTR;
END;
"#,
        macros: &[
            DemoMacro {
                name: "ascb.msw",
                source: r#"/* ascb.msw -- Address Space Control Block (COR24) */

DCL 1 ASCB BASED,
  3 ASCBEYE(4) CHAR,          /* 'ASCB' eyecatcher */
  3 ASCBVER    BYTE,
  3 ASCBFLAGS  BYTE,
  3 ASCBNEXT   PTR,           /* next ASCB in chain */
  3 ASCBPREV   PTR,           /* prev ASCB in chain */
  3 ASCBASID   INT(16),       /* address space ID */
  3 ASCBSTATE  BYTE,
  3 ASCBASXB   PTR,           /* -> ASXB */
  3 ASCBTCBH   PTR,           /* first TCB in this AS */
  3 ASCBTCBL   PTR;           /* last TCB in this AS */
/* size = 4 + 1 + 1 + 3 + 3 + 2 + 1 + 3 + 3 + 3 = 24 */
"#,
            },
            DemoMacro {
                name: "asxb.msw",
                source: r#"/* asxb.msw -- Address Space Extension Block (COR24) */

DCL 1 ASXB BASED,
  3 ASXBEYE(4) CHAR,          /* 'ASXB' eyecatcher */
  3 ASXBVER    BYTE,
  3 ASXBFLAGS  BYTE,
  3 ASXBASCB   PTR,           /* back pointer to ASCB */
  3 ASXBFTCB   PTR,           /* first TCB */
  3 ASXBLTCB   PTR,           /* last TCB */
  3 ASXBCTCB   PTR;           /* current TCB */
/* size = 4 + 1 + 1 + 3 + 3 + 3 + 3 = 18 */
"#,
            },
            DemoMacro {
                name: "cvt.msw",
                source: r#"/* cvt.msw -- Communications Vector Table (COR24) */

DCL 1 CVT BASED,
  3 CVTEYE(4)  CHAR,          /* 'CVT ' eyecatcher */
  3 CVTVER     BYTE,
  3 CVTFLAGS   BYTE,
  3 CVTASCBH   PTR,           /* head of ASCB chain */
  3 CVTASCBL   PTR,           /* tail of ASCB chain */
  3 CVTCTCB    PTR;           /* current TCB (bootstrap) */
/* size = 4 + 1 + 1 + 3 + 3 + 3 = 15. Padded to CVT_SIZE=19 */
"#,
            },
            DemoMacro {
                name: "tcb.msw",
                source: r#"/* tcb.msw -- Task Control Block (COR24) */

DCL 1 TCB BASED,
  3 TCBEYE(4)  CHAR,          /* 'TCB ' eyecatcher */
  3 TCBVER     BYTE,
  3 TCBFLAGS   BYTE,
  3 TCBNEXT    PTR,           /* next TCB in chain */
  3 TCBPREV    PTR,           /* prev TCB in chain */
  3 TCBASCB    PTR,           /* owning ASCB */
  3 TCBSTATE   BYTE,
  3 TCBPRI     BYTE;
/* size = 4 + 1 + 1 + 3 + 3 + 3 + 1 + 1 = 17 */
"#,
            },
        ],
    },
    // ── Define ───────────────────────────────────────────────────────
    Demo {
        name: "Define",
        description: "%DEFINE compile-time constants and value substitution",
        source: r#"/* define.plsw -- %DEFINE Value Substitution Demo
 * Demonstrates compile-time constants via %DEFINE.
 * Constants are substituted at lex time -- zero runtime cost. */

%DEFINE UART_DATA 16711936;   /* 0xFF0100 */
%DEFINE NEWLINE 10;
%DEFINE MAX_COUNT 5;

DCL DIGITS(12) CHAR;

/* Print an integer to UART */
PRINT_INT: PROC(N INT);
    DCL D INT;
    DCL POS INT;

    IF (N = 0) THEN DO;
        CALL UART_PUTCHAR(48);
        RETURN;
    END;

    POS = 0;
    DO WHILE (N > 0);
        D = N / 10;
        DIGITS(POS) = N - D * 10 + 48;
        N = D;
        POS = POS + 1;
    END;

    DO WHILE (POS > 0);
        POS = POS - 1;
        CALL UART_PUTCHAR(DIGITS(POS));
    END;
END;

/* Main: count from 1 to MAX_COUNT */
MAIN: PROC;
    DCL I INT;
    DO I = 1 TO MAX_COUNT;
        CALL PRINT_INT(I);
        CALL UART_PUTCHAR(NEWLINE);
    END;
END;
"#,
        macros: &[],
    },
    // ── Else Print ──────────────────────────────────────────────────
    Demo {
        name: "Else Print",
        description: "IF/ELSE branch demo with false condition",
        source: r#"/* else_print.plsw -- IF/ELSE demo
 * Prints the ELSE branch because 2 < 1 is false. */

DCL THEN_MSG(16) CHAR INIT('then path');
DCL ELSE_MSG(16) CHAR INIT('else path');

MAIN: PROC;
    IF (2 < 1) THEN DO;
        CALL UART_PUTS(ADDR(THEN_MSG));
        CALL UART_PUTCHAR(10);
    END;
    ELSE DO;
        CALL UART_PUTS(ADDR(ELSE_MSG));
        CALL UART_PUTCHAR(10);
    END;
END;
"#,
        macros: &[],
    },
    // ── Hello ────────────────────────────────────────────────────────
    Demo {
        name: "Hello",
        description: "Print a greeting via UART",
        source: r#"/* hello.plsw -- Hello World in PL/SW
 * First end-to-end demo of the PL/SW compiler.
 * Declares a static string and prints it via UART. */

DCL MSG(20) CHAR INIT('Hello from PL/SW!');

MAIN: PROC;
    CALL UART_PUTS(ADDR(MSG));
END;
"#,
        macros: &[],
    },
    // ── Hello Macro ──────────────────────────────────────────────────
    Demo {
        name: "Hello Macro",
        description: "Hello World using ?GREET macro",
        source: r#"/* hello_macro.plsw -- Hello World using macros
 * Demonstrates ?GREET macro from greet.msw */

%INCLUDE greet;

DCL MSG(20) CHAR INIT('Hello from macros!');

MAIN: PROC;
    ?GREET MSG(_MSG);
END;
"#,
        macros: &[DemoMacro {
            name: "greet.msw",
            source: r#"/* greet.msw -- greeting macros for PL/SW */

/* ?GREET(MSG(label)) -- print a string via UART_PUTS */
MACRODEF GREET;
    REQUIRED MSG(lvalue);
    GEN DO;
        'la      r0,{MSG}';
        'push    r0';
        'la      r2,_UART_PUTS';
        'jal     r1,(r2)';
        'add     sp,3';
    END;
END;
"#,
        }],
    },
    // ── LED Toggle ───────────────────────────────────────────────────
    Demo {
        name: "LED Toggle",
        description: "Toggle LED via MMIO with inline ASM",
        source: r#"/* led.plsw -- LED Toggle Demo for PL/SW
 * Toggles the LED at MMIO address 0xFF0000 on/off
 * in a loop with a delay between toggles.
 * Demonstrates: inline ASM, MMIO access, DO WHILE loop. */

DCL LED_STATE BYTE INIT(0);
DCL COUNT INT;

/* Delay loop -- waste cycles for visible toggle */
DELAY: PROC;
    COUNT = 0;
    DO WHILE (COUNT < 50000);
        COUNT = COUNT + 1;
    END;
END;

/* Write a byte to the LED MMIO register via inline ASM */
LED_WRITE: PROC(VAL BYTE);
    ASM DO;
        'la      r0,0xFF0000';
        'lw      r1,9(fp)';
        'sb      r1,0(r0)';
    END;
END;

/* Main: toggle LED on/off 10 times */
MAIN: PROC;
    DCL I INT;
    I = 0;
    DO WHILE (I < 10);
        /* Toggle state: 0->1, 1->0 */
        IF (LED_STATE = 0) THEN
            LED_STATE = 1;
        ELSE
            LED_STATE = 0;

        /* Write to LED hardware */
        CALL LED_WRITE(LED_STATE);

        /* Wait for visible effect */
        CALL DELAY();

        I = I + 1;
    END;

    /* Turn LED off at exit */
    CALL LED_WRITE(0);
END;
"#,
        macros: &[],
    },
    // ── Loop ─────────────────────────────────────────────────────────
    Demo {
        name: "Loop",
        description: "Print numbers 1-10 with DO counted loop and PRINT_INT",
        source: r#"/* loop.plsw -- Counted Loop Demo for PL/SW
 * Prints numbers 1 through 10 using DO I = 1 TO 10.
 * Demonstrates: DO count syntax, procedure calls, arithmetic,
 * integer-to-decimal output via UART. */

DCL DIGITS(12) CHAR;

/* Print an integer to UART as decimal digits */
PRINT_INT: PROC(N INT);
    DCL D INT;
    DCL POS INT;

    /* Handle zero */
    IF (N = 0) THEN DO;
        CALL UART_PUTCHAR(48);
        RETURN;
    END;

    /* Extract digits into buffer (reverse order) */
    POS = 0;
    DO WHILE (N > 0);
        D = N / 10;
        DIGITS(POS) = N - D * 10 + 48;
        N = D;
        POS = POS + 1;
    END;

    /* Print digits in forward order */
    DO WHILE (POS > 0);
        POS = POS - 1;
        CALL UART_PUTCHAR(DIGITS(POS));
    END;
END;

/* Main: print numbers 1 through 10 */
MAIN: PROC;
    DCL I INT;
    DO I = 1 TO 10;
        CALL PRINT_INT(I);
        CALL UART_PUTCHAR(10);
    END;
END;
"#,
        macros: &[],
    },
    // ── Macro ────────────────────────────────────────────────────────
    Demo {
        name: "Macro",
        description: "LED and NOP macros from system.msw",
        source: r#"/* macro.plsw -- Macro System Demo
 * Includes system.msw, invokes ?LED_SET and ?NOP macros.
 * Uses runtime UART_PUTS for output.
 * Demonstrates: %INCLUDE, MACRODEF, ?MACRO() invocation,
 * GEN block expansion to inline ASM. */

%INCLUDE system;

DCL MSG(20) CHAR INIT('Macro demo OK');

MAIN: PROC;
    /* Print message using runtime UART */
    CALL UART_PUTS(ADDR(MSG));

    /* LED on via macro (active-low: 0=on) */
    ?LED_SET VAL(0);

    /* NOP macro -- just loads a constant */
    ?NOP COUNT(99);

    /* LED off via macro */
    ?LED_SET VAL(1);
END;
"#,
        macros: &[DemoMacro {
            name: "system.msw",
            source: r#"/* system.msw -- System service macros for COR24 */

/* ?LED_SET(VAL(n)) -- write a byte to LED MMIO register */
MACRODEF LED_SET;
    REQUIRED VAL(expr);
    GEN DO;
        'la      r2,16711680';
        'lc      r0,{VAL}';
        'sb      r0,0(r2)';
    END;
END;

/* ?NOP(COUNT(n)) -- emit n NOP instructions */
MACRODEF NOP;
    REQUIRED COUNT(expr);
    GEN DO;
        'lc      r0,{COUNT}';
    END;
END;
"#,
        }],
    },
    // ── Multi-file Project ──────────────────────────────────────────
    Demo {
        name: "Multi-file Project",
        description: "Main source with include files for DCLs, PROCs, and MACRODEFs",
        source: r#"/* multi_file_project.plsw -- Main program
 * Uses .msw include files as project units:
 * - project_data.msw: shared DCLs
 * - project_lib.msw: helper PROCs
 * - project_macros.msw: MACRODEF skeleton
 */

%INCLUDE project_data;
%INCLUDE project_lib;
%INCLUDE project_macros;

MAIN: PROC;
    DCL SUM INT(24);
    DCL TOTAL INT(24);

    SUM = ADD2(2, 3);
    TOTAL = ADD3(7, 8, 9);

    CALL UART_PUTS(ADDR(APP_MSG));
    CALL PRINT_INT(SUM);
    CALL UART_PUTCHAR(10);

    CALL UART_PUTS(ADDR(ADD3_MSG));
    CALL PRINT_INT(TOTAL);
    CALL UART_PUTCHAR(10);

    ?EMIT_NOP COUNT(3);
END;
"#,
        macros: &[
            DemoMacro {
                name: "project_data.msw",
                source: r#"/* project_data.msw -- shared declarations */

DCL APP_MSG(16) CHAR INIT('sum = ');
DCL ADD3_MSG(16) CHAR INIT('add3 = ');
DCL DIGITS(12) CHAR;
"#,
            },
            DemoMacro {
                name: "project_lib.msw",
                source: r#"/* project_lib.msw -- callable helper library */

ADD2: PROC(A INT(24), B INT(24)) RETURNS(INT(24));
    RETURN(A + B);
END;

ADD3: PROC(A INT(24), B INT(24), C INT(24)) RETURNS(INT(24));
    RETURN(A + B + C);
END;

/* UART_PUTCHAR writes one character code. Use PRINT_INT for decimal numbers. */
PRINT_INT: PROC(N INT(24));
    DCL D INT(24);
    DCL POS INT(24);

    IF (N = 0) THEN DO;
        CALL UART_PUTCHAR(48);
        RETURN;
    END;

    POS = 0;
    DO WHILE (N > 0);
        D = N / 10;
        DIGITS(POS) = N - D * 10 + 48;
        N = D;
        POS = POS + 1;
    END;

    DO WHILE (POS > 0);
        POS = POS - 1;
        CALL UART_PUTCHAR(DIGITS(POS));
    END;
END;
"#,
            },
            DemoMacro {
                name: "project_macros.msw",
                source: r#"/* project_macros.msw -- project macros */

MACRODEF EMIT_NOP;
    REQUIRED COUNT(expr);
    GEN DO;
        'lc      r0,{COUNT}';
    END;
END;
"#,
            },
        ],
    },
    // ── Record ───────────────────────────────────────────────────────
    Demo {
        name: "Record",
        description: "Records, pointers, and field access",
        source: r#"/* record.plsw -- Record and Pointer Demo for PL/SW
 * Declares a multi-level record, fills fields, takes address,
 * accesses fields via pointer dereference.
 * Demonstrates: level-based DCL, record field access, ADDR(),
 * pointer dereference (P->field), arithmetic on fields. */

DCL LBL_X   (8) CHAR INIT('X = ');
DCL LBL_Y   (8) CHAR INIT('Y = ');
DCL LBL_PX  (8) CHAR INIT('P->X = ');
DCL LBL_PY  (8) CHAR INIT('P->Y = ');
DCL LBL_SUM (8) CHAR INIT('Sum = ');
DCL DIGITS (12) CHAR;

/* Print an integer to UART as decimal digits */
PRINT_INT: PROC(N INT);
    DCL D INT;
    DCL POS INT;

    IF (N = 0) THEN DO;
        CALL UART_PUTCHAR(48);
        RETURN;
    END;

    POS = 0;
    DO WHILE (N > 0);
        D = N / 10;
        DIGITS(POS) = N - D * 10 + 48;
        N = D;
        POS = POS + 1;
    END;

    DO WHILE (POS > 0);
        POS = POS - 1;
        CALL UART_PUTCHAR(DIGITS(POS));
    END;
END;

/* Main: record and pointer operations */
MAIN: PROC;
    DCL 1 POINT,
        3 X INT,
        3 Y INT;
    DCL P PTR;

    /* Fill record fields directly */
    POINT.X = 100;
    POINT.Y = 200;

    /* Print field values */
    CALL UART_PUTS(ADDR(LBL_X));
    CALL PRINT_INT(POINT.X);
    CALL UART_PUTCHAR(10);

    CALL UART_PUTS(ADDR(LBL_Y));
    CALL PRINT_INT(POINT.Y);
    CALL UART_PUTCHAR(10);

    /* Take address, access via pointer */
    P = ADDR(POINT);

    CALL UART_PUTS(ADDR(LBL_PX));
    CALL PRINT_INT(P->X);
    CALL UART_PUTCHAR(10);

    CALL UART_PUTS(ADDR(LBL_PY));
    CALL PRINT_INT(P->Y);
    CALL UART_PUTCHAR(10);

    /* Compute sum via pointer */
    CALL UART_PUTS(ADDR(LBL_SUM));
    CALL PRINT_INT(P->X + P->Y);
    CALL UART_PUTCHAR(10);
END;
"#,
        macros: &[],
    },
    // ── Select ───────────────────────────────────────────────────────
    Demo {
        name: "Select",
        description: "SELECT/WHEN/OTHERWISE token classifier",
        source: r#"/* SELECT/WHEN demo -- token classifier */

MAIN: PROC;
    DCL X INT(24);

    X = 2;

    SELECT;
        WHEN (X = 1) CALL UART_PUTCHAR(65);
        WHEN (X = 2) CALL UART_PUTCHAR(66);
        WHEN (X = 3) CALL UART_PUTCHAR(67);
        OTHERWISE CALL UART_PUTCHAR(63);
    END;

    CALL UART_PUTCHAR(10);
END;
"#,
        macros: &[],
    },
    // ── Select Nested ────────────────────────────────────────────────
    Demo {
        name: "Select Nested",
        description: "Nested SELECT/WHEN handler chains for token dispatch",
        source: "/* SELECT/WHEN demo -- nested handler chains for token dispatch */

DCL S1(10) CHAR INIT('keyword');
DCL S2(10) CHAR INIT('number');
DCL S3(10) CHAR INIT('string');
DCL S4(10) CHAR INIT('unknown');
DCL S5(10) CHAR INIT('operator');
DCL SP(10) CHAR INIT('operator: ');
DCL NL(2) CHAR INIT('
');
DCL T1(10) CHAR INIT('Test 1: ');
DCL T2(10) CHAR INIT('Test 2: ');
DCL T3(10) CHAR INIT('Test 3: nested');
DCL PA(2) CHAR INIT('+');
DCL MI(2) CHAR INIT('-');
DCL ST(2) CHAR INIT('*');
DCL QU(2) CHAR INIT('?');

MAIN: PROC;
    DCL X INT(24);
    DCL Y INT(24);

    CALL UART_PUTS(ADDR(T1));
    X = 1;
    SELECT;
        WHEN (X = 1) CALL UART_PUTS(ADDR(S1));
        WHEN (X = 2) CALL UART_PUTS(ADDR(S2));
        WHEN (X = 3) CALL UART_PUTS(ADDR(S3));
        OTHERWISE CALL UART_PUTS(ADDR(S4));
    END;
    CALL UART_PUTS(ADDR(NL));

    CALL UART_PUTS(ADDR(T2));
    X = 4;
    SELECT;
        WHEN (X = 1) CALL UART_PUTS(ADDR(S1));
        WHEN (X = 2) CALL UART_PUTS(ADDR(S2));
        WHEN (X = 3) CALL UART_PUTS(ADDR(S3));
        OTHERWISE CALL UART_PUTS(ADDR(S4));
    END;
    CALL UART_PUTS(ADDR(NL));

    CALL UART_PUTS(ADDR(T3));
    CALL UART_PUTS(ADDR(NL));
    X = 4;
    Y = 2;
    SELECT;
        WHEN (X = 1) CALL UART_PUTS(ADDR(S1));
        WHEN (X = 4) DO;
            CALL UART_PUTS(ADDR(SP));
            SELECT;
                WHEN (Y = 1) CALL UART_PUTS(ADDR(PA));
                WHEN (Y = 2) CALL UART_PUTS(ADDR(MI));
                WHEN (Y = 3) CALL UART_PUTS(ADDR(ST));
                OTHERWISE CALL UART_PUTS(ADDR(QU));
            END;
        END;
        OTHERWISE CALL UART_PUTS(ADDR(S4));
    END;
    CALL UART_PUTS(ADDR(NL));
END;
",
        macros: &[],
    },
    // ── Storage: Basic ───────────────────────────────────────────────
    Demo {
        name: "Storage: Basic",
        description: "?GETMAIN/?FREEMAIN macros — single allocation cycle",
        source: r#"/* storage_basic.plsw -- single alloc, free via ?GETMAIN/?FREEMAIN.
 *
 * Allocates a 12-byte block, frees it, prints status from each.
 * Uses the macros that wrap _PLSW_GETMAIN/_PLSW_FREEMAIN. */

%DEFINE PLSW_HEAP_SIZE 1024;
%INCLUDE _plsw_storage;

DCL P INT;
DCL RC INT;

MAIN: PROC;
    ?GETMAIN LENGTH(12) SET(P) RC(RC);
    IF (RC = 0) THEN
        CALL UART_PUTS(ADDR(MSG_OK));
    ELSE
        CALL UART_PUTS(ADDR(MSG_OOM));

    ?FREEMAIN LENGTH(12) ADDR(P) RC(RC);
    IF (RC = 0) THEN
        CALL UART_PUTS(ADDR(MSG_FREED));
    ELSE
        CALL UART_PUTS(ADDR(MSG_FAIL));
END;

DCL MSG_OK    (16) CHAR INIT('alloc 12: ok');
DCL MSG_OOM   (16) CHAR INIT('alloc 12: oom');
DCL MSG_FREED (16) CHAR INIT('free: ok');
DCL MSG_FAIL  (16) CHAR INIT('free: fail');
"#,
        macros: STORAGE_MACROS,
    },
    // ── Storage: Coalesce ────────────────────────────────────────────
    Demo {
        name: "Storage: Coalesce",
        description: "Forward-coalesce on free reconstitutes a large free block",
        source: r#"/* storage_coalesce.plsw -- forward-coalesce on free.
 *
 * Allocates three 100-byte blocks. Frees them in reverse order
 * (C, B, A). Each free should coalesce with the next-higher free
 * block, eventually reconstituting a single ~1024-byte free
 * region. Verifies by re-allocating a block larger than any
 * original (800 bytes) -- only succeeds if coalesce worked. */

%DEFINE PLSW_HEAP_SIZE 1024;
%INCLUDE _plsw_storage;

DCL A INT;
DCL B INT;
DCL C INT;
DCL BIG INT;
DCL RC INT;

MAIN: PROC;
    ?GETMAIN LENGTH(100) SET(A) RC(RC);
    ?GETMAIN LENGTH(100) SET(B) RC(RC);
    ?GETMAIN LENGTH(100) SET(C) RC(RC);

    IF (A != 0 AND B != 0 AND C != 0) THEN
        CALL UART_PUTS(ADDR(MSG_3OK));
    ELSE
        CALL UART_PUTS(ADDR(MSG_3FAIL));

    /* Free in reverse so each forward-coalesce extends the gap */
    ?FREEMAIN LENGTH(100) ADDR(C) RC(RC);
    ?FREEMAIN LENGTH(100) ADDR(B) RC(RC);
    ?FREEMAIN LENGTH(100) ADDR(A) RC(RC);

    ?GETMAIN LENGTH(800) SET(BIG) RC(RC);
    IF (RC = 0) THEN
        CALL UART_PUTS(ADDR(MSG_BIG_OK));
    ELSE
        CALL UART_PUTS(ADDR(MSG_BIG_FAIL));
END;

DCL MSG_3OK     (24) CHAR INIT('three 100-byte: ok');
DCL MSG_3FAIL   (24) CHAR INIT('three 100-byte: fail');
DCL MSG_BIG_OK  (24) CHAR INIT('alloc 800 after: ok');
DCL MSG_BIG_FAIL(24) CHAR INIT('alloc 800 after: fail');
"#,
        macros: STORAGE_MACROS,
    },
    // ── Storage: Double Free ─────────────────────────────────────────
    Demo {
        name: "Storage: Double Free",
        description: "_PLSW_FREEMAIN returns RC=1 on a second free of the same block",
        source: r#"/* storage_double_free.plsw -- double-free detection.
 *
 * Allocates a block, frees it once (RC=0), tries to free again
 * (RC=1; the alloc-magic sentinel was cleared by the first free
 * so the header no longer looks allocated). */

%DEFINE PLSW_HEAP_SIZE 256;
%INCLUDE _plsw_storage;

DCL P INT;
DCL RC1 INT;
DCL RC2 INT;
DCL RC INT;

MAIN: PROC;
    ?GETMAIN LENGTH(20) SET(P) RC(RC);
    ?FREEMAIN LENGTH(20) ADDR(P) RC(RC1);
    ?FREEMAIN LENGTH(20) ADDR(P) RC(RC2);

    IF (RC1 = 0) THEN
        CALL UART_PUTS(ADDR(MSG_FIRST_OK));
    ELSE
        CALL UART_PUTS(ADDR(MSG_FIRST_BAD));

    IF (RC2 = 1) THEN
        CALL UART_PUTS(ADDR(MSG_SECOND_DETECTED));
    ELSE
        CALL UART_PUTS(ADDR(MSG_SECOND_MISSED));
END;

DCL MSG_FIRST_OK        (32) CHAR INIT('first free: rc=0');
DCL MSG_FIRST_BAD       (32) CHAR INIT('first free: rc!=0');
DCL MSG_SECOND_DETECTED (32) CHAR INIT('second free: rc=1 (detected)');
DCL MSG_SECOND_MISSED   (32) CHAR INIT('second free: missed!');
"#,
        macros: STORAGE_MACROS,
    },
    // ── Storage: OOM ─────────────────────────────────────────────────
    Demo {
        name: "Storage: OOM",
        description: "_PLSW_GETMAIN returns 0 when the request exceeds the heap",
        source: r#"/* storage_oom.plsw -- out-of-memory return.
 *
 * Allocates a block larger than the heap. ?GETMAIN must set
 * RC=4 (the OOM code) and the allocator must remain in a valid
 * state (subsequent in-bounds alloc still succeeds). */

%DEFINE PLSW_HEAP_SIZE 256;
%INCLUDE _plsw_storage;

DCL TOO_BIG INT;
DCL OK_PTR INT;
DCL RC INT;

MAIN: PROC;
    /* 1024 > heap (256). Must set RC=4. */
    ?GETMAIN LENGTH(1024) SET(TOO_BIG) RC(RC);
    IF (RC = 4) THEN
        CALL UART_PUTS(ADDR(MSG_OOM));
    ELSE
        CALL UART_PUTS(ADDR(MSG_NOT_OOM));

    /* Allocator should still work for in-bounds requests after OOM */
    ?GETMAIN LENGTH(100) SET(OK_PTR) RC(RC);
    IF (RC = 0) THEN
        CALL UART_PUTS(ADDR(MSG_OK_AFTER));
    ELSE
        CALL UART_PUTS(ADDR(MSG_BROKEN));
END;

DCL MSG_OOM      (28) CHAR INIT('alloc 1024/256: oom');
DCL MSG_NOT_OOM  (28) CHAR INIT('alloc 1024/256: not oom!');
DCL MSG_OK_AFTER (28) CHAR INIT('alloc 100 after oom: ok');
DCL MSG_BROKEN   (28) CHAR INIT('alloc 100 after oom: fail');
"#,
        macros: STORAGE_MACROS,
    },
    // ── Storage: Size Mismatch ───────────────────────────────────────
    Demo {
        name: "Storage: Size Mismatch",
        description: "_PLSW_FREEMAIN returns RC=2 when LEN doesn't match the alloc size",
        source: r#"/* storage_size_mismatch.plsw -- size-mismatch detection.
 *
 * Allocates 12 bytes; tries to free with LEN=11. Must return
 * RC=2 (size mismatch). The block stays allocated, so a correct
 * free with LEN=12 afterwards must succeed (RC=0). */

%DEFINE PLSW_HEAP_SIZE 256;
%INCLUDE _plsw_storage;

DCL P INT;
DCL RC INT;
DCL RET_BAD INT;
DCL RET_OK INT;

MAIN: PROC;
    ?GETMAIN LENGTH(12) SET(P) RC(RC);

    ?FREEMAIN LENGTH(11) ADDR(P) RC(RET_BAD);
    IF (RET_BAD = 2) THEN
        CALL UART_PUTS(ADDR(MSG_MISMATCH));
    ELSE
        CALL UART_PUTS(ADDR(MSG_NO_MISMATCH));

    /* Block must still be allocated; correct length frees it */
    ?FREEMAIN LENGTH(12) ADDR(P) RC(RET_OK);
    IF (RET_OK = 0) THEN
        CALL UART_PUTS(ADDR(MSG_FREED));
    ELSE
        CALL UART_PUTS(ADDR(MSG_BROKEN));
END;

DCL MSG_MISMATCH    (28) CHAR INIT('free len=11: rc=2');
DCL MSG_NO_MISMATCH (28) CHAR INIT('free len=11: missed!');
DCL MSG_FREED       (28) CHAR INIT('free len=12: ok');
DCL MSG_BROKEN      (28) CHAR INIT('free len=12: broken');
"#,
        macros: STORAGE_MACROS,
    },
    // ── Then Print ──────────────────────────────────────────────────
    Demo {
        name: "Then Print",
        description: "IF/ELSE branch demo with true condition",
        source: r#"/* then_print.plsw -- IF/ELSE demo
 * Prints the THEN branch because 2 > 1 is true. */

DCL THEN_MSG(16) CHAR INIT('then path');
DCL ELSE_MSG(16) CHAR INIT('else path');

MAIN: PROC;
    IF (2 > 1) THEN DO;
        CALL UART_PUTS(ADDR(THEN_MSG));
        CALL UART_PUTCHAR(10);
    END;
    ELSE DO;
        CALL UART_PUTS(ADDR(ELSE_MSG));
        CALL UART_PUTCHAR(10);
    END;
END;
"#,
        macros: &[],
    },
];
