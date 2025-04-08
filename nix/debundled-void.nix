{
  shimmeringextra,
  glass-bundler,
  runCommand,
}:
runCommand "debundled-void" { } ''
  mkdir $out

  for file in ${shimmeringextra}/bundles/*.cb; do
    no_ext="''${file%.cb}"
    meta_file="$no_ext.json"
    ${glass-bundler}/bin/glass-bundler debundle \
      --input $file -m $meta_file \
      --output $out/$(basename $no_ext)
  done
''
