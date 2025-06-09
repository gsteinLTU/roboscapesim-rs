use derivative::Derivative;
use log::trace;

/// Possible drive modes
#[derive(Debug, PartialEq, Eq)]
pub enum DriveState {
    /// Run wheels at requested speed
    SetSpeed,
    /// Drive until distance reached
    SetDistance
}

impl Default for DriveState {
    fn default() -> Self {
        DriveState::SetSpeed
    }
}

/// Speed used when using SetDistance
pub const SET_DISTANCE_DRIVE_SPEED: f32 = 75.0 / -32.0;

/// Data for robot motors, used for controlling speed and distance
#[derive(Derivative)]
#[derivative(Debug, Default)]
pub struct RobotMotorData {
    /// Speed of left wheel
    pub speed_l: f32,
    /// Speed of right wheel
    pub speed_r: f32,
    /// Ticks for left wheel
    pub ticks: [f64; 2],
    /// Current drive state
    pub drive_state: DriveState,
    /// Distance to travel for left wheel
    pub distance_l: f64,
    /// Distance to travel for right wheel
    pub distance_r: f64,
    /// Speed scale factor
    #[derivative(Default(value = "1.0"))]
    pub speed_scale: f32,
}

impl RobotMotorData {
    pub fn update_wheel_state(&mut self, dt: f64) {
        if self.drive_state == DriveState::SetDistance {

            // Stop robot if distance reached
            if f64::abs(self.distance_l) < f64::abs(self.speed_l as f64 * -32.0 * dt) {
                trace!("Distance reached L");
                self.speed_l = 0.0;
            } else {
                self.distance_l -= (self.speed_l * -32.0) as f64 * dt;
            }

            if f64::abs(self.distance_r) < f64::abs(self.speed_r as f64 * -32.0 * dt) {
                trace!("Distance reached R");
                self.speed_r = 0.0;
            } else {
                self.distance_r -= (self.speed_r * -32.0) as f64 * dt;
            }

            if self.speed_l == 0.0 && self.speed_r == 0.0 {
                self.drive_state = DriveState::SetSpeed;
            }
        }

        // Update ticks
        self.ticks[0] += (self.speed_l * self.speed_scale * -32.0) as f64 * dt;
        self.ticks[1] += (self.speed_r * self.speed_scale * -32.0) as f64 * dt;
    }
}