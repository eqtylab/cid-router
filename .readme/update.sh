#!/usr/bin/env bash

# README.md update script
#
# originally copied from @tjbrunk's script in eqtylab/integrity-monorepo

README=$1

root_dir=$(git rev-parse --show-toplevel)
declare -A top_level_dirs
declare output

# Array of directories to ignore
ignore=("node_modules" "tmp" "openapi-client" "pkg")

# Function to check if a value is in an array
contains() {
    local value="$1"
    shift
    for item; do
        [[ $item == "$value" ]] && return 0
    done
    return 1
}

# Function to extract the #Overview section from a README.md file
get_overview() {
    perl -0777 -ne 'print "$1\n" while /# Overview\n(.*?)(?=\n#|$)/sg' "$1" | perl -pe 's/^\s+|\s+$//g'
}

# Function to update sections of a README.md file managed by `present`
present_in_place() {
    local dir="$1"
    local readme="$dir/README.md"

    if [[ "$README" == "tmp/README.md" ]]; then
        local readme_dir="$root_dir/tmp/readmes/$dir"
        mkdir -p "$readme_dir"
        cp $readme "$readme_dir/"
        readme="$readme_dir/README.md"
    fi

    cd $dir
    present --in-place "$readme"
    cd -

    if [[ "$README" == "tmp/README.md" ]]; then
        diff "$readme" "$dir/README.md" || exit 1
    fi
}

# Function to recursively search directories for README.md files
search_directories() {
    local depth="$1"
    local dir="$2"

    # If we've reached the maximum depth, stop recursing
    if (( depth > 5 )); then
        return
    fi

    # If a README.md file exists in this directory, append the dir name and the #Overview section
    if [[ -f "$dir/README.md" && "$dir" != "$root_dir" ]]; then
        present_in_place "$dir"

        overview=$(get_overview "$dir/README.md")
        relative_dir="${dir#$root_dir}"
        final_dir=$(basename "$relative_dir")
        IFS='/' read -ra ADDR <<< "$relative_dir"
        path=""
        indent=""
        # handle indentation for sub directories
        for i in "${ADDR[@]}"; do
            path+="/$i"
            trimmed_path="${path#/}"
            if [[ -n "$trimmed_path" ]]; then
                if [[ -z "${top_level_dirs[$trimmed_path]}" ]]; then
                    top_level_dirs["$trimmed_path"]="$trimmed_path"
                    if [[ "$final_dir" == "$i" ]]; then
                        # echo "|$indent[$i]($trimmed_path)|$overview |  "
                        output+="|$indent[$i]($trimmed_path)|$overview |\n"
                    else
                        # echo "|$indent[$i]($trimmed_path)| |  "
                        output+="|$indent[$i]($trimmed_path)| |\n"
                    fi
                fi
                indent="${indent}&emsp;"
            fi
        done
    fi

    # Recursively search subdirectories
    for subdir in "$dir"/*; do
        if [[ -d "$subdir" ]]; then
            contains "$(basename "$subdir")" "${ignore[@]}"
            if [[ $? -ne 0 ]]; then
                search_directories $((depth + 1)) "$subdir"
            fi
        fi
    done
    # echo -n $output
}

# start markdown table
output+="|||\n|-|-|\n"
search_directories 1 "$root_dir"
output+="\n"

# Replace the text between # Repo Organization and the next # in the README.md file with the output
perl -i -p0e "s/(# Repo Organization\n).*?(\n#)/\1$(perl -e "print quotemeta qq($output)") \2/s" $README
