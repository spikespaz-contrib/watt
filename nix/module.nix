inputs: {
  config,
  pkgs,
  lib,
  ...
}: let
  inherit (lib.modules) mkIf;
  inherit (lib.options) mkOption mkEnableOption;
  inherit (lib.types) submodule;
  inherit (lib.meta) getExe;

  cfg = config.programs.superfreq;

  defaultPackage = inputs.self.packages.${pkgs.stdenv.system}.default;

  format = pkgs.formats.toml {};
in {
  options.programs.superfreq = {
    enable = mkEnableOption "Automatic CPU speed & power optimizer for Linux";

    settings = mkOption {
      default = {};
      type = submodule {freeformType = format.type;};
      description = "Configuration for Superfreq.";
    };
  };

  config = mkIf cfg.enable {
    environment.systemPackages = [defaultPackage];

    systemd = {
      packages = [defaultPackage];
      services.superfreq = {
        wantedBy = ["multi-user.target"];
        serviceConfig = let
          cfgFile = format.generate "superfreq-config.toml" cfg.settings;
        in {
          Environment = ["SUPERFREQ_CONFIG=${cfgFile}"];
          WorkingDirectory = "";
          ExecStart = "${getExe defaultPackage} daemon --verbose";
          Restart = "on-failure";

          RuntimeDirectory = "superfreq";
          RuntimeDirectoryMode = "0755";
        };
      };
    };

    assertions = [
      {
        assertion = !config.services.power-profiles-daemon.enable;
        message = ''
          You have set services.power-profiles-daemon.enable = true;
          which conflicts with Superfreq.
        '';
      }
      {
        assertion = !config.programs.auto-cpufreq.enable;
        message = ''
          You have set programs.auto-cpufreq.enable = true;
          which conflicts with Superfreq.
        '';
      }
    ];
  };
}
