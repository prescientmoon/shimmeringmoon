#!/usr/bin/env bash

if [ "$#" != 1 ]; then
    echo "Usage: $0 <to>"
    exit 1
fi

from=$SHIMMERING_DATA_DIR/db.sqlite
to=$1

echo "Creating destination..."
rm -rf "$to"
mkdir -p "$to"

echo "Exporting info..."
sqlite3 "$from" ".header on" ".mode csv" "select * from songs" \
  >  $to/songs.csv
sqlite3 "$from" ".header on" ".mode csv" "select * from charts" \
  > $to/charts.csv

echo "All done :3"
