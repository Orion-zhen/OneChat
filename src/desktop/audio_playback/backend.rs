use std::{
    fs::File,
    io::{BufReader, Cursor},
    num::{NonZeroU16, NonZeroU32},
    time::Duration,
};

use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, buffer::SamplesBuffer};

use super::PlaybackSource;

pub(super) trait PlaybackBackend: Send {
    fn start(&mut self, source: PlaybackSource) -> Result<(), String>;
    fn pause(&mut self);
    fn resume(&mut self);
    fn stop(&mut self);
    fn seek(&mut self, position: Duration) -> Result<(), String>;
    fn finished(&self) -> bool;
}

#[derive(Default)]
pub(super) struct RodioBackend {
    output: Option<MixerDeviceSink>,
    player: Option<Player>,
}

impl PlaybackBackend for RodioBackend {
    fn start(&mut self, source: PlaybackSource) -> Result<(), String> {
        self.stop();
        let mut output = DeviceSinkBuilder::open_default_sink()
            .map_err(|error| format!("Could not open the audio output device: {error}"))?;
        output.log_on_drop(false);
        let player = Player::connect_new(output.mixer());
        match source {
            PlaybackSource::Bytes(bytes) => {
                let decoder = Decoder::try_from(Cursor::new(bytes))
                    .map_err(|error| format!("Could not decode audio: {error}"))?;
                player.append(decoder);
            }
            PlaybackSource::File(path) => {
                let file = File::open(&path).map_err(|error| {
                    format!("Could not open audio file {}: {error}", path.display())
                })?;
                let decoder = Decoder::try_from(BufReader::new(file))
                    .map_err(|error| format!("Could not decode audio: {error}"))?;
                player.append(decoder);
            }
            PlaybackSource::Clip(clip) => player.append(SamplesBuffer::new(
                NonZeroU16::new(clip.channels()).expect("validated audio channels"),
                NonZeroU32::new(clip.sample_rate()).expect("validated audio sample rate"),
                clip.samples().to_vec(),
            )),
        }
        player.play();
        self.output = Some(output);
        self.player = Some(player);
        Ok(())
    }

    fn pause(&mut self) {
        if let Some(player) = &self.player {
            player.pause();
        }
    }

    fn resume(&mut self) {
        if let Some(player) = &self.player {
            player.play();
        }
    }

    fn stop(&mut self) {
        if let Some(player) = self.player.take() {
            player.stop();
        }
        self.output = None;
    }

    fn seek(&mut self, position: Duration) -> Result<(), String> {
        self.player
            .as_ref()
            .ok_or_else(|| "No audio is currently loaded.".to_string())?
            .try_seek(position)
            .map_err(|error| format!("Could not seek audio: {error}"))
    }

    fn finished(&self) -> bool {
        self.player.as_ref().is_some_and(Player::empty)
    }
}
