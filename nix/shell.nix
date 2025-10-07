{ mkShell, myriam, git, tor }:

mkShell {
  inputsFrom = [ myriam ];

  packages = [
    git
    tor
  ];
}
