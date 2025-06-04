#!/bin/bash

# Output file
OUTPUT_FILE="all_src_dump.txt"
echo "🧾 Dumping .rs and .sol source files into $OUTPUT_FILE"
echo "" > "$OUTPUT_FILE"

# Function to dump a file with header
dump_file() {
    local filepath="$1"
    echo "==================== $filepath ====================" >> "$OUTPUT_FILE"
    cat "$filepath" >> "$OUTPUT_FILE"
    echo -e "\n\n" >> "$OUTPUT_FILE"
}

# Traverse and dump .rs and .sol files only
for ext in rs sol; do
    find src -type f -name "*.${ext}" | sort | while read file; do
        dump_file "$file"
    done
done

echo "✅ Done. Code dumped to $OUTPUT_FILE (excluding .hex files)"
