//! Host environment facts pushed into the core by the frame driver — the
//! inbound mirror of events-as-data. The core never touches a window; the
//! driver (runner, FFI host) reports what it knows and views read it.
//!
//! The reading a view gets — this struct, [`SystemEnv`], [`WindowEnv`], the
//! derived budget and the frame facts beside them, under each binding's
//! spelling — is written down once in `schema::ENV_FIELDS` and every
//! binding is pinned to that table; a field added here fails `schema`'s
//! tests until it has a row.
//!
//! Every fact under [`SystemEnv`] can also be *unknown*, and unknown is the
//! default. A driver that cannot ask the OS says so rather than guessing,
//! because the guess a view would make from a wrong answer (paint the dark
//! palette, skip the animation) is worse than the one it makes from a
//! missing one.

use crate::color::Color;
use crate::window::WindowEnv;

/// What the host knows about the display/window. Defaults are safe for
/// headless drivers (tests, benches) that never set anything.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Env {
    /// Display refresh rate in Hz; `None` when the host can't tell.
    pub refresh_hz: Option<f32>,
    /// Whether the window has keyboard focus.
    pub focused: bool,
    /// What the OS is set to: appearance, accent, motion, locale — and
    /// whether assistive technology is listening.
    pub system: SystemEnv,
    /// Window chrome facts (custom chrome, maximized, native control rect).
    pub window: WindowEnv,
    /// The output device's state and how many playbacks are live.
    pub audio: AudioEnv,
}

impl Default for Env {
    fn default() -> Self {
        Self {
            refresh_hz: None,
            focused: true,
            system: SystemEnv::default(),
            window: WindowEnv::default(),
            audio: AudioEnv::default(),
        }
    }
}

impl Env {
    /// Budget fallback when the host doesn't report a refresh rate.
    pub const DEFAULT_HZ: f32 = 120.0;

    /// Per-frame time budget in ms: one vsync interval at the display's
    /// refresh rate (120 Hz when unreported).
    pub fn frame_budget_ms(&self) -> f32 {
        let hz = self
            .refresh_hz
            .filter(|hz| *hz > 0.0)
            .unwrap_or(Self::DEFAULT_HZ);
        1000.0 / hz
    }
}

/// The user's OS settings, as the host reports them. Not window facts and
/// not display facts: things the person chose once, in a settings app, that
/// a view is expected to honour. The core acts on two of them in one way:
/// `appearance` and `accent` derive the theme (ADR 0019), so the stock
/// widgets and a `<text>` with no colour follow the OS — and nothing else
/// moves. Reduced motion does not shorten an animation and a dark
/// appearance repaints none of the app's own colours: the view decides,
/// because only it knows which of its colours is the background and which
/// of its animations carries meaning.
///
/// Each field defaults to "the host cannot tell", which is what a headless
/// core reports and what any driver reports for a fact its platform gives
/// it no way to ask. The `kui` runner asks the OS for all four on macOS and
/// Windows (the appearance through winit, the rest in its `system_env`);
/// elsewhere it answers what it can and leaves the rest unknown. The fifth,
/// [`Assistive`], is not a setting but a fact of the same shape — the
/// user chose to run a screen reader — and comes from the accessibility
/// bridge rather than a settings query.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SystemEnv {
    /// Light or dark, when the host can tell.
    pub appearance: Appearance,
    /// The OS accent/highlight colour, `None` when the host can't tell.
    pub accent: Option<Color>,
    /// Whether the user asked for reduced motion.
    pub motion: MotionPref,
    /// The UI language as a BCP-47 tag, `None` when the host can't tell.
    pub locale: Option<Locale>,
    /// Whether assistive technology has asked for the access tree.
    pub assistive: Assistive,
}

impl SystemEnv {
    /// `self` laid over `base`: every field `self` knows wins, every field
    /// it left at "cannot tell" is `base`'s. The merge behind a launcher's
    /// pinned reading (`kui::Launcher::system`, Node's `runWindowed(..,
    /// {system})`, the context a C host hands `kui_run_with`): the app's
    /// partial over what the OS answered, applied every frame where the
    /// runner writes the real reading — so a pinned `motion` survives the
    /// write, and a real change to the accent still arrives, because that
    /// field was left unknown here and `base` is the OS's (backlog F47).
    ///
    /// Unknown *means* not pinned, which is why there is no separate
    /// override type: the four "cannot tell" readings are the defaults, so
    /// `SystemEnv { motion: MotionPref::Reduced, ..Default::default() }` is
    /// the whole of "as if this user asked for less motion". What it
    /// cannot say is "pin this to unknown" — a window on a platform that
    /// answers has no test that needs it.
    pub fn over(self, base: SystemEnv) -> SystemEnv {
        SystemEnv {
            appearance: match self.appearance {
                Appearance::Unknown => base.appearance,
                pinned => pinned,
            },
            accent: self.accent.or(base.accent),
            motion: match self.motion {
                MotionPref::Unknown => base.motion,
                pinned => pinned,
            },
            locale: self.locale.or(base.locale),
            assistive: match self.assistive {
                Assistive::Unknown => base.assistive,
                pinned => pinned,
            },
        }
    }
}

