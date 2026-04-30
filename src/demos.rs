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
    ?GREET(MSG(_MSG));
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
        description: "Print A-J with DO counted loop",
        source: r#"/* loop.plsw -- Counted Loop Demo for PL/SW
 * Prints letters A through J using DO I = 0 TO 9.
 * Demonstrates: DO count, UART_PUTCHAR, arithmetic. */

DCL MSG(16) CHAR INIT('Loop complete');

MAIN: PROC;
    DCL I INT;

    /* Print A through J (ASCII 65-74) */
    DO I = 0 TO 9;
        CALL UART_PUTCHAR(65 + I);
    END;
    CALL UART_PUTCHAR(10);

    CALL UART_PUTS(ADDR(MSG));
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
    ?LED_SET(VAL(0));

    /* NOP macro -- just loads a constant */
    ?NOP(COUNT(99));

    /* LED off via macro */
    ?LED_SET(VAL(1));
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
 * - project_lib.msw: helper PROC
 * - project_macros.msw: MACRODEF skeleton
 */

%INCLUDE project_data;
%INCLUDE project_lib;
%INCLUDE project_macros;

MAIN: PROC;
    DCL SUM INT(24);

    SUM = ADD2(2, 3);

    CALL UART_PUTS(ADDR(APP_MSG));
    CALL UART_PUTCHAR(SUM + 48);
    CALL UART_PUTCHAR(10);

    ?EMIT_NOP(COUNT(3));
END;
"#,
        macros: &[
            DemoMacro {
                name: "project_data.msw",
                source: r#"/* project_data.msw -- shared declarations */

DCL APP_MSG(16) CHAR INIT('sum = ');
"#,
            },
            DemoMacro {
                name: "project_lib.msw",
                source: r#"/* project_lib.msw -- callable helper library */

ADD2: PROC(A INT(24), B INT(24)) RETURNS(INT(24));
    RETURN(A + B);
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
        source: r#"/* record.plsw -- Record and Pointer Demo
 * Demonstrates: level-based DCL, record field access,
 * ADDR(), pointer dereference (P->field). */

DCL LBL_SUM(8) CHAR INIT('Sum: ');
DCL LBL_X(8) CHAR INIT('X: ');
DCL LBL_Y(8) CHAR INIT('Y: ');

MAIN: PROC;
    DCL 1 POINT,
        3 X INT,
        3 Y INT;
    DCL P PTR;
    DCL SUM INT;

    /* Fill record fields */
    POINT.X = 3;
    POINT.Y = 7;

    /* Access via pointer */
    P = ADDR(POINT);
    SUM = P->X + P->Y;

    /* Print values as ASCII digits */
    CALL UART_PUTS(ADDR(LBL_X));
    CALL UART_PUTCHAR(P->X + 48);
    CALL UART_PUTCHAR(10);

    CALL UART_PUTS(ADDR(LBL_Y));
    CALL UART_PUTCHAR(P->Y + 48);
    CALL UART_PUTCHAR(10);

    CALL UART_PUTS(ADDR(LBL_SUM));
    CALL UART_PUTCHAR(SUM + 48);
    CALL UART_PUTCHAR(10);
END;
"#,
        macros: &[],
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
