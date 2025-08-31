---
title: Installing Adaptuner on generic Gnu/Linux
---

Currently, there are pre-compiled binaries only for x86_64 processors. If you
have a different processor architecture, please contact me, and I'll see what I
can do. 

1. Download and unpack `adaptuner-bin-x86_64-linux.zip` from the
   [release](https://github.com/carlhammann/adaptuner/releases) of your liking.
2. Navigate to the directory that contains the binary and make it executable:
   `chmod a+x adaptuner`
3. Run the program either from the command line as `./adaptuner` or by
   (double-)clicking on the binary in your file browser. 
4. Optionally, if you have a "normal" file system hierarchy you can install the
   binary by putting it in `/usr/bin` or a similar location. You can also
   install the content of [the "share" directory ](https://github.com/carlhammann/adaptuner/tree/main/share) under
   `/usr/share`, if you want desktop icons.

Please report any problems either as GitHub issues or in an e-Mail to
`adaptuner AT carlhammann DOT com`.
