//! Wishes (proposals): canonical bytes, `proposal_id` and the author's signature (protocol §4).
//!
//! The layout (Proposed; the field list is decided in spec §17):
//!
//! ```text
//! version u8 (0) ‖ world_id [16] ‖ ruleset_id [32] ‖ action u8 ‖ params (per action)
//! ‖ author_pubkey [32] ‖ created_epoch u64 ‖ expires_epoch u64
//! ‖ hypothesis: 0 | 1 ‖ template u16 ‖ len u8 ‖ params [len ≤ 32]
//! ‖ name:       0 | 1 ‖ genus u16 ‖ epithet u16
//!
//! weather: x u8 ‖ y u8 ‖ kind u8 (0 rain, 1 drought)
//! migrate: clade_id u32 ‖ from_x u8 ‖ from_y u8 ‖ to_x u8 ‖ to_y u8
//! revive:  source u8 (0 museum, 1 spore bank) ‖ entry_id u32 ‖ steps u8 (0–2)
//!          ‖ (i u8 ‖ j u8) × steps ‖ x u8 ‖ y u8
//! ```
//!
//! Integers are little-endian. Decoding is strict: unknown values, a lifetime beyond
//! [`MAX_LIFETIME`], a name on anything but `revive` and trailing bytes are all rejected, so every
//! wish has exactly one encoding and one `proposal_id`.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

use crate::{hash, Hash};

pub const PROPOSAL_TAG: &[u8] = b"PROTOGAEA/PROPOSAL/V0";
pub const VERSION: u8 = 0;
/// A wish lives at most this many epochs (one world day).
pub const MAX_LIFETIME: u64 = 288;
pub const MAX_HYPOTHESIS_PARAMS: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weather {
    Rain = 0,
    Drought = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Museum = 0,
    SporeBank = 1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Weather {
        x: u8,
        y: u8,
        kind: Weather,
    },
    Migrate {
        clade_id: u32,
        from: (u8, u8),
        to: (u8, u8),
    },
    Revive {
        source: Source,
        entry_id: u32,
        steps: Vec<(u8, u8)>,
        at: (u8, u8),
    },
}

