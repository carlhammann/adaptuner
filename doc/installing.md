# Obtaining and running the program

## Pre-compiled binaries

Binaries for x86_64 Linux, aarch64 (Apple Silicon) MacOS, and Windows can be
found at the GitHub
["Releases"](https://github.com/carlhammann/adaptuner/releases). Please report
any problems running these programs either as GitHub issues or in an e-Mail to
`adaptuner AT carlhammann DOT com`.

## Debian-Based Linux distributions

Download and unpack the `adaptuner-deb-x86_64-linux` asset from the release of
your liking. Then install as normal with `sudo dpkg -i ...`.

## Other Linuxes and MacOS

I'll assume that you have downloaded and unpacked the relevant `.zip` file
somewhere convenient. Now, navigate to the directory that contains the binary
and make it executable:
```
chmod a+x adaptuner
```
Then, you can run `adaptuner` either from the command line like so
```
./adaptuner
```
or by (double-)clicking on the binary in your file browser. 

On Linuxes with a "normal" file system hierarchy you can install the binary by
putting it in `/usr/bin` or a similar location. You can also install the
content of [the "share" directory ](./share) under `/usr/share`, if you want
desktop icons.

On MacOS, you'll probably receive a warning the first time you try to run
`adaptuner`. I trust you'll know [the
dance](https://support.apple.com/guide/mac-help/open-an-app-by-overriding-security-settings-mh40617/mac)
to circumvent it.

## Windows (not tested on actual hardware)

Download and unpack the `adaptuner-x86_64-w64-mingw32` asset. It contains an
`.exe` that you should _hopefully_ be able to run. 

## Alternative: Use Nix

For example, you can clone this repo, and then do
```
nix run .#adaptuner-wayland
```
to run the very latest commit on `main` as a Wayland app.
