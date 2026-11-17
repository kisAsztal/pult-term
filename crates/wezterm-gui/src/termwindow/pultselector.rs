//! pult: `InputSelector` as a floating panel instead of the overlay that covers the whole tab.
//!
//! It takes the same input (title, choices with coloured labels, fuzzy, description) and sends back the same event
//! (`action_callback(window, pane, id, label)`, `nil` on Escape), so nothing changes on the Lua side.
//! Typing filters; ↑/↓ (Ctrl+P/N, Ctrl+K/J) move; Enter selects; Esc / Ctrl+G / Ctrl+C cancel.
//! In a non-fuzzy list with an empty query, 1–9 select directly.
//! Colours come from `command_palette_bg_color` / `command_palette_fg_color` (which the Lua config sets from the
//! theme); the font is the terminal's.

use crate::overlay::selector::{matcher_pattern, matcher_score};
use crate::scripting::guiwin::GuiWin;
use crate::tabbar::parse_status_text;
use crate::termwindow::box_model::*;
use crate::termwindow::modal::Modal;
use crate::termwindow::render::corners::{
    BOTTOM_LEFT_ROUNDED_CORNER, BOTTOM_RIGHT_ROUNDED_CORNER, TOP_LEFT_ROUNDED_CORNER,
    TOP_RIGHT_ROUNDED_CORNER,
};
use crate::termwindow::{DimensionContext, TermWindow};
use crate::utilsprites::RenderMetrics;
use config::keyassignment::{InputSelector, InputSelectorEntry, KeyAssignment};
use config::Dimension;
use mux_lua::MuxPane;
use std::cell::{Ref, RefCell};
use termwiz::cell::{Cell, CellAttributes};
use termwiz::surface::{Line, SEQ_ZERO};
use wezterm_term::{KeyCode, KeyModifiers, MouseEvent};
use window::color::LinearRgba;

const MAX_ROWS: usize = 14;

struct Choice {
    entry: InputSelectorEntry,
    line: Line,
    text: String,
}

pub struct PultSelector {
    args: InputSelector,
    event_name: String,
    window: GuiWin,
    pane: MuxPane,
    choices: Vec<Choice>,
    query: RefCell<String>,
    matches: RefCell<Vec<usize>>,
    selected: RefCell<usize>,
    top: RefCell<usize>,
    element: RefCell<Option<Vec<ComputedElement>>>,
}

fn mix(a: LinearRgba, b: LinearRgba, t: f32) -> LinearRgba {
    LinearRgba(
        a.0 * t + b.0 * (1. - t),
        a.1 * t + b.1 * (1. - t),
        a.2 * t + b.2 * (1. - t),
        1.,
    )
}

/// A line that is too long ends in `…` (with the colour of its last remaining cell).
fn truncate(line: &Line, cols: usize) -> Line {
    if line.len() <= cols || cols < 2 {
        return line.clone();
    }
    let mut l = line.clone();
    let attrs = l.get_cell(cols - 2).map(|c| c.attrs().clone()).unwrap_or_default();
    l.resize(cols - 1, SEQ_ZERO);
    l.set_cell(cols - 1, Cell::new('…', attrs), SEQ_ZERO);
    l
}

fn cells(n: f32) -> Dimension {
    Dimension::Cells(n)
}

fn pad(v: f32, h: f32) -> BoxDimension {
    BoxDimension { left: cells(h), right: cells(h), top: cells(v), bottom: cells(v) }
}

