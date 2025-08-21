# Adaptive MIDI tuner

This is the *adaptuner*: It *tunes* MIDI instruments on the fly, *adapt*ing to
what you're playing right now. [Here](https://adaptuner.carlhammann.com) is its
homepage.

The adaptuner is currently under heavy development. However, it is already
reasonably usable as an instrument.

## User documentation 

- [Here](./doc/installing.md) is how you obtain and run the program. Currently,
  pre-compiled binaries for x86_64 Linux, aarch64 (Apple Silicon) MacOS, and
  Windows (not yet tested using actual hardware) are available.
- [Here](./doc/instruments.md) is how you set up instrument(s) to play with
  *adaptuner*. This is needed because *adaptuner* is a MIDI-to-MIDI program; it
  doesn't make any sound on its own.
- [Here](./configs/) are a few example configuration files that showcase
  different uses of *adaptuner*. You can load and save configuration files from
  inside the program.
