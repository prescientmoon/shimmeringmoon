{
  debundled-void,
  runCommand,
}:
runCommand "shimmering-private-config" { } ''
  mkdir $out
  mkdir $out/jackets

  for version in $(ls ${debundled-void} | sort -V); do
    source=${debundled-void}/$version/songs

    for dir in $source/*; do
      out_dir=$(basename $dir)
      out_dir=''${out_dir#dl_}
      if [ -d $dir ] && [ $out_dir != "pack" ]; then
        jacket_dir=$out/jackets/$out_dir
        rm -rf $jacket_dir
        mkdir $jacket_dir

        for file in $dir/*_256.jpg; do
          filename=$(basename $file)
          cp $file $out/jackets/$out_dir/$filename
        done
      fi
    done

    if [ -f $source/songlist ]; then
      rm -rf $out/songlist.json
      cp $source/songlist $out/songlist.json
    fi
  done
''
