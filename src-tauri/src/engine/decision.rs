//! The one decision that matters, kept pure so it can be exhaustively tested.
//!
//! ```text
//! want_awake = manual_active OR (watching AND NOT paused AND agents_working)
//!              then guards veto: battery low, battery saver, lid cap
//! ```
//!
//! `paused` only appears in the agent term, exactly as specified. Pausing also
//! stops the manual session at the moment it is pressed (see [`super::Engine::set_paused`]),
//! which is why a paused Ember never holds the PC awake at all.

use serde::Serialize;

use crate::settings::STOP_BELOW_PERCENT;

/// A guardrail that refuses to hold the PC awake, with the reason we show.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Guard {
    /// Battery dropped below the fixed floor.
    BatteryLow,
    /// Windows battery saver is on.
    BatterySaver,
    /// A single lid-proof hold ran past the safety cap.
    LidCap,
}

impl Guard {
    pub fn label(self) -> &'static str {
        match self {
            Guard::BatteryLow => "Battery too low",
            Guard::BatterySaver => "Battery saver is on",
            Guard::LidCap => "Lid-proof safety cap reached",
        }
    }

    pub fn detail(self, battery_floor: u8) -> String {
        match self {
            Guard::BatteryLow => format!(
                "Battery is below {battery_floor}%, so Ember released the hold to protect your session."
            ),
            Guard::BatterySaver => {
                "Windows battery saver is active, so Ember released the hold.".to_string()
            }
            Guard::LidCap => {
                "The lid-proof hold hit its safety cap and the lid action was restored to your original setting."
                    .to_string()
            }
        }
    }
}

/// Guard inputs, already evaluated by the engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Guards {
    pub plugged_in: bool,
    pub battery_percent: Option<u8>,
    pub saver_on: bool,
    pub lid_proof: bool,
    pub lid_cap_exceeded: bool,
    /// The battery floor in percent; the hold is refused strictly below it.
    pub battery_floor: u8,
}

impl Default for Guards {
    fn default() -> Self {
        Guards {
            plugged_in: true,
            battery_percent: None,
            saver_on: false,
            lid_proof: false,
            lid_cap_exceeded: false,
            battery_floor: STOP_BELOW_PERCENT,
        }
    }
}

impl Guards {
    /// The first guard that blocks the hold, in priority order: the battery
    /// floor, then Windows battery saver, then the lid safety cap.
    pub fn blocker(&self) -> Option<Guard> {
        if self
            .battery_percent
            .is_some_and(|percent| percent < self.battery_floor)
        {
            return Some(Guard::BatteryLow);
        }
        if self.saver_on {
            return Some(Guard::BatterySaver);
        }
        if self.lid_proof && self.lid_cap_exceeded {
            return Some(Guard::LidCap);
        }
        None
    }
}

/// Everything `want_awake` needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct DecisionInput {
    pub manual_active: bool,
    pub watching: bool,
    pub paused: bool,
    pub agents_working: bool,
    pub guards: Guards,
}

impl DecisionInput {
    /// The ungated desire to stay awake.
    pub fn base_want(&self) -> bool {
        self.manual_active || (self.watching && !self.paused && self.agents_working)
    }

