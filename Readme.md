# Adaptive MIDI tuner

This is the *adaptuner*: It *tunes* MIDI instruments on the fly, *adapt*ing to
what you're playing right now. Its aim is to give anyone with an e-piano that
has MIDI input and output the opportunity to play music in (controlled
deviations from) just intonation.

The *adaptuner* is currently under heavy development. The version on the main
branch is an older proof of concept that now mostly serves as an illustration
of the general direction I want this project to take. It is already reasonably
usable as an instrument, though.

## User documentation 

- [Here](./doc/installing.md) is how you obtain and run the program. Currently,
  pre-compiled binaries for x86_64 Linux and aarch64 (Apple Silicon) MacOS are
  available.
- [Here](./doc/instruments.md) is how you set up instrument(s) to play with the
  *adaptuner*. This is needed because the *adaptuner* is a MIDI-to-MIDI
  program; you'll need some sound source.
- [Here](./doc/tui.md) is an explanation of the terminal user interface.
- [Here](./doc/configuration-file.md) is an explanation of the format of the
  configuration file, which contains the chords and voicings the *adaptuner*
  "knows" how to tune.

