CC=~/wasi-sdk-22.0/bin/clang

flags="-nostdlib -target wasm32-wasip1 -Oz -flto -Wl,--allow-undefined -mcpu=mvp -Wall -Wextra"
files="wasi-mini-libc/*.c rt.c main.c"


if [ $DEBUG -ne 0 ]; then
    flags+=" -g"
else
    flags+=" -DNDEBUG"
fi

$CC $flags $files 

if [ $DEBUG -eq 0 ]; then
    llvm-strip a.out
fi

wc -c a.out
