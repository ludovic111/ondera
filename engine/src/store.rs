use crate::{model::*, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "command", content = "params", rename_all = "camelCase")]
pub enum Command {
    Rename(String),
    RestoreTake(Box<Session>),
    AddTrack(Track),
    UpdateTrack(Track),
    RemoveTrack(String),
    MoveTrack {
        id: String,
        index: usize,
    },
    PutClip(Clip),
    RemoveClip(String),
    PutSource(Source),
    SetStrip {
        track: String,
        strip: Strip,
    },
    SetTransport(Transport),
    SetMasterVolume(f32),
    PutAutomation(crate::automation::AutomationLane),
    RemoveAutomation(String),
    Select {
        track: Option<String>,
        clip: Option<String>,
        note: Option<String>,
    },
    SetView(View),
    Undo,
    Redo,
    Batch(Vec<Command>),
}

/// Every document edit enters here, including GUI edits and headless commands.
/// Audio buffers live outside history; snapshots share immutable sessions.
pub struct Store {
    session: Arc<Session>,
    past: Vec<(Arc<Session>, u64)>,
    future: Vec<(Arc<Session>, u64)>,
    pub revision: u64,
    document_id: u64,
    saved_id: Option<u64>,
    gesture: bool,
    gesture_recorded: bool,
    /// The redo history a gesture's first edit set aside, restored if the gesture is
    /// cancelled (a failed atomic batch changes nothing, Redo included).
    gesture_future: Option<Vec<(Arc<Session>, u64)>>,
}
impl Store {
    pub fn new(mut session: Session) -> Result<Self> {
        session.normalize();
        session.validate()?;
        let session = Arc::new(session);
        Ok(Self {
            document_id: 0,
            saved_id: Some(0),
            gesture: false,
            gesture_recorded: false,
            gesture_future: None,
            session,
            past: vec![],
            future: vec![],
            revision: 0,
        })
    }
    pub fn session(&self) -> &Session {
        &self.session
    }
    pub fn snapshot(&self) -> Arc<Session> {
        self.session.clone()
    }
    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }
    /// Number of undo steps behind the current document; lets a caller return to
    /// an earlier depth with repeated `Undo` or `Redo`.
    pub fn undo_depth(&self) -> usize {
        self.past.len()
    }
    pub fn dirty(&self) -> bool {
        Some(self.document_id) != self.saved_id
    }
    pub fn mark_saved(&mut self, revision: u64) {
        if self.revision == revision {
            self.saved_id = Some(self.document_id);
        }
    }
    /// A recovered/imported copy has no saved project counterpart. This leaves
    /// history untouched and remains dirty through undo until a save succeeds.
    pub fn mark_unsaved(&mut self) {
        self.saved_id = None;
    }
    pub fn load(&mut self, mut session: Session) -> Result<()> {
        session.normalize();
        session.validate()?;
        self.session = Arc::new(session);
        self.past.clear();
        self.future.clear();
        self.revision += 1;
        self.document_id = self.revision;
        self.saved_id = Some(self.document_id);
        self.gesture_recorded = false;
        self.gesture_future = None;
        Ok(())
    }
    /// Update derived data (captured plugin state) without touching history
    /// or the dirty flag. The revision still advances so audio resyncs.
    pub fn amend(&mut self, edit: impl FnOnce(&mut Session)) -> Result<()> {
        let mut next = (*self.session).clone();
        edit(&mut next);
        next.validate()?;
        self.session = Arc::new(next);
        self.revision += 1;
        Ok(())
    }
    /// Whether edits are being coalesced into one undo step (a drag, a batch).
    pub fn gesture_active(&self) -> bool {
        self.gesture
    }
    /// Coalesce a slider drag or one focused text edit into one undo step.
    pub fn set_gesture(&mut self, active: bool) {
        if !active || !self.gesture {
            self.gesture_recorded = false;
            self.gesture_future = None;
        }
        self.gesture = active;
    }
    /// Drop every edit made since the gesture began, as if it never happened.
    /// Returns whether there was anything to drop.
    pub fn cancel_gesture(&mut self) -> bool {
        let recorded = self.gesture && self.gesture_recorded;
        if recorded {
            if let Some((session, id)) = self.past.pop() {
                self.session = session;
                self.document_id = id;
                self.revision += 1;
            }
            if let Some(future) = self.gesture_future.take() {
                self.future = future;
            }
        }
        self.gesture = false;
        self.gesture_recorded = false;
        self.gesture_future = None;
        recorded
    }
    pub fn dispatch(&mut self, command: Command) -> Result<bool> {
        let transient = matches!(command, Command::Select { .. } | Command::SetView(_));
        if let Command::Undo = command {
            if let Some((s, id)) = self.past.pop() {
                self.future.push((self.session.clone(), self.document_id));
                self.session = s;
                self.document_id = id;
                self.gesture_recorded = false;
                self.gesture_future = None;
                self.revision += 1;
                return Ok(true);
            }
            return Ok(false);
        }
        if let Command::Redo = command {
            if let Some((s, id)) = self.future.pop() {
                self.past.push((self.session.clone(), self.document_id));
                self.session = s;
                self.document_id = id;
                self.gesture_recorded = false;
                self.gesture_future = None;
                self.revision += 1;
                return Ok(true);
            }
            return Ok(false);
        }
        let mut next = (*self.session).clone();
        apply(&mut next, command, 0)?;
        next.validate()?;
        if transient {
            self.session = Arc::new(next);
        } else {
            if !self.gesture || !self.gesture_recorded {
                self.past.push((self.session.clone(), self.document_id));
                self.gesture_recorded = self.gesture;
                if self.gesture {
                    self.gesture_future = Some(std::mem::take(&mut self.future));
                }
            }
            if self.past.len() > 200 {
                self.past.remove(0);
            }
            self.future.clear();
            self.session = Arc::new(next);
            self.revision += 1;
            self.document_id = self.revision;
        }
        Ok(!transient)
    }
}
fn apply(s: &mut Session, command: Command, depth: usize) -> Result<()> {
    if depth > 8 {
        return Err("Command batch nesting exceeds capacity".into());
    }
    match command {
        Command::Rename(name) => s.name = name,
        Command::RestoreTake(session) => *s = *session,
        Command::AddTrack(track) => {
            if s.tracks.iter().any(|t| t.id == track.id) {
                return Err("Track ID already exists".into());
            }
            s.view.selected_track_id = Some(track.id.clone());
            s.tracks.push(track);
        }
        Command::UpdateTrack(track) => {
            let t = s
                .tracks
                .iter_mut()
                .find(|t| t.id == track.id)
                .ok_or("Track not found")?;
            *t = track;
        }
        Command::RemoveTrack(id) => {
            s.tracks.retain(|t| t.id != id);
            s.clips.retain(|c| c.track_id != id);
            s.strips.remove(&id);
            crate::automation::retain_targets(s);
            if s.view.selected_track_id.as_ref() == Some(&id) {
                s.view.selected_track_id = s.tracks.first().map(|t| t.id.clone());
            }
            sanitize_selection(s);
        }
        Command::MoveTrack { id, index } => {
            let from = s
                .tracks
                .iter()
                .position(|t| t.id == id)
                .ok_or("Track not found")?;
            let t = s.tracks.remove(from);
            s.tracks.insert(index.min(s.tracks.len()), t);
        }
        Command::PutClip(clip) => {
            if let Some(c) = s.clips.iter_mut().find(|c| c.id == clip.id) {
                *c = clip;
            } else {
                s.clips.push(clip);
            }
        }
        Command::RemoveClip(id) => {
            s.clips.retain(|c| c.id != id);
            sanitize_selection(s);
        }
        Command::PutSource(source) => {
            s.sources.insert(source.id.clone(), source);
        }
        Command::SetStrip { track, strip } => {
            if !is_bus(&track) && !s.tracks.iter().any(|t| t.id == track) {
                return Err("Track not found".into());
            }
            s.strips.insert(track, strip);
            crate::automation::retain_targets(s);
        }
        Command::PutAutomation(mut lane) => {
            lane.points.sort_by(|a, b| a.beat.total_cmp(&b.beat));
            if let Some(old) = s.automation.iter_mut().find(|old| old.id == lane.id) {
                *old = lane;
            } else {
                s.automation.push(lane);
            }
        }
        Command::RemoveAutomation(id) => s.automation.retain(|lane| lane.id != id),
        Command::SetTransport(t) => s.transport = t,
        Command::SetMasterVolume(v) => s.master_volume = v,
        Command::Select { track, clip, note } => {
            s.view.selected_track_id = track;
            s.view.selected_clip_id = clip.clone();
            s.view.editor_clip_id = clip;
            s.view.selected_note_id = note;
        }
        Command::SetView(v) => s.view = v,
        Command::Batch(commands) => {
            if commands.len() > 10_000 {
                return Err("Command batch exceeds capacity".into());
            }
            for command in commands {
                if matches!(
                    command,
                    Command::Undo | Command::Redo | Command::SetView(_) | Command::Select { .. }
                ) {
                    return Err("History and view commands cannot be batched".into());
                }
                apply(s, command, depth + 1)?;
            }
        }
        Command::Undo | Command::Redo => return Err("History command cannot be nested".into()),
    }
    Ok(())
}
fn sanitize_selection(s: &mut Session) {
    if !s
        .clips
        .iter()
        .any(|c| Some(&c.id) == s.view.selected_clip_id.as_ref())
    {
        s.view.selected_clip_id = None;
        s.view.selected_note_id = None;
    }
    if !s
        .clips
        .iter()
        .any(|c| Some(&c.id) == s.view.editor_clip_id.as_ref())
    {
        s.view.editor_clip_id = None;
    }
}

