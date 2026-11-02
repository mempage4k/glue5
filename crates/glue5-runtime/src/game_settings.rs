//! The engine's graphics settings: `settings.xml` in the user directory,
//! which its pause menu edits and saves (Settings › Graphics). Changes apply
//! at once, or, for those that need it, at a restart the engine asks for
//! (see [`crate::exit`]).
//!
//! The host never overrides them on the command line, which would undo the
//! menu's choices at every start: it only writes the low-memory preset over
//! them, once, and puts them back to the engine's defaults when asked.

use std::io;
use std::path::{Path, PathBuf};

fn path(user_dir: &Path) -> PathBuf {
    user_dir.join("Documents/Rockstar Games/GTA V/settings.xml")
}

/// The low-memory preset (the browser build's `?low=1`): the lowest texture,
/// reflection, particle and grass quality, shadows at the lowest level,
/// minimum ped and vehicle variety, city density and level of detail.
/// Measured in a Node run at the world: GPU memory 1.16 GB -> 0.5 GB,
/// streamed assets 1.38 GB -> 0.95 GB.
const LOW: &[(&str, &str)] = &[
    ("TextureQuality", "0"),
    ("ShadowQuality", "1"),
    ("ReflectionQuality", "0"),
    ("ParticleQuality", "0"),
    ("GrassQuality", "0"),
    ("CityDensity", "0.000000"),
    ("PedVarietyMultiplier", "0.000000"),
    ("VehicleVarietyMultiplier", "0.000000"),
    ("LodScale", "0.000000"),
    ("PedLodBias", "0.000000"),
    ("VehicleLodBias", "0.000000"),
];

/// What the engine writes when it first starts without settings (its
/// defaults for the adapter it is given, the same everywhere).
const DEFAULTS: &str = include_str!("settings.xml");

/// The phone preset: the same world, with the parts that cost the most
/// bandwidth and fill rate turned down.
///
/// A phone's GPU (a Mali-G78 on the Galaxy S21+) is fed by a phone's
/// memory: the texture quality is what it feels most (the bigger textures
/// are read again and again, from memory the phone shares with everything
/// else), the anisotropic filtering reads each texel several times, and the
/// shadows, the reflections, the water and the grass are what fill the
/// screen. What is left is the same city, the same cars and the same
/// people.
const PHONE: &[(&str, &str)] = &[
    ("TextureQuality", "1"),
    ("ShadowQuality", "1"),
    ("Shadow_SoftShadows", "false"),
    ("Shadow_LongShadows", "false"),
    ("Shadow_ParticleShadows", "false"),
    ("Shadow_Distance", "0.500000"),
    ("ReflectionQuality", "0"),
    ("ReflectionMSAA", "0"),
    ("SSAO", "0"),
    ("AnisotropicFiltering", "0"),
    ("MSAA", "0"),
    ("MSAAFragments", "0"),
    ("MSAAQuality", "0"),
    ("WaterQuality", "0"),
    ("ParticleQuality", "0"),
    ("GrassQuality", "0"),
    ("ShaderQuality", "1"),
    ("PostFX", "0"),
    ("DoF", "false"),
    ("MotionBlurStrength", "0.000000"),
];

/// Writes the low-memory preset over the current settings (or the engine's
/// defaults, before its first start).
pub fn apply_low_preset(user_dir: &Path) -> io::Result<()> {
    apply(user_dir, LOW)
}

/// Writes the phone preset over the current settings (or the engine's
/// defaults, before its first start).
pub fn apply_phone_preset(user_dir: &Path) -> io::Result<()> {
    apply(user_dir, PHONE)
}

/// Writes `settings` (name, value) over the current settings (or the
/// engine's defaults, before its first start). Names the file lacks are
/// skipped, with a warning.
pub fn apply<N: AsRef<str>, V: AsRef<str>>(user_dir: &Path, settings: &[(N, V)]) -> io::Result<()> {
    let path = path(user_dir);
    let mut xml = std::fs::read_to_string(&path).unwrap_or_else(|_| DEFAULTS.to_string());
    for (name, value) in settings {
        let name = name.as_ref();
        if !xml.contains(&format!("<{name} value=\"")) {
            log::warn!("game settings: no {name} in settings.xml");
        }
        xml = set(&xml, name, value.as_ref());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, xml)
}

/// Puts the graphics settings back to the engine's defaults, keeping the
/// file as it was in `settings.xml.bak` (the rest of it, and the saves,
/// stay): the way out of a setting the engine cannot run with, which stops
/// it before its menu can change it back.
pub fn reset_graphics(user_dir: &Path) -> io::Result<()> {
    let path = path(user_dir);
    let Ok(xml) = std::fs::read_to_string(&path) else {
        return Ok(()); // none: the engine starts from its defaults
    };
    std::fs::write(path.with_extension("xml.bak"), &xml)?;
    std::fs::write(&path, with_graphics(&xml, DEFAULTS))
}

/// `xml` with the `<graphics>` section of `from` (all of `from` if either
/// lacks one).
fn with_graphics(xml: &str, from: &str) -> String {
    const END: &str = "</graphics>";
    let section = |s: &str| Some(s.find("<graphics>")?..s.find(END)? + END.len());
    match (section(xml), section(from)) {
        (Some(mine), Some(theirs)) => format!("{}{}{}", &xml[..mine.start], &from[theirs], &xml[mine.end..]),
        _ => from.to_string(),
    }
}

/// `xml` with `<name value="..." />` set to `value` (unchanged if absent).
fn set(xml: &str, name: &str, value: &str) -> String {
    let tag = format!("<{name} value=\"");
    let Some(start) = xml.find(&tag).map(|i| i + tag.len()) else { return xml.to_string() };
    let Some(len) = xml[start..].find('"') else { return xml.to_string() };
    format!("{}{value}{}", &xml[..start], &xml[start + len..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_values_by_exact_name() {
        let xml = "<LodScale value=\"1.000000\" />\n<MaxLodScale value=\"1.000000\" />";
        assert_eq!(
            set(xml, "LodScale", "0.000000"),
            "<LodScale value=\"0.000000\" />\n<MaxLodScale value=\"1.000000\" />"
        );
        assert_eq!(set(xml, "Missing", "0"), xml);
    }

    #[test]
    fn graphics_only_go_back_to_the_defaults() {
        let xml = "<Settings>\n  <graphics>\n    <MSAA value=\"2\" />\n  </graphics>\n  <video>\n    <VSync value=\"0\" />\n  </video>\n</Settings>";
        let reset = with_graphics(xml, DEFAULTS);
        assert!(reset.contains("<MSAA value=\"0\" />"));
        assert!(reset.contains("<VSync value=\"0\" />\n  </video>\n</Settings>"));
        assert_eq!(reset.matches("<graphics>").count(), 1);
        assert_eq!(with_graphics("broken", DEFAULTS), DEFAULTS);
    }

    #[test]
    fn preset_names_are_in_the_defaults() {
        for (name, _) in LOW {
            assert!(DEFAULTS.contains(&format!("<{name} value=\"")), "{name}");
        }
        for (name, _) in PHONE {
            assert!(DEFAULTS.contains(&format!("<{name} value=\"")), "{name}");
        }
    }
}
