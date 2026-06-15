cd /forcefield/
for file in ./*; do
    if [ -f "$file" ] && [ -x "$file" ]; then
        "$file"
        break
    fi
done