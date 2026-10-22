{
  description = "Polygon nesting Node 22 and Rust 1.95 development shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
        rustToolchain = pkgs.rust-bin.stable."1.95.0".default.override {
          extensions = [
            "clippy"
            "rust-analyzer"
            "rust-src"
            "rustfmt"
          ];
        };
        # use the official security release while retaining the existing Nixpkgs lock
        nodeVersion = "22.23.4";
        nodeAssets = {
          x86_64-linux = {
            file = "linux-x64.tar.xz";
            hash = "sha256-GAW8015oFVWb0uuX20ONP9sOs6XmqXe3wbrcp2M36c4=";
          };
          aarch64-linux = {
            file = "linux-arm64.tar.xz";
            hash = "sha256-YoeqRrbDddgW5jrmbQJPu/1wQEglqr39+JipcIcdL0s=";
          };
          x86_64-darwin = {
            file = "darwin-x64.tar.xz";
            hash = "sha256-9Cq8p6qsYe/FfUn7QuqE/FO766mnScsDopHObk56acE=";
          };
          aarch64-darwin = {
            file = "darwin-arm64.tar.xz";
            hash = "sha256-A3ZQ0IOfRXdHOW8FIHP7SDM9Mb9avTyYt02iDYnrsa0=";
          };
        };
        nodeAsset = nodeAssets.${system};
        nodejs = pkgs.stdenv.mkDerivation {
          pname = "nodejs";
          version = nodeVersion;
          src = pkgs.fetchurl {
            url = "https://nodejs.org/dist/v${nodeVersion}/node-v${nodeVersion}-${nodeAsset.file}";
            hash = nodeAsset.hash;
          };
          nativeBuildInputs = pkgs.lib.optionals pkgs.stdenv.isLinux [ pkgs.autoPatchelfHook ];
          buildInputs = pkgs.lib.optionals pkgs.stdenv.isLinux [ pkgs.stdenv.cc.cc.lib ];
          passthru.python = pkgs.python3;
          meta.platforms = builtins.attrNames nodeAssets;
          dontConfigure = true;
          dontBuild = true;
          dontStrip = true;
          installPhase = ''
            runHook preInstall
            mkdir -p "$out"
            cp -R . "$out"
            runHook postInstall
          '';
        };
        # pin the native pnpm release beside the patched Node runtime
        pnpmVersion = "12.14.0";
        pnpmAssets = {
          x86_64-linux = {
            file = "pnpm-linux-x64.tar.gz";
            hash = "sha256-ORDDhSE2vtW8FRG5E3pF/UNX7Ws5cNnQiJNkHpAFhn4=";
          };
          aarch64-linux = {
            file = "pnpm-linux-arm64.tar.gz";
            hash = "sha256-z8eAueaQVJ5mM+EEX7H7bvhOOQcR4gmebMuukcMkVuo=";
          };
          x86_64-darwin = {
            file = "pnpm-darwin-x64.tar.gz";
            hash = "sha256-LEEuBzggSbTtsE+m2njz1O94ELdYU8gFRNz7fA7XmIA=";
          };
          aarch64-darwin = {
            file = "pnpm-darwin-arm64.tar.gz";
            hash = "sha256-UPSODhdQiVJ5Cj+/WPzxTzuC76X1lMoErZBOKar4VLA=";
          };
        };
        pnpmAsset = pnpmAssets.${system};
        pnpm = pkgs.stdenv.mkDerivation {
          pname = "pnpm";
          version = pnpmVersion;
          src = pkgs.fetchurl {
            url = "https://github.com/pnpm/pnpm/releases/download/v${pnpmVersion}/${pnpmAsset.file}";
            hash = pnpmAsset.hash;
          };
          nativeBuildInputs = pkgs.lib.optionals pkgs.stdenv.isLinux [ pkgs.autoPatchelfHook ];
          buildInputs = pkgs.lib.optionals pkgs.stdenv.isLinux [ pkgs.stdenv.cc.cc.lib ];
          # the archive has the pnpm binary next to dist/. Nix otherwise
          # treats the only directory, dist/, as the source root and the
          # binary is left behind. dist/ is the bundled node-gyp. pnpm looks
          # for it beside the executable, and beside a symlink's target.
          sourceRoot = ".";
          dontConfigure = true;
          dontBuild = true;
          installPhase = ''
            runHook preInstall
            mkdir -p "$out/lib/pnpm" "$out/bin"
            cp -r pnpm dist "$out/lib/pnpm/"
            ln -s "$out/lib/pnpm/dist" "$out/bin/dist"
            chmod 755 "$out/lib/pnpm/pnpm"
            ln "$out/lib/pnpm/pnpm" "$out/bin/pnpm"
            ln "$out/lib/pnpm/pnpm" "$out/bin/pn"
            ln "$out/lib/pnpm/pnpm" "$out/bin/pnx"
            ln "$out/lib/pnpm/pnpm" "$out/bin/pnpx"
            runHook postInstall
          '';
        };
      in
      {
        formatter = pkgs.nixfmt;

        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            git
            nixfmt
            nodejs
            pnpm
            rustToolchain
          ];
        };
      }
    );
}
