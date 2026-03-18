use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationSoundPreset {
    None,
    Chime,
    Bell,
    Ping,
    Peon,
}

impl NotificationSoundPreset {
    pub fn all() -> [Self; 5] {
        [Self::None, Self::Chime, Self::Bell, Self::Ping, Self::Peon]
    }

    pub fn label(self) -> &'static str {
        self.display_label()
    }

    pub fn display_label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Chime => "Chime",
            Self::Bell => "Bell",
            Self::Ping => "Ping",
            Self::Peon => "Peon",
        }
    }

    /// Parse a preset from a case-insensitive string label.
    pub fn from_label(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "none" => Some(Self::None),
            "chime" => Some(Self::Chime),
            "bell" => Some(Self::Bell),
            "ping" => Some(Self::Ping),
            "peon" => Some(Self::Peon),
            _ => Option::None,
        }
    }
}

impl fmt::Display for NotificationSoundPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.display_label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SoundPlatform {
    Windows,
    MacOs,
    Linux,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SoundCommandSpec {
    program: &'static str,
    args: &'static [&'static str],
}

const WINDOWS_CHIME_ARGS: &[&str] = &[
    "-NoProfile",
    "-NonInteractive",
    "-Command",
    "[console]::beep(880,120); [console]::beep(1320,160)",
];
const WINDOWS_BELL_ARGS: &[&str] = &[
    "-NoProfile",
    "-NonInteractive",
    "-Command",
    "[console]::beep(740,220)",
];
const WINDOWS_PING_ARGS: &[&str] = &[
    "-NoProfile",
    "-NonInteractive",
    "-Command",
    "[console]::beep(1320,90)",
];

const MAC_CHIME_ARGS: &[&str] = &["/System/Library/Sounds/Glass.aiff"];
const MAC_BELL_ARGS: &[&str] = &["/System/Library/Sounds/Basso.aiff"];
const MAC_PING_ARGS: &[&str] = &["/System/Library/Sounds/Ping.aiff"];

const LINUX_CHIME_ARGS: &[&str] = &["-c", "printf '\\a'; sleep 0.06; printf '\\a'"];
const LINUX_BELL_ARGS: &[&str] = &["-c", "printf '\\a'"];
const LINUX_PING_ARGS: &[&str] = &["-c", "printf '\\a'; sleep 0.03; printf '\\a'"];

pub fn play_notification_sound_async(preset: NotificationSoundPreset) -> Result<(), String> {
    if preset == NotificationSoundPreset::Peon {
        return play_peon_sound_async();
    }

    let Some(command_spec) = command_for_platform(preset, current_platform()) else {
        return Ok(());
    };

    let mut command = Command::new(command_spec.program);
    command
        .args(command_spec.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let mut child = command.spawn().map_err(|error| {
        format!(
            "Failed to launch notification sound command '{}': {}",
            command_spec.program, error
        )
    })?;

    let _ = std::thread::Builder::new()
        .name("notification-sound-reaper".to_string())
        .spawn(move || {
            let _ = child.wait();
        });

    Ok(())
}

fn current_platform() -> SoundPlatform {
    if cfg!(target_os = "windows") {
        SoundPlatform::Windows
    } else if cfg!(target_os = "macos") {
        SoundPlatform::MacOs
    } else {
        SoundPlatform::Linux
    }
}

fn command_for_platform(
    preset: NotificationSoundPreset,
    platform: SoundPlatform,
) -> Option<SoundCommandSpec> {
    match platform {
        SoundPlatform::Windows => match preset {
            NotificationSoundPreset::None | NotificationSoundPreset::Peon => None,
            NotificationSoundPreset::Chime => Some(SoundCommandSpec {
                program: "powershell",
                args: WINDOWS_CHIME_ARGS,
            }),
            NotificationSoundPreset::Bell => Some(SoundCommandSpec {
                program: "powershell",
                args: WINDOWS_BELL_ARGS,
            }),
            NotificationSoundPreset::Ping => Some(SoundCommandSpec {
                program: "powershell",
                args: WINDOWS_PING_ARGS,
            }),
        },
        SoundPlatform::MacOs => match preset {
            NotificationSoundPreset::None | NotificationSoundPreset::Peon => None,
            NotificationSoundPreset::Chime => Some(SoundCommandSpec {
                program: "afplay",
                args: MAC_CHIME_ARGS,
            }),
            NotificationSoundPreset::Bell => Some(SoundCommandSpec {
                program: "afplay",
                args: MAC_BELL_ARGS,
            }),
            NotificationSoundPreset::Ping => Some(SoundCommandSpec {
                program: "afplay",
                args: MAC_PING_ARGS,
            }),
        },
        SoundPlatform::Linux => match preset {
            NotificationSoundPreset::None | NotificationSoundPreset::Peon => None,
            NotificationSoundPreset::Chime => Some(SoundCommandSpec {
                program: "sh",
                args: LINUX_CHIME_ARGS,
            }),
            NotificationSoundPreset::Bell => Some(SoundCommandSpec {
                program: "sh",
                args: LINUX_BELL_ARGS,
            }),
            NotificationSoundPreset::Ping => Some(SoundCommandSpec {
                program: "sh",
                args: LINUX_PING_ARGS,
            }),
        },
    }
}

// ---------------------------------------------------------------------------
// Peon soundpack playback
// ---------------------------------------------------------------------------

/// Resolve the default soundpack directory relative to the running executable.
/// Works for both development (`target/debug/soundpacks/default/`) and
/// production (`%LOCALAPPDATA%/Godly Terminal/soundpacks/default/`).
fn resolve_soundpack_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;
    let dir = exe_dir.join("soundpacks").join("default");
    if dir.is_dir() {
        Some(dir)
    } else {
        Option::None
    }
}

