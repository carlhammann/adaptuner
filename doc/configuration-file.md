# Configuration file

A usable example configuration file is in
[lorem.hjson](../configs/lorem.hjson). The configuration file's format will
soon(ish) be replaced with something both more comprehensive and more
user-friendly. However, the main ideas will stay unchanged.

The purpose of the configuration file is to provide a list of chords together
with their tunings. Hence:
- You'll see that the whole file is a big list between the square brackets `[`
  and `]`.
- Each entry in this list is delimited by curly braces `{` and `}`, and
  describes one chord-tuning.
- The order of entries matters: The first pattern that fits will be chosen. It
  might be that no pattern fits all currently sounding MIDI keys. Then, the
  *adaptuner* fits the currently sounding MIDI keys "from the bottom up", such
  that  the pattern that fits the most of the low notes is chosen. If not even
  such a partial fit can be found, the reference notes are used. 

## Anatomy of one entry

Every entry looks like this:

```hjson
{
  "name": ...,
  "keyshape": ...,
  "neighbourhood": ...
}
```

The `name` attribute will describe the type of chord or voicing, and will be
shown in the user interface. The `keyshape` attribute specifies which MIDI keys
have to be sounding in order for the pattern to trigger. The `neighbourhood`
attribute specifies how to tune the notes played by the currently sounding
keys. It specifies tunings for notes relative to the reference note of the
chord (or: "in the *neighbourhood* of the reference note").

## Example 1: All major chords

Consider this entry:

```hjson
{
  "name": "major",
  "keyshape": { "ClassesRelative": { "period_keys": 12, "classes": [ 0, 4, 7 ] } },
  "neighbourhood": [
      [ 0, [ 0, 0, 0 ] ],
      [ 4, [ 0, 0, 1 ] ],
      [ 7, [ 0, 1, 0 ] ] 
  ]
}
```

We want to capture "all major chords", so we need a `keyshape` pattern that
matches all transpositions and all voicings of a major chord. This is
accomplished with the `ClassesRelative` constructor, which specifies a chord by
its pitch classes modulo 12 (the `period_keys` attribute) relative to some
reference. 

The simplest major chord in root position consists of three notes at key
offsets 0, 4, and 7. (Count it on your piano!) Another way to think about these
numbers is that all notes in any major chord will have key offsets from the
fundamental which are equal, modulo 12, to one of these three numbers. This is
what the `classes` attribute specifies.

Now that the `keyshape` captures all major chords, we have to specify how the
notes should be tuned. This happens though the `neighbourhood`. You can see
that it holds a list that associates key offsets (here, the numbers 0, 4, 7
from the `keyshape`'s `classes`) with triples of numbers. These triples
describe how many octaves, perfect fifths, and perfect major thirds above the
reference the notes corresponding to these key offsets should be tuned to.
Thus, the `neighbourhood` specifies that
- The note at offset 0 should be tuned zero octaves, fifths, and thirds above
  the reference.
- The note at offset 4 should be tuned one major third above the reference 
- The note at offset 7 should be tuned one major fifth above the reference 

The adaptuner is clever enough to infer the correct tunings for octave-shifted
notes from these data.

Finally, note that the `neighbourhood` should at least specify tunings for all
pitch classes used in the `keyshape` (in this example: for 0, 4, and 7). After
all, why would you match on a chord and then not tune its notes?

You may, however, specify tunings for *more* pitch classes than the `keyshape`
mentions. This is useful because the *adaptuner* matches chords "from the
bottom up", as mentioned above: If you know that you will play a major chord in
the base, with some spicy notes above, and if you do not want to include
entries for the chord(s) thus obtained, you can specify tunings for the higher
notes here as well.

## Example 2: What if the voicing matters?

Entries using `ClassesRelative` are very useful, but sometimes they match too
many chords, because they disregard the chord's structure by only looking at
the pitch classes relative to the reference.

As an example, consider the "so what" chord E-A-D-G-B. This chord is
interesting because it "cannot be in tune": If all fourths E-A-D-G and the
major third G-B are justly tuned, the composite interval E-B will not be a just
twelfth: it will be one syntonic comma too small. In this particular voicing,
the big "detuned" interval E-B might still be a good compromise, because
detuning the stack of fourths or the major third might be more disturbing.

However, if we order the notes differently, say as E-B-G-D-A, that assessment
might change. If we regard this voicing as an E minor 7 add11 chord, an in-tune
fifth E-B is essential.

Relative to the reference E, the pitch classes modulo 12 in the "so what"
voicing E-A-D-G-B are 0-5-10-3-7. Using

```hjson 
"keyshape": { "ClassesRelative": { "period_keys": 12, "classes": [ 0, 5, 10, 3, 7] } },
```

would yield an entry that matches both the "so what" and the "minor 7 add11"
voicings, so we wouldn't know which tunings to specify. This is where the
`VoicingRelative` constructor can be used. It allows to specify, from the
lowest note to the highest, the order in which pitch classes are allowed to
occur in the voicing. This is accomplished using sub-lists like so:

```hjson
"keyshape": { "VoicingRelative": { "period_keys": 12, "blocks": [ [0], [5, 10, 3], [7] ] } },
```

In our running example, this `keyshape` will match any voicing that has a
surrounding interval E-B, with the other notes G, D, and A somewhere in
between. This ensures that E-B is at least a twelfth (because the D has to be
higher than the E and lower than the B). A complete entry for the "so what"
voicing might thus look like this:

```hjson
{
  "name": "so what",
  "keyshape": { "VoicingRelative": { "period_keys": 12, "blocks": [ [0], [5, 10, 3], [7] ] } },
  "neighbourhood": [
      [ 0, [ 0, 0, 0 ] ],
      [ 5, [ 1, -1, 0 ] ],   # <-- one fourth above the reference
      [ 10, [ 2, -2, 0 ] ],  # <-- two fourths above the reference
      [ 3, [ 2, -3, 0 ] ],   # <-- three fourths (minus one octave) above the reference
      [ 7, [ 2, -3, 1 ] ]    # <-- three fourths plus one third (minus one octave) above the reference
  ]
}
```

An entry for the "minor 7 add11" voicing might look like this: 

```hjson
{
  "name": "minor 7 add11",
  "keyshape": { "VoicingRelative": { "period_keys": 12, "blocks": [ [0, 3, 7], [10, 5] ] } },
  "neighbourhood": [
      [ 0, [ 0, 0, 0 ] ],
      [ 3, [ 0, 1, -1 ] ],   # <-- minor third above the reference
      [ 7, [ 0, 1, 0 ] ]     # <-- fifth above the reference
      [ 5, [ 1, -1, 0 ] ],   # <-- one fourth above the reference
      [ 10, [ 0, 2, -1 ] ],  # <-- minor third plus fifth above the reference
  ]
}
```

