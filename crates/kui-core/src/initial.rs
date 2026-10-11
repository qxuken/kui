//! An initial letter — a drop cap — laid out inside one text
//! (`TextStyle::initial_letter`, backlog F164): the first letter set as
//! tall as a few lines, its top level with the first line's capitals and
//! its foot on the last one's baseline, the lines beside it moved in past
//! it and the ones below at full width again.
//!
//! cosmic-text lays a paragraph out at one width, and its layout lines
//! cannot be edited from outside, so the paragraph is cut instead: the
//! letter is the buffer's first line on its own, shaped at the size the
//! drop asks, and the paragraph is cut again where the lines beside the
//! letter end, the lines before the cut laid out narrower and the rest at
//! the full width. A cut has no line ending, so the content's bytes stay
//! where they were (`text::paragraph_starts` finds every line by its
//! length), and the buffer stays the one a caret, a hit, a selection, a
//! copy and the access tree read: each reads a run where
//! [`Initial::shift`] moves it.

use cosmic_text::{AttrsList, Buffer, BufferLine, FontSystem, LineEnding};

use crate::geom::Rect;

/// The punctuation that opens a text and goes with its first letter into
/// the initial, as CSS's `::first-letter` takes it: quotes, brackets, the
/// Spanish openers.
fn opens(c: char) -> bool {
    matches!(
        c,
        '"' | '\''
            | '('
            | '['
            | '{'
            | '¿'
            | '¡'
            | '\u{201c}' // “
            | '\u{2018}' // ‘
            | '\u{201e}' // „
            | '\u{201a}' // ‚
            | '\u{ab}' // «
            | '\u{2039}' // ‹
            | '\u{300c}' // 「
            | '\u{300e}' // 『
    )
}

/// The bytes of `text` its initial is: the first grapheme, when it is a
/// letter or a digit, with the opening punctuation before it (three at
/// most). `None` when the text starts with anything else — a space, a
/// symbol, nothing.
pub(crate) fn initial_len(text: &str) -> Option<usize> {
    use unicode_segmentation::UnicodeSegmentation;
    let mut at = 0;
    for g in text.graphemes(true).take(4) {
        let c = g.chars().next()?;
        if c.is_alphanumeric() {
            return Some(at + g.len());
        }
        if !opens(c) {
            return None;
        }
        at += g.len();
    }
    None
}

/// Where an initial is, while a text has one. Physical px, from the
/// text's origin.
#[derive(Clone)]
pub(crate) struct Initial {
    /// The lines it drops through.
    pub(crate) lines: u32,
    /// Between the letter and the text beside it.
    gap: f32,
    /// The paragraph line cut where the lines beside the letter end, as
    /// it was before the cut: put back before the next layout.
    cut: Option<(usize, BufferLine)>,
    /// Buffer lines `1..beside` are beside the letter.
    beside: usize,
    /// How far right they are moved: the letter's advance and the gap,
    /// whole pixels.
    pub(crate) indent: f32,
    /// How far down the letter's run is moved from where the buffer lays
    /// it: onto the baseline of the last line it drops through.
    drop: f32,
    /// How far up every other run is moved: the height the buffer
    /// stacks the letter's line at, whole pixels.
    lift: f32,
    /// The letter's box: its advance wide, from the first line's top to
    /// the bottom of the last it drops through. Where its caret and its
    /// selection are drawn, and what a point beside the rows hits.
    pub(crate) rect: Rect,
    /// Where the rows beside it end: a point below is the full width's.
    beside_bottom: f32,
}

/// Cap height over the em of the face the first glyph of `line` in
/// `buffer`'s layout is shaped in, among the glyphs `pick` keeps.
fn cap_ratio(
    buffer: &Buffer,
    fs: &mut FontSystem,
    line: usize,
    pick: impl Fn(usize) -> bool,
) -> Option<f32> {
    let run = buffer.layout_runs().find(|r| r.line_i == line)?;
    let g = run.glyphs.iter().rev().find(|g| pick(g.start))?;
    let font = fs.get_font(g.font_id, g.font_weight)?;
    let m = font.as_swash().metrics(&[]);
    (m.cap_height > 0.0 && m.units_per_em > 0).then(|| m.cap_height / m.units_per_em as f32)
}

