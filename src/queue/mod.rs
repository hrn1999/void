use crate::ytm::models::Track;
use rand::seq::SliceRandom;

#[derive(Debug, Clone, Default)]
pub struct Queue {
    tracks: Vec<Track>,
    current_index: Option<usize>,
    shuffle_enabled: bool,
    shuffle_order: Vec<usize>,
}

impl Queue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a single track to the end of the queue
    pub fn add(&mut self, track: Track) {
        self.tracks.push(track);
        self.rebuild_shuffle_order();
    }

    /// Add multiple tracks to the end of the queue
    pub fn add_many(&mut self, tracks: Vec<Track>) {
        self.tracks.extend(tracks);
        self.rebuild_shuffle_order();
    }

    /// Replace the entire queue with new tracks and start playing from the beginning
    pub fn replace(&mut self, tracks: Vec<Track>) {
        self.tracks = tracks;
        self.current_index = if self.tracks.is_empty() { None } else { Some(0) };
        self.rebuild_shuffle_order();
    }

    /// Remove a track at the given index
    pub fn remove(&mut self, index: usize) -> Option<Track> {
        if index >= self.tracks.len() {
            return None;
        }

        let track = self.tracks.remove(index);

        // Adjust current_index if needed
        if let Some(current) = self.current_index {
            if index < current {
                self.current_index = Some(current - 1);
            } else if index == current {
                // Current track was removed
                if self.tracks.is_empty() {
                    self.current_index = None;
                } else if current >= self.tracks.len() {
                    self.current_index = Some(self.tracks.len() - 1);
                }
            }
        }

        self.rebuild_shuffle_order();
        Some(track)
    }

    /// Clear the entire queue
    pub fn clear(&mut self) {
        self.tracks.clear();
        self.current_index = None;
        self.shuffle_order.clear();
    }

    /// Move a track from one position to another
    pub fn move_track(&mut self, from: usize, to: usize) {
        if from >= self.tracks.len() || to >= self.tracks.len() || from == to {
            return;
        }

        let track = self.tracks.remove(from);
        self.tracks.insert(to, track);

        // Adjust current_index if needed
        if let Some(current) = self.current_index {
            if from == current {
                self.current_index = Some(to);
            } else if from < current && to >= current {
                self.current_index = Some(current - 1);
            } else if from > current && to <= current {
                self.current_index = Some(current + 1);
            }
        }

        self.rebuild_shuffle_order();
    }

    /// Toggle shuffle mode
    pub fn toggle_shuffle(&mut self) {
        self.shuffle_enabled = !self.shuffle_enabled;
        if self.shuffle_enabled {
            self.rebuild_shuffle_order();
        }
    }

    /// Get shuffle state
    pub fn is_shuffle_enabled(&self) -> bool {
        self.shuffle_enabled
    }

    /// Set the current playing index
    pub fn set_current(&mut self, index: usize) {
        if index < self.tracks.len() {
            self.current_index = Some(index);
        }
    }

    /// Get the current track
    pub fn current_track(&self) -> Option<&Track> {
        self.current_index.and_then(|i| self.tracks.get(i))
    }

    /// Get the current index
    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    /// Get the next track (respecting shuffle)
    #[allow(dead_code)]
    pub fn next_track(&self) -> Option<(usize, &Track)> {
        let current = self.current_index?;
        let next_index = self.next_index(current)?;
        self.tracks.get(next_index).map(|t| (next_index, t))
    }

    /// Get the previous track (respecting shuffle)
    #[allow(dead_code)]
    pub fn prev_track(&self) -> Option<(usize, &Track)> {
        let current = self.current_index?;
        let prev_index = self.prev_index(current)?;
        self.tracks.get(prev_index).map(|t| (prev_index, t))
    }

    /// Advance to the next track, returns the new current track
    pub fn advance(&mut self) -> Option<&Track> {
        let current = self.current_index?;
        let next_index = self.next_index(current)?;
        self.current_index = Some(next_index);
        self.tracks.get(next_index)
    }

    /// Go to the previous track, returns the new current track
    pub fn go_back(&mut self) -> Option<&Track> {
        let current = self.current_index?;
        let prev_index = self.prev_index(current)?;
        self.current_index = Some(prev_index);
        self.tracks.get(prev_index)
    }

    /// Get all tracks in the queue
    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    /// Get the number of tracks in the queue
    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    /// Check if the queue is empty
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    /// Check if we're at the end of the queue
    #[allow(dead_code)]
    pub fn is_at_end(&self) -> bool {
        match self.current_index {
            Some(i) => {
                if self.shuffle_enabled && !self.shuffle_order.is_empty() {
                    self.shuffle_order.iter().position(|&x| x == i)
                        == Some(self.shuffle_order.len() - 1)
                } else {
                    i >= self.tracks.len().saturating_sub(1)
                }
            }
            None => true,
        }
    }

    /// Check if we're at the beginning of the queue
    #[allow(dead_code)]
    pub fn is_at_start(&self) -> bool {
        match self.current_index {
            Some(i) => {
                if self.shuffle_enabled && !self.shuffle_order.is_empty() {
                    self.shuffle_order.iter().position(|&x| x == i) == Some(0)
                } else {
                    i == 0
                }
            }
            None => true,
        }
    }

    fn next_index(&self, current: usize) -> Option<usize> {
        if self.tracks.is_empty() {
            return None;
        }

        if self.shuffle_enabled && !self.shuffle_order.is_empty() {
            let pos = self.shuffle_order.iter().position(|&x| x == current)?;
            if pos + 1 < self.shuffle_order.len() {
                Some(self.shuffle_order[pos + 1])
            } else {
                None // End of shuffled queue
            }
        } else {
            if current + 1 < self.tracks.len() {
                Some(current + 1)
            } else {
                None // End of queue
            }
        }
    }

    fn prev_index(&self, current: usize) -> Option<usize> {
        if self.tracks.is_empty() {
            return None;
        }

        if self.shuffle_enabled && !self.shuffle_order.is_empty() {
            let pos = self.shuffle_order.iter().position(|&x| x == current)?;
            if pos > 0 {
                Some(self.shuffle_order[pos - 1])
            } else {
                None // Start of shuffled queue
            }
        } else {
            if current > 0 {
                Some(current - 1)
            } else {
                None // Start of queue
            }
        }
    }

    fn rebuild_shuffle_order(&mut self) {
        if !self.shuffle_enabled || self.tracks.is_empty() {
            self.shuffle_order.clear();
            return;
        }

        let mut rng = rand::rng();
        self.shuffle_order = (0..self.tracks.len()).collect();
        self.shuffle_order.shuffle(&mut rng);

        // If we have a current track, make sure it's at the front of shuffle order
        if let Some(current) = self.current_index {
            if let Some(pos) = self.shuffle_order.iter().position(|&x| x == current) {
                self.shuffle_order.swap(0, pos);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_track(id: &str) -> Track {
        Track {
            video_id: id.to_string(),
            title: format!("Track {}", id),
            artists: vec!["Artist".to_string()],
            album: None,
            duration_seconds: Some(180),
        }
    }

    #[test]
    fn test_add_and_len() {
        let mut queue = Queue::new();
        assert!(queue.is_empty());

        queue.add(make_track("1"));
        assert_eq!(queue.len(), 1);

        queue.add(make_track("2"));
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn test_replace() {
        let mut queue = Queue::new();
        queue.add(make_track("1"));

        queue.replace(vec![make_track("2"), make_track("3")]);
        assert_eq!(queue.len(), 2);
        assert_eq!(queue.current_index(), Some(0));
    }

    #[test]
    fn test_advance() {
        let mut queue = Queue::new();
        queue.replace(vec![make_track("1"), make_track("2"), make_track("3")]);

        assert_eq!(queue.current_track().unwrap().video_id, "1");
        queue.advance();
        assert_eq!(queue.current_track().unwrap().video_id, "2");
        queue.advance();
        assert_eq!(queue.current_track().unwrap().video_id, "3");
        assert!(queue.advance().is_none()); // End of queue
    }

    #[test]
    fn test_remove() {
        let mut queue = Queue::new();
        queue.replace(vec![make_track("1"), make_track("2"), make_track("3")]);
        queue.set_current(1); // Playing track 2

        queue.remove(0); // Remove track 1
        assert_eq!(queue.current_index(), Some(0)); // Index shifted
        assert_eq!(queue.current_track().unwrap().video_id, "2");
    }

    #[test]
    fn test_clear() {
        let mut queue = Queue::new();
        queue.replace(vec![make_track("1"), make_track("2")]);

        queue.clear();
        assert!(queue.is_empty());
        assert!(queue.current_index().is_none());
    }
}
