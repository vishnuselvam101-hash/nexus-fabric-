# Review Handoff — for the Fabrication Engineer

18 November 2026 · One page · Accompanies the [Fabrication Engineering Review Package](FABRICATION_ENGINEERING_REVIEW_PACKAGE.md)

## What we are asking

We are designing software that turns fabrication drawings into cutting files and checks its own work. To do that it must know **which things depend on which** in a fabricated assembly: if a header diameter was misunderstood, what else is now wrong?

We wrote 56 short statements of such relationships for three situations. **They are at evidence level EV-1 and have not been independently reviewed; they were not written by an engineer.** We need someone who has actually built these parts to tell us which are right, which are wrong, and what is missing.

You do not need to know anything about the software.

## What to review, in order of priority

1. **Header with an oblique branch** — statements H1–H18 (most important).
2. **"If this, then check that" rules** — RT1–RT10.
3. **Reducer in halves** — R1–R14.
4. **Rectangular-to-round transition** — T1–T14.

Reviewing only part is useful. Please say which sections you covered.

## How to review

1. For each statement, from your own knowledge, give one code:
   **U** always true · **P** true for a particular process (say which) · **F** a rule of your factory or customer (say what) · **C** depends on a value that is measured or calibrated (say how you get it) · **X** wrong (please correct) · **?** cannot say.
2. Write one or two lines of reasoning: experience, a procedure, a past job, a rule you can name. Plain words are fine.
3. **Then** open Annex A and correct our guesses about what goes wrong and how serious it is.
4. Add anything we missed (section 6a). This is the most valuable part: the checks an experienced engineer makes that are not on our list.
5. Answer the general questions in section 6 if you can — especially past errors you have seen.
6. Fill in the reviewer record (name, qualifications, experience, date) and return the document.

## What we need back

| Item | Why |
|---|---|
| The package with your classifications and reasoning | It is the only evidence that a statement was reviewed |
| Your name, role and relevant experience | A review counts only when we know who made it |
| Corrections and missing relationships | These change the design |
| Which statements you did **not** review | So we do not claim more than was done |

Return it as an edited copy, a marked-up printout or scan, or notes keyed to the statement numbers (H5, R2, …) — whichever is easiest.

## What your review will and will not be used for

- A statement you classify becomes "engineer-reviewed" **for the scope you state** — not "validated", and not for any other factory.
- Your corrections are recorded with your name unless you ask otherwise.
- Nothing you mark is treated as a measured or calibrated value.
- We will not describe unreviewed statements as reviewed.

## Practical notes

- Estimated effort: unknown — please tell us how long it took, so we can plan further reviews.
- Language: this is the English master. A Japanese version does not exist yet; it must be translated and checked by a Japanese-speaking fabrication engineer before use at the factory.
- Confidential drawings are **not** needed for this review. If an example from your own work would help explain a point, describe it in words.

Questions about any statement: please write them beside the statement and mark it "?".
