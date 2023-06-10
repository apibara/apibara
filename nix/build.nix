{ pkgs, crane, rustToolchain, workspaceDir }:
let
  # script needed to install tests from cargo build log
  installTestsFromCargoBuildLogHook = pkgs.makeSetupHook
    { name = "installTestsFromCargoBuildLogHook"; } ./installTestsFromCargoBuildLogHook.sh;
  craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;
in
rec {
  inherit (craneLib) cargoFmt cargoClippy buildPackage;

  src = pkgs.lib.cleanSourceWith {
    src = craneLib.path workspaceDir; # all sources
    filter = path: type:
      (builtins.match ".*proto$" path != null) # include protobufs
      || (builtins.match ".*js$" path != null) # include js (for deno runtime)
      || (craneLib.filterCargoSources path type); # include rust/cargo
  };

  buildArgs = {
    nativeBuildInputs = with pkgs; [
      clang
      pkg-config
      protobuf
    ] ++ pkgs.lib.optional stdenv.isDarwin (with pkgs.darwin.apple_sdk.frameworks; [
      CoreFoundation
      CoreServices
      Security
    ]);

    buildInputs = with pkgs; [
      rustToolchain
      librusty_v8
    ];

    RUSTY_V8_ARCHIVE = "${pkgs.librusty_v8}/lib/librusty_v8.a";
    LIBCLANG_PATH = pkgs.lib.makeLibraryPath [ pkgs.llvmPackages.libclang.lib ];
  };

  commonArgs = (buildArgs // {
    inherit src;
  });

  cargoArtifacts = craneLib.buildDepsOnly (commonArgs // {
    pname = "apibara";
    version = "0.0.0";
  });

  buildCrate = { crate }:
    let
      manifest = builtins.fromTOML (builtins.readFile (crate + "/Cargo.toml"));
      pname = manifest.package.name;
      version = manifest.package.version;
      bin = craneLib.buildPackage (commonArgs // {
        inherit pname version cargoArtifacts;

        cargoExtraArgs = "--package ${pname}";
        doCheck = false;
      });
    in
    {
      inherit pname version bin;
    };

  buildCrateTests = args:
    craneLib.buildPackage (args // {
      doCheck = false;
      cargoExtraArgs = "--tests";

      nativeBuildInputs = [
        installTestsFromCargoBuildLogHook
      ] ++ (commonArgs.nativeBuildInputs or [ ]);

      # extract the test binary from the build log
      installPhaseCommand = ''
        if [ -n "$cargoBuildLog" -a -f "$cargoBuildLog" ]; then
          installTestsFromCargoBuildLog "$out" "$cargoBuildLog"
        else
          echo ${pkgs.lib.strings.escapeShellArg ''
            !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
            $cargoBuildLog is either undefined or does not point to a valid file location!
            By default `buildPackage` will capture cargo's output and use it to determine which binaries
            should be installed (instead of just guessing based on what is present in cargo's target directory).
            If you are overriding the derivation with a custom build step, you have two options:
            1. override `installPhaseCommand` with the appropriate installation steps
            2. ensure that cargo's build log is captured in a file and point $cargoBuildLog at it
            At a minimum, the latter option can be achieved with running:
                cargoBuildLog=$(mktemp cargoBuildLogXXXX.json)
                cargo build --release --message-format json-render-diagnostics >"$cargoBuildLog"
          ''}

          false
        fi
      '';
    });

  dockerizeCrateBin = { crate, volumes ? null, ports ? null }:
    pkgs.dockerTools.buildImage {
      name = crate.pname;
      # we're publishing images, so make it less confusing
      tag = "latest";
      created = "now";
      copyToRoot = with pkgs.dockerTools; [
        usrBinEnv
        binSh
        caCertificates
      ];
      config = {
        Entrypoint = [
          "${crate.bin}/bin/${crate.pname}"
        ];
        Volumes = volumes;
        ExposedPorts = ports;
      };
    };
}
