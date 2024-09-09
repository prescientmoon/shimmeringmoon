#!/usr/bin/env bash

if [ "$#" != 2 ]; then
    echo "Usage: $0 <from> <to>"
    echo "This script copies the chart/song data from a db to another. Useful for creating new dbs for testing."
    exit 1
fi

a="$1/db.sqlite"
b="$2/db.sqlite"

sqlite3 $b "DROP TABLE songs"
sqlite3 $b "DROP TABLE charts"
sqlite3 $a ".dump songs"  | sqlite3 $b
sqlite3 $a ".dump charts" | sqlite3 $b
