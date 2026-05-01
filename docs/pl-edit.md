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

The `?` button opens the available trigger list for the active editor. Macro
editors include both macro boilerplate triggers and normal PL/SW source
triggers, so control structures such as `IF` expand inside `.msw` files too.

## Formatting

The `Format` button formats the whole current editor buffer. In `.msw` editors
it uses the current textarea contents when the file is open, so unsaved edits
are formatted in place before being written back to the app state.

The formatter is intentionally simple and line based. It normalizes common
spacing outside quoted generated strings, including extra spaces before
semicolons, commas, closing parentheses, and call/procedure parameter lists.
It follows normal PL/I-style block indentation for:

- `PROC ... END`
- `IF ... THEN DO; ... END; ELSE DO; ... END;`
- `DO WHILE` and counted `DO`
- `SELECT`, `WHEN`, and `OTHERWISE`
- `ASM DO` and `GEN DO`
- `MACRODEF`
- Multi-line `DCL` records, with continuation indentation based on level
  numbers such as `1`, `3`, and `5`

For character arrays, PL/SW uses the dimension on the identifier, for example
`DCL MSG(20) CHAR;` and `3 BAT(8) CHAR;`. Format fixes accidental
`CHAR(8)` field declarations in DCL lines to that form.

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
| `REC` | Level DCL record with 1/3/5 entries |
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

Macro editors also accept all source triggers listed above.

Macro invocation arguments use PL/SW keyword-call syntax:
`?NAME(KEYWORD(value));`. For example, a macro with `REQUIRED BAR(expr);`
is invoked as `?FOO(BAR(0));`, not `?FOO(BAR=0);`.

| Trigger | Expansion |
|---------|-----------|
| `MD` | MACRODEF block |
| `REQ` | REQUIRED parameter |
| `OPT` | OPTIONAL parameter |
| `GEN` | GEN DO block |
| `INC` | %INCLUDE directive |
| `INV` | Macro invocation |
