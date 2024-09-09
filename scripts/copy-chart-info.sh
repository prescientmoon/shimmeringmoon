#!/usr/bin/env bash

if [ "$#" != 2 ]; then
    echo "Usage: $0 <from> <to>"
    echo "This script copies the chart/song data from a db to another. Useful for creating new dbs for testing."
    exit 1
fi

from="$1/db.sqlite"
to  ="$2/db.sqlite"

sqlite3 $from ".dump songs"  | sqlite3 $to
sqlite3 $from ".dump charts" | sqlite3 $to
