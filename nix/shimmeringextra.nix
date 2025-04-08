# Collection combining every file stored in another repo
{
  shimmeringdarkness,
  shimmeringvoid,
  symlinkJoin,
}:
symlinkJoin {
  name = "shimmeringextra";
  paths = [
    shimmeringvoid
    shimmeringdarkness
  ];
}
