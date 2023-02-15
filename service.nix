{ config, lib, pkgs, ... }:

with lib;  # use the functions from lib, such as mkIf

let
  # the values of the options set for the service by the user of the service
  sitecfg = config.services.valence_playground;

  baseDir = "/var/minecraft/valence_playground";

  siteEnv = {
    RUST_LOG = "valence_playground=trace,minecraft=trace";
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
      { description = "Altagos web server";
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
              "RUST_LOG=valence_playground=trace,minecraft=trace,warn ${baseDir}/bin/valence_playground";
            User = "valence-playground";
            PermissionsStartOnly = true;
            Restart = "on-failure";
            WorkingDirectory = baseDir;
          };
      };
  };
}