/// Pick a random WAV from the "complete" category of the default soundpack.
fn pick_peon_sound() -> Option<PathBuf> {
    let dir = resolve_soundpack_dir()?;
    let manifest_path = dir.join("manifest.json");
    let manifest = std::fs::read_to_string(&manifest_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&manifest).ok()?;
    let complete = json.get("sounds")?.get("complete")?.as_array()?;
    if complete.is_empty() {
        return Option::None;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    let idx = (now.subsec_nanos() as usize) % complete.len();
    let filename = complete[idx].as_str()?;
    Some(dir.join(filename))
}

fn play_peon_sound_async() -> Result<(), String> {
    let path = pick_peon_sound().ok_or("Could not resolve Peon soundpack file")?;
    play_wav_file_async(&path)
}

/// Play a WAV file using platform-native tooling.
fn play_wav_file_async(path: &Path) -> Result<(), String> {
    let path_str = path
        .to_str()
        .ok_or_else(|| "Sound file path contains invalid characters".to_string())?;

    let mut command = if cfg!(target_os = "windows") {
        let mut cmd = Command::new("powershell");
        cmd.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!(
                "(New-Object System.Media.SoundPlayer '{}').PlaySync()",
                path_str
            ),
        ]);
        cmd
    } else if cfg!(target_os = "macos") {
        let mut cmd = Command::new("afplay");
        cmd.arg(path_str);
        cmd
    } else {
        let mut cmd = Command::new("aplay");
        cmd.args(["-q", path_str]);
        cmd
    };

    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let mut child = command.spawn().map_err(|error| {
        format!(
            "Failed to play sound file '{}': {}",
            path.display(),
            error
        )
    })?;

    let _ = std::thread::Builder::new()
        .name("peon-sound-reaper".to_string())
        .spawn(move || {
            let _ = child.wait();
        });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_labels_match_expected_strings() {
        assert_eq!(NotificationSoundPreset::None.display_label(), "None");
        assert_eq!(NotificationSoundPreset::Chime.display_label(), "Chime");
        assert_eq!(NotificationSoundPreset::Bell.display_label(), "Bell");
        assert_eq!(NotificationSoundPreset::Ping.display_label(), "Ping");
        assert_eq!(NotificationSoundPreset::Peon.display_label(), "Peon");
        assert_eq!(NotificationSoundPreset::Ping.to_string(), "Ping");
    }

    #[test]
    fn from_label_round_trips() {
        for preset in NotificationSoundPreset::all() {
            let parsed = NotificationSoundPreset::from_label(preset.label());
            assert_eq!(parsed, Some(preset));
        }
    }

    #[test]
    fn from_label_is_case_insensitive() {
        assert_eq!(
            NotificationSoundPreset::from_label("PEON"),
            Some(NotificationSoundPreset::Peon)
        );
        assert_eq!(
            NotificationSoundPreset::from_label("pEoN"),
            Some(NotificationSoundPreset::Peon)
        );
    }

    #[test]
    fn from_label_returns_none_for_unknown() {
        assert_eq!(NotificationSoundPreset::from_label("unknown"), Option::None);
    }

    #[test]
    fn command_mapping_is_deterministic_per_platform() {
        let windows_chime =
            command_for_platform(NotificationSoundPreset::Chime, SoundPlatform::Windows)
                .expect("windows chime should map to a command");
        assert_eq!(windows_chime.program, "powershell");
        assert_eq!(windows_chime.args, WINDOWS_CHIME_ARGS);

        let windows_bell =
            command_for_platform(NotificationSoundPreset::Bell, SoundPlatform::Windows)
                .expect("windows bell should map to a command");
        assert_eq!(windows_bell.program, "powershell");
        assert_eq!(windows_bell.args, WINDOWS_BELL_ARGS);

        let windows_ping =
            command_for_platform(NotificationSoundPreset::Ping, SoundPlatform::Windows)
                .expect("windows ping should map to a command");
        assert_eq!(windows_ping.program, "powershell");
        assert_eq!(windows_ping.args, WINDOWS_PING_ARGS);

        let mac_chime = command_for_platform(NotificationSoundPreset::Chime, SoundPlatform::MacOs)
            .expect("mac chime should map to a command");
        assert_eq!(mac_chime.program, "afplay");
        assert_eq!(mac_chime.args, MAC_CHIME_ARGS);

        let mac_bell = command_for_platform(NotificationSoundPreset::Bell, SoundPlatform::MacOs)
            .expect("mac bell should map to a command");
        assert_eq!(mac_bell.program, "afplay");
        assert_eq!(mac_bell.args, MAC_BELL_ARGS);

        let mac_ping = command_for_platform(NotificationSoundPreset::Ping, SoundPlatform::MacOs)
            .expect("mac ping should map to a command");
        assert_eq!(mac_ping.program, "afplay");
        assert_eq!(mac_ping.args, MAC_PING_ARGS);

        let linux_chime =
            command_for_platform(NotificationSoundPreset::Chime, SoundPlatform::Linux)
                .expect("linux chime should map to a command");
        assert_eq!(linux_chime.program, "sh");
        assert_eq!(linux_chime.args, LINUX_CHIME_ARGS);

        let linux_bell = command_for_platform(NotificationSoundPreset::Bell, SoundPlatform::Linux)
            .expect("linux bell should map to a command");
        assert_eq!(linux_bell.program, "sh");
        assert_eq!(linux_bell.args, LINUX_BELL_ARGS);

        let linux_ping = command_for_platform(NotificationSoundPreset::Ping, SoundPlatform::Linux)
            .expect("linux ping should map to a command");
        assert_eq!(linux_ping.program, "sh");
        assert_eq!(linux_ping.args, LINUX_PING_ARGS);
    }

    #[test]
    fn none_preset_maps_to_no_command_on_all_platforms() {
        assert_eq!(
            command_for_platform(NotificationSoundPreset::None, SoundPlatform::Windows),
            None
        );
        assert_eq!(
            command_for_platform(NotificationSoundPreset::None, SoundPlatform::MacOs),
            None
        );
        assert_eq!(
            command_for_platform(NotificationSoundPreset::None, SoundPlatform::Linux),
            None
        );
    }

    #[test]
    fn peon_preset_does_not_use_beep_command() {
        assert_eq!(
            command_for_platform(NotificationSoundPreset::Peon, SoundPlatform::Windows),
            None
        );
        assert_eq!(
            command_for_platform(NotificationSoundPreset::Peon, SoundPlatform::MacOs),
            None
        );
        assert_eq!(
            command_for_platform(NotificationSoundPreset::Peon, SoundPlatform::Linux),
            None
        );
    }
}
