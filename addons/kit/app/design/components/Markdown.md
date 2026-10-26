# Markdown

A document written in Markdown, drawn in the skin: a guideline, a brand book, release notes, help.


## When to use

Use Markdown for prose the app ships or reads rather than builds: a help page, a changelog, a component's guideline (the gallery's Guide button draws these with it). Headings step down from the title size in the UI face, body text is in fg at the body size, lists indent 14px a level, a quote is muted beside a vertical rule, fenced code is a `Code` block, and a pipe table is a grid with a bold header row over a hairline. Every stretch of text is selectable.

It reads a working subset of Markdown: `#` to `####` headings, paragraphs (lines joined), `-` / `*` / `1.` lists nested by two spaces, `>` quotes, `---` rules, fenced code, pipe tables, and inline `code`, **bold**, *italic* and links, a link drawn as its text. Raw HTML, images and footnotes stay text.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `text` | `text` | string | The Markdown. |

Every component also takes `key` and `fresh`.

## Do

- Put a document in a panel or a scrolling column: it is as tall as its text.
- Keep tables narrow: their columns are as wide as their widest cell.

## Don't

- Use Markdown for text the user edits: use a text area.
- Build a screen's layout out of Markdown: place components.
- Count on a link opening: it is drawn as its text.