impl Action {
    pub fn code(&self) -> u8 {
        match self {
            Action::Weather { .. } => 0,
            Action::Migrate { .. } => 1,
            Action::Revive { .. } => 2,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hypothesis {
    pub template: u16,
    pub params: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wish {
    pub world_id: [u8; 16],
    pub ruleset_id: Hash,
    pub action: Action,
    pub author: [u8; 32],
    pub created_epoch: u64,
    pub expires_epoch: u64,
    pub hypothesis: Option<Hypothesis>,
    /// Genus and epithet indexes into the root dictionary; `revive` only.
    pub name: Option<(u16, u16)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WishError {
    /// The bytes end early, have bytes left over, or hold a value out of range.
    Format(&'static str),
    Signature,
}

impl Wish {
    /// The canonical bytes. Panics on a wish that [`Wish::check`] rejects.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.check().expect("a valid wish");
        let mut out = vec![VERSION];
        out.extend_from_slice(&self.world_id);
        out.extend_from_slice(&self.ruleset_id);
        out.push(self.action.code());
        match &self.action {
            Action::Weather { x, y, kind } => out.extend_from_slice(&[*x, *y, *kind as u8]),
            Action::Migrate { clade_id, from, to } => {
                out.extend_from_slice(&clade_id.to_le_bytes());
                out.extend_from_slice(&[from.0, from.1, to.0, to.1]);
            }
            Action::Revive {
                source,
                entry_id,
                steps,
                at,
            } => {
                out.push(*source as u8);
                out.extend_from_slice(&entry_id.to_le_bytes());
                out.push(steps.len() as u8);
                for (i, j) in steps {
                    out.extend_from_slice(&[*i, *j]);
                }
                out.extend_from_slice(&[at.0, at.1]);
            }
        }
        out.extend_from_slice(&self.author);
        out.extend_from_slice(&self.created_epoch.to_le_bytes());
        out.extend_from_slice(&self.expires_epoch.to_le_bytes());
        match &self.hypothesis {
            None => out.push(0),
            Some(h) => {
                out.push(1);
                out.extend_from_slice(&h.template.to_le_bytes());
                out.push(h.params.len() as u8);
                out.extend_from_slice(&h.params);
            }
        }
        match self.name {
            None => out.push(0),
            Some((genus, epithet)) => {
                out.push(1);
                out.extend_from_slice(&genus.to_le_bytes());
                out.extend_from_slice(&epithet.to_le_bytes());
            }
        }
        out
    }

    /// The rules the encoding enforces, besides the layout itself.
    pub fn check(&self) -> Result<(), WishError> {
        if self.expires_epoch <= self.created_epoch
            || self.expires_epoch - self.created_epoch > MAX_LIFETIME
        {
            return Err(WishError::Format("lifetime"));
        }
        if let Action::Revive { steps, .. } = &self.action {
            if steps.len() > 2 {
                return Err(WishError::Format("more than two mutation steps"));
            }
        } else if self.name.is_some() {
            return Err(WishError::Format("a name without revive"));
        }
        if self
            .hypothesis
            .as_ref()
            .is_some_and(|h| h.params.len() > MAX_HYPOTHESIS_PARAMS)
        {
            return Err(WishError::Format("hypothesis parameters too long"));
        }
        Ok(())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Wish, WishError> {
        let mut r = Reader { bytes, at: 0 };
        if r.u8()? != VERSION {
            return Err(WishError::Format("version"));
        }
        let world_id = r.array::<16>()?;
        let ruleset_id = r.array::<32>()?;
        let action = match r.u8()? {
            0 => Action::Weather {
                x: r.u8()?,
                y: r.u8()?,
                kind: match r.u8()? {
                    0 => Weather::Rain,
                    1 => Weather::Drought,
                    _ => return Err(WishError::Format("weather kind")),
                },
            },
            1 => Action::Migrate {
                clade_id: r.u32()?,
                from: (r.u8()?, r.u8()?),
                to: (r.u8()?, r.u8()?),
            },
            2 => {
                let source = match r.u8()? {
                    0 => Source::Museum,
                    1 => Source::SporeBank,
                    _ => return Err(WishError::Format("revive source")),
                };
                let entry_id = r.u32()?;
                let n = r.u8()?;
                if n > 2 {
                    return Err(WishError::Format("more than two mutation steps"));
                }
                let steps = (0..n)
                    .map(|_| Ok((r.u8()?, r.u8()?)))
                    .collect::<Result<_, _>>()?;
                Action::Revive {
                    source,
                    entry_id,
                    steps,
                    at: (r.u8()?, r.u8()?),
                }
            }
            _ => return Err(WishError::Format("action")),
        };
        let author = r.array::<32>()?;
        let created_epoch = r.u64()?;
        let expires_epoch = r.u64()?;
        let hypothesis = match r.u8()? {
            0 => None,
            1 => {
                let template = r.u16()?;
                let len = r.u8()? as usize;
                if len > MAX_HYPOTHESIS_PARAMS {
                    return Err(WishError::Format("hypothesis parameters too long"));
                }
                Some(Hypothesis {
                    template,
                    params: r.take(len)?.to_vec(),
                })
            }
            _ => return Err(WishError::Format("hypothesis presence")),
        };
        let name = match r.u8()? {
            0 => None,
            1 => Some((r.u16()?, r.u16()?)),
            _ => return Err(WishError::Format("name presence")),
        };
        if r.at != bytes.len() {
            return Err(WishError::Format("trailing bytes"));
        }
        let wish = Wish {
            world_id,
            ruleset_id,
            action,
            author,
            created_epoch,
            expires_epoch,
            hypothesis,
            name,
        };
        wish.check()?;
        Ok(wish)
    }

    /// `proposal_id = BLAKE3("PROTOGAEA/PROPOSAL/V0" ‖ canonical_bytes)`.
    pub fn id(&self) -> Hash {
        hash(&[PROPOSAL_TAG, &self.to_bytes()])
    }
}

/// The author's signature: Ed25519 over `proposal_id`.
pub fn sign(secret: &[u8; 32], proposal_id: &Hash) -> [u8; 64] {
    SigningKey::from_bytes(secret).sign(proposal_id).to_bytes()
}

/// The public key of a secret key.
pub fn public_key(secret: &[u8; 32]) -> [u8; 32] {
    SigningKey::from_bytes(secret).verifying_key().to_bytes()
}

/// Strict Ed25519 verification (RFC 8032 with a canonical `S`, rejecting small-order keys and
/// `R`), as every implementation must agree on which signatures are valid.
pub fn verify(
    author: &[u8; 32],
    proposal_id: &Hash,
    signature: &[u8; 64],
) -> Result<(), WishError> {
    let key = VerifyingKey::from_bytes(author).map_err(|_| WishError::Signature)?;
    key.verify_strict(proposal_id, &Signature::from_bytes(signature))
        .map_err(|_| WishError::Signature)
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], WishError> {
        let out = self
            .bytes
            .get(self.at..self.at + n)
            .ok_or(WishError::Format("too short"))?;
        self.at += n;
        Ok(out)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], WishError> {
        Ok(self.take(N)?.try_into().expect("N bytes"))
    }
    fn u8(&mut self) -> Result<u8, WishError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, WishError> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    fn u32(&mut self) -> Result<u32, WishError> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, WishError> {
        Ok(u64::from_le_bytes(self.array()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hex;

    pub fn sample() -> Wish {
        Wish {
            world_id: [1; 16],
            ruleset_id: [2; 32],
            action: Action::Revive {
                source: Source::Museum,
                entry_id: 77,
                steps: vec![(1, 4)],
                at: (10, 20),
            },
            author: public_key(&[7; 32]),
            created_epoch: 1000,
            expires_epoch: 1288,
            hypothesis: Some(Hypothesis {
                template: 3,
                params: vec![9, 9],
            }),
            name: Some((5, 6)),
        }
    }

    #[test]
    fn round_trips_and_is_canonical() {
        let w = sample();
        let bytes = w.to_bytes();
        assert_eq!(Wish::from_bytes(&bytes), Ok(w.clone()));
        let weather = Wish {
            action: Action::Weather {
                x: 3,
                y: 4,
                kind: Weather::Drought,
            },
            hypothesis: None,
            name: None,
            ..w.clone()
        };
        assert_eq!(Wish::from_bytes(&weather.to_bytes()), Ok(weather.clone()));
        let migrate = Wish {
            action: Action::Migrate {
                clade_id: 12345,
                from: (1, 2),
                to: (30, 40),
            },
            name: None,
            ..w
        };
        assert_eq!(Wish::from_bytes(&migrate.to_bytes()), Ok(migrate));
        // One encoding only: trailing bytes, bad presence bytes and bad enums are rejected.
        let mut long = bytes.clone();
        long.push(0);
        assert!(Wish::from_bytes(&long).is_err());
        assert!(Wish::from_bytes(&bytes[..bytes.len() - 1]).is_err());
        let mut bad = weather.to_bytes();
        let last = bad.len() - 1;
        bad[last] = 2;
        assert!(Wish::from_bytes(&bad).is_err());
    }

    #[test]
    fn rejects_what_the_rules_forbid() {
        let mut w = sample();
        w.expires_epoch = w.created_epoch + MAX_LIFETIME + 1;
        assert!(w.check().is_err());
        let mut w = sample();
        w.action = Action::Weather {
            x: 0,
            y: 0,
            kind: Weather::Rain,
        };
        assert!(w.check().is_err(), "a name only goes with revive");
    }

    /// Test vector: the sample wish, its `proposal_id` and signature with the secret key [7; 32].
    #[test]
    fn id_and_signature_vector() {
        let w = sample();
        let id = w.id();
        let sig = sign(&[7; 32], &id);
        assert_eq!(
            hex(&id),
            "c98c406ecd64be70942e7c31a347daf10ad53c826318a41c1b0ee6a9d6540992"
        );
        assert_eq!(hex(&sig), "c9eeb0b569f66ee467b6c831546f0b5df97f06e849fc435d1fe0d785e33dfa08039e9a7214824b1275d704b6602a1e298004fe68acc90b25552168b8dd4ec006");
        assert_eq!(verify(&w.author, &id, &sig), Ok(()));
        let mut other = id;
        other[0] ^= 1;
        assert_eq!(verify(&w.author, &other, &sig), Err(WishError::Signature));
    }

    /// Strictness: a non-canonical S (S + L) and a small-order key are rejected.
    #[test]
    fn strict_verification() {
        let id = sample().id();
        let author = public_key(&[7; 32]);
        let sig = sign(&[7; 32], &id);
        // L, the order of the base point, little-endian.
        let l: [u8; 32] = [
            0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58, 0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9,
            0xde, 0x14, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x10,
        ];
        let mut s_plus_l = sig;
        let mut carry = 0u16;
        for i in 0..32 {
            let v = u16::from(s_plus_l[32 + i]) + u16::from(l[i]) + carry;
            s_plus_l[32 + i] = v as u8;
            carry = v >> 8;
        }
        assert_eq!(verify(&author, &id, &s_plus_l), Err(WishError::Signature));
        // The identity point (a small-order key), with an all-zero signature.
        let mut identity = [0u8; 32];
        identity[0] = 1;
        let mut zero_sig = [0u8; 64];
        zero_sig[0] = 1;
        assert_eq!(verify(&identity, &id, &zero_sig), Err(WishError::Signature));
    }
}
