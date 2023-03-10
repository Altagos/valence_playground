{ config, lib, pkgs, ... }:

with lib;

let
  sitecfg = config.services.valence_playground;

  baseDir = "/var/minecraft/valence_playground";

  siteEnv = {
    RUST_LOG = "valence_playground=trace,minecraft=trace,warn";
    RUST_LOG_PATH = "/var/minecraft/valence_playground/logs";
  };
in {
  ##### interface. here we define the options that users of our service can specify
  options = {
    # the options for our service will be located under services.foo
    services.valence_playground = { 
      enable = mkOption {
        type = types.bool;
        default = false;
        description = ''
          Whether to enable valence_playground.
        '';
      };
    };
  };

  ##### implementation
  config = mkIf sitecfg.enable { # only apply the following settings if enabled
    users.extraUsers.valence-playground =
      { description = "Altagos's Minecraft server";
        isNormalUser = true;
        group = "altagos";
        useDefaultShell = true;
      };

    environment.variables = siteEnv;

    systemd.services.valence_playground =
      { wantedBy = [ "multi-user.target" ];
        requires = [ "caddy.service" ];
        after = [ "caddy.service" ];
        environment = siteEnv;
        serviceConfig =
          { ExecStart =
              "${baseDir}/bin/valence_playground";
            User = "valence-playground";
            Restart = "never";
            WorkingDirectory = baseDir;
          };
      };
  };
}
