//! Embedded PL/SW demo programs for the source editor.

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

pub const DEMOS: &[Demo] = &[
    Demo {
        name: "Hello World",
        description: "Print a greeting via UART",
        source: r#"/* Hello World -- PL/SW demo */

HELLO: PROC;
  DCL 1 MSG CHAR(14) INIT('Hello, PL/SW!');
  DCL 1 I INT;
  DCL 1 UART_TX PTR INIT(0xFF0102);

  DO I = 0 TO 13;
    UART_TX->BYTE = MSG(I);
  END;
  RETURN;
END HELLO;
"#,
        macros: &[],
    },
    Demo {
        name: "Blink LED",
        description: "Toggle LED bits in a loop",
        source: r#"/* Blink LED -- toggle LED register */

BLINK: PROC;
  DCL 1 LED PTR INIT(0xFF0000);
  DCL 1 DELAY INT;
  DCL 1 VAL BYTE INIT(0x55);

  DO WHILE(1);
    LED->BYTE = VAL;

    /* Busy-wait delay */
    DO DELAY = 0 TO 10000;
    END;

    VAL = VAL ^ 0xFF;  /* toggle all bits */
  END;
END BLINK;
"#,
        macros: &[],
    },
    Demo {
        name: "Arithmetic",
        description: "Compute 7 * 6 and print ASCII result",
        source: r#"/* Arithmetic -- compute 7 * 6 = 42 */

MAIN: PROC;
  DCL 1 A INT INIT(7);
  DCL 1 B INT INIT(6);
  DCL 1 RESULT INT;
  DCL 1 UART_TX PTR INIT(0xFF0102);

  RESULT = A * B;

  /* Print ASCII '*' (42) */
  UART_TX->BYTE = RESULT;
  RETURN;
END MAIN;
"#,
        macros: &[],
    },
    Demo {
        name: "Inline Assembly",
        description: "Mix PL/SW with COR24 assembly",
        source: r#"/* Inline assembly demo */

ASMTEST: PROC;
  DCL 1 X INT INIT(10);
  DCL 1 Y INT;

  /* Use inline assembly to double X */
  ASM DO;
    ld  r0, [fp-4]    ; load X
    add r0, r0, r0    ; double it
    st  r0, [fp-8]    ; store to Y
  END;

  /* Y now holds 20 */
  CALL ?PUTINT(Y);
  RETURN;
END ASMTEST;
"#,
        macros: &[],
    },
    Demo {
        name: "Procedures",
        description: "Procedures with params, returns, and options",
        source: r#"/* Procedures -- parameters, returns, and options */

SQUARE: PROC(X INT(24)) RETURNS(INT(24));
  RETURN(X * X);
END SQUARE;

ABS: PROC(X INT(24)) RETURNS(INT(24));
  IF X < 0 THEN RETURN(-X);
  RETURN(X);
END ABS;

MAIN: PROC OPTIONS(FREESTANDING);
  DCL 1 A INT(24) INIT(7);
  DCL 1 B INT(24);
  DCL 1 UART_TX PTR INIT(0xFF0102);

  B = CALL SQUARE(A);   /* B = 49 */

  UART_TX->BYTE = B;
  RETURN;
END MAIN;
"#,
        macros: &[],
    },
    Demo {
        name: "Control Flow",
        description: "IF/ELSE, DO WHILE, DO counted loops",
        source: r#"/* Control flow demo */

MAIN: PROC OPTIONS(FREESTANDING);
  DCL 1 UART_TX PTR INIT(0xFF0102);
  DCL 1 N INT(24) INIT(10);
  DCL 1 SUM INT(24) INIT(0);
  DCL 1 I INT(24);

  /* Counted loop: sum 1..N */
  DO I = 1 TO N;
    SUM = SUM + I;
  END;

  /* IF/ELSE: check result */
  IF SUM = 55 THEN DO;
    UART_TX->BYTE = 'Y';  /* correct */
  END;
  ELSE DO;
    UART_TX->BYTE = 'N';  /* wrong */
  END;

  /* DO WHILE countdown */
  DO WHILE (N > 0);
    UART_TX->BYTE = N + 48;  /* ASCII digit */
    N = N - 1;
  END;

  RETURN;
END MAIN;
"#,
        macros: &[],
    },
    Demo {
        name: "Records & Pointers",
        description: "Record types and pointer access",
        source: r#"/* Records and pointers */

MAIN: PROC OPTIONS(FREESTANDING);
  DCL 1 POINT,
      2 X INT(24),
      2 Y INT(24);
  DCL 1 UART PTR INIT(0xFF0100);

  POINT.X = 10;
  POINT.Y = 20;

  /* Pointer field access */
  UART->BYTE = POINT.X + POINT.Y;

  RETURN;
END MAIN;
"#,
        macros: &[],
    },
    Demo {
        name: "Typed Declarations",
        description: "INT, BYTE, CHAR, PTR, and arrays",
        source: r#"/* Typed declarations demo */

MAIN: PROC OPTIONS(FREESTANDING);
  DCL 1 COUNT INT(24) INIT(0);
  DCL 1 FLAGS BYTE INIT(0xAA);
  DCL 1 LETTER CHAR INIT('Z');
  DCL 1 BUFFER(16) BYTE;
  DCL 1 GPIO PTR INIT(0xFF0000);

  /* Byte-width operations */
  FLAGS = FLAGS ^ 0xFF;
  GPIO->BYTE = FLAGS;

  /* Array fill */
  DCL 1 I INT(24);
  DO I = 0 TO 15;
    BUFFER(I) = I + 65;  /* 'A'..'P' */
  END;

  RETURN;
END MAIN;
"#,
        macros: &[],
    },
    Demo {
        name: "Macro Usage",
        description: "Demonstrate ?MACRO invocation",
        source: r#"/* Macro usage demo */
%INCLUDE UART;

/* Invoke a macro to set up UART output */
?UART_INIT(PORT=0xFF0100);

MAIN: PROC;
  DCL 1 CH CHAR;

  CH = 'A';
  ?UART_PUTC(CH);

  CH = 'B';
  ?UART_PUTC(CH);

  RETURN;
END MAIN;
"#,
        macros: &[DemoMacro {
            name: "UART.msw",
            source: r#"/* UART.msw -- UART I/O macros for COR24 */

MACRODEF UART_INIT;
  REQUIRED PORT(expr);
  GEN DO;
    "lc r0, {PORT}";
    "st r0, [0xFF0100]";
  END;
END;

MACRODEF UART_PUTC;
  REQUIRED CH(expr);
  GEN DO;
    "ld r0, {CH}";
    "st r0, [0xFF0102]";
  END;
END;
"#,
        }],
    },
];
