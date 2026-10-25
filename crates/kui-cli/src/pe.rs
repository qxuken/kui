//! The Windows PE subsystem flag, for `kui pack --format exe` (ADR 0068,
//! decision 13).
//!
//! The Windows runner is released as a console program so `kui run` keeps
//! its terminal. A packed app is a GUI program unless its config says
//! `pack = { console = true }`: Windows opens a console window for a
//! console-subsystem exe started from Explorer, which a windowed app does
//! not want. The subsystem is one field of the optional header, so packing
//! flips it in place rather than shipping two runners.

// Some of what is here (a reader's conveniences, the parsing back) only
// the tests and `kui pack` itself use; it stays as the format's whole API.
#![cfg_attr(not(test), allow(dead_code))]

// `kui pack` is wired to this module in a later step; until then its API
// is only reached from the tests.

/// `IMAGE_SUBSYSTEM_WINDOWS_GUI`.
pub const GUI: u16 = 2;
/// `IMAGE_SUBSYSTEM_WINDOWS_CUI`.
pub const CONSOLE: u16 = 3;

const PE32: u16 = 0x10b;
const PE32_PLUS: u16 = 0x20b;
/// Offsets within the optional header; the same for PE32 and PE32+.
const CHECKSUM: usize = 64;
const SUBSYSTEM: usize = 68;

/// The optional header's position, after checking the file is a PE image
/// whose optional header reaches the subsystem field.
fn optional_header(exe: &[u8]) -> Result<usize, String> {
    let u16_at = |at: usize| {
        exe.get(at..at + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
    };
    if exe.get(..2) != Some(b"MZ") {
        return Err("not a Windows executable (no MZ header)".into());
    }
    let lfanew = exe
        .get(0x3c..0x40)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
        .ok_or("the executable is cut short before its PE offset")?;
    if exe.get(lfanew..lfanew.saturating_add(4)) != Some(b"PE\0\0") {
        return Err("not a PE image (no PE signature where the MZ header points)".into());
    }
    let coff = lfanew + 4;
    let opt_size = u16_at(coff + 16).ok_or("the PE header is cut short")? as usize;
    let opt = coff + 20;
    let magic = u16_at(opt).ok_or("the PE optional header is cut short")?;
    if magic != PE32 && magic != PE32_PLUS {
        return Err(format!("unknown PE optional header magic {magic:#x}"));
    }
    if opt_size < SUBSYSTEM + 2 || exe.len() < opt + SUBSYSTEM + 2 {
        return Err("the PE optional header is too short to hold a subsystem".into());
    }
    Ok(opt)
}

/// The image's subsystem: [`GUI`], [`CONSOLE`] or another Windows value.
pub fn subsystem(exe: &[u8]) -> Result<u16, String> {
    let at = optional_header(exe)? + SUBSYSTEM;
    Ok(u16::from_le_bytes([exe[at], exe[at + 1]]))
}

/// Marks the image a GUI program (`gui`) or a console one. The checksum is
/// zeroed since the file changed; Windows checks it only for drivers and
/// boot images, and 0 means "not computed".
pub fn set_subsystem(exe: &mut [u8], gui: bool) -> Result<(), String> {
    let opt = optional_header(exe)?;
    let v = if gui { GUI } else { CONSOLE };
    exe[opt + SUBSYSTEM..opt + SUBSYSTEM + 2].copy_from_slice(&v.to_le_bytes());
    exe[opt + CHECKSUM..opt + CHECKSUM + 4].fill(0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The headers of a PE32+ (or PE32) image, enough for the fields read
    /// here: a DOS stub, `PE\0\0`, a COFF header, an optional header.
    fn synthetic(magic: u16) -> Vec<u8> {
        let lfanew = 0x80usize;
        let opt_size: u16 = if magic == PE32_PLUS { 240 } else { 224 };
        let mut b = vec![0u8; lfanew + 24 + opt_size as usize + 64];
        b[..2].copy_from_slice(b"MZ");
        b[0x3c..0x40].copy_from_slice(&(lfanew as u32).to_le_bytes());
        b[lfanew..lfanew + 4].copy_from_slice(b"PE\0\0");
        let coff = lfanew + 4;
        b[coff..coff + 2].copy_from_slice(&0x8664u16.to_le_bytes()); // x86-64
        b[coff + 16..coff + 18].copy_from_slice(&opt_size.to_le_bytes());
        let opt = coff + 20;
        b[opt..opt + 2].copy_from_slice(&magic.to_le_bytes());
        b[opt + CHECKSUM..opt + CHECKSUM + 4].copy_from_slice(&0xdead_beefu32.to_le_bytes());
        b[opt + SUBSYSTEM..opt + SUBSYSTEM + 2].copy_from_slice(&CONSOLE.to_le_bytes());
        b
    }

    #[test]
    fn flips_both_ways() {
        for magic in [PE32_PLUS, PE32] {
            let mut exe = synthetic(magic);
            let before = exe.clone();
            assert_eq!(subsystem(&exe), Ok(CONSOLE));
            set_subsystem(&mut exe, true).unwrap();
            assert_eq!(subsystem(&exe), Ok(GUI));
            let opt = 0x80 + 24;
            assert_eq!(&exe[opt + CHECKSUM..opt + CHECKSUM + 4], &[0; 4]);
            // Nothing else moved.
            let changed: Vec<usize> = (0..exe.len()).filter(|&i| exe[i] != before[i]).collect();
            assert!(
                changed
                    .iter()
                    .all(|&i| (opt + CHECKSUM..opt + SUBSYSTEM + 2).contains(&i)),
                "{changed:?}"
            );
            set_subsystem(&mut exe, false).unwrap();
            assert_eq!(subsystem(&exe), Ok(CONSOLE));
        }
    }

    #[test]
    fn rejects_what_is_not_pe() {
        assert!(subsystem(b"").is_err());
        assert!(subsystem(b"\x7fELF\x02\x01\x01").is_err());
        assert!(subsystem(b"MZ").is_err());
        let mut exe = synthetic(PE32_PLUS);
        exe[0x80] = b'X';
        assert!(set_subsystem(&mut exe, true).is_err());
        let mut exe = synthetic(PE32_PLUS);
        exe[0x3c..0x40].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(subsystem(&exe).is_err());
        let mut exe = synthetic(PE32_PLUS);
        exe[0x80 + 24] = 0x07; // magic 0x207
        assert!(subsystem(&exe).is_err());
        let mut exe = synthetic(PE32_PLUS);
        exe[0x80 + 4 + 16..0x80 + 4 + 18].copy_from_slice(&40u16.to_le_bytes());
        assert!(
            subsystem(&exe).is_err(),
            "an optional header too short for the field"
        );
        let exe = synthetic(PE32_PLUS);
        assert!(subsystem(&exe[..0x80 + 24 + 60]).is_err(), "cut short");
        // Every truncation errors rather than panics.
        for cut in 0..exe.len() {
            let _ = subsystem(&exe[..cut]);
        }
    }

    #[test]
    fn real_windows_runner_if_present() {
        // A released runner dropped here by hand; skipped otherwise.
        let Ok(path) = std::env::var("KUI_TEST_WINDOWS_EXE") else {
            eprintln!("skipped: set KUI_TEST_WINDOWS_EXE to a Windows .exe");
            return;
        };
        let mut exe = std::fs::read(path).unwrap();
        let was = subsystem(&exe).unwrap();
        assert!(was == GUI || was == CONSOLE);
        set_subsystem(&mut exe, was != GUI).unwrap();
        assert_ne!(subsystem(&exe).unwrap(), was);
    }
}
