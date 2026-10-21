//! Game files changed on their way to the engine, where its code is a later
//! revision than the game's data.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

type Patch = fn(String) -> Option<String>;

/// The files (under the game's `data/`), whether this run changes them, and
/// how.
const OVERLAYS: [(&str, fn() -> bool, Patch); 8] = [
    ("common/data/gameconfig.xml", game_config_changed, game_config),
    ("common/data/ui/pausemenu.xml", always, autosave_first),
    ("common/data/ui/frontend.xml", always, feed_width),
    // The pads' layouts, then the mouse's mappings.
    ("common/data/control/standard.meta", always, radio_wheel_axes),
    ("common/data/control/southpaw.meta", always, radio_wheel_axes),
    ("common/data/control/trigger_swap.meta", always, radio_wheel_axes),
    ("common/data/control/southpaw_trigger_swap.meta", always, radio_wheel_axes),
    ("x64/data/control/fixed.meta", always, mouse_axes),
];

/// The changed files' paths under `data/`.
pub(crate) fn names() -> impl Iterator<Item = &'static str> {
    OVERLAYS.iter().map(|(name, _, _)| *name)
}

fn online() -> bool {
    crate::rt().config.online.is_some()
}

/// Script stacks added to the MICRO ones (20), for mods' script threads.
static MORE_SCRIPT_STACKS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Has gameconfig.xml offer `n` more of the smallest script stacks (0..=79),
/// for threads of the host's (glue5-mods). Before the game starts.
pub fn more_script_stacks(n: u32) {
    MORE_SCRIPT_STACKS.store(n.min(79), std::sync::atomic::Ordering::Relaxed);
}

fn game_config_changed() -> bool {
    online() || MORE_SCRIPT_STACKS.load(std::sync::atomic::Ordering::Relaxed) > 0
}

/// gameconfig.xml: GTA Online's broadcast data; more MICRO script stacks.
fn game_config(text: String) -> Option<String> {
    let text = if online() { crate::online::more_broadcast_data(text)? } else { text };
    match MORE_SCRIPT_STACKS.load(std::sync::atomic::Ordering::Relaxed) {
        0 => Some(text),
        n => micro_stacks(text, 20 + n),
    }
}

/// The MICRO stacks' count, two digits for two (the same length).
fn micro_stacks(text: String, count: u32) -> Option<String> {
    const COUNT: &str = "<NumberOfStacksOfThisSize value=\"20\"/>";
    let at = text.find("<StackName>MICRO</StackName>")?;
    let field = at + text[at..].find(COUNT)?;
    // Within MICRO's item.
    if text[at..field].contains("</Item>") || !(20..=99).contains(&count) {
        return None;
    }
    let mut text = text;
    text.replace_range(field..field + COUNT.len(), &format!("<NumberOfStacksOfThisSize value=\"{count}\"/>"));
    Some(text)
}

fn always() -> bool {
    true
}

/// Where the file at `rel` (under the game's `data/`) is changed, the changed
/// copy (made once, in `scratch`). Same length as the original: the engine
/// may size its reads from the game's manifest.
pub(crate) fn game_file(rel: &str, real: &Path, scratch: &Path) -> Option<PathBuf> {
    static MADE: Mutex<Vec<(&str, Option<PathBuf>)>> = Mutex::new(Vec::new());
    let (name, _, patch) = OVERLAYS.iter().find(|(name, applies, _)| rel.eq_ignore_ascii_case(name) && applies())?;
    let mut made = MADE.lock().unwrap();
    if let Some((_, path)) = made.iter().find(|(n, _)| n == name) {
        return path.clone();
    }
    let path = (|| {
        let text = std::fs::read_to_string(real).ok()?;
        let patched = patch(text.clone()).filter(|p| p.len() == text.len())?;
        let path = scratch.join("overlay").join(real.file_name()?);
        std::fs::create_dir_all(path.parent()?).ok()?;
        std::fs::write(&path, patched).ok()?;
        log::info!("overlay: {rel} changed");
        Some(path)
    })();
    made.push((name, path.clone()));
    path
}

