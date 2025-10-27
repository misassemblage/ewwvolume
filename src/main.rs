use data::*;
use std::env;
use window::VolWindow;

fn main() {
    let action = parse_args();
    if let Ok(server) = server::try_connect() {
        server::try_update(&server, action).unwrap();
        std::process::exit(0);
    } else {
        action.run().unwrap();
        server::start_server_from(action);
    }
}
fn parse_args() -> Action {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        panic!("no arguments provided");
    }
    match args[1].as_str() {
        "up" => Action::Up,
        "down" => Action::Down,
        "mute-toggle" => Action::MuteToggle,
        _ => panic!("unexpected argument: {}", args[1]),
    }
}
mod data {
    use crate::eww;
    use crate::wpctl;
    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Action {
        Up = 0,
        Down = 1,
        MuteToggle = 2,
        MicToggle = 3,
    }
    impl Action {
        pub fn run(self) -> Result<Self, std::io::Error> {
            match self {
                Self::Up => {
                    wpctl::vol_up()?;
                    Ok(Self::Up)
                }
                Self::Down => {
                    wpctl::vol_down()?;
                    Ok(Self::Down)
                }

                Self::MuteToggle => {
                    wpctl::mute_toggle()?;
                    Ok(Self::MuteToggle)
                }
                Self::MicToggle => {
                    wpctl::mute_toggle()?;
                    Ok(Self::MicToggle)
                }
            }
        }
        pub fn to_bytes(&self) -> [u8; 1] {
            let bytes = *self as u8;
            [bytes]
        }
        pub fn from_bytes(bytes: &[u8; 1]) -> Self {
            match bytes[0] {
                0 => Self::Up,
                1 => Self::Down,
                2 => Self::MuteToggle,
                3 => Self::MicToggle,
                _ => panic!("unexpected value for action"),
            }
        }
    }

    pub trait AudioState {
        fn from_system() -> Result<Self, Box<dyn std::error::Error>>
        where
            Self: Sized;
        fn update_from(&mut self, action: Action) -> Result<(), &'static str>;
        fn sync_eww(&self);
        fn should_break_on(action: Action) -> bool;
    }
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub enum MicState {
        Muted,
        Hot,
    }
    impl AudioState for MicState {
        fn from_system() -> Result<Self, Box<dyn std::error::Error>> {
            wpctl::get_mic_state()
        }
        fn update_from(&mut self, action: Action) -> Result<(), &'static str> {
            match action {
                Action::MicToggle => {
                    self.toggle();
                    Ok(())
                }
                _ => Err("unexpected action"),
            }
        }
        fn should_break_on(action: Action) -> bool {
            match action {
                Action::MicToggle => false,
                _ => true,
            }
        }
        fn sync_eww(&self) {
            if self == &MicState::Hot {
                eww::update(eww::var::VAR_MIC_STATE, "HOT")
            } else {
                eww::update(eww::var::VAR_MIC_STATE, "MUTE");
            }
        }
    }
    impl MicState {
        pub fn toggle(&mut self) -> () {
            if self == &MicState::Muted {
                *self = MicState::Hot;
            } else {
                *self = MicState::Muted;
            }
        }
    }
    #[derive(Debug)]
    pub struct CachedVolume {
        pub level: f32,
        pub is_muted: bool,
    }

    impl AudioState for CachedVolume {
        fn from_system() -> Result<Self, Box<dyn std::error::Error>>
        where
            Self: Sized,
        {
            let vol = wpctl::get_vol()?.into();
            Ok(vol)
        }
        fn update_from(&mut self, action: Action) -> Result<(), &'static str> {
            match action {
                Action::Up => {
                    self.level += 0.02;
                    self.level = self.level.clamp(0.0, 1.0);
                    self.is_muted = false;
                    Ok(())
                }
                Action::Down => {
                    self.level -= 0.02;
                    self.level = self.level.clamp(0.0, 1.0);
                    Ok(())
                }
                Action::MuteToggle => {
                    self.toggle();
                    Ok(())
                }
                _ => Err("unexpected action"),
            }
        }
        fn should_break_on(action: Action) -> bool {
            if action == Action::MicToggle {
                true
            } else {
                false
            }
        }
        fn sync_eww(&self) {
            eww::update(
                eww::var::VAR_VOL_LEVEL,
                format!("{:.2}", self.level * 100.0), //write percent volume to Eww var
            );
        }
    }
    impl CachedVolume {
        pub fn toggle(&mut self) {
            if self.is_muted {
                self.is_muted = false;
            } else {
                self.is_muted = true;
            }
        }
    }
    impl From<wpctl::WpctlVolume> for CachedVolume {
        fn from(vol: wpctl::WpctlVolume) -> Self {
            Self {
                level: vol.level,
                is_muted: vol.is_muted,
            }
        }
    }
}
mod wpctl {
    use std::process::{Command, Output};

