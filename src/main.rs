use std::error::Error;
use std::fs;
use std::io::prelude::*;
use std::os::unix::net::{UnixListener, UnixStream};
use std::time::Duration;
use std::{env, time};

fn main() {}

pub enum Action {
    Up,
    Down,
    MuteToggle,
}
pub struct CachedVolume {
    level: f32,
    is_muted: bool,
}
pub struct Window {
    time: time::Instant,
    icon: Icon,
}
pub enum Icon {
    Mute,
    Low,
    Mid,
    High,
}

pub struct Listener {
    volume: CachedVolume,
}

mod wpctl {
    use crate::*;
    use std::process::{Command, Output};
    pub fn get_vol() -> Result<CachedVolume, std::io::Error> {
        let volume = String::from_utf8(
            Command::new("wpctl")
                .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap();
        let mut level = 0.0;
        let mut is_muted = false;
        for slice in volume.split_whitespace() {
            match slice.parse::<f32>() {
                Ok(float) => level = float,
                Err(_) => {
                    if slice == "[MUTED]" {
                        is_muted = true
                    }
                }
            }
        }
        Ok(CachedVolume { level, is_muted })
    }
}

mod eww {
    use std::{fmt::Display, process::Command};
    pub mod var {
        pub const VOLUME_LEVEL: &'static str = "volume-level=";
        pub const ICON_PATH: &'static str = "volume-icon-path=";
    }
    pub fn update(var: &str, val: impl Display) {
        let arg = &format!("{var}{val}");
        Command::new("eww").args(["update", arg]).output().unwrap();
    }
    pub fn open_window(name: &str) {
        Command::new("eww").args(["open", name]).output().unwrap();
    }
    pub fn close_window(name: &str) {
        Command::new("eww").args(["close", name]).output().unwrap();
    }
}

mod resource {
    pub mod icon {
        pub const MUTE: &'static str = "$HOME/.config/eww/resources/volume-mute.png";
        pub const LOW: &'static str = "$HOME/.config/eww/resources/volume-low.png";
        pub const MID: &'static str = "$HOME/.config/eww/resources/volume-mid.png";
        pub const HIGH: &'static str = "$HOME/.config/eww/resources/volume-high.png";
    }
}