pub fn demo() -> Session {
    let mut s: Session = serde_json::from_str(include_str!("../tests/fixtures/nightfall.json"))
        .expect("Bundled demo is validated by tests");
    s.transport.position_beats = 0.0;
    s.transport.playing = false;
    s.transport.recording = false;
    s.extra.insert("agent".into(),serde_json::json!({"status":"idle","transport":"no agent connected","current":null,"log":[],"draft":""}));
    s.normalize();
    s
}
pub fn empty() -> Session {
    let mut s = demo();
    s.name = "Untitled.ondera".into();
    s.clips.clear();
    s.sources.clear();
    s.strips.clear();
    s.normalize();
    s.tracks.truncate(2);
    for t in &mut s.tracks {
        t.mute = false;
        t.solo = false;
        t.armed = false;
        t.volume = 0.75;
        t.pan = 0.0;
    }
    s.transport.cycle = false;
    s.transport.cycle_start_bar = 0.0;
    s.transport.cycle_end_bar = 4.0;
    s.view.selected_track_id = s.tracks.first().map(|t| t.id.clone());
    s.view.selected_clip_id = None;
    s.view.editor_clip_id = None;
    s.view.selected_note_id = None;
    s
}

/// Clip splitting keeps offsets and notes aligned, including notes crossing the cut.
pub fn split(clip: &Clip, bar: f64, id: String, bpb: f64, tempo: f64) -> Result<(Clip, Clip)> {
    let relative = bar - clip.start_bar;
    if relative <= 0.0 || relative >= clip.length_bars {
        return Err("Split position must be inside the clip".into());
    }
    let mut left = clip.clone();
    let mut right = clip.clone();
    right.id = id;
    right.start_bar = bar;
    left.length_bars = relative;
    right.length_bars = clip.length_bars - relative;
    match &clip.data {
        ClipData::Audio {
            offset_seconds,
            source_id,
        } => {
            right.data = ClipData::Audio {
                source_id: source_id.clone(),
                offset_seconds: offset_seconds + relative * bpb * 60.0 / tempo,
            }
        }
        ClipData::Midi { notes } => {
            let cut = relative * bpb;
            left.data = ClipData::Midi {
                notes: notes
                    .iter()
                    .filter(|n| n.start < cut)
                    .map(|n| {
                        let mut n = n.clone();
                        n.length = n.length.min(cut - n.start);
                        n
                    })
                    .collect(),
            };
            right.data = ClipData::Midi {
                notes: notes
                    .iter()
                    .filter(|n| n.start + n.length > cut)
                    .map(|n| {
                        let mut n = n.clone();
                        let end = n.start + n.length - cut;
                        n.start = (n.start - cut).max(0.0);
                        n.length = end - n.start;
                        n
                    })
                    .collect(),
            };
        }
    }
    Ok((left, right))
}
