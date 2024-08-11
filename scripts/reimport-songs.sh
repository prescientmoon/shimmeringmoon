#!/usr/bin/env bash
echo "delete from songs"  | sqlite3 $SHIMMERING_DATA_DIR/db.sqlite
echo "delete from charts" | sqlite3 $SHIMMERING_DATA_DIR/db.sqlite
./scripts/main.py import charts
