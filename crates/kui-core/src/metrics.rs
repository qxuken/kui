//! [`Metrics`]: the sizes the stock widgets are built from, one struct
//! beside the palette.
//!
//! Every field is logical px (or a text size in logical px), applied
//! before `env.scale`, which the renderer multiplies everything by.
//! Density is the app's choice, as the palette is: [`Metrics::compact`]
//! is a tighter set, [`Metrics::scaled`] multiplies every length for a
//! density slider, and `Core::set_metrics` makes one the frame's. An app
//! that never sets one gets [`Metrics::default`], the stock geometry.
//!
//! Fields are roles, like the palette's: `control_pad_x` is a button's
//! horizontal padding, not "spacing unit 3", and a view that wants a
//! number between two has arithmetic.

/// The titlebar's height on Windows and everywhere else: the caption
/// height the OS draws, so the one metric that is the platform's rather
/// than a density's. Named here so [`Metrics::comfortable`] picks the
/// running one and the schema row (`MetricRole::platform`) carries both,
/// which is what keeps a generated table from saying which machine wrote
/// it.
pub const TITLEBAR_H_WINDOWS: f32 = 32.0;
pub const TITLEBAR_H_ELSEWHERE: f32 = 34.0;

/// The sizes the stock widgets are built from. Plain data and [`Copy`]: a
/// view reads it off `ui.metrics()` and may keep or change its own copy,
/// and `Core::set_metrics` makes one the frame's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    // -- text ----------------------------------------------------------
    /// A stock control's label: the button's text size.
    pub control_text: f32,
    /// The chrome's text: a menu row, a menu-bar title, the titlebar's
    /// title.
    pub chrome_text: f32,
    /// A tooltip's text.
    pub hint_text: f32,

    // -- corners -------------------------------------------------------
    /// The corner of every stock surface: a button, a field, a menu, a
    /// tooltip.
    pub radius: f32,
    /// The corner of a row inside one: a menu row, a menu-bar title.
    pub radius_inner: f32,

    // -- padding, x then y -----------------------------------------------
    /// A button's.
    pub control_pad_x: f32,
    pub control_pad_y: f32,
    /// A text field's.
    pub field_pad_x: f32,
    pub field_pad_y: f32,
    /// A tooltip's.
    pub hint_pad_x: f32,
    pub hint_pad_y: f32,
    /// A menu row's; a menu-bar title's is two px shorter, so the bar's
    /// height and not the title's padding decides the strip.
    pub menu_pad_x: f32,
    pub menu_pad_y: f32,

    // -- extents -------------------------------------------------------
    /// A menu panel's width.
    pub menu_width: f32,
    /// The drawn menu bar's height.
    pub menu_bar_h: f32,
    /// The titlebar's height where the strip is the app's alone: the
    /// platform's caption height (32 on Windows, 34 elsewhere), which
    /// [`Metrics::compact`] leaves alone. A driver that opened the strip
    /// taller (the runner's `Launcher::titlebar` off macOS, backlog W22)
    /// makes that window's platform height its own, and the stock number
    /// stands for it in [`Core::set_metrics`](crate::runtime::Core::set_metrics).
    /// Where the OS keeps controls of
    /// its own over the strip — the macOS traffic lights under custom
    /// chrome — the strip is as tall as the OS's titlebar, which the
    /// driver measures into `env.window.native_controls`, and this row
    /// is not read (`widgets::titlebar_height`).
    pub titlebar_h: f32,
}

impl Default for Metrics {
    /// What the widgets have always drawn: the constants `widgets.rs`
    /// carried, restated once.
    fn default() -> Self {
        Self::comfortable()
    }
}

impl Metrics {
    /// The stock set — [`Default`], named.
    pub const fn comfortable() -> Self {
        Metrics {
            control_text: 15.0,
            chrome_text: 13.0,
            hint_text: 12.0,
            radius: 6.0,
            radius_inner: 4.0,
            control_pad_x: 14.0,
            control_pad_y: 8.0,
            field_pad_x: 10.0,
            field_pad_y: 8.0,
            hint_pad_x: 10.0,
            hint_pad_y: 6.0,
            menu_pad_x: 8.0,
            menu_pad_y: 5.0,
            menu_width: 200.0,
            menu_bar_h: 26.0,
            titlebar_h: if cfg!(target_os = "windows") {
                TITLEBAR_H_WINDOWS
            } else {
                TITLEBAR_H_ELSEWHERE
            },
        }
    }

    /// A tighter set for a dense tool — a mux, an inspector, a table of
    /// controls: smaller text, shallower padding, sharper corners. The
    /// titlebar keeps the platform's height, since that is the OS's
    /// number and not a density.
    pub const fn compact() -> Self {
        Metrics {
            control_text: 13.0,
            chrome_text: 12.0,
            hint_text: 11.0,
            radius: 4.0,
            radius_inner: 3.0,
            control_pad_x: 10.0,
            control_pad_y: 5.0,
            field_pad_x: 8.0,
            field_pad_y: 5.0,
            hint_pad_x: 8.0,
            hint_pad_y: 4.0,
            menu_pad_x: 8.0,
            menu_pad_y: 3.0,
            menu_width: 180.0,
            menu_bar_h: 22.0,
            ..Self::comfortable()
        }
    }

    /// Every density multiplied by `factor` — a density slider, or an OS
    /// text-size setting the day one is plumbed. Logical px in, logical
    /// px out: `env.scale` is applied after this by the renderer and is
    /// never folded in here. A row the schema marks the platform's
    /// (`titlebar_h`: the OS's caption height, which the traffic lights
    /// are drawn against) is left alone, as [`Metrics::compact`] leaves
    /// it — a 1.5 slider drew a 51 px strip beside 34 px buttons.
    pub fn scaled(self, factor: f32) -> Self {
        let mut m = self;
        for row in crate::schema::METRIC_ROLES {
            if row.platform.is_some() {
                continue;
            }
            (row.set)(&mut m, (row.get)(&self) * factor);
        }
        m
    }
}
