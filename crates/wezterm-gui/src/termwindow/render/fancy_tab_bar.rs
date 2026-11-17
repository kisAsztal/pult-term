use crate::color::LinearRgba;
use crate::customglyph::*;
use crate::tabbar::{TabBarItem, TabEntry};
use crate::termwindow::box_model::*;
use crate::termwindow::render::corners::*;

use crate::termwindow::render::window_buttons::window_button_element;
use crate::termwindow::{UIItem, UIItemType};
use crate::utilsprites::RenderMetrics;
use config::{Dimension, DimensionContext, TabBarColors};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use termwiz::cell::Cell;
use termwiz::color::SrgbaTuple;
use termwiz::surface::{Line, SEQ_ZERO};
use wezterm_font::LoadedFont;
use wezterm_term::color::{ColorAttribute, ColorPalette};
use window::{IntegratedTitleButtonAlignment, IntegratedTitleButtonStyle};

const X_BUTTON: &[Poly] = &[
    Poly {
        path: &[
            PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Zero),
            PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::Outline,
    },
    Poly {
        path: &[
            PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
            PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::Outline,
    },
];

const PLUS_BUTTON: &[Poly] = &[
    Poly {
        path: &[
            PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
            PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::Outline,
    },
    Poly {
        path: &[
            PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
            PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::Outline,
    },
];

impl crate::TermWindow {
    pub fn invalidate_fancy_tab_bar(&mut self) {
        self.fancy_tab_bar.take();
    }

    pub fn build_fancy_tab_bar(&self, palette: &ColorPalette) -> anyhow::Result<ComputedElement> {
        let tab_bar_height = self.tab_bar_pixel_height()?;
        let font = self.fonts.title_font()?;
        let metrics = RenderMetrics::with_font_metrics(&font.metrics());
        let items = self.tab_bar.items();
        let colors = self
            .config
            .colors
            .as_ref()
            .and_then(|c| c.tab_bar.as_ref())
            .cloned()
            .unwrap_or_else(TabBarColors::default);

        let mut left_status = vec![];
        let mut left_eles = vec![];
        let mut right_eles = vec![];
        let bar_colors = ElementColors {
            border: BorderColor::default(),
            bg: if self.focused.is_some() {
                self.config.window_frame.active_titlebar_bg
            } else {
                self.config.window_frame.inactive_titlebar_bg
            }
            .to_linear()
            .into(),
            text: if self.focused.is_some() {
                self.config.window_frame.active_titlebar_fg
            } else {
                self.config.window_frame.inactive_titlebar_fg
            }
            .to_linear()
            .into(),
        };
        let tab_vertical_alignment = if self.config.tab_bar_at_bottom {
            VerticalAlign::Top
        } else {
            VerticalAlign::Bottom
        };

        // pult: tab title animation (spinner, pulse); collects the shortest frame interval needed
        let pult_anim = std::cell::Cell::new(None);
        // an unfocused window pulses half as often, to use less CPU in the background
        let pult_slow = self.focused.is_none();
        let item_to_elem = |item: &TabEntry| -> Element {
            let animated = match item.item {
                TabBarItem::Tab { tab_idx, .. } => {
                    let line = pult_animate(&item.title, palette, &pult_anim, pult_slow);
                    Some(pult_fade(&self.pult_fade, tab_idx, &item.title, line, palette, &pult_anim))
                }
                TabBarItem::RightStatus => Some(pult_animate(&item.title, palette, &pult_anim, pult_slow)),
                _ => None,
            };
            let src = animated.as_ref().unwrap_or(&item.title);
            let accent = match item.item {
                TabBarItem::Tab { .. } => pult_accent(src, palette),
                _ => None,
            };
            let title = accent.as_ref().map(|(l, _)| l).unwrap_or(src);
            let element = Element::with_line(&font, title, palette);

            let bg_color = title
                .get_cell(0)
                .and_then(|c| match c.attrs().background() {
                    ColorAttribute::Default => None,
                    col => Some(palette.resolve_bg(col)),
                });
            let fg_color = title
                .get_cell(0)
                .and_then(|c| match c.attrs().foreground() {
                    ColorAttribute::Default => None,
                    col => Some(palette.resolve_fg(col)),
                });

            let new_tab = colors.new_tab();
            let new_tab_hover = colors.new_tab_hover();
            let active_tab = colors.active_tab();
            let is_bottom = self.config.tab_bar_at_bottom;

            let elem = match item.item {
                TabBarItem::RightStatus | TabBarItem::LeftStatus | TabBarItem::None => element
                    .item_type(UIItemType::TabBar(TabBarItem::None))
                    .line_height(Some(1.75))
                    .margin(BoxDimension {
                        left: Dimension::Cells(0.),
                        right: Dimension::Cells(0.),
                        top: Dimension::Cells(0.0),
                        bottom: Dimension::Cells(0.),
                    })
                    .padding(BoxDimension {
                        left: Dimension::Cells(0.5),
                        right: Dimension::Cells(0.),
                        top: Dimension::Cells(0.),
                        bottom: Dimension::Cells(0.),
                    })
                    .border(BoxDimension::new(Dimension::Pixels(0.)))
                    .colors(bar_colors.clone()),
                TabBarItem::NewTabButton => Element::new(
                    &font,
                    ElementContent::Poly {
                        line_width: metrics.underline_height.max(2),
                        poly: SizedPoly {
                            poly: PLUS_BUTTON,
                            width: Dimension::Pixels(metrics.cell_size.height as f32 / 2.),
                            height: Dimension::Pixels(metrics.cell_size.height as f32 / 2.),
                        },
                    },
                )
                .vertical_align(VerticalAlign::Middle)
                .item_type(UIItemType::TabBar(item.item.clone()))
                .margin(BoxDimension {
                    left: Dimension::Cells(0.5),
                    right: Dimension::Cells(0.),
                    top: Dimension::Cells(0.2),
                    bottom: Dimension::Cells(0.),
                })
                .padding(BoxDimension {
                    left: Dimension::Cells(0.5),
                    right: Dimension::Cells(0.5),
                    top: Dimension::Cells(0.2),
                    bottom: Dimension::Cells(0.25),
                })
                .border(BoxDimension::new(Dimension::Pixels(1.)))
                .colors(ElementColors {
                    border: BorderColor::default(),
                    bg: new_tab.bg_color.to_linear().into(),
                    text: new_tab.fg_color.to_linear().into(),
                })
                .hover_colors(Some(ElementColors {
                    border: BorderColor::default(),
                    bg: new_tab_hover.bg_color.to_linear().into(),
                    text: new_tab_hover.fg_color.to_linear().into(),
                })),
                TabBarItem::Tab { active, .. } if active => element
                    .vertical_align(tab_vertical_alignment)
                    .item_type(UIItemType::TabBar(item.item.clone()))
                    .margin(if is_bottom {
                        BoxDimension {
                            left: Dimension::Cells(0.),
                            right: Dimension::Cells(0.),
                            top: Dimension::Cells(0.),
                            bottom: Dimension::Cells(0.2),
                        }
                    } else {
                        BoxDimension {
                            left: Dimension::Cells(0.),
                            right: Dimension::Cells(0.),
                            top: Dimension::Cells(0.2),
                            bottom: Dimension::Cells(0.),
                        }
                    })
                    .padding(BoxDimension {
                        left: Dimension::Cells(0.5),
                        right: Dimension::Cells(0.5),
                        top: Dimension::Cells(0.25),
                        bottom: Dimension::Cells(0.25),
                    })
                    .border(BoxDimension::new(Dimension::Pixels(1.)))
                    .border_corners(Some(if is_bottom {
                        Corners {
                            top_left: SizedPoly::none(),
                            top_right: SizedPoly::none(),
                            bottom_left: SizedPoly {
                                width: Dimension::Cells(0.5),
                                height: Dimension::Cells(0.5),
                                poly: BOTTOM_LEFT_ROUNDED_CORNER,
                            },
                            bottom_right: SizedPoly {
                                width: Dimension::Cells(0.5),
                                height: Dimension::Cells(0.5),
                                poly: BOTTOM_RIGHT_ROUNDED_CORNER,
                            },
                        }
                    } else {
                        Corners {
                            top_left: SizedPoly {
                                width: Dimension::Cells(0.5),
                                height: Dimension::Cells(0.5),
                                poly: TOP_LEFT_ROUNDED_CORNER,
                            },
                            top_right: SizedPoly {
                                width: Dimension::Cells(0.5),
                                height: Dimension::Cells(0.5),
                                poly: TOP_RIGHT_ROUNDED_CORNER,
                            },
                            bottom_left: SizedPoly::none(),
                            bottom_right: SizedPoly::none(),
                        }
                    }))
                    .colors(ElementColors {
                        border: BorderColor::new(
                            bg_color
                                .unwrap_or_else(|| active_tab.bg_color.into())
                                .to_linear(),
                        ),
                        bg: bg_color
                            .unwrap_or_else(|| active_tab.bg_color.into())
                            .to_linear()
                            .into(),
                        text: fg_color
                            .unwrap_or_else(|| active_tab.fg_color.into())
                            .to_linear()
                            .into(),
                    }),
                TabBarItem::Tab { .. } => element
                    .vertical_align(tab_vertical_alignment)
                    .item_type(UIItemType::TabBar(item.item.clone()))
                    .margin(if is_bottom {
                        BoxDimension {
                            left: Dimension::Cells(0.),
                            right: Dimension::Cells(0.),
                            top: Dimension::Cells(0.),
                            bottom: Dimension::Cells(0.2),
                        }
                    } else {
                        BoxDimension {
                            left: Dimension::Cells(0.),
                            right: Dimension::Cells(0.),
                            top: Dimension::Cells(0.2),
                            bottom: Dimension::Cells(0.),
                        }
                    })
                    .padding(BoxDimension {
                        left: Dimension::Cells(0.5),
                        right: Dimension::Cells(0.5),
                        top: Dimension::Cells(0.25),
                        bottom: Dimension::Cells(0.25),
                    })
                    .border(BoxDimension::new(Dimension::Pixels(1.)))
                    .border_corners(Some(if is_bottom {
                        Corners {
                            top_left: SizedPoly {
                                width: Dimension::Cells(0.),
                                height: Dimension::Cells(0.33),
                                poly: &[],
                            },
                            top_right: SizedPoly {
                                width: Dimension::Cells(0.),
                                height: Dimension::Cells(0.33),
                                poly: &[],
                            },
                            bottom_left: SizedPoly {
                                width: Dimension::Cells(0.5),
                                height: Dimension::Cells(0.5),
                                poly: BOTTOM_LEFT_ROUNDED_CORNER,
                            },
                            bottom_right: SizedPoly {
                                width: Dimension::Cells(0.5),
                                height: Dimension::Cells(0.5),
                                poly: BOTTOM_RIGHT_ROUNDED_CORNER,
                            },
                        }
                    } else {
                        Corners {
                            top_left: SizedPoly {
                                width: Dimension::Cells(0.5),
                                height: Dimension::Cells(0.5),
                                poly: TOP_LEFT_ROUNDED_CORNER,
                            },
                            top_right: SizedPoly {
                                width: Dimension::Cells(0.5),
                                height: Dimension::Cells(0.5),
                                poly: TOP_RIGHT_ROUNDED_CORNER,
                            },
                            bottom_left: SizedPoly {
                                width: Dimension::Cells(0.),
                                height: Dimension::Cells(0.33),
                                poly: &[],
                            },
                            bottom_right: SizedPoly {
                                width: Dimension::Cells(0.),
                                height: Dimension::Cells(0.33),
                                poly: &[],
                            },
                        }
                    }))
                    .colors({
                        let inactive_tab = colors.inactive_tab();
                        let bg = bg_color
                            .unwrap_or_else(|| inactive_tab.bg_color.into())
                            .to_linear();
                        let edge = colors.inactive_tab_edge().to_linear();
                        ElementColors {
                            border: BorderColor {
                                left: bg,
                                right: edge,
                                top: bg,
                                bottom: bg,
                            },
                            bg: bg.into(),
                            text: fg_color
                                .unwrap_or_else(|| inactive_tab.fg_color.into())
                                .to_linear()
                                .into(),
                        }
                    })
                    .hover_colors({
                        let inactive_tab_hover = colors.inactive_tab_hover();
                        Some(ElementColors {
                            border: BorderColor::new(
                                bg_color
                                    .unwrap_or_else(|| inactive_tab_hover.bg_color.into())
                                    .to_linear(),
                            ),
                            bg: bg_color
                                .unwrap_or_else(|| inactive_tab_hover.bg_color.into())
                                .to_linear()
                                .into(),
                            text: fg_color
                                .unwrap_or_else(|| inactive_tab_hover.fg_color.into())
                                .to_linear()
                                .into(),
                        })
                    }),
                TabBarItem::WindowButton(button) => window_button_element(
                    button,
                    self.window_state.contains(window::WindowState::MAXIMIZED),
                    &font,
                    &metrics,
                    &self.config,
                ),
            };
            match accent {
                Some((_, color)) => pult_accent_elem(elem, color),
                None => elem,
            }
        };

        let num_tabs: f32 = items
            .iter()
            .map(|item| match item.item {
                TabBarItem::NewTabButton | TabBarItem::Tab { .. } => 1.,
                _ => 0.,
            })
            .sum();
        let max_tab_width = ((self.dimensions.pixel_width as f32 / num_tabs)
            - (1.5 * metrics.cell_size.width as f32))
            .max(0.);

        // Reserve space for the native titlebar buttons
        if self
            .config
            .window_decorations
            .contains(::window::WindowDecorations::INTEGRATED_BUTTONS)
            && self.config.integrated_title_button_style == IntegratedTitleButtonStyle::MacOsNative
            && !self.window_state.contains(window::WindowState::FULL_SCREEN)
        {
            left_status.push(
                Element::new(&font, ElementContent::Text("".to_string())).margin(BoxDimension {
                    left: Dimension::Cells(4.0), // FIXME: determine exact width of macos ... buttons
                    right: Dimension::Cells(0.),
                    top: Dimension::Cells(0.),
                    bottom: Dimension::Cells(0.),
                }),
            );
        }

        for item in items {
            match item.item {
                TabBarItem::LeftStatus => left_status.push(item_to_elem(item)),
                TabBarItem::None | TabBarItem::RightStatus => right_eles.push(item_to_elem(item)),
                TabBarItem::WindowButton(_) => {
                    if self.config.integrated_title_button_alignment
                        == IntegratedTitleButtonAlignment::Left
                    {
                        left_eles.push(item_to_elem(item))
                    } else {
                        right_eles.push(item_to_elem(item))
                    }
                }
                TabBarItem::Tab { tab_idx, active } => {
                    let mut elem = item_to_elem(item);
                    elem.max_width = Some(Dimension::Pixels(max_tab_width));
                    elem.content = match elem.content {
                        ElementContent::Text(_) => unreachable!(),
                        ElementContent::Poly { .. } => unreachable!(),
                        ElementContent::Children(mut kids) => {
                            if self.config.show_close_tab_button_in_tabs {
                                kids.push(make_x_button(&font, &metrics, &colors, tab_idx, active));
                            }
                            ElementContent::Children(kids)
                        }
                    };
                    left_eles.push(elem);
                }
                _ => left_eles.push(item_to_elem(item)),
            }
        }

        let mut children = vec![];

        if !left_status.is_empty() {
            children.push(
                Element::new(&font, ElementContent::Children(left_status))
                    .colors(bar_colors.clone()),
            );
        }

        let window_buttons_at_left = self
            .config
            .window_decorations
            .contains(window::WindowDecorations::INTEGRATED_BUTTONS)
            && (self.config.integrated_title_button_alignment
                == IntegratedTitleButtonAlignment::Left
                || self.config.integrated_title_button_style
                    == IntegratedTitleButtonStyle::MacOsNative);

        let left_padding = if window_buttons_at_left {
            if self.config.integrated_title_button_style == IntegratedTitleButtonStyle::MacOsNative
            {
                if !self.window_state.contains(window::WindowState::FULL_SCREEN) {
                    Dimension::Pixels(70.0)
                } else {
                    Dimension::Cells(0.5)
                }
            } else {
                Dimension::Pixels(0.0)
            }
        } else {
            Dimension::Cells(0.5)
        };

        children.push(
            Element::new(&font, ElementContent::Children(left_eles))
                .vertical_align(tab_vertical_alignment)
                .colors(bar_colors.clone())
                .padding(BoxDimension {
                    left: left_padding,
                    right: Dimension::Cells(0.),
                    top: Dimension::Cells(0.),
                    bottom: Dimension::Cells(0.),
                })
                .zindex(1),
        );
        children.push(
            Element::new(&font, ElementContent::Children(right_eles))
                .colors(bar_colors.clone())
                .float(Float::Right),
        );

        let content = ElementContent::Children(children);

        let tabs = Element::new(&font, content)
            .display(DisplayType::Block)
            .item_type(UIItemType::TabBar(TabBarItem::None))
            .min_width(Some(Dimension::Pixels(self.dimensions.pixel_width as f32)))
            .min_height(Some(Dimension::Pixels(tab_bar_height)))
            .vertical_align(tab_vertical_alignment)
            .colors(bar_colors);

        let border = self.get_os_border();

        let mut computed = self.compute_element(
            &LayoutContext {
                height: DimensionContext {
                    dpi: self.dimensions.dpi as f32,
                    pixel_max: self.dimensions.pixel_height as f32,
                    pixel_cell: metrics.cell_size.height as f32,
                },
                width: DimensionContext {
                    dpi: self.dimensions.dpi as f32,
                    pixel_max: self.dimensions.pixel_width as f32,
                    pixel_cell: metrics.cell_size.width as f32,
                },
                bounds: euclid::rect(
                    border.left.get() as f32,
                    0.,
                    self.dimensions.pixel_width as f32 - (border.left + border.right).get() as f32,
                    tab_bar_height,
                ),
                metrics: &metrics,
                gl_state: self.render_state.as_ref().unwrap(),
                zindex: 10,
            },
            &tabs,
        )?;

        computed.translate(euclid::vec2(
            0.,
            if self.config.tab_bar_at_bottom {
                self.dimensions.pixel_height as f32
                    - (computed.bounds.height() + border.bottom.get() as f32)
            } else {
                border.top.get() as f32
            },
        ));

        self.pult_anim.set(pult_anim.get());
        Ok(computed)
    }

    pub fn paint_fancy_tab_bar(&self) -> anyhow::Result<Vec<UIItem>> {
        let computed = self.fancy_tab_bar.as_ref().ok_or_else(|| {
            anyhow::anyhow!("paint_fancy_tab_bar called but fancy_tab_bar is None")
        })?;
        let ui_items = computed.ui_items();

        let gl_state = self.render_state.as_ref().unwrap();
        self.render_element(&computed, gl_state, None)?;

        Ok(ui_items)
    }
}

/// pult: shared clock of the tab title animations. Frames fall on this grid, so they stay even when the tab bar
/// is also redrawn for other reasons.
static PULT_EPOCH: LazyLock<Instant> = LazyLock::new(Instant::now);
const PULT_SPIN: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const PULT_SPIN_FRAME: Duration = Duration::from_millis(100);
const PULT_PULSE_FRAME: Duration = Duration::from_millis(33);
const PULT_PULSE_PERIOD: f32 = 1.6;
/// The title of a pulsing tab is `A \u{e000} B`: A and B are the same label in the colours of the two ends of the pulse.
const PULT_PULSE_SEP: &str = "\u{e000}";

/// pult: time of the next frame on the grid measured from `PULT_EPOCH`.
pub(crate) fn pult_next_frame(interval: Duration) -> Instant {
    let iv = interval.as_nanos();
    let next = PULT_EPOCH.elapsed().as_nanos() / iv + 1;
    *PULT_EPOCH + Duration::from_nanos((next * iv) as u64)
}

fn pult_need(anim: &std::cell::Cell<Option<Duration>>, iv: Duration) {
    anim.set(Some(anim.get().map_or(iv, |cur| cur.min(iv))));
}

fn pult_lerp_color(
    a: ColorAttribute,
    b: ColorAttribute,
    w: f32,
    resolve: impl Fn(ColorAttribute) -> SrgbaTuple,
) -> ColorAttribute {
    if a == b || a == ColorAttribute::Default || b == ColorAttribute::Default {
        return a;
    }
    let (x, y) = (resolve(a), resolve(b));
    let l = |p: f32, q: f32| p + (q - p) * w;
    ColorAttribute::TrueColorWithDefaultFallback(SrgbaTuple(
        l(x.0, y.0),
        l(x.1, y.1),
        l(x.2, y.2),
        l(x.3, y.3),
    ))
}

/// pult: tab title animation without Lua, on every frame:
/// - spinner: the braille character in the title is replaced by the current frame (10 per second);
/// - pulse: for an `A \u{e000} B` title, a per-cell blend between the colours of A and B (sine, 1.6 s, ~30 per second).
fn pult_animate(
    title: &Line,
    palette: &ColorPalette,
    anim: &std::cell::Cell<Option<Duration>>,
    slow: bool,
) -> Line {
    let mut line = title.clone();
    let sep = line
        .visible_cells()
        .find(|c| c.str() == PULT_PULSE_SEP)
        .map(|c| c.cell_index());
    if let Some(i) = sep {
        let mut b = line.split_off(i, SEQ_ZERO).split_off(1, SEQ_ZERO);
        if b.len() == line.len() {
            let t = PULT_EPOCH.elapsed().as_secs_f32() % PULT_PULSE_PERIOD / PULT_PULSE_PERIOD;
            let w = 0.5 - 0.5 * (t * std::f32::consts::TAU).cos();
            for (ca, cb) in line.cells_mut().iter_mut().zip(b.cells_mut().iter()) {
                let fg = pult_lerp_color(ca.attrs().foreground(), cb.attrs().foreground(), w, |c| {
                    palette.resolve_fg(c)
                });
                let bg = pult_lerp_color(ca.attrs().background(), cb.attrs().background(), w, |c| {
                    palette.resolve_bg(c)
                });
                ca.attrs_mut().set_foreground(fg).set_background(bg);
            }
            pult_need(anim, if slow { PULT_PULSE_FRAME * 2 } else { PULT_PULSE_FRAME });
        }
    }
    let frame = (PULT_EPOCH.elapsed().as_millis() / PULT_SPIN_FRAME.as_millis()) as usize;
    let glyph = PULT_SPIN[frame % PULT_SPIN.len()];
    for c in line.cells_mut().iter_mut() {
        let mut chars = c.str().chars();
        if let (Some(ch), None) = (chars.next(), chars.next()) {
            if ('\u{2801}'..='\u{28ff}').contains(&ch) {
                *c = Cell::new(glyph, c.attrs().clone());
                pult_need(anim, PULT_SPIN_FRAME);
            }
        }
    }
    line
}

/// pult: on a status change (different background or stripe colour) the tab does not jump to the new colour but
/// fades into it over `PULT_FADE`. The change is detected from the first cell of the unanimated title (the `▎`
/// stripe: foreground = stripe, background = tab), so the pulse does not count as a change.
const PULT_FADE: Duration = Duration::from_millis(220);
/// Flash (done / failed): if the title contains `PULT_FLASH_MARK`, the fade starts from a background lightened with
/// the stripe colour on the change and takes longer (a one-off signal, no repetition).
const PULT_FLASH: Duration = Duration::from_millis(900);
const PULT_FLASH_MARK: &str = "\u{e001}";
const PULT_FADE_FRAME: Duration = Duration::from_millis(16);

#[derive(Clone, Copy, Debug)]
pub struct PultFade {
    /// the target: colours (stripe, background) of the first cell of the unanimated title
    target: (SrgbaTuple, SrgbaTuple),
    /// where the fade starts (the colours visible at the moment of the change)
    from: (SrgbaTuple, SrgbaTuple),
    start: Instant,
    dur: Duration,
}

fn pult_mix(a: SrgbaTuple, b: SrgbaTuple, w: f32) -> SrgbaTuple {
    let l = |p: f32, q: f32| p + (q - p) * w;
    SrgbaTuple(l(a.0, b.0), l(a.1, b.1), l(a.2, b.2), l(a.3, b.3))
}

fn pult_fade(
    state: &RefCell<HashMap<usize, PultFade>>,
    tab_idx: usize,
    title: &Line,
    mut line: Line,
    palette: &ColorPalette,
    anim: &std::cell::Cell<Option<Duration>>,
) -> Line {
    // the flash marker is always removed (stock WezTerm does not get it, and has no use for it)
    let mark = line.visible_cells().find(|c| c.str() == PULT_FLASH_MARK).map(|c| c.cell_index());
    if let Some(i) = mark {
        let rest = line.split_off(i, SEQ_ZERO).split_off(1, SEQ_ZERO);
        line.append_line(rest, SEQ_ZERO);
    }
    let Some(cell) = title.get_cell(0) else { return line };
    if cell.str() != "\u{258e}" {
        return line;
    }
    let target = (palette.resolve_fg(cell.attrs().foreground()), palette.resolve_bg(cell.attrs().background()));
    let now = Instant::now();
    let ease = |f: &PultFade| {
        let t = (now - f.start).as_secs_f32() / f.dur.as_secs_f32();
        if t >= 1. { 1. } else { 1. - (1. - t) * (1. - t) * (1. - t) }
    };
    let mut map = state.borrow_mut();
    let fade = match map.get(&tab_idx).copied() {
        None => {
            map.insert(tab_idx, PultFade { target, from: target, start: now - PULT_FADE, dur: PULT_FADE });
            return line;
        }
        Some(f) if f.target != target => {
            // start from the colour currently shown (possibly halfway through a fade)
            let w = ease(&f);
            let shown = (pult_mix(f.from.0, f.target.0, w), pult_mix(f.from.1, f.target.1, w));
            let f = if mark.is_some() {
                let mut glow = target.0;
                glow.3 = target.1 .3.max(0.85);
                PultFade { target, from: (target.0, pult_mix(target.1, glow, 0.6)), start: now, dur: PULT_FLASH }
            } else {
                PultFade { target, from: shown, start: now, dur: PULT_FADE }
            };
            map.insert(tab_idx, f);
            f
        }
        Some(f) => f,
    };
    let w = ease(&fade);
    if w >= 1. {
        return line;
    }
    pult_need(anim, PULT_FADE_FRAME);
    for (i, c) in line.cells_mut().iter_mut().enumerate() {
        let bg = pult_mix(fade.from.1, palette.resolve_bg(c.attrs().background()), w);
        c.attrs_mut().set_background(ColorAttribute::TrueColorWithDefaultFallback(bg));
        if i == 0 {
            let fg = pult_mix(fade.from.0, palette.resolve_fg(c.attrs().foreground()), w);
            c.attrs_mut().set_foreground(ColorAttribute::TrueColorWithDefaultFallback(fg));
        }
    }
    line
}

/// pult: a tab title starting with `▎` carries the tab's status stripe. The character is removed and its colour
/// becomes a thick stripe on top of the tab (see `pult_accent_elem`). Stock WezTerm shows the same title with a left stripe.
fn pult_accent(title: &Line, palette: &ColorPalette) -> Option<(Line, LinearRgba)> {
    let cell = title.get_cell(0)?;
    if cell.str() != "\u{258e}" {
        return None;
    }
    let color = match cell.attrs().foreground() {
        ColorAttribute::Default => return None,
        col => palette.resolve_fg(col).to_linear(),
    };
    let mut rest = title.clone();
    Some((rest.split_off(1, SEQ_ZERO), color))
}

/// pult: a square tab with the status colour on top and a separator on the right (the inactive tab's border colour stays).
fn pult_accent_elem(mut elem: Element, color: LinearRgba) -> Element {
    elem.border = BoxDimension {
        left: Dimension::Pixels(0.),
        right: Dimension::Pixels(1.),
        top: Dimension::Points(2.25),
        bottom: Dimension::Pixels(0.),
    };
    elem.border_corners = None;
    elem.colors.border.top = color;
    if let Some(hover) = elem.hover_colors.as_mut() {
        hover.border = elem.colors.border;
    }
    elem
}

fn make_x_button(
    font: &Rc<LoadedFont>,
    metrics: &RenderMetrics,
    colors: &TabBarColors,
    tab_idx: usize,
    active: bool,
) -> Element {
    Element::new(
        &font,
        ElementContent::Poly {
            line_width: metrics.underline_height.max(2),
            poly: SizedPoly {
                poly: X_BUTTON,
                width: Dimension::Pixels(metrics.cell_size.height as f32 / 2.),
                height: Dimension::Pixels(metrics.cell_size.height as f32 / 2.),
            },
        },
    )
    // Ensure that we draw our background over the
    // top of the rest of the tab contents
    .zindex(1)
    .vertical_align(VerticalAlign::Middle)
    .float(Float::Right)
    .item_type(UIItemType::CloseTab(tab_idx))
    .hover_colors({
        let inactive_tab_hover = colors.inactive_tab_hover();
        let active_tab = colors.active_tab();

        Some(ElementColors {
            border: BorderColor::default(),
            bg: (if active {
                inactive_tab_hover.bg_color
            } else {
                active_tab.bg_color
            })
            .to_linear()
            .into(),
            text: (if active {
                inactive_tab_hover.fg_color
            } else {
                active_tab.fg_color
            })
            .to_linear()
            .into(),
        })
    })
    .padding(BoxDimension {
        left: Dimension::Cells(0.25),
        right: Dimension::Cells(0.25),
        top: Dimension::Cells(0.25),
        bottom: Dimension::Cells(0.25),
    })
    .margin(BoxDimension {
        left: Dimension::Cells(0.5),
        right: Dimension::Cells(0.),
        top: Dimension::Cells(0.),
        bottom: Dimension::Cells(0.),
    })
}
