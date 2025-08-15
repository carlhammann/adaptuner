# Obtaining and running the program

## Pre-compiled binaries

Binaries for x86_64 Linux and aarch64 (Apple Silicon) MacOS can be found at the
GitHub ["Releases"](https://github.com/carlhammann/adaptuner/releases). Each
release has two assets, which contain the binaries:
- `adaptuner-macos-latest.zip`, and
- `adaptuner-ubuntu-latest.zip` (which can be used for all x86_64 Linux
  systems; the "ubuntu" in the name only means that it was *built* on ubuntu).

## Running the program

I'll assume that you have unpacked the`.zip` file somewhere convenient. Now,
navigate to the directory that contains the binary and make it executable:
```
chmod a+x adaptuner
```
Then, you can run `adaptuner` either from the command line like so
```
./adaptuner
```
or by (double-)clicking on the binary in your file browser. 

On Linuxes with a "normal" file system hierarchy you can install the binary by
putting it in `/usr/bin` or a similar location.

On MacOS, you'll probably receive a warning the first time you try to run
`adaptuner`. I trust you'll know [the
dance](https://support.apple.com/guide/mac-help/open-an-app-by-overriding-security-settings-mh40617/mac)
to circumvent it.

## Alternative: Use Nix

The flake in this repository has one `package` output, which is the `adaptuner`
program. For example, you can clone this repo, and then do
```
nix run .#
```
to run the very latest commit on `main`.