/// Lays line `i` of `buffer` out again at `width`, as the buffer would
/// at its own: the rows it makes.
fn lay(buffer: &mut Buffer, fs: &mut FontSystem, i: usize, width: Option<f32>) -> usize {
    let size = buffer.metrics().font_size;
    let (wrap, ellipsize, mono, tab, hinting) = (
        buffer.wrap(),
        buffer.ellipsize(),
        buffer.monospace_width(),
        buffer.tab_width(),
        buffer.hinting(),
    );
    let line = &mut buffer.lines[i];
    line.reset_layout();
    line.layout(fs, size, width, wrap, ellipsize, mono, tab, hinting)
        .len()
}

impl Initial {
    /// Cuts `buffer`'s first letter onto a line of its own and sizes it
    /// to drop through `lines` lines, for a buffer shaped and laid out
    /// at its width; then [`Self::place`]. `None`, the buffer as it was,
    /// when its first line does not start with a letter. `gap` is the
    /// room after the letter, physical px.
    pub(crate) fn cut(
        buffer: &mut Buffer,
        fs: &mut FontSystem,
        lines: u32,
        gap: f32,
    ) -> Option<Self> {
        let k = initial_len(buffer.lines.first()?.text())?;
        let metrics = buffer.metrics();
        // The letter's cap height reaches from the last line's baseline
        // to the first line's capitals: the lines between, and the text's
        // own cap height. Its size is that over its face's cap height.
        let own = cap_ratio(buffer, fs, 0, |b| b < k).unwrap_or(0.7);
        let text = cap_ratio(buffer, fs, 0, |b| b >= k).unwrap_or(own);
        let cap = (lines - 1) as f32 * metrics.line_height + text * metrics.font_size;
        let size = (cap / own).max(1.0);
        let first = &mut buffer.lines[0];
        if k < first.text().len() {
            let rest = first.split_off(k);
            buffer.lines.insert(1, rest);
        }
        // The letter at its size, its line as tall as whole pixels.
        let m = crate::text::shaper_metrics(size, size.round());
        let old = buffer.lines[0].attrs_list().clone();
        let mut list = AttrsList::new(&old.defaults().metrics(m));
        for (range, attrs) in old.spans_iter() {
            list.add_span(range.clone(), &attrs.as_attrs().metrics(m));
        }
        buffer.lines[0].set_attrs_list(list);
        let width = buffer.size().0;
        lay(buffer, fs, 0, width);
        let mut initial = Self {
            lines,
            gap,
            cut: None,
            beside: 1,
            indent: 0.0,
            drop: 0.0,
            lift: 0.0,
            rect: Rect::default(),
            beside_bottom: 0.0,
        };
        initial.place(buffer, fs);
        Some(initial)
    }

    /// Puts back the cut [`Self::place`] made, before the buffer is laid
    /// out at another width.
    pub(crate) fn uncut(&mut self, buffer: &mut Buffer) {
        if let Some((at, whole)) = self.cut.take() {
            buffer.lines.splice(at..at + 2, [whole]);
        }
    }