/// pausemenu.xml, "Saving And Startup": the engine greys "Startup Flow" out
/// while the landing page preference is on, which later data has an item
/// for; this data has none, so it stays on. With its first item greyed out,
/// the list sends left and right to that item whichever one is highlighted,
/// and "Autosave" could not be turned off. Autosave goes first.
fn autosave_first(text: String) -> Option<String> {
    let line = |pref: &str| {
        let at = text.find(&format!("<MenuPref>{pref}</MenuPref>"))?;
        let start = text[..at].rfind('\n')? + 1;
        Some(start..at + text[at..].find('\n')? + 1)
    };
    let (flow, autosave) = (line("PREF_STARTUP_FLOW")?, line("PREF_AUTOSAVE")?);
    (flow.end == autosave.start)
        .then(|| format!("{}{}{}{}", &text[..flow.start], &text[autosave.clone()], &text[flow], &text[autosave.end..]))
}

/// frontend.xml, the feed (GameStream): the engine stretches its movie to the
/// rectangle given here (exact fit). The movie is drawn for the minimap's
/// width (0.150, the PC's value), but this data gives 0.1989, so the
/// notifications came out a third wider, portraits no longer square. Same
/// length: zeros pad it.
fn feed_width(text: String) -> Option<String> {
    let start = text.find("<GameStream>")?;
    let end = start + text[start..].find("</GameStream>")?;
    let block = &text[start..end];
    let (old, new) = ("sizeX=\"0.19890859375\"", "sizeX=\"0.15000000000\"");
    block.contains(old).then(|| format!("{}{}{}", &text[..start], block.replace(old, new), &text[end..]))
}

/// The control files: the radio wheel picks with axes of its own
/// (`INPUT_RADIO_WHEEL_LR`/`UD`), which later data maps as the weapon
/// wheel's (the right stick, the mouse). This data maps them to nothing, so
/// the selection never moved; and the wheel makes them exclusive while open,
/// which disabled nothing then (the camera turned with the stick). Each
/// weapon wheel axis' mapping gets a copy for the radio wheel's. Same length:
/// the lines' indentation, from the end, makes room for the copies.
fn radio_wheel_axes(text: String) -> Option<String> {
    let mut out = text.clone();
    for (weapon, radio) in [("WEAPON_WHEEL_LR", "RADIO_WHEEL_LR"), ("WEAPON_WHEEL_UD", "RADIO_WHEEL_UD")] {
        let input = format!("<Input>INPUT_{weapon}</Input>");
        let at = out.find(&input)?;
        // The mapping, from its line's start to the end of the line closing
        // it, after its parameters (themselves items).
        let start = out[..out[..at].rfind("<Item>")?].rfind('\n').map_or(0, |n| n + 1);
        let params = at + out[at..].find("</Parameters>")?;
        let end = params + out[params..].find("</Item>")?;
        let end = out[end..].find('\n').map_or(out.len(), |n| end + n + 1);
        let copy = out[start..end].replace(&input, &format!("<Input>INPUT_{radio}</Input>"));
        out.insert_str(end, &copy);
    }
    dedent_to(out, text.len())
}

/// Scripted cameras (e.g. Paparazzo - The Sex Tape) read
/// INPUT_SCALED_LOOK_LR/UD (290/291) for mouse movement. This data has no
/// mappings for them, so the camera stays still despite normal mouse look
/// working. CControl applies the vertical inversion preference to UD.
/// Keep the radio wheel mappings and the manifest's file size too.
fn mouse_axes(text: String) -> Option<String> {
    let text = radio_wheel_axes(text)?;
    let mut out = text.clone();
    let at = out.find("</Mappings>")?;
    let mut mappings = String::new();
    for (input, axis) in [("SCALED_LOOK_LR", "X"), ("SCALED_LOOK_UD", "Y")] {
        mappings.push_str(&format!(
            "<Item><Input>INPUT_{input}</Input><Source>IOMS_MOUSE_SCALEDAXIS</Source><Parameters><Item>IOM_AXIS_{axis}</Item></Parameters></Item>\n"
        ));
    }
    out.insert_str(at, &mappings);
    dedent_to(out, text.len())
}

/// `text` cut to `len` bytes from its lines' indentation, the last lines
/// first; `None` if it has not that much.
fn dedent_to(text: String, len: usize) -> Option<String> {
    let mut excess = text.len().checked_sub(len)?;
    let mut lines: Vec<&str> = text.split_inclusive('\n').collect();
    for line in lines.iter_mut().rev() {
        let cut = (line.len() - line.trim_start_matches([' ', '\t']).len()).min(excess);
        *line = &line[cut..];
        excess -= cut;
    }
    (excess == 0).then(|| lines.concat())
}

