---
title: Adaptuner note names
script: <script defer src="https://cdn.jsdelivr.net/npm/mathjax@4/tex-mml-chtml.js"></script>
---

# What's the problem with notations for just intonation, anyway?

Ask yourself what the difference between, say, $G\sharp$ and $A\flat$ is. For
most modern musicians (especially keyboard players) the answer is probably that
it's a matter of orthography that has something to do with the harmonic
context.

This "harmonic context" is determined by how we imagine chords and melodies as
*stacks of intervals*. For instance, a major chord is a minor third stacked on
top of a major third. In tempered tunings, any difference from that spelling
merely conveys intent: You won't press a different key on the piano, but you
might press it differently. 

In just intonation, on the other hand, we assume a set of "basis" intervals --
octaves, pure fifths, pure major thirds, natural sevenths, and so on -- and
*actually* construct all other intervals as stacks of basis intervals.
Different constructions of the "same" interval (i.e. the same keys on the
piano) will yield different pitches. In a sense, you could say that just
intonation takes the "harmonic context" idea seriously.

This means that a notation for just intonation must distinguish not only
$G\sharp$ and $A\flat$, but even *which specific* $G\sharp$ or $A\flat$ we
mean. It must be able to accommodate all of the (infinitely many) classes of
intervals that can be obtained as stacks of basis intervals. The challenge is
to find a system that does so in a non-confusing, musically useful way.

# Why Ben Johnston's note names?

The adaptuner uses the note names invented by [Ben
Johnston](https://en.wikipedia.org/wiki/Ben_Johnston_(composer)). Compared to
some other notations for just intonation, this system has the drawback that it
is slightly less regular -- it's not what a mathematician would invent.
However, it has a number of very good properties that in my opinion make
up for any initial irritation:

- It is completely unambiguous: There's exactly one way to write every note. In
  particular, there are no enharmonic equivalents.
- It is relatively light on additional accidentals for music that emphasises
  "normal" harmonies. By and large, most pieces of music won't look too strange
  with their pitches explicated by Ben Johnston's notation, and where they do
  look strange, this is mostly warranted by actual harmonic complexities.
- Great music has been written in it.

# Explaining Ben Johnston's note names

## The notes of the C major scale

The $C$ major, $F$ major, and $G$ major triads are defined to be pure major
chords (i.e. they have frequency ratio $4:5:6$). This defines all notes of the
$C$ major scale, and this tuning is known by [many
names](https://en.wikipedia.org/wiki/Ptolemy%27s_intense_diatonic_scale).

We could now calculate the frequency ratios of all notes, but I don't think
they're a useful thing to think about when making music. So, here are a few
facts that I find useful:

- $F-A-C-E-G-B-D$ is an alternating sequence of pure major thirds and pure
  minor thirds.
- $F-C-G-D$ and $A-E-B$ are stacks of pure fifths.
- $D-F$ is **not** a pure minor third.
- $D-A$ is **not** a pure fifth.
- The intervals $C-D$, $F-G$, and $A-B$ are **bigger** than an equally tempered
  whole tone. They have frequency ratio $\frac{9}{8}$, an interval sometimes
  called the *major tone*.
- The intervals $D-E$ and $G-A$ are **smaller** than an equally tempered whole
  tone. They have frequency ratio $\frac{10}{9}$, an interval sometimes called
  a *minor tone*.
- The semitones $E-F$ and $B-C$ are of the same size, which is exactly the
  difference between a pure fourth and a pure major third. This is interval is
  sometimes called a *diatonic semitone* and is **bigger** than an equally
  tempered semitone.
- The diminished fifth $B-F$ is slightly bigger than the augmented fourth
  $F-B$.

## Sharps and flats

Sharps and flats denote the difference between a pure major third and a pure
minor third (i.e. the frequency ratio $\frac{25}{24}$). Again, a few hopefully useful
facts:

- $F-A\flat-C-E\flat-G-B\flat-D$ and $F\sharp-A-C\sharp-E-G\sharp-B-D\sharp$
  are alternating sequences of pure minor and major thirds.
- Notes with a sharp are **lower** than their "enharmonic equivalents" with a
  flat. The difference can be quite startling: the interval $G\sharp-A\flat$ is
  about 40 cents wide, $C\sharp-D\flat$ is about 60 cents!
- The semitones you get by adding an accidental to major tones are **very
  big**. These are intervals like $C\sharp-D$ or $A-B\flat$.
- The semitones you get by by adding an accidental to minor tones are of the
  same size as the $E-F$ semitone, which, as already noted, is **bigger** than
  an equally tempered semitone. These are intervals like $D\sharp-E$ or
  $G-A\flat$.
- Augmented unisons like $C-C\sharp$ are **smaller** than an equally tempered
  semitone. This interval is sometimes called a *chromatic semitone*.

## Plus and minus

The nerd's way to explain the accidentals $+$ and $-$ is to say that they
denote the syntonic comma (i.e. the frequency ratio $\frac{81}{80}$). For the
working musician, this means that we can extend the alternating sequence
$F-A-C-E-G-B-D$ of pure major an minor thirds by using plus and minus signs
like so:

$$
\cdots
G^{-}-B\flat^{-}-D^{-}-
F-A-C-E-G-B-D-
F\sharp^{+}-A^{+}-C\sharp^{+}
\cdots
$$

This allows us to construct many more pitches, which I'll illustrate with a few
examples:

- The fifth of $D$ major is $A^+$. The third of $F$ major is $A$, which is also
  the fifth of $D^-$ major.
- $C-E^+$ is a [pythagorean major
  third](https://en.wikipedia.org/wiki/Pythagorean_tuning).
- $C-D\flat^-$ is a chromatic semitone.
- $D-E^+-F\sharp^+-G-A^+-B-C\sharp^+$ are the notes of the $D$ major scale, if
  it is tuned like the "accidental-free" $C$ major scale.
- A frequently-discussed tuning of all twelve notes of the chromatic scale adds
  to the $C$ major scale the notes $D\flat^-$, $E\flat$, $F\sharp^+$ or
  $G\flat^-$, $A\flat$, and $B\flat^-$. For example, Paul Hindemith constructs
  this scale at the outset of his [*Unterweisung im
  Tonsatz*](https://de.wikipedia.org/wiki/Unterweisung_im_Tonsatz#Neue_Herleitung_der_chromatischen_Tonleiter). 

## Notations for higher harmonics

At the moment, the GUI of the adaptuner only exposes the fragment of just
intonation known as *five-limit* just intonation: All of the intervals you can
get by stacking octaves, pure fifths, and pure major thirds. They can all be
notated with the part of Ben Johnston's system I described so far. Once I
start exposing the higher harmonics, I'll add an explanation of their notation
as well.
