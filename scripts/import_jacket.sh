#!/usr/bin/env bash

if [ "$#" != 2 ]; then
    echo "Usage: $0 <name> <url>"
    exit 1
fi

name=$1
url=$2

curr=$(pwd)

dir_path=$SHIMMERING_DATA_DIR/songs/$name
mkdir $dir_path
cd $dir_path

http GET "$url" > temp
convert ./temp ./base.jpg
convert ./base.jpg -resize 256x256 ./base_256.jpg
rm temp

cd $curr
