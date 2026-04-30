# PL/EDIT

PL/EDIT is an optional hotkey-driven editing mode for PL/SW source and `.msw`
macro include files. It is meant to reduce syntax typing and keep common PL/SW
forms easy to remember.

## Starting PL/EDIT

1. Press the `PL/EDIT` button in a source or macro editor header.
2. Type a template trigger, such as `IF`.
3. Press `F4` to expand the trigger. `Ctrl+Space` also expands.
4. Use `Tab` or `Enter` to advance through fill fields.
5. Use `Shift+Tab` to move to the previous field.
6. Use `Ctrl+Enter` to insert a newline while editing block content.

The `?` button opens the available trigger list for the active editor.

## Formatting

The `Format` button re-indents the current editor content. It is intentionally
simple and line based. It follows normal PL/I-style block indentation for:

- `PROC ... END`
- `IF ... THEN DO; ... END; ELSE DO; ... END;`
- `DO WHILE` and counted `DO`
- `SELECT`, `WHEN`, and `OTHERWISE`
- `ASM DO` and `GEN DO`
- `MACRODEF`
- Multi-line `DCL` records

Use Format after expanding templates or moving nested blocks around.

## Source Triggers

| Trigger | Expansion |
|---------|-----------|
| `IF` | IF/ELSE block |
| `IFS` | Single-statement IF |
| `DW` | DO WHILE block |
| `DO` | Counted DO block |
| `SEL` | SELECT/WHEN dispatch |
| `WHEN` | WHEN branch |
| `DCL` | Scalar declaration |
| `REC` | Level DCL record |
| `BASED` | BASED record declaration |
| `P` | PROC block |
| `PR` | PROC with RETURNS |
| `NAK` | OPTIONS(NAKED) PROC |
| `ASM` | ASM DO block |
| `CALL` | CALL statement |
| `RET` | RETURN expression |
| `RETV` | Void RETURN |
| `G` | GOTO statement |

## Macro Triggers

| Trigger | Expansion |
|---------|-----------|
| `MD` | MACRODEF block |
| `REQ` | REQUIRED parameter |
| `OPT` | OPTIONAL parameter |
| `GEN` | GEN DO block |
| `INC` | %INCLUDE directive |
| `INV` | Macro invocation |
