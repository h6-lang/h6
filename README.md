# h6
h6 is a minimal stack-based progragramming language.

[language reference](./langref/README.md)

h6 is also a stack-based, minimal, advanced, bytecode runtime & linker.

## building
run `git clone https://github.com/h6-lang/h6` somewhere.

then run `cargo install -F repl --path /path/to/h6`, to install the h6 cli and repl.

## repl
run `h6 repl /path/to/h6/std/*.h6` to start the repl.

Use the arrow-right key to auto-complete `}` to the corresponding `{`.

Use tab for symbol auto-completetion.

To exit, press CTRL+C twice.

## compiler
first, each file has to be compiled wtih `h6 compile a.h6 -o a.h6b`.

Then, all files have to be linked together (even when having only a single file!) with `h6 ld a.h6b b.h6b c.h6b -o o.h6b`.

Finally, it can be executed by doing `h6 run o.h6b`

## links
- [language reference](./langref/)
- [standard library](./std)
- [alternative runtime](./crt) can be embedded easier, but is more unsafe. (runtime ONLY)
