# calc

A Rust CLI to evaluate arithmetic expressions

## Why this exists

Doing arithmetic in a shell usually means `bc`, `awk`, or opening a REPL, each with its own syntax and output quirks. `calc` takes one expression, evaluates it with ordinary operator precedence, prints a clean number, and optionally emits JSON. It is a single small binary with no runtime dependencies, so it works the same in an interactive shell, a script, or a CI job.

## How it works

The expression is lexed and parsed directly to a floating-point result, with no AST or intermediate allocation. Precedence follows standard rules: unary `+` and `-`, then `^` (right-associative), then `*`, `/`, and `%`, then `+` and `-`, with parentheses to override. Function calls take comma-separated arguments and constants are identifiers.

Results use IEEE-754 double precision throughout. Output is rounded to 12 decimal places by default, trailing zeros are removed, and very large or very small magnitudes switch to scientific notation, so `0.1 + 0.2` prints `0.3` instead of `0.30000000000000004`. Pass `--precision` to change the number of decimal places.

Non-finite results follow IEEE-754 too: division by zero yields `inf`, and `sqrt(-1)` yields `nan`. With `--json`, non-finite values are emitted as `null` while `display` still shows `inf` or `nan`.

## Usage

```text
calc [OPTIONS] <EXPRESSION>...
calc [OPTIONS] -
... | calc [OPTIONS]

Arguments:
  <EXPRESSION>  Arithmetic expression; multiple arguments are joined with spaces

Options:
  -j, --json            Print a JSON object instead of plain text
  -p, --precision <N>   Decimal places for output, 0-15 [default: 12]
  -h, --help            Print help
  -V, --version         Print version
```

Operators are `+ - * / % ^ ( )` and functions are `sqrt cbrt abs ln log log2 exp sin cos tan asin acos atan floor ceil round trunc sign pow atan2 min max`. Constants are `pi`, `e`, and `tau`. Numbers accept decimal and scientific notation and `_` separators, as in `1_000` or `1e-3`.

Examples:

```bash
calc "2 + 3 * 4"
calc -p 4 "1/3"
calc "sqrt(2) + min(3, 4)"
echo "2^10" | calc --json
```

`--json` prints one object per run. A successful result has `expression`, `value`, and `display`; a failed evaluation has `expression` and `error`.

## Notes

All positional arguments are joined with spaces before evaluation, so `calc 2 + 3` and `calc "2 + 3"` are equivalent. Use `-` or pipe input to read the expression from stdin; newlines are treated as whitespace. Running `calc` with no arguments prints help, unless stdin is piped, in which case the expression is read from it.

Exit codes: 0 success, 2 invalid expression or usage error. Errors go to stderr and results to stdout, so pipelines stay clean. Help output is unstyled, with no color or bold text.

`calc` does not keep history, config, or state. Every run is independent and non-interactive.

## Install

**Requirements:** Rust 1.88+ and `git`.

```bash
cargo install --git https://github.com/batusaribay/calc.git --locked
```

`cargo install` places the binary in `~/.cargo/bin`, which the Rust installer puts on `PATH`.

## License

This project is licensed under the [MIT License](LICENSE).
