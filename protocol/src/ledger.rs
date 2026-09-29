//! The ledger's selection (spec §19, protocol §7): which wishes are ready, which of them become the
//! epoch's miracles, and the next price. Pure, so the server and any watcher decide the same way.

use crate::wish::Action;
use crate::{hash, Hash};

/// Miracles per epoch at most, and each action's price multiplier in percent, by action code:
/// weather, migrate, revive (spec v0.2); shelter, forage, cure, gift, hybrid, blight, expose,
/// sickness (spec v0.3, draft, candidates).
pub const PER_EPOCH: usize = 3;
pub const PRICE_MULT: [u128; 11] = [100, 120, 200, 100, 100, 100, 150, 250, 120, 120, 120];
pub const TIEBREAK_TAG: &[u8] = b"PROTOGAEA/TIEBREAK/V0";

pub fn mult(a: &Action) -> u128 {
    PRICE_MULT[usize::from(a.code())]
}

/// A wish's price at the epoch price: `price × mult / 100 × share / 100`.
pub fn price_of(price: u128, c: &Candidate) -> u128 {
    price * mult(&c.action) * c.share / 10_000
}

/// Two miracles that cannot go in one epoch (spec §5): overlapping weather areas (centers within
/// 6 cells), one clade relocated twice, or targets whose 3 × 3 areas overlap (within 2 cells);
/// and (spec v0.3, draft) one clade given the same relief or harm twice, or two gifts, or one
/// pair crossed twice. The world would refuse the second of each; this keeps it waiting instead.
pub fn conflicts(a: &Action, b: &Action) -> bool {
    use crate::wish::CladeAction::Gift;
    let far = |p: (u8, u8), q: (u8, u8)| {
        (i32::from(p.0) - i32::from(q.0))
            .abs()
            .max((i32::from(p.1) - i32::from(q.1)).abs())
    };
    let target = |a: &Action| match a {
        Action::Migrate { to, .. } => Some(*to),
        Action::Revive { at, .. } => Some(*at),
        Action::Hybrid { at, .. } => Some(*at),
        Action::Weather { .. } | Action::Clade { .. } => None,
    };
    let pair = |x: u32, y: u32| (x.min(y), x.max(y));
    match (a, b) {
        (Action::Weather { x, y, .. }, Action::Weather { x: x2, y: y2, .. }) => {
            far((*x, *y), (*x2, *y2)) <= 6
        }
        (Action::Migrate { clade_id: c1, .. }, Action::Migrate { clade_id: c2, .. })
            if c1 == c2 =>
        {
            true
        }
        (
            Action::Clade {
                action: k1,
                clade_id: c1,
                ..
            },
            Action::Clade {
                action: k2,
                clade_id: c2,
                ..
            },
        ) if c1 == c2 => k1 == k2 || matches!((k1, k2), (Gift(_), Gift(_))),
        (
            Action::Hybrid {
                clade_a: a1,
                clade_b: b1,
                ..
            },
            Action::Hybrid {
                clade_a: a2,
                clade_b: b2,
                ..
            },
        ) if pair(*a1, *b1) == pair(*a2, *b2) => true,
        _ => matches!((target(a), target(b)), (Some(p), Some(q)) if far(p, q) <= 2),
    }
}

/// A wish that may be selected: open or queued, with its accumulated work and the share
/// multiplier of the clade it names, in percent (100 for actions without one; spec v0.3 §6),
/// computed from the state the miracle applies to. A wish the share rule closes for now is not a
/// candidate.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub id: Hash,
    pub action: Action,
    pub work: u128,
    pub share: u128,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    /// Every wish whose work covers its price, in rank order.
    pub ready: Vec<Hash>,
    /// The epoch's miracles, in rank order (the order the world is given them).
    pub selected: Vec<Hash>,
    pub next_price: u128,
}

