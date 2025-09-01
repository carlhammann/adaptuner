---
title: Temperaments and Commas
script: <script defer src="https://cdn.jsdelivr.net/npm/mathjax@4/tex-mml-chtml.js"></script>
---

The adaptuner's internal logic allows very general tuning systems. In
particular, there's no requirement that tuning systems be based on any form of
just intonation, nor that there be 12 pitch classes per octave, nor for octave
periodicity (or any periodicity). However, the graphical user interface (for
now) only exposes [five-limit just
intonation](https://en.wikipedia.org/wiki/Five-limit_tuning), so that's what
I'll use as the framework in the following explanation.

Some parts of the explanation are a bit mathematical in flavour. They are
intended to provide some background for those who are interested. Phrases like
"more formally..." mark these paragraphs. You won't have to understand (or even
read) them order to use the adaptuner.

# Pure Intervals

Adaptuner internally represents pure intervals as stacks of basis intervals on
middle $C$, i.e. $C\ 4$. Concretely, assuming the three basis intervals octave,
just fifth, and just major third, every note corresponds to a vector of three
integers: 

| note | vector |
| :---: | :---: |
| $C\ 4$ | $(0,0,0)$ |
| $C\ 5$ | $(1,0,0)$ |
| $G\ 4$ | $(0,1,0)$ |
| $E\ 4$ | $(0,0,1)$ |
| $C\ 3$ | $(-1,0,0)$ |
| $A^{+}\ 4$ | $(-1,3,0)$ |
| $A\ 4$ | $(1,-1,1)$ |
| ... | ... |

This table uses the note names explained [here](./notenames.md). The graphical
user interface of the adaptuner normally only shows the fifths and thirds, to
reduce clutter. Octaves are shown only when you're actually playing something,
when you'll see them as horizontal "antennas".

More formally, every note is an *integer linear combination* of basis
intervals, and the three numbers in the vectors above are the coefficients in
this linear combination. If $o$, $f$, and $t$ denote, respectively, the
(logarithmic) sizes of octaves, fifths, and thirds in equally tempered
semitones, we can read the entries of the table above like 

$$
\text{``pitch of \(A^{+}\ 4\) in equally tempered semitones''}
= \text{``pitch of middle \(C\) in equally tempered semitones''}-o+3f\ .
$$

Implicitly, adaptuner assumes $o$, $f$, and $t$ to be linearly independent over
the integers, to ensure that every pitch has exactly one representation. This
is true in the case of five-limit just intonation: In equally tempered semitones, we have

$$ o=12\ , $$
$$ f=12\cdot\log_2\left(\frac{3}{2}\right)\ , $$
$$ t=12\cdot\log_2\left(\frac{5}{4}\right)\ . $$

# Tempered intervals

If we imagine the pure intervals as forming a grid (like the one shown by the
adaptuner's main graphical user interface), temperaments can be understood as a
slight warping of that grid.

# Commas