/// The OS light/dark setting. `Unknown` is a real answer — a host with no
/// way to ask says it, and a view that has one palette per appearance picks
/// its own default for it rather than being handed a guess.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Appearance {
    #[default]
    Unknown,
    Light,
    Dark,
}

impl Appearance {
    /// Wire order: the index is the code C passes and the position in
    /// `schema::APPEARANCES`, so `unknown` is 0 and a zeroed C host means
    /// what it says.
    pub const ALL: &'static [Appearance] =
        &[Appearance::Unknown, Appearance::Light, Appearance::Dark];

    pub fn name(self) -> &'static str {
        match self {
            Appearance::Unknown => "unknown",
            Appearance::Light => "light",
            Appearance::Dark => "dark",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|a| a.name() == name)
    }

    /// The code a C host passes; `unknown` is 0.
    pub fn code(self) -> u32 {
        Self::ALL.iter().position(|a| *a == self).unwrap() as u32
    }

    /// A code past the end is `None` — ignored rather than folded onto a
    /// real appearance, the way an unknown role code is.
    pub fn from_code(code: u32) -> Option<Self> {
        Self::ALL.get(code as usize).copied()
    }
}

/// The OS reduce-motion setting: `Reduced` is "the user asked for less
/// animation", `Full` is "the user did not", `Unknown` is "nobody asked the
/// OS". Spelled as what the user wants rather than as a `reduce_motion`
/// boolean because the third reading has no place in a boolean, and a
/// missing answer is not the same as a "no".
///
/// `Pref` because `edit` already means cosmic-text's caret `Motion` by that
/// name, and a crate with two of them would be one letter of ambiguity in
/// every use. The field is `system.motion` and every binding spells it
/// `motion`; only Rust sees the longer type name.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MotionPref {
    #[default]
    Unknown,
    Full,
    Reduced,
}

impl MotionPref {
    /// Wire order, as [`Appearance::ALL`].
    pub const ALL: &'static [MotionPref] =
        &[MotionPref::Unknown, MotionPref::Full, MotionPref::Reduced];

    pub fn name(self) -> &'static str {
        match self {
            MotionPref::Unknown => "unknown",
            MotionPref::Full => "full",
            MotionPref::Reduced => "reduced",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|m| m.name() == name)
    }

    /// The code a C host passes; `unknown` is 0.
    pub fn code(self) -> u32 {
        Self::ALL.iter().position(|m| *m == self).unwrap() as u32
    }

    pub fn from_code(code: u32) -> Option<Self> {
        Self::ALL.get(code as usize).copied()
    }

    /// Whether a view should skip or shorten decorative motion. `Unknown`
    /// answers `false`: a host that cannot tell gets the animations it
    /// would have had before this field existed.
    pub fn is_reduced(self) -> bool {
        self == MotionPref::Reduced
    }
}

/// Whether assistive technology is listening: the difference between an
/// alert that blinks and one that announces (backlog F48). `Listening` is
/// "an accessibility client has asked this window for its tree", which is
/// the one signal the platform adapters give and the moment the runner
/// starts deriving trees (ADR 0016 measures its cache from there).
/// `None` is "the bridge is up and nobody has asked"; `Unknown` is "there
/// is no bridge" — a headless core, a driver built without the
/// `accesskit` feature, a C host that never called the setter.
///
/// Two limits are the reading's, not the row's. *Any* client counts: a
/// probe, an accessibility inspector, a test harness driving the AX API
/// and VoiceOver alike all ask for the tree, and nothing tells them apart.
/// And whether it ever falls back to `None` is the platform's: only the
/// AT-SPI adapter (Unix) reports deactivation, when the session's
/// accessibility bus goes away; on macOS and Windows the adapters never
/// call the deactivation handler, so once a client has asked the reading
/// stays `Listening` for the window's life.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Assistive {
    #[default]
    Unknown,
    None,
    Listening,
}

impl Assistive {
    /// Wire order, as [`Appearance::ALL`]: `unknown` is 0.
    pub const ALL: &'static [Assistive] =
        &[Assistive::Unknown, Assistive::None, Assistive::Listening];

