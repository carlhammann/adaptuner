# Obtaining and running the program

## Pre-compiled binaries

You download binaries for x86_64 Linux and aarch64 MacOS on GitHub by
navigating to the ["Actions"
tab](https://github.com/carlhammann/adaptuner/actions), clicking on the most
recent workflow run that has a green tick mark, and scrolling down until you
see the heading "Artifacts".

Each workflow run creates two artifacts, which contain the binaries:
- `adaptuner-macos-latest.zip`, and 
- `adaptuner-ubuntu-latest.zip` (which can be used for all x86_64 Linux
  systems; the "ubuntu" in the name only means that it was *built* on ubuntu).

## Configuration file
 
You'll also need a configuration file for the *adaptuner*. This file contains a
user-definable list of the chords and voicings the program recognizes and knows
how to tune. I propose you download [lorem.hjson](../configs/lorem.hjson)
(there's a download link at the file) and use that initially. Later, you can
familiarise yourself with its contents (and the explanation of the format
[here](./configuration-file.md)), and edit it to your needs.

## Running the program

You'll have to run *adaptuner* from the command line.

I'll assume that you have unpacked the pre-compiled binary and saved the
configuration file in a directory structure that looks like this:

```
.
├── bin
│   └── adaptuner
└── lorem.hjson
```

Now, 
1. Start your e-piano and/or synthesizer and connect it to the computer.
2. Navigate to the directory whose contents are shown above and
   run `adaptuner` like so:
  ```
  ./bin/adaptuner lorem.hjson
  ```
3. You'll be greeted with something like 
  ```
  Available input ports:
  0: Midi Through:Midi Through Port-0 14:0
    ...
    ...
  Please select input port: 
  ```
  Enter the number of the port that connects to your e-piano or input keyboard
  and press return. This will repeat for the output port (which connects to the
  synthesizer or back to the e-piano). 
4. Now, the *adaptuner* is connected and ready to play, and you'll see the user
   interface described [here](./tui.md). 

## Alternative: Using Nix

Clone this repo, and then run
```
nix run .# ./configs/lorem.hjson
```