    /// Lays the lines beside the letter out narrower, cutting the
    /// paragraph where they end, and works out where every run is
    /// moved: for a buffer laid out at its width (since [`Self::uncut`]).
    pub(crate) fn place(&mut self, buffer: &mut Buffer, fs: &mut FontSystem) {
        let width = buffer.size().0;
        // A line the buffer has not laid out ends its runs: every line is.
        for i in 0..buffer.lines.len() {
            if buffer.lines[i].layout_opt().is_none() {
                lay(buffer, fs, i, if i == 0 { None } else { width });
            }
        }
        let Some((w0, h0, y0)) = buffer
            .layout_runs()
            .next()
            .map(|r| (r.line_w, r.line_height, r.line_y))
        else {
            return;
        };
        self.indent = (w0 + self.gap).ceil();
        // Whole pixels, so a row moved up by it draws on the pixel grid.
        self.lift = h0.round();
        let narrow = width.map(|w| (w - self.indent).max(1.0));
        let want = self.lines as usize;
        let (mut rows, mut i) = (0usize, 1usize);
        self.beside = buffer.lines.len();
        while i < buffer.lines.len() {
            let n = lay(buffer, fs, i, narrow);
            if rows + n >= want {
                let keep = want - rows;
                // Where the first row past the letter starts.
                let at = buffer.lines[i]
                    .layout_opt()
                    .and_then(|l| l.get(keep))
                    .and_then(|l| l.glyphs.iter().map(|g| g.start).min())
                    .filter(|&at| at > 0 && at < buffer.lines[i].text().len());
                if let Some(at) = at {
                    let whole = buffer.lines[i].clone();
                    let rest = buffer.lines[i].split_off(at);
                    buffer.lines.insert(i + 1, rest);
                    lay(buffer, fs, i, narrow);
                    lay(buffer, fs, i + 1, width);
                    self.cut = Some((i, whole));
                }
                self.beside = i + 1;
                break;
            }
            rows += n;
            i += 1;
        }
        // The last row the letter drops through: one beside it, or past
        // the text's own rows, as many more of its line height down.
        let lh = buffer.metrics().line_height;
        let (mut count, mut nth, mut bottom) = (0usize, None, 0.0f32);
        for run in buffer.layout_runs().skip(1) {
            if run.line_i >= self.beside {
                break;
            }
            count += 1;
            let top = run.line_top - self.lift;
            if count <= want {
                nth = Some((top, run.line_y.round() - self.lift, run.line_height));
            }
            bottom = top + run.line_height;
        }
        self.beside_bottom = bottom;
        let more = want.saturating_sub(count.max(1)) as f32 * lh;
        let (top, base, h) = nth.unwrap_or((0.0, (lh * 0.8).round(), lh));
        self.drop = (base + more - y0.round()).round();
        self.rect = Rect::new(0.0, 0.0, w0, top + h + more);
    }

    /// How far a run of buffer line `line_i` is drawn from where the
    /// buffer lays it.
    pub(crate) fn shift(&self, line_i: usize) -> (f32, f32) {
        if line_i == 0 {
            (0.0, self.drop)
        } else if line_i < self.beside {
            (self.indent, -self.lift)
        } else {
            (0.0, -self.lift)
        }
    }

    /// Where a point (physical, from the text's origin) is in the
    /// buffer's own layout, for its hit test: on the letter, a point
    /// along the letter's line; beside it, moved back by the indent;
    /// otherwise moved down past the letter's line.
    pub(crate) fn to_buffer(&self, x: f32, y: f32) -> (f32, f32) {
        if x < self.rect.w + self.gap / 2.0 && y < self.rect.h.max(self.beside_bottom) {
            return (x, self.lift / 2.0);
        }
        let x = if y < self.beside_bottom {
            x - self.indent
        } else {
            x
        };
        (x, (y + self.lift).max(self.lift))
    }
}

/// Whether buffer line `i` goes on into the next without a line ending:
/// a cut the initial made.
pub(crate) fn joined(buffer: &Buffer, i: usize) -> bool {
    i + 1 < buffer.lines.len() && buffer.lines[i].ending() == LineEnding::None
}

#[cfg(test)]
mod tests {
    use super::initial_len;

    #[test]
    fn the_initial_is_the_first_letter_with_its_opening_punctuation() {
        assert_eq!(initial_len("Once upon"), Some(1));
        assert_eq!(initial_len("“Once”"), Some(4));
        assert_eq!(initial_len("¿Qué?"), Some(3));
        assert_eq!(initial_len("Éire"), Some(2));
        assert_eq!(initial_len("1984"), Some(1));
        assert_eq!(initial_len(" Once"), None);
        assert_eq!(initial_len("— Once"), None);
        assert_eq!(initial_len(""), None);
        assert_eq!(initial_len("\"\"\"\"a"), None);
    }
}