impl PultSelector {
    /// `None` if the action is not an `action_callback` (the stock overlay is used then).
    pub fn new(args: InputSelector, window: GuiWin, pane: MuxPane) -> Option<Self> {
        let event_name = match *args.action {
            KeyAssignment::EmitEvent(ref id) => id.to_string(),
            _ => return None,
        };
        let choices = args
            .choices
            .iter()
            .map(|entry| {
                let line = parse_status_text(&entry.label, CellAttributes::default());
                let text = line.as_str().trim_end().to_string();
                Choice { entry: entry.clone(), line, text }
            })
            .collect::<Vec<_>>();
        let matches = (0..choices.len()).collect();
        Some(Self {
            args,
            event_name,
            window,
            pane,
            choices,
            query: RefCell::new(String::new()),
            matches: RefCell::new(matches),
            selected: RefCell::new(0),
            top: RefCell::new(0),
            element: RefCell::new(None),
        })
    }

    fn refilter(&self) {
        let query = self.query.borrow();
        let mut matches = self.matches.borrow_mut();
        if query.is_empty() {
            *matches = (0..self.choices.len()).collect();
        } else {
            let pattern = matcher_pattern(&query);
            let mut scored: Vec<(usize, u32)> = self
                .choices
                .iter()
                .enumerate()
                .filter_map(|(i, c)| matcher_score(&pattern, &c.text).map(|s| (i, s)))
                .collect();
            scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            *matches = scored.into_iter().map(|(i, _)| i).collect();
        }
        *self.selected.borrow_mut() = 0;
        *self.top.borrow_mut() = 0;
    }

    fn move_by(&self, delta: isize) {
        let n = self.matches.borrow().len();
        if n == 0 {
            return;
        }
        let mut sel = self.selected.borrow_mut();
        *sel = (*sel as isize + delta).clamp(0, n as isize - 1) as usize;
        let mut top = self.top.borrow_mut();
        if *sel < *top {
            *top = *sel;
        } else if *sel >= *top + MAX_ROWS {
            *top = *sel + 1 - MAX_ROWS;
        }
    }

    fn finish(&self, term_window: &mut TermWindow, entry: Option<InputSelectorEntry>) {
        term_window.cancel_modal();
        crate::overlay::selector::trampoline(
            self.event_name.clone(),
            self.window.clone(),
            self.pane.clone(),
            entry,
        );
    }

