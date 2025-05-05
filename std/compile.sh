set -e

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

rm -rf $SCRIPT_DIR/build
mkdir -p "$SCRIPT_DIR/build"

for f in "$SCRIPT_DIR"/*.h6; do
    h6 compile "$f" -o "$SCRIPT_DIR/build/$(basename "$f").h6b"
done

h6 ld --allow-unresolved $SCRIPT_DIR/build/*.h6b -o $SCRIPT_DIR/build/_out.h6b
echo $SCRIPT_DIR/build/_out.h6b
