# General

- forbid indirectly looping MIDI connections
- handle the other two pedals correctly
  - treat ChannelVoice Messages in the backend, tweak PedalHold...?
- I just learned that Rust logical operators have short-circuiting variants. Use them!
- verify the invariant of chord list entries when deserializing: every bound
  offset mut have an associated tuning
- "exact" chord matches are anchored on the enharmonically equivalent note in
  the current neighbourhood, but they should stay put. (Should they?)

# UI 

- coordination of zoom and scroll
- flattening the grid layout to increase space for keyboard
- multi-touch keyboard
