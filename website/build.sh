#!/usr/bin/env bash

# relative path to the current directory, but must not start with ./
OUTDIR=build

rm -rf "$OUTDIR"
mkdir "$OUTDIR"

# find . -mindepth 1 -not \( -path ./"$OUTDIR" -prune \) -type d -exec sh -c ' \
#     echo $2
#     ' find-sh "$OUTDIR" {} \;
#
# echo ""
#
# find . -mindepth 1 -not \( -path ./"$OUTDIR" -prune \) -type f -name '*.md' -exec sh -c ' \
#     echo $2
#     ' find-sh "$OUTDIR" {} \;


find . -mindepth 1 -not \( -path ./"$OUTDIR" -prune \) -type d -exec sh -c ' \
    outdir="$1"
    inpath="$2"
    mkdir -p "$outdir/$inpath"
    ' find-sh "$OUTDIR" {} \;

cp favicon.png "$OUTDIR"
cp -r assets/* "$OUTDIR"/assets/

find . -mindepth 1 -not \( -path ./"$OUTDIR" -prune \) -type f -name '*.md' -exec sh -c ' \
    outdir="$1"
    inpath="$2"
    template="${inpath%/*}/template.html"
    set -x
    pandoc --mathjax \
	   --toc \
	   --shift-heading-level-by=1 \
	   --template "$template" \
	   --output "$outdir/${inpath%.md}.html" \
	   --lua-filter=links-to-html.lua \
	   "$inpath"
    ' find-sh "$OUTDIR" {} \;

