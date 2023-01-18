{
  hcMkShell,
  lib,
  pkgs,
  crate2nix,
  rustc,
  cargo,
  coreScripts,
}:
hcMkShell {
  buildInputs =
    (builtins.attrValues (coreScripts))
    ++ (with pkgs;[
      cargo-nextest
      cargo-sweep
      gdb
      gh
      nixpkgs-fmt
      rustup
      sqlcipher
    ])
    # the latest crate2nix is currently broken on darwin
    ++ (lib.optionals pkgs.stdenv.isLinux [
      crate2nix
    ])
    ++ (lib.optionals pkgs.stdenv.isDarwin
      (with pkgs; [
        libiconv
        darwin.Security
        darwin.IOKit
        darwin.apple_sdk_11_0.frameworks.CoreFoundation
      ])
    )
    ;
}