    pub fn name(self) -> &'static str {
        match self {
            Assistive::Unknown => "unknown",
            Assistive::None => "none",
            Assistive::Listening => "listening",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|a| a.name() == name)
    }

    /// The code a C host passes; `unknown` is 0.
    pub fn code(self) -> u32 {
        Self::ALL.iter().position(|a| *a == self).unwrap() as u32
    }

    pub fn from_code(code: u32) -> Option<Self> {
        Self::ALL.get(code as usize).copied()
    }

    /// Whether something is listening. `Unknown` answers `false`, as
    /// [`MotionPref::is_reduced`] does: a host that cannot tell gets the
    /// blink it would have had before this field existed.
    pub fn is_listening(self) -> bool {
        self == Assistive::Listening
    }
}

/// What the driver's audio output is doing, for views to read. A fact,
/// not a verb: nothing here lets a view close the device, which stays the
/// driver's decision (it closes an idle one itself, after a while).
///
/// Worth a row because an open output stream is a real-time thread that
/// runs whether or not anything plays — ~94 buffer callbacks a second at
/// the usual period — which is the whole of an idle app's CPU once a
/// session has held a sound. A view that shows `device` still `Open` ten
/// seconds after its last click is showing a bug that otherwise only
/// `top` can see. Headless drivers leave it at the default, which is the
/// truth for them: no device, nothing playing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AudioEnv {
    /// Whether the output device is open, and so costing something.
    pub device: AudioDevice,
    /// Playbacks started and not yet ended, plus the ones waiting on the
    /// device to open.
    pub live: u32,
}

/// The output device's state. `Closed` is the default and what a headless
/// driver reports; `Opening` is the ~90 ms the open takes on its own
/// thread; `Failed` is a device that refused to open, after which commands
/// are dropped.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AudioDevice {
    #[default]
    Closed,
    Opening,
    Open,
    Failed,
}

impl AudioDevice {
    /// Wire order, as [`Appearance::ALL`]: `closed` is 0, so a zeroed C
    /// call means what it says.
    pub const ALL: &'static [AudioDevice] = &[
        AudioDevice::Closed,
        AudioDevice::Opening,
        AudioDevice::Open,
        AudioDevice::Failed,
    ];

    pub fn name(self) -> &'static str {
        match self {
            AudioDevice::Closed => "closed",
            AudioDevice::Opening => "opening",
            AudioDevice::Open => "open",
            AudioDevice::Failed => "failed",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|d| d.name() == name)
    }

    /// The code a C host passes; `closed` is 0.
    pub fn code(self) -> u32 {
        Self::ALL.iter().position(|d| *d == self).unwrap() as u32
    }

    pub fn from_code(code: u32) -> Option<Self> {
        Self::ALL.get(code as usize).copied()
    }
}

/// A language tag as the host reports it — `"en"`, `"en-US"`,
/// `"zh-Hans-CN"`. Carried inline rather than as a `String` so [`Env`]
/// stays `Copy`: a view reads `ui.env()` every frame, and a tag that
/// allocated would allocate on every one of them.
///
/// The core does not parse it. It is passed through to the view, which
/// hands it to whatever formats dates and numbers — kui has no opinion
/// about what is a language and what is a region.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Locale {
    /// ASCII, zero-padded past `len` so the derived `Eq` compares tags and
    /// not whatever was in the tail.
    bytes: [u8; Locale::CAP],
    len: u8,
}

impl Locale {
    /// Longest tag that fits. RFC 5646 allows longer in principle; 31
    /// holds every tag anyone ships, including the script-and-region ones
    /// (`zh-Hant-HK`) and a private-use suffix.
    pub const CAP: usize = 31;

    /// `None` for a tag that is empty, longer than [`Locale::CAP`], or not
    /// ASCII — the three things a well-formed language tag is not. A host
    /// that hands one of those over reads back "the host cannot tell",
    /// which is true: what it said was not a tag.
    pub fn new(tag: &str) -> Option<Self> {
        if tag.is_empty() || tag.len() > Self::CAP || !tag.is_ascii() {
            return None;
        }
        let mut bytes = [0u8; Self::CAP];
        bytes[..tag.len()].copy_from_slice(tag.as_bytes());
        Some(Self {
            bytes,
            len: tag.len() as u8,
        })
    }

    pub fn as_str(&self) -> &str {
        // ASCII by construction in `new`, the only constructor.
        std::str::from_utf8(&self.bytes[..self.len as usize]).unwrap_or("")
    }
}