#[test]
fn feed_is_the_minimap_width() {
    let text = "<GameStream>\r\n\t<position posX=\"0.0\" sizeX=\"0.19890859375\" />\r\n\t<posloading sizeX=\"0.19890859375\" />\r\n</GameStream>\r\n";
    let want = "<GameStream>\r\n\t<position posX=\"0.0\" sizeX=\"0.15000000000\" />\r\n\t<posloading sizeX=\"0.15000000000\" />\r\n</GameStream>\r\n";
    assert_eq!(feed_width(text.into()).as_deref(), Some(want));
}

#[test]
fn autosave_goes_first() {
    let text = "<a>\r\n\t<Item> <MenuPref>PREF_STARTUP_FLOW</MenuPref> </Item>\r\n\t<Item> <MenuPref>PREF_AUTOSAVE</MenuPref> </Item>\r\n</a>\r\n";
    let want = "<a>\r\n\t<Item> <MenuPref>PREF_AUTOSAVE</MenuPref> </Item>\r\n\t<Item> <MenuPref>PREF_STARTUP_FLOW</MenuPref> </Item>\r\n</a>\r\n";
    assert_eq!(autosave_first(text.into()).as_deref(), Some(want));
}

#[test]
fn radio_wheel_mapped_as_weapon_wheel() {
    let item = |input: &str, axis: &str| {
        format!(
            "\t\t<Item>\r\n\t\t\t<Input>INPUT_{input}</Input>\r\n\t\t\t<Source>IOMS_PAD_AXIS</Source>\r\n\t\t\t<Parameters>\r\n\t\t\t\t<Item>IOM_AXIS_{axis}</Item>\r\n\t\t\t</Parameters>\r\n\t\t</Item>\r\n"
        )
    };
    let (lr, ud) = (item("WEAPON_WHEEL_LR", "RX"), item("WEAPON_WHEEL_UD", "RY"));
    let tail = "\t\t<Item>\r\n\t\t\t<Input>INPUT_JUMP</Input>\r\n\t\t</Item>\r\n".repeat(60);
    let text = format!("<a>\r\n\t<Mappings>\r\n{lr}{ud}{tail}\t</Mappings>\r\n</a>\r\n");
    let out = radio_wheel_axes(text.clone()).unwrap();
    assert_eq!(out.len(), text.len());
    let flat = |s: &str| s.split_whitespace().collect::<String>();
    let (radio_lr, radio_ud) = (item("RADIO_WHEEL_LR", "RX"), item("RADIO_WHEEL_UD", "RY"));
    assert_eq!(flat(&out), flat(&format!("<a><Mappings>{lr}{radio_lr}{ud}{radio_ud}{tail}</Mappings></a>")));
    assert!(out.starts_with(&format!("<a>\r\n\t<Mappings>\r\n{lr}{radio_lr}{ud}{radio_ud}")));
    // Without room for the copies, the file stays as it is.
    assert_eq!(radio_wheel_axes(format!("<a>{}{}</a>", flat(&lr), flat(&ud))), None);
}

#[test]
fn scripted_cameras_have_both_scaled_mouse_axes() {
    let item = |input: &str, axis: &str| {
        format!(
            "<Item><Input>INPUT_{input}</Input><Source>IOMS_MOUSE_NORMALIZED</Source><Parameters><Item>IOM_AXIS_{axis}</Item></Parameters></Item>\n"
        )
    };
    let lr = item("WEAPON_WHEEL_LR", "X");
    let ud = item("WEAPON_WHEEL_UD", "Y");
    let padding = "\t\t\t\t<!-- padding -->\n".repeat(200);
    let text = format!("<Mappings>\n{lr}{ud}{padding}</Mappings>\n");
    let out = mouse_axes(text.clone()).unwrap();
    assert_eq!(out.len(), text.len());
    // Existing wheel sources remain normalized, and the radio wheel still
    // gets both axes. Scripted camera input uses relative, scaled motion.
    assert!(out.contains(&lr) && out.contains(&ud));
    assert!(out.contains(&item("RADIO_WHEEL_LR", "X")));
    assert!(out.contains(&item("RADIO_WHEEL_UD", "Y")));
    for (input, axis) in [("SCALED_LOOK_LR", "X"), ("SCALED_LOOK_UD", "Y")] {
        assert!(out.contains(&format!(
            "<Input>INPUT_{input}</Input><Source>IOMS_MOUSE_SCALEDAXIS</Source><Parameters><Item>IOM_AXIS_{axis}</Item></Parameters>"
        )));
    }
    assert_eq!(out.matches("<Item>").count(), out.matches("</Item>").count());
    assert!(out.ends_with("</Mappings>\n"));
}
