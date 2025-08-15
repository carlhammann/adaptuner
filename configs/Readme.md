# Example *adaptuner* configurations

- [template.yaml](./template.yaml) is the standard configuration from which
  *adaptuner* always starts. It showcases everything the program can do, but it
  will very likely not be useful for every piece of music. It is intended to be
  an illustrative starting point for your own configurations.
- [cembalo_cromatico.yaml](./cembalo_cromatico.yaml) is a "better cembalo
  cromatico" that allows you to play with reference notes in 1/4-comma
  meantone, but just harmonic intervals.
  - It knows the "correct" (just) tunings of most relevant chords for the period.
  - Sostenuto pedal toggles between two meantone scales for the reference notes: one with the chromatic notes, one with the enharmonic notes.
  - Soft pedal resets the reference of the scales to the reference of the currently sounding chord.

