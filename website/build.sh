#!/usr/bin/env bash

OUTDIR=build

rm -rf build
mkdir build

cp favicon.png build
cp -r assets build

find content -mindepth 1 -type d -exec sh -c ' \
    outdir="$1"
    inpath="$2"
    relpath=${inpath#"content/"}
    mkdir -p "$outdir/$relpath"
    ' find-sh build {} \;

find content -type f -name '*.md' -exec sh -c ' \
    outdir="$1"
    inpath="$2"
    relpath=${inpath#"content/"}
    template="${inpath%/*}/template.html"
    set -x
    pandoc --mathjax \
	   --toc \
	   --shift-heading-level-by=1 \
	   --template "$template" \
	   --output "$outdir/${relpath%.md}.html" \
	   --lua-filter=links-to-html.lua \
	   "$inpath"
    ' find-sh build {} \;