/// Ready wishes are ranked by the share of the price they cover (`work / (mult × share)`,
/// compared by cross-multiplication), ties broken by `BLAKE3(tag ‖ beacon ‖ proposal_id)`,
/// smaller first; up to [`PER_EPOCH`] that do not conflict are selected. The price then rises by
/// an eighth if ready wishes remain, falls by an eighth (not below `price_min`) if fewer than
/// three were selected, and holds otherwise.
pub fn select(candidates: &[Candidate], price: u128, price_min: u128, beacon: &Hash) -> Selection {
    let mut ready: Vec<&Candidate> = candidates
        .iter()
        .filter(|c| c.work >= price_of(price, c))
        .collect();
    let weight = |c: &Candidate| mult(&c.action) * c.share;
    ready.sort_by(|a, b| {
        (b.work * weight(a))
            .cmp(&(a.work * weight(b)))
            .then_with(|| {
                hash(&[TIEBREAK_TAG, beacon, &a.id]).cmp(&hash(&[TIEBREAK_TAG, beacon, &b.id]))
            })
    });
    let mut selected: Vec<&Candidate> = Vec::new();
    for c in &ready {
        if selected.len() == PER_EPOCH {
            break;
        }
        if selected.iter().any(|s| conflicts(&s.action, &c.action)) {
            continue;
        }
        selected.push(c);
    }
    let next_price = if ready.len() > selected.len() {
        price + price / 8
    } else if selected.len() < PER_EPOCH {
        (price - price / 8).max(price_min)
    } else {
        price
    };
    Selection {
        ready: ready.iter().map(|c| c.id).collect(),
        selected: selected.iter().map(|c| c.id).collect(),
        next_price,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wish::Weather;

    fn rain(id: u8, x: u8, work: u128) -> Candidate {
        Candidate {
            id: [id; 32],
            action: Action::Weather {
                x,
                y: 10,
                kind: Weather::Rain,
            },
            work,
            share: 100,
        }
    }

    #[test]
    fn ranks_selects_and_moves_the_price() {
        let price = 1000;
        // Four ready rains (one conflicting with a better one) and one short of its price.
        let c = vec![
            rain(1, 10, 3000),
            rain(2, 12, 5000), // overlaps 1, ranks above it
            rain(3, 40, 1000),
            rain(4, 60, 2000),
            rain(5, 30, 999),
        ];
        let s = select(&c, price, 800, &[0; 32]);
        assert_eq!(s.ready, vec![[2; 32], [1; 32], [4; 32], [3; 32]]);
        assert_eq!(
            s.selected,
            vec![[2; 32], [4; 32], [3; 32]],
            "1 conflicts with 2 and waits"
        );
        assert_eq!(s.next_price, 1125, "a ready wish remains: the price rises");
        // Nothing ready: the price falls, not below the floor.
        assert_eq!(select(&c[4..], price, 800, &[0; 32]).next_price, 875);
        assert_eq!(select(&c[4..], 850, 800, &[0; 32]).next_price, 800);
        // A revival costs twice as much.
        let revive = Candidate {
            id: [6; 32],
            action: Action::Revive {
                source: crate::wish::Source::Museum,
                entry_id: 1,
                steps: vec![],
                at: (5, 5),
            },
            work: 1500,
            share: 100,
        };
        assert!(select(&[revive], price, 800, &[0; 32]).ready.is_empty());
    }

    #[test]
    fn ties_follow_the_beacon() {
        let c = vec![
            rain(1, 10, 1000),
            rain(2, 40, 1000),
            rain(3, 60, 1000),
            rain(4, 90, 1000),
        ];
        let a = select(&c, 1000, 1000, &[1; 32]).selected;
        let b = select(&c, 1000, 1000, &[2; 32]).selected;
        assert_eq!(a.len(), 3);
        assert_ne!(a, b, "another beacon, another order among equals");
    }

    #[test]
    fn the_share_multiplies_the_price() {
        use crate::wish::CladeAction;
        let shelter = |id: u8, clade_id: u32, work: u128, share: u128| Candidate {
            id: [id; 32],
            action: Action::Clade {
                action: CladeAction::Shelter,
                clade_id,
                at: (id, id),
            },
            work,
            share,
        };
        // Help for a clade with a 20% share costs ×4; an outsider's ×0.75.
        let c = [
            shelter(1, 1, 3999, 400),
            shelter(2, 2, 750, 75),
            shelter(3, 3, 4000, 400),
        ];
        let mut ready = select(&c, 1000, 1000, &[0; 32]).ready;
        ready.sort();
        assert_eq!(
            ready,
            vec![[2; 32], [3; 32]],
            "both cover exactly their price"
        );
        // The same relief for one clade twice waits; another relief for it does not.
        let mut forage = shelter(4, 3, 4000, 400);
        forage.action = Action::Clade {
            action: CladeAction::Forage,
            clade_id: 3,
            at: (4, 4),
        };
        let twice = [shelter(3, 3, 4000, 400), shelter(5, 3, 4000, 400), forage];
        assert_eq!(
            select(&twice, 1000, 1000, &[0; 32]).selected,
            vec![[3; 32], [4; 32]]
        );
        let cross = |a, b| Action::Hybrid {
            clade_a: a,
            clade_b: b,
            at: (0, 0),
        };
        let far = |a, b| Action::Hybrid {
            clade_a: a,
            clade_b: b,
            at: (50, 50),
        };
        assert!(
            conflicts(&cross(1, 2), &far(2, 1)),
            "one pair, either order"
        );
        assert!(!conflicts(&cross(1, 2), &far(1, 3)));
        assert!(
            conflicts(&cross(1, 2), &cross(3, 4)),
            "the hybrids' areas overlap"
        );
    }
}
