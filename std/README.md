# h6-std
the source code contains documentation.

## linking against the stdlib
running the `compile.sh` script, compiles and links together all stdlib files, and echos the path of the output file.

it can be used like this:
`h6 compile myfile.h6 -o myfile.h6b && h6 ld myfile.h6b $(h6-std/compile.sh) -o out.h6b`
