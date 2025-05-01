# h6
h6 is a minimal stack-based progragramming language.

[language reference](https://github.com/h6-lang/h6-langref)

h6 is also a stack-based, minimal, advanced, bytecode runtime & linker.

## building
run `cargo install -F repl --git https://github.com/h6-lang/h6`, to install the h6 cli and repl.

after that, you have to download to standard library from https://github.com/h6-lang/h6-std (it doesn't matter where you donwload it to)

## repl
run `h6 repl /path/to/std/*` to start the repl.

Use the arrow-right key to auto-complete `}` to the corresponding `{`.

Use tab for symbol auto-completetion.

To exit, press CTRL+C twice.

## compiler
first, each file has to be compiled wtih `h6 compile a.h6 -o a.h6b`.

Then, all files have to be linked together (even when having only a single file!) with `h6 ld a.h6b b.h6b c.h6b -o o.h6b`.

Finally, it can be executed by doing `h6 run o.h6b`

## links
- [language reference](https://github.com/h6-lang/h6-langref)
- [standard library](https://github.com/h6-lang/h6-std)
- [alternative runtime](https://github.com/h6-lang/h6-crt) can be embedded easier, but is more unsafe. (runtime ONLY)
