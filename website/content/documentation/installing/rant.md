---
title: A rant about code signing
---

Apple and Microsoft require binaries to be signed, in the name of security.
Superficially, there is some merit to this idea: If a binary is
cryptographically signed by its creator, users can be more certain it hasn't
been tampered with.

My first problem with code signing is that it would cost me real money to buy
the certificates -- Apple and Microsoft want me to pay them so that they don't
annoy or scare you if you want to run my code on your computer. They are not
content with the money you gave them, nor happy about the fact that I'm writing
software that enriches their platform. They want my money as well. In their
world, it's my (and everyone's) job to consume, not create.

(In the case of Apple, the barrier comes even before code signing, as it's
essentially impossible to create a binary that will run on a Mac without using
a Mac. I don't own a Mac, and I'm only able to create MacOS binaries using the
compute that GitHub provides to public repositories.)

(GitHub is of course a can of worms in it's own right: It's the world's biggest
source code host. If your project isn't on GitHub, it might as well not exist.
Microsoft acquired GitHub in 2018, which means that the world's largest
repository of open-source and free software is owned by a company whose stance
on software freedom has been hostile from its very inception. If that's not
["Embrace, extend,
extinguish"](https://en.wikipedia.org/wiki/Embrace-extend-extinguish), I don't
know what is.)

Another problem with code signing is that in the idea of "cryptographic
signatures on binaries to prevent tampering", there's *no need at all* for
Apple or Microsoft to play any role, apart maybe from providing convenient OS
facilities to check signatures. If you know and trust me, the fewer
intermediaries come between me signing the software and you checking the
signature, the better. If you don't know and trust me, my signature will not be
of any value to you, and a rubber stamp (even an expensive one) from Apple or
Microsoft shouldn't change that.

Many free software projects have code signing based on some form "reputation".
This is a good idea in principle, because such a system might help you trust my
signature even if you don't know and trust me personally. But for Apple and
Microsoft, what reputation really boils down to is (a) I give them money, and
(b) I have proof that I'm a legitimate company. (Because, of course, there has
to be a *business*, right? And where there's a business, there's
accountability, right?) These requirements ensure that you're being scammed by
people in suits -- either illegally by criminals who are organised enough to
incorporate and buy a certificate, or legally by Apple and Microsoft.

This brings me to the next problem. As often, the advertised "protection"
really applies not to users, but to corporate interest. Sure, some malware will
get caught by code signing. But Apple and Microsoft are pushing for code
signing, and making themselves central to this process, because it allows them
to keep a thumb on *all* software: Imagine the potential losses a big software
vendor will make if Windows or MacOS suddenly decides to show a scary warning
about malware to any user trying to start their program, and imagine the amount
of money they will be willing to pay for this not to happen.

Code signing can afford a degree of added security, but Apple and Microsoft are
not primarily using it to build a *better* product; they're using it to ensure
theirs is *the only* product. They're using it to extort programmers by holding
users hostage in walled-in software ecosystems ("If you want access to *our*
users, pay!"), while still leeching off their creativity. They're using it to
poison the general public's understanding of security, insisting that their
(incompetent) users need "protection", instead of giving them tools to control
what software they want to run on their computers.

We're not powerless against all of this. We can educate each other -- as I'm
doing right now -- and we can *stop using their products*. Thankfully, when it
comes to software, there *are* real alternatives, which are at least as usable
and at least as secure as what Apple and Microsoft are forcing down our
throats. They are also completely free (they cost no money, but are also "free"
as in freedom). This is different from, say, the transition processes that are
needed to soften the blow of climate chaos. We ordinary people really have
power here.
