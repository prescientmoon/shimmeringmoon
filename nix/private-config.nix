{
  debundled-void,
  runCommand,
}:
let
  jacketVersion = "6.2.3";
  songlistVersion = "6.2.3.10";
in
runCommand "shimmering-private-config" { } ''
  mkdir $out
  mkdir $out/jackets

  source=${debundled-void}/${jacketVersion}/songs
  for dir in $source/*; do
    out_dir=$(basename $dir)
    out_dir=''${out_dir#dl_}
    if [ -d $dir ] && [ $out_dir != "pack" ]; then
      mkdir $out/jackets/$out_dir

      for file in $dir/*_256.jpg; do
        filename=$(basename $file)
        cp $file $out/jackets/$out_dir/$filename
      done
    fi
  done

  cp ${debundled-void}/${songlistVersion}/songs/songlist $out/songlist.json
''
