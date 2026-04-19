use crate::job::entity::{ScreenPoint, ScreenRect};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelectionPhaseStart(pub ScreenPoint);

impl SelectionPhaseStart {
    pub fn dragging_to(self, to: ScreenPoint) -> SelectionPhaseDragging {
        SelectionPhaseDragging(self.0, to)
    }

    pub fn point(self) -> ScreenPoint {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelectionPhaseDragging(ScreenPoint, ScreenPoint);

impl SelectionPhaseDragging {
    pub fn dragging_to(self, to: ScreenPoint) -> SelectionPhaseDragging {
        SelectionPhaseDragging(self.0, to)
    }

    pub fn confirm(self) -> SelectionPhasePreview {
        SelectionPhasePreview(ScreenRect::from_points(self.0, self.1))
    }

    pub fn get_region(&self) -> ScreenRect {
        ScreenRect::from_points(self.0, self.1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelectionPhasePreview(pub ScreenRect);

impl SelectionPhasePreview {
    pub fn region(self) -> ScreenRect {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SelectionPhase {
    Idle,
    Start(SelectionPhaseStart),
    Dragging(SelectionPhaseDragging),
    Preview(SelectionPhasePreview),
}

impl Default for SelectionPhase {
    fn default() -> Self {
        SelectionPhase::Idle
    }
}

impl SelectionPhase {
    pub fn start_at(at: ScreenPoint) -> SelectionPhase {
        SelectionPhase::Start(SelectionPhaseStart(at))
    }

    pub fn update_by(self, point: ScreenPoint) -> SelectionPhase {
        match self {
            SelectionPhase::Idle => Self::start_at(point),
            SelectionPhase::Start(s) => SelectionPhase::Dragging(s.dragging_to(point)),
            SelectionPhase::Dragging(s) => SelectionPhase::Dragging(s.dragging_to(point)),
            SelectionPhase::Preview(_) => self,
        }
    }
}
