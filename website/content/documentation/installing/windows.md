---
title: Installing Adaptuner on Windows
---

In principle: "Download and unpack `.zip` containing the version you like. Run
the contained `.exe`". However, Microsoft has to be annoying about code
signing. (You can read my [rant here](./rant.md).)

On Windows 11, these steps seem to work:

1. Turn off "real time protection" in the [Windows Security
   App](https://support.microsoft.com/en-us/windows/virus-and-threat-protection-in-the-windows-security-app-1362f4cd-d71a-b52a-0b66-c2820032b65e).
   If you don't do this, you'll likely already be blocked from downloading or
   saving the `.zip` file.
2. Download and unpack `adaptuner-deb-x86_64-linux.zip` from the
   [release](https://github.com/carlhammann/adaptuner/releases) of your liking.
3. Run the contained `.exe`.
4. Hope that Microsoft Defender Antivirus doesn't randomly decide to delete the
   program.

Please report any problems either as GitHub issues or in an e-Mail to
`adaptuner AT carlhammann DOT com`.
