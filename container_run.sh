cd /forcefield/
for file in ./*; do
    if [ -f "$file" ] && [ -x "$file" ]; then
        RUST_BACKTRACE=1 "$file"
        break
    fi
done