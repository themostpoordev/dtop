use crate::model::ContainerRow;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainerAction {
    Start,
    Stop,
    Restart,
    Pause,
    Unpause,
}

impl ContainerAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
            Self::Pause => "pause",
            Self::Unpause => "unpause",
        }
    }
    pub fn requires_confirmation(self) -> bool {
        matches!(self, Self::Stop | Self::Restart | Self::Pause)
    }
    pub fn available_for(self, container: &ContainerRow) -> bool {
        match self {
            Self::Start => container.state == "exited" || container.state == "created",
            Self::Stop | Self::Restart => container.state == "running",
            Self::Pause => container.state == "running",
            Self::Unpause => container.state == "paused",
        }
    }
}
