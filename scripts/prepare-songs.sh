#!/usr/bin/env bash
dir_path=$SHIMMERING_ASSETS_DIR/songs/raw

# Find all files in the directory and its subdirectories
find "$dir_path" -type f | while read -r file; do
    # Get the filename without the directory path
    filename=$(basename "$file")

    # Check if the filename starts with "1080_"
    if [[ $filename == 1080_* ]]; then
        # Remove the "1080_" prefix
        new_filename="${filename#1080_}"

        # Get the directory path without the filename
        file_dir=$(dirname "$file")

        # Construct the new file path
        new_file_path="$file_dir/$new_filename"

        # Rename the file
        mv "$file" "$new_file_path"
        echo "Renamed: $file -> $new_file_path"
    fi
done

mv $dir_path/dropdead/3*.jpg $dir_path/overdead 2>/dev/null
mv $dir_path/singularity/3*.jpg $dir_path/singularityvvvip 2>/dev/null
mv $dir_path/redandblue/3*.jpg $dir_path/redandblueandgreen 2>/dev/null
mv $dir_path/ignotus/3*.jpg $dir_path/ignotusafterburn 2>/dev/null
rm -rf $dir_path/ifirmxrmx 2>/dev/null