    fn compute(&self, term_window: &mut TermWindow) -> anyhow::Result<Vec<ComputedElement>> {
        let font = term_window.fonts.default_font()?;
        let metrics = RenderMetrics::with_font_metrics(&font.metrics());
        let palette = term_window.palette().clone();

        let bg = term_window.config.command_palette_bg_color.to_linear();
        let fg = term_window.config.command_palette_fg_color.to_linear();
        let dim = mix(fg, bg, 0.5);
        let input_bg = mix(LinearRgba(0., 0., 0., 1.), bg, 0.06);
        let line_col = mix(fg, bg, 0.12);
        let sel_bg = mix(fg, bg, 0.09);
        let border_col = mix(fg, bg, 0.35);

        // width: fits the longest label, at least 60 columns, at most 130 or the window minus 4
        let size = term_window.terminal_size;
        let longest = self.choices.iter().map(|c| c.line.len()).max().unwrap_or(0);
        let want_cols = (longest + 6).max(60).min(130).min(size.cols.saturating_sub(4).max(20));
        let text_cols = want_cols.saturating_sub(6);

        let query = self.query.borrow();
        let matches = self.matches.borrow();
        let selected = *self.selected.borrow();
        let top = *self.top.borrow();

        // search line: the description, dimmed, while empty; the title on the right
        let placeholder = [&self.args.fuzzy_description, &self.args.description]
            .iter()
            .map(|s| s.trim().trim_end_matches(':').trim())
            .find(|s| !s.is_empty() && !s.contains("Enter="))
            .unwrap_or("search…")
            .to_string();
        let (input_text, input_col) = if query.is_empty() {
            (placeholder, dim)
        } else {
            (format!("{query}▏"), fg)
        };
        let mut input_kids = vec![Element::new(&font, ElementContent::Text(input_text))
            .colors(ElementColors { border: BorderColor::default(), bg: LinearRgba::TRANSPARENT.into(), text: input_col.into() })];
        if !self.args.title.is_empty() {
            input_kids.push(
                Element::new(&font, ElementContent::Text(self.args.title.clone()))
                    .float(Float::Right)
                    .padding(BoxDimension { left: cells(2.), right: cells(0.), top: cells(0.), bottom: cells(0.) })
                    .colors(ElementColors { border: BorderColor::default(), bg: LinearRgba::TRANSPARENT.into(), text: dim.into() }),
            );
        }
        let mut rows = vec![Element::new(&font, ElementContent::Children(input_kids))
            .display(DisplayType::Block)
            .min_width(Some(Dimension::Percent(1.)))
            .padding(pad(0.6, 1.))
            .border(BoxDimension { left: Dimension::Pixels(0.), right: Dimension::Pixels(0.), top: Dimension::Pixels(0.), bottom: Dimension::Pixels(1.) })
            .colors(ElementColors {
                border: BorderColor { left: input_bg, right: input_bg, top: input_bg, bottom: line_col },
                bg: input_bg.into(),
                text: fg.into(),
            })];

        let mut list = vec![];
        for (i, &ci) in matches.iter().enumerate().skip(top).take(MAX_ROWS) {
            let c = &self.choices[ci];
            let row_bg = if i == selected { sel_bg } else { LinearRgba::TRANSPARENT };
            list.push(
                Element::with_line(&font, &truncate(&c.line, text_cols), &palette)
                    .display(DisplayType::Block)
                    .min_width(Some(Dimension::Percent(1.)))
                    .padding(pad(0.3, 1.))
                    .colors(ElementColors { border: BorderColor::default(), bg: row_bg.into(), text: fg.into() }),
            );
        }
        if matches.is_empty() {
            list.push(
                Element::new(&font, ElementContent::Text("No matches.".into()))
                    .display(DisplayType::Block)
                    .padding(pad(0.5, 1.))
                    .colors(ElementColors { border: BorderColor::default(), bg: LinearRgba::TRANSPARENT.into(), text: dim.into() }),
            );
        } else if matches.len() > MAX_ROWS {
            let shown_to = (top + MAX_ROWS).min(matches.len());
            list.push(
                Element::new(&font, ElementContent::Text(format!("{}–{} / {}", top + 1, shown_to, matches.len())))
                    .display(DisplayType::Block)
                    .padding(pad(0.2, 1.))
                    .colors(ElementColors { border: BorderColor::default(), bg: LinearRgba::TRANSPARENT.into(), text: dim.into() }),
            );
        }
        rows.push(
            Element::new(&font, ElementContent::Children(list))
                .display(DisplayType::Block)
                .min_width(Some(Dimension::Percent(1.)))
                .padding(BoxDimension { left: cells(0.), right: cells(0.), top: cells(0.25), bottom: cells(0.25) }),
        );

        let cell_w = metrics.cell_size.width as f32;
        let width = want_cols as f32 * cell_w;

        let corner = |poly| SizedPoly { width: cells(0.5), height: cells(0.5), poly };
        let panel = Element::new(&font, ElementContent::Children(rows))
            .colors(ElementColors { border: BorderColor::new(border_col), bg: bg.into(), text: fg.into() })
            .border(BoxDimension::new(Dimension::Pixels(1.)))
            .border_corners(Some(Corners {
                top_left: corner(TOP_LEFT_ROUNDED_CORNER),
                top_right: corner(TOP_RIGHT_ROUNDED_CORNER),
                bottom_left: corner(BOTTOM_LEFT_ROUNDED_CORNER),
                bottom_right: corner(BOTTOM_RIGHT_ROUNDED_CORNER),
            }))
            .min_width(Some(Dimension::Pixels(width)));

        let top_bar = if term_window.show_tab_bar && !term_window.config.tab_bar_at_bottom {
            term_window.tab_bar_pixel_height().unwrap_or(0.)
        } else {
            0.
        };
        let border = term_window.get_os_border();
        let dims = term_window.dimensions;
        let avail = dims.pixel_width as f32 - (border.left + border.right).get() as f32;
        let x = border.left.get() as f32 + ((avail - width) / 2.).max(0.);
        let y = top_bar + border.top.get() as f32 + metrics.cell_size.height as f32 * 0.6;

        let computed = term_window.compute_element(
            &LayoutContext {
                height: DimensionContext {
                    dpi: dims.dpi as f32,
                    pixel_max: dims.pixel_height as f32,
                    pixel_cell: metrics.cell_size.height as f32,
                },
                width: DimensionContext {
                    dpi: dims.dpi as f32,
                    pixel_max: dims.pixel_width as f32,
                    pixel_cell: cell_w,
                },
                bounds: euclid::rect(x, y, width, dims.pixel_height as f32 - y),
                metrics: &metrics,
                gl_state: term_window.render_state.as_ref().unwrap(),
                zindex: 100,
            },
            &panel,
        )?;
        Ok(vec![computed])
    }
}

