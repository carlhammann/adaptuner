---
title: Installing Adaptuner on MacOS
---

Currently, there are pre-compiled binaries only for aarch64 (Apple Silicon)
Macs. If you have a different processor architecture, please contact me, and
I'll see what I can do. 

1. Download and unpack `adaptuner-bin-aarch64-darwin.zip` from the
   [release](https://github.com/carlhammann/adaptuner/releases) of your liking.
2. Navigate to the directory that contains the binary and make it executable:
   `chmod a+x adaptuner`
3. Run the program either from the command line as `./adaptuner` or by
   (double-)clicking on the binary in your file browser.

In principle, we're finished, but Apple has to be annoying about code signing.
(You can read my [rant here](./rant.md).)

4. The first time you start the program, a dialog will be shown telling you
   that the software isn't signed and will therefore not be run. 
5. Go to your system settings, and allow running the program [as described
   here](https://support.apple.com/guide/mac-help/open-an-app-by-overriding-security-settings-mh40617/mac).
6. Now, when you start the program again, a dialog containing the same message
   will appear, but you'll have the option to start the program anyway. 
7. On subsequent runs, the program will start normally.

Please report any problems either as GitHub issues or in an e-Mail to
`adaptuner AT carlhammann DOT com`.
