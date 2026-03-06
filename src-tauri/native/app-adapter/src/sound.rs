use std::fmt;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationSoundPreset {
    None,
    Chime,
    Bell,
    Ping,
}

impl NotificationSoundPreset {
    pub fn all() -> [Self; 4] {
        [Self::None, Self::Chime, Self::Bell, Self::Ping]
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
            NotificationSoundPreset::None => None,
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
            NotificationSoundPreset::None => None,
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
            NotificationSoundPreset::None => None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_labels_match_expected_strings() {
        assert_eq!(NotificationSoundPreset::None.display_label(), "None");
        assert_eq!(NotificationSoundPreset::Chime.display_label(), "Chime");
        assert_eq!(NotificationSoundPreset::Bell.display_label(), "Bell");
        assert_eq!(NotificationSoundPreset::Ping.display_label(), "Ping");
        assert_eq!(NotificationSoundPreset::Ping.to_string(), "Ping");
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
}