impl std::fmt::Display for Locale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::fmt::Debug for Locale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The tag, not 31 bytes of padding.
        write!(f, "Locale({:?})", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_follows_reported_rate() {
        let env = Env {
            refresh_hz: Some(60.0),
            ..Default::default()
        };
        assert!((env.frame_budget_ms() - 16.666).abs() < 1e-2);
    }

    #[test]
    fn budget_falls_back_when_unreported_or_bogus() {
        assert!((Env::default().frame_budget_ms() - 8.333).abs() < 1e-2);
        let bogus = Env {
            refresh_hz: Some(0.0),
            ..Default::default()
        };
        assert!((bogus.frame_budget_ms() - 8.333).abs() < 1e-2);
    }

    /// Nobody asked the OS anything, and the reading says exactly that
    /// rather than "light, unreduced, English".
    #[test]
    fn the_system_facts_default_to_unknown() {
        let s = Env::default().system;
        assert_eq!(s.appearance, Appearance::Unknown);
        assert_eq!(s.accent, None);
        assert_eq!(s.motion, MotionPref::Unknown);
        assert_eq!(s.locale, None);
        assert_eq!(s.assistive, Assistive::Unknown);
        assert!(!s.motion.is_reduced(), "unknown is not a request to reduce");
        assert!(!s.assistive.is_listening(), "unknown is not a listener");
        // And a headless driver holds no device: closed, nothing live.
        assert_eq!(Env::default().audio, AudioEnv::default());
        assert_eq!(AudioEnv::default().device, AudioDevice::Closed);
    }

    /// The schema's name lists are the wire order: an index means the same
    /// setting in every binding, so the two cannot drift. Zero is
    /// `unknown` in both, which is what makes a zeroed C call honest.
    #[test]
    fn schema_names_are_all_in_order() {
        let appearances: Vec<&str> = Appearance::ALL.iter().map(|a| a.name()).collect();
        assert_eq!(appearances, crate::schema::APPEARANCES);
        let motions: Vec<&str> = MotionPref::ALL.iter().map(|m| m.name()).collect();
        assert_eq!(motions, crate::schema::MOTIONS);
        let devices: Vec<&str> = AudioDevice::ALL.iter().map(|d| d.name()).collect();
        assert_eq!(devices, crate::schema::AUDIO_DEVICES);
        let assistive: Vec<&str> = Assistive::ALL.iter().map(|a| a.name()).collect();
        assert_eq!(assistive, crate::schema::ASSISTIVE);
        assert_eq!(Appearance::default().code(), 0);
        assert_eq!(MotionPref::default().code(), 0);
        assert_eq!(AudioDevice::default().code(), 0);
        assert_eq!(Assistive::default().code(), 0);
    }

    #[test]
    fn codes_and_names_round_trip_and_reject_the_rest() {
        for a in Appearance::ALL {
            assert_eq!(Appearance::from_code(a.code()), Some(*a));
            assert_eq!(Appearance::parse(a.name()), Some(*a));
        }
        for m in MotionPref::ALL {
            assert_eq!(MotionPref::from_code(m.code()), Some(*m));
            assert_eq!(MotionPref::parse(m.name()), Some(*m));
        }
        for d in AudioDevice::ALL {
            assert_eq!(AudioDevice::from_code(d.code()), Some(*d));
            assert_eq!(AudioDevice::parse(d.name()), Some(*d));
        }
        for a in Assistive::ALL {
            assert_eq!(Assistive::from_code(a.code()), Some(*a));
            assert_eq!(Assistive::parse(a.name()), Some(*a));
        }
        assert_eq!(Appearance::from_code(3), None);
        assert_eq!(MotionPref::from_code(3), None);
        assert_eq!(AudioDevice::from_code(4), None);
        assert_eq!(Assistive::from_code(3), None);
        assert_eq!(Appearance::parse("Dark"), None, "spelling is exact");
    }

    /// A tag is carried whole and compares as a tag; the three things that
    /// are not a tag read back as "the host cannot tell".
    #[test]
    fn a_locale_is_the_tag_it_was_given() {
        let tag = Locale::new("en-US").unwrap();
        assert_eq!(tag.as_str(), "en-US");
        assert_eq!(tag.to_string(), "en-US");
        assert_eq!(Locale::new("zh-Hant-HK").unwrap().as_str(), "zh-Hant-HK");
        // Two tags of different lengths cannot compare equal through the
        // padding, and one built twice is the same tag.
        assert_eq!(Locale::new("en"), Locale::new("en"));
        assert_ne!(Locale::new("en"), Locale::new("en-US"));

        assert_eq!(Locale::new(""), None);
        assert_eq!(Locale::new(&"x".repeat(Locale::CAP + 1)), None);
        assert_eq!(Locale::new("ру-RU"), None, "a tag is ASCII");
        assert!(Locale::new(&"x".repeat(Locale::CAP)).is_some(), "CAP fits");
    }

    /// `Env` is `Copy` and small enough to read every frame — the reason
    /// a locale is an inline tag and not a `String`.
    #[test]
    fn env_is_copy() {
        fn takes_copy<T: Copy>(_: T) {}
        takes_copy(Env::default());
    }
}
