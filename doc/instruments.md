# Using with MIDI instruments

The *adaptuner* is a MIDI-to-MIDI program. So, in order to hear any sound,
you'll have to use some kind of synthesizer. 

Instead of using the MIDI tuning standard (which would be perfectly suited to
this application, but isn't implemented by most hardware and software
instruments), *adaptuner* uses pitch bend messages. This entails some
assumptions on the settings of the synthesizer:

- The synthesizer "listens" to all 16 channels of its MIDI input.
- The pitch bend range is set to 2 (equally tempered) semitones in both
  directions.

## Using with an e-piano

This is the intended use case: The piano should sound and play "as normal",
while delegating the on-the-fly tuning to *adaptuner*. 

In order for this to work, I make the following assumptions:

- The piano has a MIDI input and a MIDI output.
- The piano sends all events like key and pedal presses and program changes
  through its MIDI output, but plays no sound while doing so. It only plays
  whatever signals arrive at its MIDI input. In many instruments, the relevant
  setting is called "Local Control" (and should be turned off).
- The piano "listens" to all 16 channels of its MIDI input. The relevant
  setting is sometimes called "Multi-Timbral Mode".
- The pitch bend range is set to 2 (equally tempered) semitones in both
  directions. This is a default setting for every instrument I've seen so
  far.
- The piano reacts to input events on different channels in accordance with
  "General MIDI". If you hear keys playing with different instruments
  (especially common: sounds of a drum set on one key in each octave):
  - Try changing the sound program to a different instrument (and back).
  - Under *adaptuner*'s "MIDI connections", choose a different set of 12 output
    channels. 
  - Check the manual for your piano again: Maybe there are different
    "Multi-Timbral" modes or even a "General MIDI" (or "GM") mode.
  If nothing helps, please open an issue.
- The piano uses MIDI "note off" messages: It sends such a message when a key
  is released, and it terminates notes upon receiving such a message (unless
  the note is held by the sustain pedal). Some (old Yamaha?) instruments use
  "note on" messages with the velocity attribute set to zero instead of "note
  off" messages. If that behaviour turns out to be very common, I'll
  accommodate it. So, please open an issue if you're affected.
