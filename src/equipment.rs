use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Equipment {
    pub slot: Slot,
    pub equipped: bool,
}

/// An object that can be equipped, yielding bonuses.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Equippable {
    pub slot: Slot,
    pub equipped: bool,
    //pub power_bonus: bool,
    //pub defense_bonus: bool,
    //pub max_hp_bonus: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Slot {
    LeftHand,
    RightHand,
    Head,
}

impl fmt::Display for Slot {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Slot::LeftHand => write!(f, "left hand"),
            Slot::RightHand => write!(f, "right hand"),
            Slot::Head => write!(f, "head"),
        }
    }
}
