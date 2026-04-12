{
  description = "Unofficial command line client for Bitwarden";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          inherit (pkgs) lib stdenv;
        in
        let
          mkRbw =
            {
              withFzf ? false,
              withRofi ? false,
              withPass ? false,
            }:
            pkgs.rustPlatform.buildRustPackage rec {
              pname = "rbw";
              version = "1.16.0";

              src = pkgs.fetchzip {
                url = "https://github.com/cmlsharp/rbw/archive/refs/tags/${version}.tar.gz";
                hash = "sha256-Kr5MGRDJ2ozcl7DW1/RXXie71ZEh53P6wVxt9hVU81s=";
              };

              cargoHash = "sha256-X8hXlTqqJeXmqlHktmwzsjLiTBaWMgW66EqKR7yOX9U=";

              nativeBuildInputs = [
                pkgs.installShellFiles
              ]
              ++ lib.optionals stdenv.hostPlatform.isLinux [ pkgs.pkg-config ];

              buildInputs = [ pkgs.bash ];

              preConfigure = lib.optionalString stdenv.hostPlatform.isLinux ''
                export OPENSSL_INCLUDE_DIR="${pkgs.openssl.dev}/include"
                export OPENSSL_LIB_DIR="${lib.getLib pkgs.openssl}/lib"
              '';

              postInstall = ''
                install -Dm755 -t $out/bin bin/git-credential-rbw
              ''
              + lib.optionalString (stdenv.buildPlatform.canExecute stdenv.hostPlatform) ''
                installShellCompletion --cmd rbw \
                  --bash <($out/bin/rbw gen-completions bash) \
                  --fish <($out/bin/rbw gen-completions fish) \
                  --nushell <($out/bin/rbw gen-completions nushell) \
                  --zsh <($out/bin/rbw gen-completions zsh)
              ''
              + lib.optionalString withFzf ''
                install -Dm755 -t $out/bin bin/rbw-fzf
                substituteInPlace $out/bin/rbw-fzf \
                  --replace fzf ${pkgs.fzf}/bin/fzf \
                  --replace perl ${pkgs.perl}/bin/perl
              ''
              + lib.optionalString withRofi ''
                install -Dm755 -t $out/bin bin/rbw-rofi
                substituteInPlace $out/bin/rbw-rofi \
                  --replace rofi ${pkgs.rofi}/bin/rofi \
                  --replace xclip ${pkgs.xclip}/bin/xclip
              ''
              + lib.optionalString withPass ''
                install -Dm755 -t $out/bin bin/pass-import
                substituteInPlace $out/bin/pass-import \
                  --replace pass ${pkgs.pass}/bin/pass
              '';

              meta = {
                description = "Unofficial command line client for Bitwarden (fork)";
                license = lib.licenses.mit;
                mainProgram = "rbw";
              };
            };
        in
        {
          rbw = mkRbw { };
          default = self.packages.${system}.rbw;
        }
      );

      overlays.default = final: prev: {
        rbw = self.packages.${final.system}.rbw;
      };
    };
}
