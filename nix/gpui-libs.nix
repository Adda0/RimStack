# The system libraries gpui-fast builds against on Linux, and those it opens at run time.
pkgs:
let
  linux = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux;
in
{
  build = linux (
    with pkgs;
    [
      fontconfig
      freetype
      libxkbcommon
      wayland
      libxcb
      libx11
      libxext
      libGL
    ]
  );
  run = linux (
    with pkgs;
    [
      libGL
      vulkan-loader
      wayland
    ]
  );
}
