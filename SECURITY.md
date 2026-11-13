# Security

Please report security problems privately, not in a public issue: use
GitHub's **Report a vulnerability** button on this repository's Security
tab.

Things that count as security problems in JoyrIDE include:

- a GitHub token (JoyrIDE should never hold one: `gh` does) or a keystore
  password shown, logged or written to a file (passwords belong in the OS
  keychain);
- the toolchain updater installing anything that fails its signature check;
- a crafted project, `love.toml` or dependency that makes JoyrIDE read or
  write outside the project and its own app-data folder, or run code
  other than the project's own game when you press Run;
- a crafted LÖVE error message that runs code through the console links.

You'll hear back within a week. Fixes go in the next release, with credit
if you'd like it.
