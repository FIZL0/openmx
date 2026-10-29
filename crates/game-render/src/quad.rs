//! **The quad's chassis: the nodes MX vs. ATV Unleashed's ATV models carry where a bike has six.**
//!
//! A bike's chassis is one fork and one swingarm ([`crate::vehicle::Nodes`]). The three ATV chassis
//! models on SLUS_211.04 (`ATVS_R_Chasis`, `atv_s_chasis`, `250R_atv_chasis`, each named by its own
//! motion set's `SLTFile`, MD37c) share one skeleton of 43 nodes, rooted at a node spelt
//! `250r_atv_chasis` in all three:
//!
//! ```text
//!   root ─┬─ AarmUpperLeft ─┬─ AarmUpperLeftNode
//!         │                 └─ LeftHub ── WheelLeftFront, LeftHubNode, LeftUpperHubNode,
//!         │                               TieRodLeftNode, FrontLeftParticle01
//!         ├─ AarmUpperRight ── (the same, Right)
//!         ├─ AarmLowerLeft/Right ── AarmLower…Node, FrontLower…Strut
//!         ├─ SwingArm ─┬─ RearAxle ── WheelLeftRear, WheelRightRear
//!         │            └─ RearLowerStrut, RearLeft/RightParticle01
//!         ├─ HandleBars ── BrakeHandle, ClutchHandle, Dummy_Hand_L/R
//!         └─ TieRodLeft/Right, FrontUpper…Strut, RearUpperStrut, Dummy_Foot_L/R, ShadowBody
//! ```
//!
//! (the tree as openmx-mods' `import-atv check` prints it, the same in all three; which of these the
//! original drives from its step, and how, is MD37b's — nothing here poses them). [`Nodes::of`] finds them by name, as the
//! game does, case-insensitively.

use crate::skin::Skeleton;

pub const BARS: &str = "HandleBars";
pub const SWINGARM: &str = "SwingArm";
pub const REAR_AXLE: &str = "RearAxle";
/// The four wheels: front left, front right, rear left, rear right.
pub const WHEELS: [&str; 4] = ["WheelLeftFront", "WheelRightFront", "WheelLeftRear", "WheelRightRear"];
/// The front hubs, left and right: each carries its wheel and turns with the tie rod.
pub const HUBS: [&str; 2] = ["LeftHub", "RightHub"];
/// The A-arms: upper left, upper right, lower left, lower right.
pub const A_ARMS: [&str; 4] = ["AarmUpperLeft", "AarmUpperRight", "AarmLowerLeft", "AarmLowerRight"];
pub const TIE_RODS: [&str; 2] = ["TieRodLeft", "TieRodRight"];

/// Where each of the quad's moving parts is in a chassis skeleton; `None` for a name it lacks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Nodes {
    pub bars: Option<usize>,
    pub swingarm: Option<usize>,
    pub rear_axle: Option<usize>,
    pub wheels: [Option<usize>; 4],
    pub hubs: [Option<usize>; 2],
    pub a_arms: [Option<usize>; 4],
    pub tie_rods: [Option<usize>; 2],
}

impl Nodes {
    pub fn of(skeleton: &Skeleton) -> Nodes {
        Nodes {
            bars: skeleton.find(BARS),
            swingarm: skeleton.find(SWINGARM),
            rear_axle: skeleton.find(REAR_AXLE),
            wheels: WHEELS.map(|n| skeleton.find(n)),
            hubs: HUBS.map(|n| skeleton.find(n)),
            a_arms: A_ARMS.map(|n| skeleton.find(n)),
            tie_rods: TIE_RODS.map(|n| skeleton.find(n)),
        }
    }

    /// Every name [`Nodes::of`] looked for and did not find.
    pub fn missing(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        let mut one = |at: Option<usize>, name: &'static str| {
            if at.is_none() {
                out.push(name);
            }
        };
        one(self.bars, BARS);
        one(self.swingarm, SWINGARM);
        one(self.rear_axle, REAR_AXLE);
        for (at, name) in self.wheels.iter().zip(WHEELS) {
            one(*at, name);
        }
        for (at, name) in self.hubs.iter().zip(HUBS) {
            one(*at, name);
        }
        for (at, name) in self.a_arms.iter().zip(A_ARMS) {
            one(*at, name);
        }
        for (at, name) in self.tie_rods.iter().zip(TIE_RODS) {
            one(*at, name);
        }
        out
    }
}
