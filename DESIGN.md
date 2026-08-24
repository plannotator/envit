---
name: envit
colors:
  ink: "#1b1b1b"
  paper: "#ffffff"
  dim: "#6a6a6a"
  line: "#e4e4e4"
  wash: "#f6f6f4"
  link: "#0b63ce"
  dark:
    ink: "#dcdcdc"
    paper: "#121212"
    dim: "#909090"
    line: "#2b2b2b"
    wash: "#1b1b1b"
    link: "#6aa9f4"
typography:
  body:
    fontFamily: system-ui, -apple-system, "Segoe UI", sans-serif
    fontSize: 16px
    lineHeight: 1.6
  h1:
    fontFamily: system-ui, -apple-system, "Segoe UI", sans-serif
    fontSize: 1.5rem
    fontWeight: 650
    letterSpacing: -0.01em
  h2:
    fontFamily: system-ui, -apple-system, "Segoe UI", sans-serif
    fontSize: 1.05rem
    fontWeight: 650
  code:
    fontFamily: ui-monospace, "SF Mono", Menlo, Consolas, monospace
    fontSize: 13px
    lineHeight: 1.6
  caption:
    fontFamily: ui-monospace, "SF Mono", Menlo, Consolas, monospace
    fontSize: 0.75rem
    color: dim
rounded:
  sm: 4px
  md: 6px
spacing:
  xs: 4px
  sm: 8px
  md: 16px
  lg: 40px
  xl: 64px
measure:
  column: 42rem
---

## Overview

envit is a tool for people who read source code. The brand is the
restraint of a good man page: white space, one column, real terminal
output, and nothing that competes with the content. The product promises
no daemon, one static binary, and files you can inspect. The visual
language keeps the same promise. No gradients, no illustrations, no
webfonts, no decorative labels.

The test for any element: would it survive on a Hacker News reader's
screen without making them suspicious? Ornament fails that test.
Evidence passes it.

## Colors

The palette is two neutrals, two grays, and one link color.

- **Ink (#1b1b1b):** all text. Not pure black; pure black on white reads
  as harsh at 16px.
- **Paper (#ffffff):** the page. Pure white on purpose. Warm off-whites
  signal "designed"; this brand signals "written".
- **Dim (#6a6a6a):** captions, comments in code blocks, table headers,
  the tagline. Anything secondary to the ink.
- **Line (#e4e4e4):** borders on code blocks and tables. Never used as a
  fill.
- **Wash (#f6f6f4):** code block background. The only tinted surface. A
  faint warmth so blocks read as paper, not as UI panels.
- **Link (#0b63ce):** links and hover states only. The brand has no
  accent color for emphasis; if something needs emphasis, the words do
  it.

Dark mode maps each token to its counterpart under `dark`. It follows
the OS setting with no toggle. Both themes get equal care: check every
element in both before shipping.

## Typography

Body text uses the system UI font. The reader's own machine renders the
page; envit does not ship a font. Monospace is for machine text only:
commands, output, file paths, JSON, trees. A page set entirely in
monospace is a costume. A page with no monospace has nothing real on it.

Headings are the body face at weight 650, barely larger than body. The
hierarchy comes from spacing, not size. Headings use sentence case.

Code blocks are 13px in the system monospace at 1.6 line height, inside
a wash background with a line border, radius `md`. Comments inside
blocks use the dim color.

## Layout

One column, 42rem wide, centered. Sections are separated by `xl`
spacing. Tables use the `line` color between rows and no vertical
rules. Code blocks and tables scroll horizontally inside their own
container; the page never scrolls sideways.

Every section is anchored by a copyable block. Prose between blocks is
two or three sentences.

## Voice

Write for a tired engineer reading on a phone.

- Short sentences. Under 20 words for instructions, under 25 for
  explanations.
- No em dashes. Use a period, a comma, a colon, or parentheses.
- No adjectives that do the reader's judging for them: no "powerful",
  "seamless", "blazingly fast". Give the number instead.
- Show the command, then say what it did.
- Name the constraint, not the benefit: "No daemon. `ps` shows nothing
  between invocations."

## The mark

Two squares joined by a short line. The filled square is the store: the
one copy of a repository on the machine. The outlined square is a
project: a link to that copy, not a duplicate. The line is the symlink.

Rendered in `wash` on an `ink` rounded square (radius `md` scaled). It
reads at 16px because it is two rectangles and a stroke. Do not add a
gradient, a shadow, or a third element.

The wordmark is "envit" in the body face at weight 650 with tight
letter spacing. Always lowercase. Never "Envit" or "ENVIT".

## Do not

- Do not add a hero image, gradient, or animation.
- Do not use a webfont.
- Do not set body copy in monospace.
- Do not add labels, badges, pills, or callouts.
- Do not add a second accent color.
- Do not write marketing copy. If a sentence could appear in an ad,
  delete it.
