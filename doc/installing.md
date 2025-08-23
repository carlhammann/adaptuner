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
content of [the "share" directory ](../share/) under `/usr/share`, if you want
desktop icons.

On MacOS, you'll probably receive a warning the first time you try to run
`adaptuner`. This is because the binaries aren't signed. These steps should
work:

1. Download the binary and make it executable as described above.
2. The first time you start the program, a dialog will be shown telling you
   that the software isn't signed and will therefore not be run. 
3. Go to your system settings, and allow running the program [as described
   here](https://support.apple.com/guide/mac-help/open-an-app-by-overriding-security-settings-mh40617/mac).
4. Now, when you start the program again, a dialog containing the same message
   will appear, but you'll have the option to start the program anyway. 
5. On subsequent runs, the program will start normally.

## Windows

Download and unpack the `adaptuner-x86_64-w64-mingw32` asset. It contains an
`.exe` that you should _hopefully_ be able to run. However, Microsoft Defender
Antivirus makes our life difficult, because the binaries aren't signed. On
Windows 11, these steps seem to work:

1. Turn off "real time protection" in the [Windows Security
   App](https://support.microsoft.com/en-us/windows/virus-and-threat-protection-in-the-windows-security-app-1362f4cd-d71a-b52a-0b66-c2820032b65e).
   If you don't do this, you'll likely already be blocked from downloading or
   saving the `.zip` file.
2. Download the `.zip` file and unpack it.
3. Run the contained `.exe`.

## Alternative: Use Nix

For example, you can clone this repo, and then do
```
nix run .#adaptuner-wayland
```
to run the very latest commit on `main` as a Wayland app.
