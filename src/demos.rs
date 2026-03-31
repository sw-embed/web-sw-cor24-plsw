//! Embedded PL/SW demo programs for the source editor.

/// A demo program with a name, description, and PL/SW source.
#[derive(Clone, PartialEq)]
pub struct Demo {
    pub name: &'static str,
    pub description: &'static str,
    pub source: &'static str,
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
    },
    Demo {
        name: "Macro Usage",
        description: "Demonstrate ?MACRO invocation",
        source: r#"/* Macro usage demo */

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
    },
];
