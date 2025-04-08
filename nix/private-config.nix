{
  debundled-void,
  runCommand,
}:
runCommand "shimmering-private-config" { } ''
  mkdir -p $out/jackets

  for version in $(ls ${debundled-void} | sort -V); do
    source=${debundled-void}/$version/songs

    for dir in $source/*; do
      out_dir=$(basename $dir)
      out_dir=''${out_dir#dl_}
      if [ -d $dir ] && [ $out_dir != "pack" ]; then
        mkdir -p $out/jackets/$out_dir

        for file in $dir/*_256.jpg; do
          jacket_path=$out/jackets/$out_dir/$(basename $file)
          rm -rf $jacket_path
          cp $file $jacket_path
        done
      fi
    done

    if [ -f $source/songlist ]; then
      rm -rf $out/songlist.json
      cp $source/songlist $out/songlist.json
    fi
  done
''
