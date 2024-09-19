#!/usr/bin/env nix-shell
#!nix-shell -p libsixel
#!nix-shell -i bash

if [ "$#" != 2 ]; then
    echo "Usage: $0 <name> <url>"
    exit 1
fi

name=$1
url=$2

curr=$(pwd)

dir_path=$SHIMMERING_ASSET_DIR/songs/raw/$name
mkdir -p $dir_path
cd $dir_path

http GET "$url" > temp
convert ./temp ./base.jpg
convert ./base.jpg -resize 256x256 ./base_256.jpg
rm temp
img2sixel ./base.jpg

cd $curr