    /// `base_want` after the guardrails have had their say.
    pub fn want_awake(&self) -> bool {
        self.base_want() && self.guards.blocker().is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean_guards() -> Guards {
        Guards {
            plugged_in: true,
            battery_percent: Some(80),
            saver_on: false,
            lid_proof: false,
            lid_cap_exceeded: false,
            battery_floor: STOP_BELOW_PERCENT,
        }
    }

    #[test]
    fn want_awake_truth_table_without_guards() {
        let mut cas: u32 = 0;
        for manual_active in [false, true] {
            for watching in [false, true] {
                for paused in [false, true] {
                    for agents_working in [false, true] {
                        let input = DecisionInput {
                            manual_active,
                            watching,
                            paused,
                            agents_working,
                            guards: clean_guards(),
                        };
                        let expected = manual_active || (watching && !paused && agents_working);
                        assert_eq!(
                            input.want_awake(),
                            expected,
                            "manual={manual_active} watching={watching} paused={paused} working={agents_working}"
                        );
                        cas += 1;
                    }
                }
            }
        }
        assert_eq!(cas, 16, "every combination must be exercised");
    }

    #[test]
    fn pause_only_gates_the_agent_term() {
        // With a manual session live, the agent term cannot matter.
        let input = DecisionInput {
            manual_active: true,
            watching: true,
            paused: true,
            agents_working: true,
            guards: clean_guards(),
        };
        assert!(input.want_awake());
        // The engine stops the manual session when pausing, so the real paused
        // state is manual_active == false:
        let paused_everything = DecisionInput {
            manual_active: false,
            ..input
        };
        assert!(!paused_everything.want_awake());
    }

    #[test]
    fn every_guard_vetoes_an_otherwise_positive_decision() {
        let base = DecisionInput {
            manual_active: false,
            watching: true,
            paused: false,
            agents_working: true,
            guards: clean_guards(),
        };
        assert!(base.want_awake());

        let cases: [(&str, Guards, Guard); 3] = [
            (
                "battery low",
                Guards {
                    battery_percent: Some(9),
                    ..clean_guards()
                },
                Guard::BatteryLow,
            ),
            (
                "saver on",
                Guards {
                    saver_on: true,
                    ..clean_guards()
                },
                Guard::BatterySaver,
            ),
            (
                "lid cap",
                Guards {
                    lid_proof: true,
                    lid_cap_exceeded: true,
                    ..clean_guards()
                },
                Guard::LidCap,
            ),
        ];

        for (name, guards, expected) in cases {
            let input = DecisionInput { guards, ..base };
            assert_eq!(input.guards.blocker(), Some(expected), "{name}");
            assert!(!input.want_awake(), "{name} must veto");
        }
    }

    #[test]
    fn a_vetoed_hold_is_still_false_for_manual_sessions() {
        let input = DecisionInput {
            manual_active: true,
            watching: true,
            paused: false,
            agents_working: true,
            guards: Guards {
                battery_percent: Some(5),
                ..clean_guards()
            },
        };
        assert!(input.base_want());
        assert!(!input.want_awake(), "guards outrank the manual session too");
    }

    #[test]
    fn battery_rules_boundary_is_strictly_below() {
        // Explicitly a 15% floor: value < floor blocks, value == floor does not.
        let at_floor = Guards {
            battery_percent: Some(15),
            battery_floor: 15,
            ..clean_guards()
        };
        assert_eq!(at_floor.blocker(), None, "15% is not below a 15% floor");
        let below = Guards {
            battery_percent: Some(14),
            ..at_floor
        };
        assert_eq!(below.blocker(), Some(Guard::BatteryLow));
    }

    #[test]
    fn a_custom_floor_moves_the_boundary() {
        // A user-set 30% floor blocks at 29 but not at 30.
        let at_floor = Guards {
            battery_percent: Some(30),
            battery_floor: 30,
            ..clean_guards()
        };
        assert_eq!(at_floor.blocker(), None, "30% is not below a 30% floor");
        let below = Guards {
            battery_percent: Some(29),
            ..at_floor
        };
        assert_eq!(below.blocker(), Some(Guard::BatteryLow));
    }

    #[test]
    fn desktops_without_a_battery_never_trip_battery_guards() {
        let guards = Guards {
            battery_percent: None,
            ..clean_guards()
        };
        assert_eq!(guards.blocker(), None);
    }

    #[test]
    fn guard_priority_is_deterministic() {
        let guards = Guards {
            battery_percent: Some(3),
            saver_on: true,
            lid_proof: true,
            lid_cap_exceeded: true,
            ..clean_guards()
        };
        assert_eq!(guards.blocker(), Some(Guard::BatteryLow));
        let guards = Guards {
            battery_percent: Some(90),
            ..guards
        };
        assert_eq!(guards.blocker(), Some(Guard::BatterySaver));
        let guards = Guards {
            saver_on: false,
            ..guards
        };
        assert_eq!(guards.blocker(), Some(Guard::LidCap));
    }
}