impl Modal for PultSelector {
    fn mouse_event(&self, _event: MouseEvent, _term_window: &mut TermWindow) -> anyhow::Result<()> {
        Ok(())
    }

    fn key_down(&self, key: KeyCode, mods: KeyModifiers, term_window: &mut TermWindow) -> anyhow::Result<bool> {
        let plain = mods == KeyModifiers::NONE || mods == KeyModifiers::SHIFT;
        match (key, mods) {
            (KeyCode::Escape, _) | (KeyCode::Char('g' | 'c'), KeyModifiers::CTRL) => {
                self.finish(term_window, None);
                return Ok(true);
            }
            (KeyCode::UpArrow, _) | (KeyCode::Char('p' | 'k'), KeyModifiers::CTRL) => self.move_by(-1),
            (KeyCode::DownArrow, _) | (KeyCode::Char('n' | 'j'), KeyModifiers::CTRL) => self.move_by(1),
            (KeyCode::PageUp, _) => self.move_by(-(MAX_ROWS as isize)),
            (KeyCode::PageDown, _) => self.move_by(MAX_ROWS as isize),
            (KeyCode::Enter, _) => {
                let sel = *self.selected.borrow();
                let entry = self.matches.borrow().get(sel).map(|&i| self.choices[i].entry.clone());
                if entry.is_some() {
                    self.finish(term_window, entry);
                }
                return Ok(true);
            }
            (KeyCode::Char(c), _)
                if plain
                    && !self.args.fuzzy
                    && self.query.borrow().is_empty()
                    && ('1'..='9').contains(&c)
                    && (c as usize - '1' as usize) < self.choices.len() =>
            {
                let entry = self.choices[c as usize - '1' as usize].entry.clone();
                self.finish(term_window, Some(entry));
                return Ok(true);
            }
            (KeyCode::Char(c), _) if plain => {
                self.query.borrow_mut().push(c);
                self.refilter();
            }
            (KeyCode::Backspace, _) => {
                self.query.borrow_mut().pop();
                self.refilter();
            }
            (KeyCode::Char('u'), KeyModifiers::CTRL) => {
                self.query.borrow_mut().clear();
                self.refilter();
            }
            _ => return Ok(false),
        }
        self.element.borrow_mut().take();
        term_window.invalidate_modal();
        Ok(true)
    }

    fn computed_element(&self, term_window: &mut TermWindow) -> anyhow::Result<Ref<'_, [ComputedElement]>> {
        if self.element.borrow().is_none() {
            let e = self.compute(term_window)?;
            self.element.borrow_mut().replace(e);
        }
        Ok(Ref::map(self.element.borrow(), |v| v.as_ref().unwrap().as_slice()))
    }

    fn reconfigure(&self, _term_window: &mut TermWindow) {
        self.element.borrow_mut().take();
    }
}