    use crate::MicState;
    pub struct WpctlVolume {
        pub level: f32,
        pub is_muted: bool,
    }
    pub fn get_vol() -> Result<WpctlVolume, Box<dyn std::error::Error>> {
        let volume = String::from_utf8(
            Command::new("wpctl")
                .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
                .output()?
                .stdout,
        )?;
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
        Ok(WpctlVolume { level, is_muted })
    }
    pub fn get_mic_state() -> Result<MicState, Box<dyn std::error::Error>> {
        let state = String::from_utf8(
            Command::new("wpctl")
                .args(["get-volume", "@DEFAULT_AUDIO_SOURCE@"])
                .output()?
                .stdout,
        )?;
        if state.contains("[MUTED]") {
            Ok(MicState::Muted)
        } else {
            Ok(MicState::Hot)
        }
    }
    pub fn vol_up() -> Result<Output, std::io::Error> {
        let mut cmd1 = Command::new("wpctl");
        cmd1.args(["set-mute", "@DEFAULT_AUDIO_SINK@", "0"]);
        let mut cmd2 = Command::new("wpctl");
        cmd2.args(["set-volume", "@DEFAULT_AUDIO_SINK@", "0.02+", "-l", "1"]);
        cmd1.output()?;
        cmd2.output()
    }
    pub fn vol_down() -> Result<Output, std::io::Error> {
        let mut cmd1 = Command::new("wpctl");
        cmd1.args(["set-volume", "@DEFAULT_AUDIO_SINK@", "0.02-", "-l", "1"]);
        cmd1.output()
    }
    pub fn mute_toggle() -> Result<Output, std::io::Error> {
        let mut cmd1 = Command::new("wpctl");
        cmd1.args(["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"]);
        cmd1.output()
    }
    pub fn mic_toggle() -> Result<Output, std::io::Error> {
        let mut cmd1 = Command::new("wpctl");
        cmd1.args(["set-mute", "@DEFAULT_AUDIO_SOURCE@", "toggle"]);
        cmd1.output()
    }
}
mod eww {
    use std::{fmt::Display, process::Command};
    pub mod var {
        pub const VAR_VOL_LEVEL: &'static str = "volume-level=";
        pub const VAR_VOL_ICON: &'static str = "volume-icon-resource=";
        pub const VAR_MIC_STATE: &'static str = "mic-state=";
        pub const VAR_MIC_ICON: &'static str = "mic-icon-resource=";
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
        pub const MUTE: &'static str = "volume-mute.png";
        pub const LOW: &'static str = "volume-low.png";
        pub const MID: &'static str = "volume-mid.png";
        pub const HIGH: &'static str = "volume-high.png";
        pub const MIC_MUTE: &'static str = "mic-mute.png";
        pub const MIC_HOT: &'static str = "mic-hot.png";
    }
}

mod window {

    use crate::{CachedVolume, MicState, eww};
    use std::time;
    pub trait Window: Sized {
        type State;
        fn instant(&self) -> &time::Instant;
        fn instant_mut(&mut self) -> &mut time::Instant;

