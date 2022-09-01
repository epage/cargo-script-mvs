- Feature Name: Rust-script extension for Cargo
- Start Date: 7-29-2022
- RFC PR: TBD
- Rust Issue: TBD

# Summary
[summary]: #summary

The rust-script plugin for Cargo is expected to be a start point for expediting productivity in newly interested developers.  The idea is to remove some of the startup project complexity in favor of a small amount of predictable rigidity to allow idiomatic project creation. There are several additional areas where this tool may also be useful. These will be further highlighted in this RFC.

# Motivation
[motivation]: #motivation

Why are we doing this? 

To continue the adoption of the Rust programming language to an expanded audience of developers.  Additionally, it is the hope that simplicity can be attained for certain subsets of development activities.

What is the expected outcome?

Introduction to and productivity in Rust for the newcomer and novice, this can help them gain confidence and indoctrinate in the Rust environment. It is also expected to expedite writing small tools that may fit into the cargo xtasks genre of activities.

# Guide-level explanation
[guide-level-explanation]: #guide-level-explanation

TBD

Explain the proposal as if it was already included in the language and you were teaching it to another Rust programmer. That generally means:

- Introducing new named concepts.
- Explaining the feature largely in terms of examples.
- Explaining how Rust programmers should *think* about the feature, and how it should impact the way they use Rust. It should explain the impact as concretely as possible.
- If applicable, provide sample error messages, deprecation warnings, or migration guidance.
- If applicable, describe the differences between teaching this to existing Rust programmers and new Rust programmers.

For implementation-oriented RFCs (e.g. for compiler internals), this section should focus on how compiler contributors should think about the change, and give examples of its concrete impact. For policy RFCs, this section should provide an example-driven introduction to the policy, and explain its impact in concrete terms.

# Reference-level explanation
[reference-level-explanation]: #reference-level-explanation

This is the technical portion of the RFC. Explain the design in sufficient detail that:

- Its interaction with other features is clear.
- It is reasonably clear how the feature would be implemented.
- Corner cases are dissected by example.

The section should return to the examples given in the previous section, and explain more fully how the detailed proposal makes those examples work.

# Drawbacks
[drawbacks]: #drawbacks

1. Conflicting work and projects
2. Cargo evolutional conflicts
3. The possibility to confuse or distract developers

# Rationale and alternatives
[rationale-and-alternatives]: #rationale-and-alternatives

- Why is this design the best in the space of possible designs?   TBD
- What other designs have been considered and what is the rationale for not choosing them?  Too many others
- What is the impact of not doing this?  Missed opportunity

# Prior art
[prior-art]: #prior-art

  * cargo-script - the unmaintained project that rust-script was forked from.
  * cargo-eval - maintained fork of cargo-script.
  * cargo-play - local Rust playground.
  * runner - tool for running Rust snippets.
  * scriptisto - language-agnostic "shebang interpreter" that enables you to write scripts in compiled languages.

Next we will generically discuss this prior art, both the good and the bad, in relation to this proposal.

This section is intended to encourage you as an author to think about the lessons from other languages, provide readers of your RFC with a fuller picture.

Note that while precedent set by other languages is some motivation, it does not on its own motivate an RFC.
Please also take into consideration that rust sometimes intentionally diverges from common language features.

# Unresolved questions
[unresolved-questions]: #unresolved-questions

  * How do we interact with workspaces? question
  * Use rust-version field for toolchain version enhancement  question
  * Define cargo-xtask behaviors enhancement
  * Share a lock file across scripts enhancement  question
  * How much should main detection align with rustdoc? question
  * How do we balance reducing boilerplate in scripts while allowing reproducibility across systems question
  * Define how this interacts with cargo config files enhancement
  * Rust toolchain files interactions definition
  * Identify a final command name and file extension

- What parts of the design do you expect to resolve through the RFC process before this gets merged?
- What parts of the design do you expect to resolve through the implementation of this feature before stabilization?
- What related issues do you consider out of scope for this RFC that could be addressed in the future independently of the solution that comes out of this RFC?

# Future possibilities
[future-possibilities]: #future-possibilities

Think about what the natural extension and evolution of your proposal would
be and how it would affect the language and project as a whole in a holistic
way. Try to use this section as a tool to more fully consider all possible
interactions with the project and language in your proposal.
Also consider how this all fits into the roadmap for the project
and of the relevant sub-team.

This is also a good place to "dump ideas", if they are out of scope for the
RFC you are writing but otherwise related.

If you have tried and cannot think of any future possibilities,
you may simply state that you cannot think of anything.

Note that having something written down in the future-possibilities section
is not a reason to accept the current or a future RFC; such notes should be
in the section on motivation or rationale in this or subsequent RFCs.
The section merely provides additional information.