        fn age(&self) -> time::Duration {
            self.instant().elapsed()
        }
        fn reset(&mut self) {
            *self.instant_mut() = time::Instant::now();
        }
        fn update_icon(&self, state: &Self::State);
    }
    macro_rules! define_window {
        ($name:ident, $window_str:literal) => {
            pub struct $name(time::Instant);

            impl $name {
                pub fn new() -> Self {
                    eww::open_window($window_str);
                    Self(time::Instant::now())
                }
            }
            impl Drop for $name {
                fn drop(&mut self) {
                    eww::close_window($window_str);
                }
            }
        };
    }
    define_window!(VolWindow, "volume-float");
    impl Window for VolWindow {
        type State = CachedVolume;
        fn instant(&self) -> &time::Instant {
            &self.0
        }

        fn instant_mut(&mut self) -> &mut time::Instant {
            &mut self.0
        }

        fn update_icon(&self, state: &CachedVolume) {
            use crate::resource::icon;
            use eww::var::*;
            if state.is_muted {
                eww::update(VAR_VOL_ICON, icon::MUTE);
            } else {
                match state.level {
                    level if level < 0.01 => eww::update(VAR_VOL_ICON, icon::MUTE),
                    level if level < 0.33 => eww::update(VAR_VOL_ICON, icon::LOW),
                    level if level < 0.66 => eww::update(VAR_VOL_ICON, icon::MID),
                    _ => eww::update(VAR_VOL_ICON, icon::HIGH),
                }
            }
        }
    }
    define_window!(MicWindow, "mic-float");
    impl Window for MicWindow {
        type State = MicState;
        fn instant(&self) -> &time::Instant {
            &self.0
        }

        fn instant_mut(&mut self) -> &mut time::Instant {
            &mut self.0
        }

        fn update_icon(&self, state: &MicState) {
            use crate::resource::icon;
            use eww::var::*;
            if state == &MicState::Muted {
                eww::update(VAR_MIC_ICON, icon::MIC_MUTE);
            } else {
                eww::update(VAR_MIC_ICON, icon::MIC_HOT);
            }
        }
    }
}

mod server {
    use crate::window::{MicWindow, VolWindow, Window};
    use crate::*;
    use std::io::prelude::*;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::time::Duration;
    use std::{error::Error, fs};
    pub fn try_connect() -> Result<UnixStream, &'static str> {
        if let Ok(stream) = UnixStream::connect("/tmp/ewwvolume.sock") {
            return Ok(stream);
        } else {
            Err("server not found")
        }
    }
    pub fn try_update(mut stream: &UnixStream, action: Action) -> Result<(), Box<dyn Error>> {
        stream.write_all(&action.to_bytes())?;
        Ok(())
    }
    pub fn start_server_from(initial_action: Action) {
        let mut next_action = Some(initial_action);
        while let Some(action) = next_action {
            next_action = match action {
                Action::MicToggle => run_window(MicWindow::new()),
                _ => run_window(VolWindow::new()),
            };
        }
    }
    pub fn run_window<W>(mut window: W) -> Option<Action>
    where
        W: Window,
        W::State: AudioState,
    {
        fs::remove_file("/tmp/ewwvolume.sock").ok();
        let mut state = W::State::from_system().unwrap();
        let listener = UnixListener::bind("/tmp/ewwvolume.sock").unwrap();
        listener.set_nonblocking(true).unwrap();
        loop {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0; 1];
                if let Ok(_) = stream.read(&mut buffer) {
                    let action = Action::from_bytes(&buffer);
                    if W::State::should_break_on(action) {
                        action.run().unwrap();
                        return Some(action);
                    } else {
                        state.update_from(action).unwrap();
                        state.sync_eww();
                        window.update_icon(&state);
                    }
                }
                window.reset();
            }
            if window.age() > Duration::from_millis(900) {
                return None;
            }
            std::thread::sleep(Duration::from_millis(8));
        }
    }
}
